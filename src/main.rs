mod agents;
mod llm_client;
mod orchestrator;
mod rag;

use agents::{
    Agent, AgentRequest, AgentResponse, CTOAgent, OpsChainAgent, ResearcherAgent,
    SeniorEngineerAgent,
};
use anyhow::{bail, Context};
use chrono::Utc;
use clap::{Parser, Subcommand};
use llm_client::{build_llm_client_from_env, LlmClient, SharedLlmClient};
use orchestrator::{routing::SemanticRouter, OrchestratorRouter};
use rag::config::RagConfig;
use rag::embed::{EmbeddingsProvider, OpenAiEmbeddingsClient};
use rag::topic_registry::TopicRegistry;
use rag::{
    build_rag_agent_from_env, HelixClient, HelixConfig, MemoryFilters, MemoryQuery, MemoryRecord,
    MemoryRequest, MemoryWriteRequest, SharedRagAgent,
};
use serde_json::{json, Map as JsonMap, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use tracing::{error, info, warn};
use tree_sitter::{Language as TsLanguage, Node, Parser as TsParser};

#[derive(Parser, Debug)]
#[command(
    name = "vidkosha-cortex",
    about = "CLI entrypoint into the Vidkosha Cortex agent network (powered by Nervos CKB)"
)]
struct Cli {
    /// Optional one-shot prompt; if omitted the CLI enters interactive mode.
    #[arg(short, long)]
    prompt: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
#[allow(clippy::enum_variant_names)]
enum Commands {
    /// Write a sample memory and immediately read it back to verify RAG wiring.
    RagSmoke,
    /// Hit the Helix health + namespace endpoints to verify connectivity before switching storage.
    HelixSmoke,
    /// Full-path HelixQL smoke with conversation, artifact, and tool call coverage.
    HelixRichSmoke,
    /// Index a single file chunk into Helix via the RAG pipeline.
    IndexChunk {
        /// Path to the file to index (first N bytes will be used).
        path: String,
        /// Maximum bytes from the file to ingest as one chunk.
        #[arg(long, default_value_t = 2000)]
        max_bytes: usize,
    },
    /// Chunk a file (with overlap) and index all chunks into Helix.
    IndexFile {
        /// Path to the file to index.
        path: String,
        /// Target chunk size in bytes.
        #[arg(long, default_value_t = 1200)]
        chunk_bytes: usize,
        /// Overlap between chunks in bytes.
        #[arg(long, default_value_t = 200)]
        overlap_bytes: usize,
        /// Use heuristic labels instead of LLM to speed up ingestion.
        #[arg(long, default_value_t = false)]
        no_llm_labels: bool,
    },
    /// Index the repository respecting .gitignore using chunked ingestion.
    IndexRepo {
        /// Target chunk size in bytes.
        #[arg(long, default_value_t = 1200)]
        chunk_bytes: usize,
        /// Overlap between chunks in bytes.
        #[arg(long, default_value_t = 200)]
        overlap_bytes: usize,
        /// Maximum file size to ingest (bytes); larger files are skipped.
        #[arg(long, default_value_t = 200_000)]
        max_file_bytes: u64,
        /// Use heuristic labels instead of LLM to speed up ingestion.
        #[arg(long, default_value_t = false)]
        no_llm_labels: bool,
        /// Only ingest files changed since the given git ref (e.g., HEAD~1).
        #[arg(long)]
        changed_since: Option<String>,
        /// Threshold for non-printable ratio (0-1) to consider a file binary.
        #[arg(long, default_value_t = 0.33)]
        binary_threshold: f64,
        /// Allow ingesting files detected as binary.
        #[arg(long, default_value_t = false)]
        allow_binary: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();
    let cli = Cli::parse();

    let llm_client =
        build_llm_client_from_env(false).context("LLM client initialization failed")?;
    let rag_agent = build_rag_agent_from_env(false)
        .await
        .context("Failed to initialize RAG agent")?;

    let topic_registry = HelixConfig::from_env()
        .ok()
        .and_then(|cfg| TopicRegistry::new(cfg).ok())
        .map(std::sync::Arc::new);

    let agent = Agent::new(llm_client.clone(), rag_agent.clone(), topic_registry);
    let mut router = OrchestratorRouter::new(agent)
        .with_specialist("CTOAgent", CTOAgent::new(llm_client.clone()))
        .with_specialist(
            "SeniorEngineerAgent",
            SeniorEngineerAgent::new(llm_client.clone()),
        )
        .with_specialist("ResearcherAgent", ResearcherAgent::new(llm_client.clone()))
        .with_specialist("OpsChainAgent", OpsChainAgent::new(llm_client.clone()));

    match rag_agent {
        Some(rag_agent_handle) => {
            info!("RAG enabled (Helix + embeddings)");
            router = router.with_rag_agent(rag_agent_handle);
        }
        None => {
            warn!("RAG configuration not detected; continuing without persistent memory");
        }
    }

    if let Ok(Some(semantic_router)) = SemanticRouter::from_env() {
        info!("Semantic routing enabled via ROUTING_SEMANTIC_ENABLED");
        router = router.with_semantic_router(semantic_router);
    }

    if let Some(command) = cli.command {
        match command {
            Commands::RagSmoke => {
                let rag_agent = build_rag_agent_from_env(false)
                    .await?
                    .context("RAG configuration required for smoke test")?;
                run_memory_smoke(rag_agent).await?;
                return Ok(());
            }
            Commands::HelixSmoke => {
                run_helix_smoke().await?;
                return Ok(());
            }
            Commands::HelixRichSmoke => {
                run_helix_rich_smoke().await?;
                return Ok(());
            }
            Commands::IndexChunk { path, max_bytes } => {
                let rag_agent = build_rag_agent_from_env(false)
                    .await?
                    .context("RAG configuration required for indexing")?;
                run_index_chunk(rag_agent, llm_client.clone(), path, max_bytes).await?;
                return Ok(());
            }
            Commands::IndexFile {
                path,
                chunk_bytes,
                overlap_bytes,
                no_llm_labels,
            } => {
                let rag_agent = build_rag_agent_from_env(false)
                    .await?
                    .context("RAG configuration required for indexing")?;
                run_index_file(
                    rag_agent,
                    llm_client.clone(),
                    path,
                    chunk_bytes,
                    overlap_bytes,
                    !no_llm_labels,
                )
                .await?;
                return Ok(());
            }
            Commands::IndexRepo {
                chunk_bytes,
                overlap_bytes,
                max_file_bytes,
                no_llm_labels,
                changed_since,
                binary_threshold,
                allow_binary,
            } => {
                let rag_agent = build_rag_agent_from_env(false)
                    .await?
                    .context("RAG configuration required for indexing")?;
                let opts = IndexRepoOptions {
                    chunk_bytes,
                    overlap_bytes,
                    max_file_bytes,
                    changed_since,
                    binary_threshold,
                    allow_binary,
                    use_llm_labels: !no_llm_labels,
                };
                run_index_repo(rag_agent, llm_client.clone(), opts).await?;
                return Ok(());
            }
        }
    }

    if let Some(prompt) = cli.prompt {
        run_single(&router, prompt).await?;
        return Ok(());
    }

    run_repl(&router).await
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}

async fn run_single(router: &OrchestratorRouter, prompt: String) -> anyhow::Result<()> {
    let response: AgentResponse = router
        .dispatch(AgentRequest::new(prompt))
        .await
        .map_err(|err| {
            error!(?err, "Agent request failed");
            err
        })?
        .into_output();

    println!("\nAgent:\n{}\n", response.output);
    Ok(())
}

async fn run_memory_smoke(rag_agent: SharedRagAgent) -> anyhow::Result<()> {
    let timestamp = Utc::now();
    let topic = format!("rag.smoke.{}", timestamp.timestamp());
    let summary = format!("Smoke test at {}", timestamp.to_rfc3339());

    let record = MemoryRecord {
        id: None,
        agent_name: "SmokeTester".to_string(),
        topic: topic.clone(),
        project: Some("rag-smoke".to_string()),
        conversation_id: None,
        timestamp,
        summary: summary.clone(),
        full_content: format!(
            "Memory smoke test recorded at {} for topic {}",
            timestamp.to_rfc3339(),
            topic
        ),
        confidence: 0.99,
        open_questions: Vec::new(),
        perspectives: Vec::new(),
        messages: Vec::new(),
        artifacts: Vec::new(),
        tool_calls: Vec::new(),
        metadata: Some(json!({
            "kind": "smoke_test",
            "timestamp": timestamp.to_rfc3339(),
        })),
    };

    println!("Writing smoke memory to RAG...");
    let write_response = rag_agent
        .handle(MemoryRequest::Write(MemoryWriteRequest {
            record: record.clone(),
        }))
        .await?;
    println!("{}", write_response.notes);

    println!("Querying memories back from RAG...");
    let filters = MemoryFilters {
        agent_name: Some(record.agent_name.clone()),
        topic: Some(topic.clone()),
        ..MemoryFilters::default()
    };

    let read_response = rag_agent
        .handle(MemoryRequest::Retrieve(MemoryQuery {
            query: summary,
            filters,
            limit: 5,
        }))
        .await?;

    println!(
        "Retrieved {} memories matching the smoke test filters.",
        read_response.records.len()
    );

    for memory in &read_response.records {
        println!(
            "- [{}] topic={} summary={}",
            memory.timestamp.to_rfc3339(),
            memory.topic,
            memory.summary
        );
    }

    Ok(())
}

async fn run_repl(router: &OrchestratorRouter) -> anyhow::Result<()> {
    println!("Vidkosha Cortex CLI ready. Type 'exit' to quit.\n");
    let stdin = io::stdin();

    loop {
        print!("You > ");
        io::stdout().flush()?;

        let mut buffer = String::new();
        stdin.read_line(&mut buffer)?;
        let trimmed = buffer.trim();

        if trimmed.eq_ignore_ascii_case("exit") {
            info!("User exited CLI");
            break;
        }

        if trimmed.is_empty() {
            continue;
        }

        run_single(router, trimmed.to_owned()).await?;
    }

    Ok(())
}

async fn run_helix_smoke() -> anyhow::Result<()> {
    let helix_config = HelixConfig::from_env().context("Helix configuration missing")?;
    let client = HelixClient::new(helix_config.clone()).context("Failed to build Helix client")?;

    println!(
        "Checking Helix connectivity at {} (namespace: {})...",
        helix_config.base_url, helix_config.namespace
    );

    // Try the introspect endpoint instead of health check
    match client.check_connectivity().await {
        Ok(status) if status.is_success() => {
            println!("✔ HelixDB is running (introspect endpoint responded)");
        }
        Ok(status) => {
            println!("⚠️  HelixDB responded with status: {}", status);
        }
        Err(e) => {
            println!("⚠️  Could not reach HelixDB introspect endpoint: {}", e);
        }
    }
    println!("✔ Connectivity check completed");

    let rag_agent = build_rag_agent_from_env(false)
        .await?
        .context("Helix + embeddings configuration required for helix-smoke")?;

    let timestamp = Utc::now();
    let topic = format!("helix.smoke.{}", timestamp.timestamp());
    let summary = format!("Helix memory chunk smoke @ {}", timestamp.to_rfc3339());

    let chunk_id = format!("chunk-{}", timestamp.timestamp_millis());

    let record = MemoryRecord {
        id: None,
        agent_name: "HelixSmokeTester".to_string(),
        topic: topic.clone(),
        project: Some("helix-smoke".to_string()),
        conversation_id: None,
        timestamp,
        summary: summary.clone(),
        full_content: format!(
            "Helix memory chunk smoke test inserted at {} (topic={})",
            timestamp.to_rfc3339(),
            topic
        ),
        confidence: 0.95,
        open_questions: vec![],
        perspectives: Vec::new(),
        messages: Vec::new(),
        artifacts: Vec::new(),
        tool_calls: Vec::new(),
        metadata: Some(json!({
            "kind": "helix_chunk_smoke",
            "timestamp": timestamp.to_rfc3339(),
        })),
    };

    println!("Writing smoke chunk via InsertMemoryChunk...");
    let write_response = rag_agent
        .handle(MemoryRequest::Write(MemoryWriteRequest {
            record: record.clone(),
        }))
        .await?;
    println!("✔ {}", write_response.notes);

    println!("Querying smoke chunk back via SearchMemoryChunk...");
    let filters = MemoryFilters {
        agent_name: Some(record.agent_name.clone()),
        topic: Some(topic.clone()),
        project: record.project.clone(),
        ..MemoryFilters::default()
    };

    let read_response = rag_agent
        .handle(MemoryRequest::Retrieve(MemoryQuery {
            query: summary.clone(),
            filters,
            limit: 5,
        }))
        .await?;

    println!(
        "✔ Retrieved {} memories for topic '{}'",
        read_response.records.len(),
        topic
    );
    if let Some(first) = read_response.records.first() {
        let neighbor_count = first
            .metadata
            .as_ref()
            .and_then(|meta| meta.get("helix_neighbors"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        if neighbor_count > 0 {
            println!("Neighbors attached: {}", neighbor_count);
        } else {
            println!("Neighbors attached: 0 (enable RAG_NEIGHBOR_DEPTH>0 to fetch)");
        }
    }
    for memory in &read_response.records {
        println!("- {} :: {}", memory.timestamp.to_rfc3339(), memory.summary);
    }

    println!("Cleaning up smoke chunk via DeleteMemoryChunk...");
    match client
        .post_query::<_, serde_json::Value>("DeleteMemoryChunk", &json!({ "chunk_id": chunk_id }))
        .await
    {
        Ok(resp) => println!("✔ DeleteMemoryChunk responded: {}", resp),
        Err(err) => println!("⚠️  Failed to delete smoke chunk: {err}"),
    }

    println!("Helix smoke test complete (Insert/Search/Delete MemoryChunk path).");
    Ok(())
}

async fn run_helix_rich_smoke() -> anyhow::Result<()> {
    let helix_config = HelixConfig::from_env().context("Helix configuration missing")?;
    let client = HelixClient::new(helix_config.clone()).context("Failed to build Helix client")?;

    println!(
        "Running Helix rich smoke at {} (namespace: {})...",
        helix_config.base_url, helix_config.namespace
    );

    let timestamp = Utc::now();
    let topic = format!("helix.rich.{}", timestamp.timestamp());
    let conversation_id = format!("conv-{}", timestamp.timestamp());
    let summary = format!("Helix rich smoke @ {}", timestamp.to_rfc3339());

    let chunk_id = format!("chunk-{}", timestamp.timestamp_millis());

    let record = MemoryRecord {
        id: None,
        agent_name: "HelixRichSmokeTester".to_string(),
        topic: topic.clone(),
        project: Some("helix-rich-smoke".to_string()),
        conversation_id: Some(conversation_id.clone()),
        timestamp,
        summary: summary.clone(),
        full_content: format!(
            "Helix rich smoke test inserted at {} (topic={}, conversation={})",
            timestamp.to_rfc3339(),
            topic,
            conversation_id
        ),
        confidence: 0.93,
        open_questions: vec!["Did neighbor summaries round-trip?".to_string()],
        perspectives: Vec::new(),
        messages: vec![
            rag::types::MessageRecord {
                message_id: Some("msg-1".to_string()),
                role: "user".to_string(),
                content: "How do we wire rich Helix writes?".to_string(),
                created_at: Some(timestamp),
                conversation_id: Some(conversation_id.clone()),
                reply_to: None,
                metadata: Some(json!({"channel": "cli", "kind": "helix_rich_smoke"})),
            },
            rag::types::MessageRecord {
                message_id: Some("msg-2".to_string()),
                role: "assistant".to_string(),
                content: "By adding optional conversation/messages/artifacts/tool_calls."
                    .to_string(),
                created_at: Some(timestamp + chrono::Duration::seconds(1)),
                conversation_id: Some(conversation_id.clone()),
                reply_to: Some("msg-1".to_string()),
                metadata: Some(json!({"channel": "cli"})),
            },
        ],
        artifacts: vec![rag::types::ArtifactRef {
            uri: "https://example.com/rich-smoke/artifact".to_string(),
            kind: Some("note".to_string()),
            checksum: Some("sha256:rich-smoke".to_string()),
            size_bytes: Some(1234),
            title: Some("Rich smoke artifact".to_string()),
            metadata: Some(json!({"kind": "helix_rich_smoke"})),
        }],
        tool_calls: vec![rag::types::ToolCallRecord {
            tool_call_id: Some("tc-1".to_string()),
            tool_name: "helix_smoke_tool".to_string(),
            args_json: json!({"mode": "rich"}),
            result_summary: Some("Ran rich smoke".to_string()),
            created_at: Some(timestamp + chrono::Duration::seconds(2)),
            metadata: Some(json!({"kind": "helix_rich_smoke"})),
        }],
        metadata: Some(json!({
            "kind": "helix_rich_smoke",
            "timestamp": timestamp.to_rfc3339(),
        })),
    };

    let write_query =
        std::env::var("HELIX_RICH_WRITE_QUERY").unwrap_or_else(|_| "write_memory_v2".to_string());
    let search_query =
        std::env::var("HELIX_RICH_SEARCH_QUERY").unwrap_or_else(|_| "search_memory_v2".to_string());
    let delete_query =
        std::env::var("HELIX_RICH_DELETE_QUERY").unwrap_or_else(|_| "delete_memory_v2".to_string());

    println!(
        "Writing rich smoke memory via HelixQL query '{}'...",
        write_query
    );

    let embed_config = RagConfig::from_env()?;
    let embedder = OpenAiEmbeddingsClient::from_config(&embed_config)?;
    let embed_text = format!("{}\n\n{}", record.summary, record.full_content);
    let vector: Vec<f64> = embedder
        .embed(&embed_text)
        .await?
        .into_iter()
        .map(|v| v as f64)
        .collect();
    let metadata_json = record
        .metadata
        .as_ref()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "{}".to_string());

    let payload = json!({
        "vector": vector,
        "agent_name": record.agent_name,
        "agent_role": "helix_rich_smoke",
        "agent_version": "smoke",
        "routing_intent": "smoke",
        "topic": record.topic,
        "project": record.project.clone().unwrap_or_default(),
        "summary": record.summary,
        "full_content": record.full_content,
        "timestamp": record.timestamp.to_rfc3339(),
        "confidence": record.confidence,
        "open_questions": record.open_questions,
        "metadata": metadata_json,
        "payload_hash": "sha256:rich-smoke",
        "chunk_id": chunk_id,
        "artifact_id": "artifact-rich-smoke",
        "conversation_id": record.conversation_id.clone().unwrap_or_else(|| conversation_id.clone()),
    });

    let write_resp: serde_json::Value = client.post_query(&write_query, &payload).await?;
    let memory_id = write_resp
        .get("memory_id")
        .or_else(|| write_resp.get("node_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("✔ Rich smoke memory stored with node_id={memory_id}");

    println!(
        "Querying rich smoke memory back via HelixQL query '{}'...",
        search_query
    );
    let search_resp: Value = client
        .post_query(
            &search_query,
            &json!({
                "vector": vector,
                "limit": 5,
            }),
        )
        .await?;

    let matches: Vec<Value> = if let Some(arr) = search_resp.as_array() {
        arr.clone()
    } else if let Some(arr) = search_resp
        .get("records")
        .or_else(|| search_resp.get("memories"))
        .or_else(|| search_resp.get("data"))
        .or_else(|| search_resp.get("items"))
        .or_else(|| search_resp.get("matches"))
        .and_then(|v| v.as_array())
    {
        arr.clone()
    } else {
        bail!(
            "Failed to deserialize Helix query '{}' response: expected array, got {}",
            search_query,
            search_resp
        );
    };

    let parse_timestamp = |value: &Value| -> chrono::DateTime<Utc> {
        value
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
    };

    let query_response: Vec<MemoryRecord> = matches
        .iter()
        .map(|item| {
            let summary = item
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let full_content = item
                .get("full_content")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| summary.clone());

            let timestamp = item
                .get("timestamp")
                .map(parse_timestamp)
                .unwrap_or_else(Utc::now);

            let metadata = item.get("metadata").map(|meta| match meta {
                Value::String(s) => {
                    serde_json::from_str::<Value>(s).unwrap_or(Value::String(s.clone()))
                }
                other => other.clone(),
            });

            let open_questions = item
                .get("open_questions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let confidence = item
                .get("confidence")
                .and_then(|v| v.as_f64())
                .or_else(|| item.get("score").and_then(|v| v.as_f64()))
                .unwrap_or(0.0) as f32;

            MemoryRecord {
                id: item
                    .get("id")
                    .or_else(|| item.get("memory_id"))
                    .or_else(|| item.get("chunk_id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                agent_name: item
                    .get("agent_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                topic: item
                    .get("topic")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                project: item
                    .get("project")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                conversation_id: item
                    .get("conversation_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                timestamp,
                summary,
                full_content,
                confidence,
                open_questions,
                perspectives: Vec::new(),
                messages: Vec::new(),
                artifacts: Vec::new(),
                tool_calls: Vec::new(),
                metadata,
            }
        })
        .collect();

    println!(
        "✔ Retrieved {} memories for rich smoke topic",
        query_response.len()
    );

    if let Some(first) = query_response.first() {
        let neighbor_count = first
            .metadata
            .as_ref()
            .and_then(|meta| meta.get("helix_neighbors"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        println!(
            "Context: messages={}, artifacts={}, tool_calls={}, neighbors={}",
            first.messages.len(),
            first.artifacts.len(),
            first.tool_calls.len(),
            neighbor_count
        );
    }

    let prune_orphans = std::env::var("HELIX_RICH_PRUNE_ORPHANS")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    println!(
        "Cleaning up rich smoke memory via HelixQL query '{}' (prune_orphans={})...",
        delete_query, prune_orphans
    );
    if let Err(err) = client
        .post_query::<_, serde_json::Value>(
            &delete_query,
            &json!({ "memory_id": memory_id, "prune_orphans": prune_orphans }),
        )
        .await
    {
        warn!(?err, "Failed to delete Helix rich smoke memory");
    } else {
        println!("✔ Rich smoke memory cleaned up");
    }

    println!("Helix rich smoke test complete.");
    Ok(())
}

#[derive(Debug, Clone)]
struct LabeledChunk {
    topic: String,
    project: String,
    summary: String,
    open_questions: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct IngestConfig {
    allow_extensions: Option<Vec<String>>,
    deny_extensions: Option<Vec<String>>,
    max_file_bytes: Option<u64>,
    manifest_path: Option<String>,
    binary_threshold: Option<f64>,
    allow_binary: Option<bool>,
    handlers_disabled: Option<Vec<String>>,
    handler_overrides: Option<HashMap<String, HandlerConfig>>, // keyed by handler name
    force_handlers: Option<HashMap<String, String>>,           // ext -> handler name
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct HandlerConfig {
    chunk_bytes: Option<usize>,
    overlap_bytes: Option<usize>,
    max_file_bytes: Option<u64>,
    heading_depth: Option<usize>,
    max_rows_per_chunk: Option<usize>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct ManifestEntry {
    hash: String,
    mtime: u64,
    chunk_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct IngestManifest {
    version: u8,
    files: HashMap<String, ManifestEntry>,
}

#[derive(Debug, Clone)]
struct HandlerContext {
    allow_binary: bool,
    binary_threshold: f64,
}

#[derive(Debug, Clone)]
struct PreparedChunk {
    text: String,
    chunk_index: usize,
    chunk_id_hint: Option<String>,
    metadata: JsonMap<String, Value>,
}

trait IngestHandler {
    fn name(&self) -> &'static str;
    fn supports(&self, path: &str, bytes: &[u8], ctx: &HandlerContext) -> bool;
    fn process(
        &self,
        path: &str,
        bytes: &[u8],
        ctx: &HandlerContext,
    ) -> anyhow::Result<Vec<PreparedChunk>>;
}

async fn run_index_chunk(
    rag_agent: SharedRagAgent,
    llm_client: SharedLlmClient,
    path: String,
    max_bytes: usize,
) -> anyhow::Result<()> {
    let content = read_chunk(&path, max_bytes)?;
    if content.trim().is_empty() {
        bail!("File {path} is empty after truncation");
    }

    println!(
        "Indexing chunk from {} ({} bytes, limit={})...",
        path,
        content.len(),
        max_bytes
    );

    let labels = label_chunk_with_mode(llm_client.as_ref(), &path, &content, true).await?;
    let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
    let timestamp = Utc::now();

    let record = MemoryRecord {
        id: None,
        agent_name: "Indexer".to_string(),
        topic: labels.topic.clone(),
        project: Some(labels.project.clone()),
        conversation_id: None,
        timestamp,
        summary: labels.summary.clone(),
        full_content: content.clone(),
        confidence: 0.99,
        open_questions: labels.open_questions.clone(),
        perspectives: Vec::new(),
        messages: Vec::new(),
        artifacts: Vec::new(),
        tool_calls: Vec::new(),
        metadata: Some(json!({
            "path": path,
            "hash": format!("sha256:{}", hash),
            "chunk_bytes": content.len(),
            "label_source": "llm_indexer",
            "body": content,
        })),
    };

    let response = rag_agent
        .handle(MemoryRequest::Write(MemoryWriteRequest { record }))
        .await?;

    println!("✔ {}", response.notes);
    println!(
        "Labels -> topic='{}' project='{}' summary='{}'",
        labels.topic, labels.project, labels.summary
    );

    Ok(())
}

async fn run_index_file(
    rag_agent: SharedRagAgent,
    llm_client: SharedLlmClient,
    path: String,
    chunk_bytes: usize,
    overlap_bytes: usize,
    use_llm_labels: bool,
) -> anyhow::Result<()> {
    let content = fs::read_to_string(Path::new(&path))
        .with_context(|| format!("Failed to read file {path}"))?;
    if content.trim().is_empty() {
        bail!("File {path} is empty");
    }

    let chunks = chunk_with_overlap(&content, chunk_bytes, overlap_bytes);
    println!(
        "Indexing file {} as {} chunks (size={} overlap={})...",
        path,
        chunks.len(),
        chunk_bytes,
        overlap_bytes
    );

    for (idx, chunk) in chunks.iter().enumerate() {
        let labels =
            label_chunk_with_mode(llm_client.as_ref(), &path, chunk, use_llm_labels).await?;
        let hash = blake3::hash(chunk.as_bytes()).to_hex().to_string();
        let timestamp = Utc::now();
        let chunk_id = format!("{}#chunk-{}", path, idx);

        let record = MemoryRecord {
            id: None,
            agent_name: "Indexer".to_string(),
            topic: labels.topic.clone(),
            project: Some(labels.project.clone()),
            conversation_id: None,
            timestamp,
            summary: labels.summary.clone(),
            full_content: chunk.clone(),
            confidence: 0.99,
            open_questions: labels.open_questions.clone(),
            perspectives: Vec::new(),
            messages: Vec::new(),
            artifacts: Vec::new(),
            tool_calls: Vec::new(),
            metadata: Some(json!({
                "path": path,
                "hash": format!("sha256:{}", hash),
                "chunk_bytes": chunk.len(),
                "label_source": "llm_indexer",
                "body": chunk,
                "chunk_index": idx,
                "chunk_id": chunk_id,
            })),
        };

        let response = rag_agent
            .handle(MemoryRequest::Write(MemoryWriteRequest { record }))
            .await?;

        println!("✔ chunk {} stored ({})", idx, response.notes);
    }

    println!("Completed indexing {}", path);
    Ok(())
}

struct IndexRepoOptions {
    chunk_bytes: usize,
    overlap_bytes: usize,
    max_file_bytes: u64,
    changed_since: Option<String>,
    binary_threshold: f64,
    allow_binary: bool,
    use_llm_labels: bool,
}

async fn run_index_repo(
    rag_agent: SharedRagAgent,
    llm_client: SharedLlmClient,
    opts: IndexRepoOptions,
) -> anyhow::Result<()> {
    let ingest_config = load_ingest_config();
    let handler_ctx = HandlerContext {
        allow_binary: ingest_config.allow_binary.unwrap_or(opts.allow_binary),
        binary_threshold: ingest_config
            .binary_threshold
            .unwrap_or(opts.binary_threshold)
            .clamp(0.0, 1.0),
    };
    let handlers = build_handlers(
        &ingest_config,
        opts.chunk_bytes,
        opts.overlap_bytes,
        &handler_ctx,
    );
    let mut manifest = load_manifest(ingest_config.manifest_path.as_deref());
    let files = git_ls_files()?;
    if files.is_empty() {
        bail!("git ls-files returned no files (check repository)");
    }

    let changed_only: Option<HashSet<String>> = match opts.changed_since.as_deref() {
        Some(git_ref) => Some(git_changed_since(git_ref)?),
        None => None,
    };

    println!(
        "Indexing repository files ({} files, chunk={} overlap={}, max_file_bytes={})...",
        files.len(),
        opts.chunk_bytes,
        opts.overlap_bytes,
        opts.max_file_bytes
    );

    let mut seen_hashes: HashSet<String> = HashSet::new();
    let mut files_processed = 0usize;
    let mut chunks_stored = 0usize;
    for path in files {
        if let Some(changed) = changed_only.as_ref() {
            if !changed.contains(&path) {
                continue;
            }
        }

        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        if !should_consider_file(
            &path,
            &meta,
            &data,
            &ingest_config,
            opts.max_file_bytes,
            changed_only.as_ref(),
            &handler_ctx,
        ) {
            continue;
        }

        let file_hash = blake3::hash(&data).to_hex().to_string();
        let mtime = file_mtime(&meta).unwrap_or_default();

        if is_unchanged_in_manifest(&path, &manifest, &file_hash, mtime) {
            continue;
        }

        let handler = match resolve_handler(&handlers, &ingest_config, &path, &data, &handler_ctx) {
            Some(h) => h,
            None => continue,
        };

        let prepared_chunks = handler.process(&path, &data, &handler_ctx)?;
        if prepared_chunks.is_empty() {
            continue;
        }

        let mut chunk_ids_for_manifest = Vec::new();
        for (idx, prepared) in prepared_chunks.iter().enumerate() {
            let chunk = &prepared.text;
            let hash = blake3::hash(chunk.as_bytes()).to_hex().to_string();
            if !seen_hashes.insert(hash.clone()) {
                continue;
            }

            let labels =
                label_chunk_with_mode(llm_client.as_ref(), &path, chunk, opts.use_llm_labels)
                    .await?;
            let timestamp = Utc::now();

            let chunk_id = prepared.chunk_id_hint.clone().unwrap_or_else(|| {
                format!(
                    "{}#chunk-{}-{}",
                    path,
                    idx,
                    &file_hash[..8.min(file_hash.len())]
                )
            });

            let mut metadata: JsonMap<String, Value> = prepared.metadata.clone();
            metadata
                .entry("path".to_string())
                .or_insert_with(|| json!(path));
            metadata
                .entry("file_hash".to_string())
                .or_insert_with(|| json!(format!("sha256:{}", file_hash)));
            metadata
                .entry("hash".to_string())
                .or_insert_with(|| json!(format!("sha256:{}", hash)));
            metadata
                .entry("chunk_bytes".to_string())
                .or_insert_with(|| json!(chunk.len()));
            metadata
                .entry("label_source".to_string())
                .or_insert_with(|| {
                    json!(if opts.use_llm_labels {
                        "llm_indexer"
                    } else {
                        "heuristic"
                    })
                });
            metadata
                .entry("body".to_string())
                .or_insert_with(|| json!(chunk));
            metadata
                .entry("chunk_index".to_string())
                .or_insert_with(|| json!(prepared.chunk_index));
            metadata
                .entry("chunk_id".to_string())
                .or_insert_with(|| json!(chunk_id));
            metadata
                .entry("file_len".to_string())
                .or_insert_with(|| json!(meta.len()));

            let record = MemoryRecord {
                id: None,
                agent_name: "Indexer".to_string(),
                topic: labels.topic.clone(),
                project: Some(labels.project.clone()),
                conversation_id: None,
                timestamp,
                summary: labels.summary.clone(),
                full_content: chunk.clone(),
                confidence: 0.99,
                open_questions: labels.open_questions.clone(),
                perspectives: Vec::new(),
                messages: Vec::new(),
                artifacts: Vec::new(),
                tool_calls: Vec::new(),
                metadata: Some(Value::Object(metadata.clone())),
            };

            let response = rag_agent
                .handle(MemoryRequest::Write(MemoryWriteRequest { record }))
                .await?;

            chunks_stored += 1;
            println!(
                "✔ {} [{}] chunk {} stored ({})",
                path,
                handler.name(),
                idx,
                response.notes
            );
            chunk_ids_for_manifest.push(chunk_id);
        }

        manifest.files.insert(
            path.clone(),
            ManifestEntry {
                hash: file_hash,
                mtime,
                chunk_ids: chunk_ids_for_manifest,
            },
        );

        files_processed += 1;
    }

    save_manifest(ingest_config.manifest_path.as_deref(), &manifest)?;

    println!(
        "Indexing complete. Files processed: {}. Chunks stored: {} (unique by hash).",
        files_processed, chunks_stored
    );

    Ok(())
}

fn read_chunk(path: &str, max_bytes: usize) -> anyhow::Result<String> {
    let content = fs::read_to_string(Path::new(path))
        .with_context(|| format!("Failed to read file {path}"))?;
    if content.len() <= max_bytes {
        return Ok(content);
    }

    let mut truncated = content;
    truncated.truncate(max_bytes);
    Ok(truncated)
}

fn chunk_with_overlap(content: &str, chunk_bytes: usize, overlap_bytes: usize) -> Vec<String> {
    if chunk_bytes == 0 {
        return Vec::new();
    }

    let bytes = content.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < bytes.len() {
        let end = (start + chunk_bytes).min(bytes.len());
        let slice = &bytes[start..end];
        let chunk = String::from_utf8_lossy(slice).to_string();
        chunks.push(chunk);

        if end == bytes.len() {
            break;
        }

        let overlap = overlap_bytes.min(chunk_bytes).min(end - start);
        start = end.saturating_sub(overlap);
    }

    chunks
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodeLanguage {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
}

#[derive(Debug, Clone)]
struct SymbolInfo {
    name: String,
    kind: String,
    start_byte: usize,
    end_byte: usize,
}

#[derive(Debug, Clone)]
struct SymbolChunk {
    text: String,
    symbol: SymbolInfo,
    part_index: usize,
    part_count: usize,
}

fn language_from_extension(path: &str) -> Option<CodeLanguage> {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    match ext.as_deref() {
        Some("rs") => Some(CodeLanguage::Rust),
        Some("ts") => Some(CodeLanguage::TypeScript),
        Some("tsx") => Some(CodeLanguage::Tsx),
        Some("js") => Some(CodeLanguage::JavaScript),
        Some("jsx") => Some(CodeLanguage::JavaScript),
        Some("py") => Some(CodeLanguage::Python),
        _ => None,
    }
}

fn language_name(lang: CodeLanguage) -> &'static str {
    match lang {
        CodeLanguage::Rust => "rust",
        CodeLanguage::TypeScript => "typescript",
        CodeLanguage::Tsx => "tsx",
        CodeLanguage::JavaScript => "javascript",
        CodeLanguage::Python => "python",
    }
}

fn tree_sitter_language(lang: CodeLanguage) -> TsLanguage {
    match lang {
        CodeLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        CodeLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        CodeLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        CodeLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        CodeLanguage::Python => tree_sitter_python::LANGUAGE.into(),
    }
}

fn symbol_node_kinds(lang: CodeLanguage) -> &'static [&'static str] {
    match lang {
        CodeLanguage::Rust => &[
            "function_item",
            "impl_item",
            "struct_item",
            "enum_item",
            "trait_item",
            "mod_item",
        ],
        CodeLanguage::TypeScript | CodeLanguage::Tsx => &[
            "function_declaration",
            "method_definition",
            "class_declaration",
            "arrow_function",
        ],
        CodeLanguage::JavaScript => &[
            "function_declaration",
            "method_definition",
            "class_declaration",
            "arrow_function",
        ],
        CodeLanguage::Python => &["function_definition", "class_definition"],
    }
}

fn node_text(node: &Node, content: &str) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    let bytes = content.as_bytes();
    if start >= bytes.len() || end > bytes.len() || start >= end {
        return String::new();
    }
    String::from_utf8_lossy(&bytes[start..end]).to_string()
}

fn symbol_name(node: &Node, content: &str) -> String {
    if let Some(name_node) = node.child_by_field_name("name") {
        let text = node_text(&name_node, content);
        if !text.trim().is_empty() {
            return text.trim().to_string();
        }
    }

    for field in ["identifier", "declarator", "property_identifier"] {
        if let Some(name_node) = node.child_by_field_name(field) {
            let text = node_text(&name_node, content);
            if !text.trim().is_empty() {
                return text.trim().to_string();
            }
        }
    }

    // Fallback: first named child
    if let Some(child) = node.named_child(0) {
        let text = node_text(&child, content);
        if !text.trim().is_empty() {
            return text.trim().to_string();
        }
    }

    node.kind().to_string()
}

fn extract_symbols(content: &str, lang: CodeLanguage) -> anyhow::Result<Vec<SymbolInfo>> {
    let mut parser = TsParser::new();
    parser
        .set_language(&tree_sitter_language(lang))
        .context("failed to set tree-sitter language")?;

    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return Ok(Vec::new()),
    };

    let mut symbols = Vec::new();
    let root = tree.root_node();
    let mut stack = vec![root];
    let symbol_kinds = symbol_node_kinds(lang);

    while let Some(node) = stack.pop() {
        if symbol_kinds.contains(&node.kind()) {
            let info = SymbolInfo {
                name: symbol_name(&node, content),
                kind: node.kind().to_string(),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            };
            symbols.push(info);
        }

        for idx in 0..node.named_child_count() {
            if let Some(child) = node.named_child(idx) {
                stack.push(child);
            }
        }
    }

    symbols.sort_by_key(|s| s.start_byte);
    Ok(symbols)
}

fn sanitize_symbol_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
}

fn chunk_code_symbols(
    content: &str,
    chunk_bytes: usize,
    overlap_bytes: usize,
    lang: CodeLanguage,
) -> anyhow::Result<Vec<SymbolChunk>> {
    let symbols = extract_symbols(content, lang)?;
    let mut chunks = Vec::new();
    if symbols.is_empty() {
        return Ok(chunks);
    }

    for sym in symbols {
        let bytes = content.as_bytes();
        if sym.end_byte > bytes.len() || sym.start_byte >= sym.end_byte {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes[sym.start_byte..sym.end_byte]).to_string();
        if text.trim().is_empty() {
            continue;
        }

        if chunk_bytes > 0 && text.len() > chunk_bytes {
            let parts = chunk_with_overlap(&text, chunk_bytes, overlap_bytes);
            let total_parts = parts.len();
            for (idx, part) in parts.into_iter().enumerate() {
                chunks.push(SymbolChunk {
                    text: part,
                    symbol: sym.clone(),
                    part_index: idx,
                    part_count: total_parts,
                });
            }
        } else {
            chunks.push(SymbolChunk {
                text,
                symbol: sym.clone(),
                part_index: 0,
                part_count: 1,
            });
        }
    }

    Ok(chunks)
}

#[allow(dead_code)]
fn is_probably_binary(bytes: &[u8]) -> bool {
    is_probably_binary_with_threshold(bytes, 0.33)
}

fn is_probably_binary_with_threshold(bytes: &[u8], threshold: f64) -> bool {
    if bytes.contains(&0) {
        return true;
    }

    let mut non_printable = 0usize;
    let mut total = 0usize;
    for b in bytes.iter().copied() {
        total += 1;
        // Treat common whitespace and printable ASCII as text.
        if b == b'\n' || b == b'\r' || b == b'\t' || (0x20..=0x7E).contains(&b) {
            continue;
        }
        non_printable += 1;
    }

    if total == 0 {
        return false;
    }

    let ratio = non_printable as f64 / total as f64;
    ratio >= threshold.clamp(0.0, 1.0)
}

fn should_consider_file(
    path: &str,
    meta: &fs::Metadata,
    data: &[u8],
    ingest_config: &IngestConfig,
    max_file_bytes_flag: u64,
    changed_only: Option<&HashSet<String>>,
    handler_ctx: &HandlerContext,
) -> bool {
    if let Some(changed) = changed_only {
        if !changed.contains(path) {
            return false;
        }
    }

    if !meta.is_file() {
        return false;
    }

    let file_len = meta.len();
    if file_len == 0 {
        return false;
    }

    let max_bytes = ingest_config.max_file_bytes.unwrap_or(max_file_bytes_flag);
    if file_len > max_bytes {
        return false;
    }

    if should_skip_extension(path, ingest_config) {
        return false;
    }

    if data.is_empty() {
        return false;
    }

    if !handler_ctx.allow_binary
        && is_probably_binary_with_threshold(data, handler_ctx.binary_threshold)
    {
        return false;
    }

    true
}

fn build_handlers(
    ingest_config: &IngestConfig,
    default_chunk_bytes: usize,
    default_overlap: usize,
    ctx: &HandlerContext,
) -> Vec<Box<dyn IngestHandler>> {
    let mut handlers: Vec<Box<dyn IngestHandler>> = Vec::new();

    if handler_enabled("code", ingest_config) {
        let opts = handler_options_for("code", ingest_config, default_chunk_bytes, default_overlap);
        handlers.push(Box::new(CodeHandler {
            chunk_bytes: opts.chunk_bytes.unwrap_or(default_chunk_bytes),
            overlap_bytes: opts.overlap_bytes.unwrap_or(default_overlap),
        }));
    }

    if handler_enabled("markdown", ingest_config) {
        let opts = handler_options_for(
            "markdown",
            ingest_config,
            default_chunk_bytes,
            default_overlap,
        );
        handlers.push(Box::new(MarkdownHandler {
            chunk_bytes: opts.chunk_bytes.unwrap_or(default_chunk_bytes),
            overlap_bytes: opts.overlap_bytes.unwrap_or(default_overlap),
            heading_depth: opts.heading_depth.unwrap_or(6),
        }));
    }

    if handler_enabled("data", ingest_config) {
        let opts = handler_options_for("data", ingest_config, default_chunk_bytes, default_overlap);
        handlers.push(Box::new(DataHandler {
            chunk_bytes: opts.chunk_bytes.unwrap_or(default_chunk_bytes),
            overlap_bytes: opts.overlap_bytes.unwrap_or(default_overlap),
            max_rows_per_chunk: opts.max_rows_per_chunk.unwrap_or(200),
        }));
    }

    if handler_enabled("text", ingest_config) {
        let opts = handler_options_for("text", ingest_config, default_chunk_bytes, default_overlap);
        handlers.push(Box::new(PlainTextHandler {
            chunk_bytes: opts.chunk_bytes.unwrap_or(default_chunk_bytes),
            overlap_bytes: opts.overlap_bytes.unwrap_or(default_overlap),
        }));
    }

    if ctx.allow_binary && handler_enabled("binary", ingest_config) {
        handlers.push(Box::new(BinaryHandler {}));
    }

    handlers
}

fn handler_enabled(name: &str, cfg: &IngestConfig) -> bool {
    if let Some(disabled) = cfg.handlers_disabled.as_ref() {
        return !disabled.iter().any(|n| n.eq_ignore_ascii_case(name));
    }
    true
}

fn handler_options_for(
    name: &str,
    cfg: &IngestConfig,
    default_chunk_bytes: usize,
    default_overlap: usize,
) -> HandlerConfig {
    let mut base = HandlerConfig {
        chunk_bytes: Some(default_chunk_bytes),
        overlap_bytes: Some(default_overlap),
        max_file_bytes: cfg.max_file_bytes,
        heading_depth: None,
        max_rows_per_chunk: None,
    };

    if let Some(map) = cfg.handler_overrides.as_ref() {
        if let Some(override_cfg) = map.get(name) {
            if let Some(cb) = override_cfg.chunk_bytes {
                base.chunk_bytes = Some(cb);
            }
            if let Some(ob) = override_cfg.overlap_bytes {
                base.overlap_bytes = Some(ob);
            }
            if let Some(mb) = override_cfg.max_file_bytes {
                base.max_file_bytes = Some(mb);
            }
            if override_cfg.heading_depth.is_some() {
                base.heading_depth = override_cfg.heading_depth;
            }
            if override_cfg.max_rows_per_chunk.is_some() {
                base.max_rows_per_chunk = override_cfg.max_rows_per_chunk;
            }
        }
    }

    base
}

fn resolve_handler<'a>(
    handlers: &'a [Box<dyn IngestHandler>],
    cfg: &IngestConfig,
    path: &str,
    data: &[u8],
    ctx: &HandlerContext,
) -> Option<&'a dyn IngestHandler> {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    if let Some(force) = cfg.force_handlers.as_ref() {
        if let Some(ext_val) = ext.as_ref() {
            if let Some(target) = force.get(ext_val) {
                if let Some(handler) = handlers
                    .iter()
                    .find(|h| h.name().eq_ignore_ascii_case(target))
                    .map(|h| h.as_ref())
                {
                    if handler.supports(path, data, ctx) {
                        return Some(handler);
                    }
                }
            }
        }
    }

    for handler in handlers {
        if handler.supports(path, data, ctx) {
            return Some(handler.as_ref());
        }
    }

    None
}

struct CodeHandler {
    chunk_bytes: usize,
    overlap_bytes: usize,
}

impl IngestHandler for CodeHandler {
    fn name(&self) -> &'static str {
        "code"
    }

    fn supports(&self, path: &str, bytes: &[u8], ctx: &HandlerContext) -> bool {
        if !ctx.allow_binary && is_probably_binary_with_threshold(bytes, ctx.binary_threshold) {
            return false;
        }
        if language_from_extension(path).is_none() {
            return false;
        }
        String::from_utf8(bytes.to_vec()).is_ok()
    }

    fn process(
        &self,
        path: &str,
        bytes: &[u8],
        _ctx: &HandlerContext,
    ) -> anyhow::Result<Vec<PreparedChunk>> {
        let content = String::from_utf8_lossy(bytes).to_string();
        let mut prepared = Vec::new();
        if let Some(lang) = language_from_extension(path) {
            match chunk_code_symbols(&content, self.chunk_bytes, self.overlap_bytes, lang) {
                Ok(symbol_chunks) if !symbol_chunks.is_empty() => {
                    for (idx, sc) in symbol_chunks.into_iter().enumerate() {
                        let sym_name = sanitize_symbol_name(&sc.symbol.name);
                        let suffix = format!(
                            "sym-{}-{}-p{}of{}",
                            sym_name, sc.symbol.start_byte, sc.part_index, sc.part_count
                        );
                        let mut meta = JsonMap::new();
                        meta.insert("ingest_mode".to_string(), json!("code"));
                        meta.insert("language".to_string(), json!(language_name(lang)));
                        meta.insert(
                            "ingest".to_string(),
                            json!({
                                "symbols": [{
                                    "name": sc.symbol.name,
                                    "kind": sc.symbol.kind,
                                    "start_byte": sc.symbol.start_byte,
                                    "end_byte": sc.symbol.end_byte,
                                    "part_index": sc.part_index,
                                    "part_count": sc.part_count,
                                }]
                            }),
                        );

                        prepared.push(PreparedChunk {
                            text: sc.text,
                            chunk_index: idx,
                            chunk_id_hint: Some(format!("{}#{}", path, suffix)),
                            metadata: meta,
                        });
                    }
                }
                _ => {
                    for (idx, chunk) in
                        chunk_with_overlap(&content, self.chunk_bytes, self.overlap_bytes)
                            .into_iter()
                            .enumerate()
                    {
                        let mut meta = JsonMap::new();
                        meta.insert("ingest_mode".to_string(), json!("code"));
                        meta.insert("language".to_string(), json!(language_name(lang)));
                        prepared.push(PreparedChunk {
                            text: chunk,
                            chunk_index: idx,
                            chunk_id_hint: None,
                            metadata: meta,
                        });
                    }
                }
            }
        }

        Ok(prepared)
    }
}

struct MarkdownHandler {
    chunk_bytes: usize,
    overlap_bytes: usize,
    heading_depth: usize,
}

impl IngestHandler for MarkdownHandler {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn supports(&self, path: &str, bytes: &[u8], ctx: &HandlerContext) -> bool {
        if !ctx.allow_binary && is_probably_binary_with_threshold(bytes, ctx.binary_threshold) {
            return false;
        }
        matches!(
            Path::new(path).extension().and_then(|s| s.to_str()),
            Some("md" | "markdown")
        ) && String::from_utf8(bytes.to_vec()).is_ok()
    }

    fn process(
        &self,
        _path: &str,
        bytes: &[u8],
        _ctx: &HandlerContext,
    ) -> anyhow::Result<Vec<PreparedChunk>> {
        let content = String::from_utf8_lossy(bytes).to_string();
        let mut sections: Vec<(String, String)> = Vec::new();
        let mut current_heading = String::new();
        let mut current_body = String::new();

        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                if !current_body.is_empty() {
                    sections.push((current_heading.clone(), current_body.clone()));
                    current_body.clear();
                }
                let depth = trimmed.chars().take_while(|c| *c == '#').count();
                if depth <= self.heading_depth {
                    current_heading = trimmed.trim_start_matches('#').trim().to_string();
                }
            }
            current_body.push_str(line);
            current_body.push('\n');
        }

        if !current_body.is_empty() {
            sections.push((current_heading.clone(), current_body.clone()));
        }

        let mut prepared = Vec::new();
        for (global_idx, (heading, body)) in sections.into_iter().enumerate() {
            for (idx, chunk) in chunk_with_overlap(&body, self.chunk_bytes, self.overlap_bytes)
                .into_iter()
                .enumerate()
            {
                let mut meta = JsonMap::new();
                meta.insert("ingest_mode".to_string(), json!("text"));
                if !heading.is_empty() {
                    meta.insert("markdown_heading".to_string(), json!(heading));
                }
                prepared.push(PreparedChunk {
                    text: chunk,
                    chunk_index: global_idx + idx,
                    chunk_id_hint: None,
                    metadata: meta,
                });
            }
        }

        Ok(prepared)
    }
}

struct PlainTextHandler {
    chunk_bytes: usize,
    overlap_bytes: usize,
}

impl IngestHandler for PlainTextHandler {
    fn name(&self) -> &'static str {
        "text"
    }

    fn supports(&self, path: &str, bytes: &[u8], ctx: &HandlerContext) -> bool {
        if !ctx.allow_binary && is_probably_binary_with_threshold(bytes, ctx.binary_threshold) {
            return false;
        }
        if std::str::from_utf8(bytes).is_err() {
            return false;
        }
        if let Some(ext) = Path::new(path).extension().and_then(|s| s.to_str()) {
            let ext_l = ext.to_ascii_lowercase();
            // Avoid overriding markdown/data if those handlers exist; selection order handles priority.
            return ext_l != "md" && ext_l != "markdown" && ext_l != "csv" && ext_l != "jsonl";
        }
        true
    }

    fn process(
        &self,
        _path: &str,
        bytes: &[u8],
        _ctx: &HandlerContext,
    ) -> anyhow::Result<Vec<PreparedChunk>> {
        let content = String::from_utf8_lossy(bytes).to_string();
        let mut prepared = Vec::new();
        for (idx, chunk) in chunk_with_overlap(&content, self.chunk_bytes, self.overlap_bytes)
            .into_iter()
            .enumerate()
        {
            let mut meta = JsonMap::new();
            meta.insert("ingest_mode".to_string(), json!("text"));
            prepared.push(PreparedChunk {
                text: chunk,
                chunk_index: idx,
                chunk_id_hint: None,
                metadata: meta,
            });
        }
        Ok(prepared)
    }
}

struct DataHandler {
    #[allow(dead_code)]
    chunk_bytes: usize,
    #[allow(dead_code)]
    overlap_bytes: usize,
    max_rows_per_chunk: usize,
}

impl IngestHandler for DataHandler {
    fn name(&self) -> &'static str {
        "data"
    }

    fn supports(&self, path: &str, bytes: &[u8], ctx: &HandlerContext) -> bool {
        if !ctx.allow_binary && is_probably_binary_with_threshold(bytes, ctx.binary_threshold) {
            return false;
        }
        let ext_ok = matches!(
            Path::new(path).extension().and_then(|s| s.to_str()),
            Some("csv" | "json" | "jsonl")
        );
        ext_ok && String::from_utf8(bytes.to_vec()).is_ok()
    }

    fn process(
        &self,
        path: &str,
        bytes: &[u8],
        _ctx: &HandlerContext,
    ) -> anyhow::Result<Vec<PreparedChunk>> {
        let content = String::from_utf8_lossy(bytes).to_string();
        let ext = Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let data_format = if ext == "csv" {
            "csv"
        } else if ext == "jsonl" {
            "jsonl"
        } else {
            "json"
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut prepared = Vec::new();
        let mut start = 0usize;
        let total = lines.len();

        while start < total {
            let end = (start + self.max_rows_per_chunk).min(total);
            let slice = &lines[start..end];
            let chunk = slice.join("\n");
            let mut meta = JsonMap::new();
            meta.insert("ingest_mode".to_string(), json!("data"));
            meta.insert("data_format".to_string(), json!(data_format));
            meta.insert("row_range".to_string(), json!([start, end]));

            prepared.push(PreparedChunk {
                text: chunk,
                chunk_index: prepared.len(),
                chunk_id_hint: None,
                metadata: meta,
            });

            start = end;
        }

        // fallback: if empty, treat as text
        if prepared.is_empty() {
            let mut meta = JsonMap::new();
            meta.insert("ingest_mode".to_string(), json!("data"));
            prepared.push(PreparedChunk {
                text: content,
                chunk_index: 0,
                chunk_id_hint: None,
                metadata: meta,
            });
        }

        Ok(prepared)
    }
}

struct BinaryHandler {}

impl IngestHandler for BinaryHandler {
    fn name(&self) -> &'static str {
        "binary"
    }

    fn supports(&self, _path: &str, bytes: &[u8], ctx: &HandlerContext) -> bool {
        is_probably_binary_with_threshold(bytes, ctx.binary_threshold)
    }

    fn process(
        &self,
        path: &str,
        bytes: &[u8],
        _ctx: &HandlerContext,
    ) -> anyhow::Result<Vec<PreparedChunk>> {
        let mut meta = JsonMap::new();
        meta.insert("ingest_mode".to_string(), json!("binary"));
        meta.insert("binary_size".to_string(), json!(bytes.len()));
        meta.insert("binary_path".to_string(), json!(path));

        Ok(vec![PreparedChunk {
            text: format!("<binary file: {}>", path),
            chunk_index: 0,
            chunk_id_hint: None,
            metadata: meta,
        }])
    }
}

fn git_changed_since(reference: &str) -> anyhow::Result<HashSet<String>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", reference])
        .output()
        .context("git diff --name-only failed")?;

    if !output.status.success() {
        bail!("git diff exited with status {}", output.status);
    }

    let mut paths = HashSet::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        paths.insert(trimmed.to_string());
    }
    Ok(paths)
}

fn git_ls_files() -> anyhow::Result<Vec<String>> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .output()
        .context("git ls-files failed")?;

    if !output.status.success() {
        bail!("git ls-files exited with status {}", output.status);
    }

    let mut files = Vec::new();
    for part in output.stdout.split(|b| *b == 0) {
        if part.is_empty() {
            continue;
        }
        if let Ok(path) = std::str::from_utf8(part) {
            files.push(path.to_string());
        }
    }

    Ok(files)
}

#[allow(dead_code)]
async fn label_chunk(
    llm: &dyn LlmClient,
    path: &str,
    content: &str,
) -> anyhow::Result<LabeledChunk> {
    label_chunk_llm(llm, path, content).await
}

async fn label_chunk_with_mode(
    llm: &dyn LlmClient,
    path: &str,
    content: &str,
    use_llm: bool,
) -> anyhow::Result<LabeledChunk> {
    if use_llm {
        return label_chunk_llm(llm, path, content).await;
    }
    Ok(label_chunk_heuristic(path, content))
}

async fn label_chunk_llm(
    llm: &dyn LlmClient,
    path: &str,
    content: &str,
) -> anyhow::Result<LabeledChunk> {
    let prompt = format!(
        "You are labeling a repository chunk for retrieval into Helix. Use the schema fields: topic, project, summary, open_questions (array of strings).\n\
         - topic: short topical slug based on content and path.\n\
         - project: repository/project slug (prefer repo-level context from path).\n\
         - summary: 1-2 sentences, concrete and specific.\n\
         - open_questions: list of unanswered questions implied by the chunk (empty if none).\n\
         Return a JSON object with exactly these keys.\n\
         Path: {path}\n---\n{content}\n---\nJSON:"
    );

    let raw = llm.complete(&prompt).await?;
    let parsed: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));

    let topic = parsed
        .get("topic")
        .and_then(Value::as_str)
        .unwrap_or("code")
        .to_string();

    let project = parsed
        .get("project")
        .and_then(Value::as_str)
        .unwrap_or("vidkosha_cortex")
        .to_string();

    let summary = parsed
        .get("summary")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| fallback_summary(content));

    let open_questions = parsed
        .get("open_questions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    Ok(LabeledChunk {
        topic,
        project,
        summary,
        open_questions,
    })
}

fn fallback_summary(content: &str) -> String {
    let summary = content.lines().take(3).collect::<Vec<&str>>().join(" ");

    if summary.len() > 280 {
        summary[..280].to_string()
    } else {
        summary
    }
}

fn label_chunk_heuristic(path: &str, content: &str) -> LabeledChunk {
    let topic = derive_topic_from_path(path);
    let project = "vidkosha_cortex".to_string();
    let summary = heuristic_summary(content);
    LabeledChunk {
        topic,
        project,
        summary,
        open_questions: Vec::new(),
    }
}

fn derive_topic_from_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 2 {
        return parts[parts.len() - 2].to_string();
    }
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("chunk")
        .to_string()
}

fn heuristic_summary(content: &str) -> String {
    let head = content.lines().take(4).collect::<Vec<&str>>().join(" ");
    let trimmed = head.trim();
    if trimmed.len() > 240 {
        trimmed[..240].to_string()
    } else if trimmed.is_empty() {
        "Code/document chunk".to_string()
    } else {
        trimmed.to_string()
    }
}

fn should_skip_extension(path: &str, cfg: &IngestConfig) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    if let Some(deny) = cfg.deny_extensions.as_ref() {
        if ext.as_ref().is_some_and(|e| deny.contains(e)) {
            return true;
        }
    }

    if let Some(allow) = cfg.allow_extensions.as_ref() {
        return !ext.as_ref().is_some_and(|e| allow.contains(e));
    }

    false
}

fn load_ingest_config() -> IngestConfig {
    let path = ".nervos_index_config.json";
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| IngestConfig {
            allow_extensions: Some(vec![
                "rs".into(),
                "md".into(),
                "toml".into(),
                "json".into(),
                "yml".into(),
                "yaml".into(),
                "ts".into(),
                "tsx".into(),
                "js".into(),
                "jsx".into(),
            ]),
            deny_extensions: Some(vec![
                "lock".into(),
                "bin".into(),
                "exe".into(),
                "dll".into(),
            ]),
            max_file_bytes: None,
            manifest_path: None,
            binary_threshold: Some(0.33),
            allow_binary: Some(false),
            handlers_disabled: None,
            handler_overrides: None,
            force_handlers: None,
        })
}

fn load_manifest(path: Option<&str>) -> IngestManifest {
    let path = path.unwrap_or(".vidkosha_index_manifest.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_manifest(path: Option<&str>, manifest: &IngestManifest) -> anyhow::Result<()> {
    let path = path.unwrap_or(".vidkosha_index_manifest.json");
    let data = serde_json::to_string_pretty(manifest)?;
    fs::write(path, data)?;
    Ok(())
}

fn is_unchanged_in_manifest(
    path: &str,
    manifest: &IngestManifest,
    file_hash: &str,
    mtime: u64,
) -> bool {
    manifest
        .files
        .get(path)
        .map(|entry| entry.hash == file_hash && entry.mtime == mtime)
        .unwrap_or(false)
}

fn file_mtime(meta: &fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn rust_symbols_are_extracted() {
        let content = r#"
fn foo() {}

struct Bar {}

impl Bar {
    fn baz(&self) {}
}
"#;

        let symbols = extract_symbols(content, CodeLanguage::Rust).expect("parse rust");
        assert!(symbols.iter().any(|s| s.kind == "function_item"));
        assert!(symbols.iter().any(|s| s.kind == "struct_item"));
    }

    #[test]
    fn typescript_symbols_are_extracted() {
        let content = r#"
function foo() {}
class Widget {
  method() {}
}
"#;

        let symbols = extract_symbols(content, CodeLanguage::TypeScript).expect("parse ts");
        let names: Vec<String> = symbols.iter().map(|s| s.name.clone()).collect();

        assert!(names.contains(&"foo".to_string()));
        assert!(names.contains(&"Widget".to_string()));
        assert!(names.contains(&"method".to_string()));
    }

    #[test]
    fn python_symbols_are_extracted() {
        let content = r#"
def foo():
    pass

class Bar:
    def baz(self):
        pass
"#;

        let symbols = extract_symbols(content, CodeLanguage::Python).expect("parse py");
        let names: Vec<String> = symbols.iter().map(|s| s.name.clone()).collect();

        assert!(names.contains(&"foo".to_string()));
        assert!(names.contains(&"Bar".to_string()));
        assert!(names.contains(&"baz".to_string()));
    }

    #[test]
    fn symbol_chunks_split_large_bodies() {
        let body = "let x = 1;\n".repeat(200);
        let content = format!("fn big() {{\n{}\n}}", body);

        let chunks =
            chunk_code_symbols(&content, 200, 50, CodeLanguage::Rust).expect("chunk rust symbols");

        // big() should be the only symbol, but split into multiple parts
        assert!(chunks.len() > 1);
        let parts: Vec<_> = chunks
            .iter()
            .map(|c| (c.symbol.name.clone(), c.part_index, c.part_count))
            .collect();
        assert!(parts.iter().all(|(name, _, _)| name == "big"));
        let total = parts[0].2;
        assert!(total > 1);
        assert!(parts.iter().any(|(_, idx, _)| *idx == 0));
    }

    #[test]
    fn binary_detection_respects_threshold() {
        let data = b"text\x01\x02more";
        assert!(is_probably_binary_with_threshold(data, 0.2));
        assert!(!is_probably_binary_with_threshold(data, 0.8));
    }

    #[test]
    fn cli_accepts_prompt_flag_headlessly() {
        // Ensures CLI parsing stays non-interactive under `cargo test`.
        let cli = Cli::parse_from(["nervos-cortex", "--prompt", "hello"]);
        assert_eq!(cli.prompt.as_deref(), Some("hello"));
        assert!(cli.command.is_none());
    }

    #[test]
    fn cli_help_is_emitted_as_error_kind() {
        // Clap returns DisplayHelp as an error; asserting keeps this headless and fast.
        let err = Cli::command()
            .try_get_matches_from(["nervos-cortex", "--help"])
            .expect_err("help should short-circuit");
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn ingest_smoke_filters_changed_and_binary() {
        let base = std::env::temp_dir().join(format!(
            "ncx-ingest-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(&base).expect("create temp dir");

        let text_path = base.join("keep.rs");
        let bin_path = base.join("skip.dat");
        let other_path = base.join("other.rs");

        fs::write(&text_path, "fn ok() {}\n").expect("write text file");
        fs::write(&bin_path, b"\0\0\0BIN").expect("write binary file");
        fs::write(&other_path, "fn ignore() {}\n").expect("write other file");

        let meta_text = fs::metadata(&text_path).expect("meta text");
        let meta_bin = fs::metadata(&bin_path).expect("meta bin");
        let meta_other = fs::metadata(&other_path).expect("meta other");

        let data_text = fs::read(&text_path).expect("read text");
        let data_bin = fs::read(&bin_path).expect("read bin");
        let data_other = fs::read(&other_path).expect("read other");

        let cfg = load_ingest_config();
        let handler_ctx = HandlerContext {
            allow_binary: false,
            binary_threshold: 0.33,
        };
        let mut changed = HashSet::new();
        changed.insert(text_path.to_string_lossy().to_string());
        changed.insert(bin_path.to_string_lossy().to_string());

        let max_file_bytes = 200_000;

        assert!(should_consider_file(
            text_path.to_str().unwrap(),
            &meta_text,
            &data_text,
            &cfg,
            max_file_bytes,
            Some(&changed),
            &handler_ctx,
        ));

        assert!(!should_consider_file(
            bin_path.to_str().unwrap(),
            &meta_bin,
            &data_bin,
            &cfg,
            max_file_bytes,
            Some(&changed),
            &handler_ctx,
        ));

        assert!(!should_consider_file(
            other_path.to_str().unwrap(),
            &meta_other,
            &data_other,
            &cfg,
            max_file_bytes,
            Some(&changed),
            &handler_ctx,
        ));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn handler_selection_prefers_specialized_handlers() {
        let ingest_config = load_ingest_config();
        let ctx = HandlerContext {
            allow_binary: ingest_config.allow_binary.unwrap_or(false),
            binary_threshold: ingest_config.binary_threshold.unwrap_or(0.33),
        };
        let handlers = build_handlers(&ingest_config, 512, 64, &ctx);

        let code_bytes = b"fn main() {}";
        let md_bytes = b"# Title\nbody";
        let data_bytes = b"a,b\n1,2";
        let text_bytes = b"just text";

        let code = resolve_handler(&handlers, &ingest_config, "src/lib.rs", code_bytes, &ctx)
            .expect("code handler");
        assert_eq!(code.name(), "code");

        let md = resolve_handler(&handlers, &ingest_config, "docs/readme.md", md_bytes, &ctx)
            .expect("markdown handler");
        assert_eq!(md.name(), "markdown");

        let data = resolve_handler(
            &handlers,
            &ingest_config,
            "data/sample.csv",
            data_bytes,
            &ctx,
        )
        .expect("data handler");
        assert_eq!(data.name(), "data");

        let text = resolve_handler(
            &handlers,
            &ingest_config,
            "notes/todo.txt",
            text_bytes,
            &ctx,
        )
        .expect("text handler");
        assert_eq!(text.name(), "text");
    }

    #[test]
    fn manifest_skips_unchanged_files() {
        let base = std::env::temp_dir().join(format!(
            "ncx-manifest-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(&base).expect("create temp dir");

        let file_path = base.join("file.rs");
        fs::write(&file_path, "fn a() {}\n").expect("write file");
        let meta = fs::metadata(&file_path).expect("meta");
        let data = fs::read(&file_path).expect("read");
        let hash = blake3::hash(&data).to_hex().to_string();
        let mtime = file_mtime(&meta).unwrap_or_default();

        let mut manifest = IngestManifest::default();
        manifest.files.insert(
            file_path.to_string_lossy().to_string(),
            ManifestEntry {
                hash: hash.clone(),
                mtime,
                chunk_ids: vec!["old".into()],
            },
        );

        assert!(is_unchanged_in_manifest(
            &file_path.to_string_lossy(),
            &manifest,
            &hash,
            mtime
        ));

        fs::write(&file_path, "fn a() {}\nfn b() {}\n").expect("rewrite file");
        let new_data = fs::read(&file_path).expect("read new");
        let new_hash = blake3::hash(&new_data).to_hex().to_string();

        assert!(!is_unchanged_in_manifest(
            &file_path.to_string_lossy(),
            &manifest,
            &new_hash,
            mtime
        ));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn code_handler_sets_language_and_chunk_id_hint() {
        let handler = CodeHandler {
            chunk_bytes: 256,
            overlap_bytes: 32,
        };
        let ctx = HandlerContext {
            allow_binary: false,
            binary_threshold: 0.33,
        };

        let bytes = b"fn demo() {}\n";
        assert!(handler.supports("src/lib.rs", bytes, &ctx));

        let prepared = handler
            .process("src/lib.rs", bytes, &ctx)
            .expect("process code");

        assert!(!prepared.is_empty());
        let first = &prepared[0];
        let lang = first
            .metadata
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(lang, "rust");
        assert!(first.chunk_id_hint.is_some());
        assert_eq!(
            first
                .metadata
                .get("ingest_mode")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "code"
        );
    }

    #[test]
    fn markdown_handler_attaches_headings() {
        let handler = MarkdownHandler {
            chunk_bytes: 64,
            overlap_bytes: 0,
            heading_depth: 6,
        };
        let ctx = HandlerContext {
            allow_binary: false,
            binary_threshold: 0.33,
        };

        let bytes = b"# Title\nBody line\n## Subhead\nMore text";
        assert!(handler.supports("docs/readme.md", bytes, &ctx));

        let prepared = handler
            .process("docs/readme.md", bytes, &ctx)
            .expect("process markdown");

        assert!(prepared.len() >= 2);
        assert_eq!(
            prepared[0]
                .metadata
                .get("markdown_heading")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "Title"
        );
        assert_eq!(
            prepared[1]
                .metadata
                .get("markdown_heading")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "Subhead"
        );
    }

    #[test]
    fn data_handler_chunks_rows_and_labels_format() {
        let handler = DataHandler {
            chunk_bytes: 128,
            overlap_bytes: 0,
            max_rows_per_chunk: 2,
        };
        let ctx = HandlerContext {
            allow_binary: false,
            binary_threshold: 0.33,
        };

        let bytes = b"a,b\n1,2\n3,4";
        assert!(handler.supports("data/sample.csv", bytes, &ctx));

        let prepared = handler
            .process("data/sample.csv", bytes, &ctx)
            .expect("process data");

        assert_eq!(prepared.len(), 2);
        assert_eq!(
            prepared[0]
                .metadata
                .get("data_format")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "csv"
        );
        assert_eq!(
            prepared[0]
                .metadata
                .get("row_range")
                .and_then(|v| v.as_array())
                .and_then(|arr| {
                    if arr.len() == 2 {
                        Some((arr[0].as_u64().unwrap_or(0), arr[1].as_u64().unwrap_or(0)))
                    } else {
                        None
                    }
                })
                .unwrap(),
            (0, 2)
        );
        assert_eq!(
            prepared[1]
                .metadata
                .get("row_range")
                .and_then(|v| v.as_array())
                .and_then(|arr| {
                    if arr.len() == 2 {
                        Some((arr[0].as_u64().unwrap_or(0), arr[1].as_u64().unwrap_or(0)))
                    } else {
                        None
                    }
                })
                .unwrap(),
            (2, 3)
        );
    }

    #[test]
    fn binary_handler_marks_binary_payload() {
        let handler = BinaryHandler {};
        let ctx = HandlerContext {
            allow_binary: true,
            binary_threshold: 0.1,
        };
        let bytes = b"\0\0BINARY";
        assert!(handler.supports("bin/file.bin", bytes, &ctx));

        let prepared = handler
            .process("bin/file.bin", bytes, &ctx)
            .expect("process binary");

        assert_eq!(prepared.len(), 1);
        let meta = &prepared[0].metadata;
        assert_eq!(
            meta.get("ingest_mode")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "binary"
        );
        assert_eq!(
            meta.get("binary_path")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "bin/file.bin"
        );
        assert!(prepared[0].text.contains("<binary file"));
    }
}
