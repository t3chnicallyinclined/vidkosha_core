// Front-desk guidance: agents/agent_readme.md (prompt/RAG/tool flow, save/forget knobs)
use async_trait::async_trait;
use chrono::Utc;
use serde_json;
use serde_json::{Map, Value};
use tracing::{info, instrument, warn};

use crate::llm_client::SharedLlmClient;
use crate::rag::topic_registry::SharedTopicRegistry;
use crate::rag::{
    MemoryDeleteRequest, MemoryFilters, MemoryQuery, MemoryRecord, MemoryRequest,
    MemoryWriteRequest, SharedRagAgent,
};

use super::traits::{AgentBehavior, AgentRequest, AgentResponse};

#[derive(Debug, Clone)]
struct SavePlan {
    mode: SaveMode,
    raw_input: String,
    tags: Vec<String>,
    categories: Vec<String>,
    topic: String,
    topic_source: String,
    save_reason: String,
    body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveMode {
    Immediate,
    AfterAnswer,
    Confirm,
}

/// Front-desk Agent responsible for translating user requests into LLM prompts.
pub struct Agent {
    llm_client: SharedLlmClient,
    rag_agent: Option<SharedRagAgent>,
    topic_registry: Option<SharedTopicRegistry>,
}

impl Agent {
    pub fn new(
        llm_client: SharedLlmClient,
        rag_agent: Option<SharedRagAgent>,
        topic_registry: Option<SharedTopicRegistry>,
    ) -> Self {
        Self {
            llm_client,
            rag_agent,
            topic_registry,
        }
    }

    fn system_directive(&self) -> &'static str {
        "You are Agent, the front-desk orchestrator of Vidkosha Cortex. Always follow the user instruction before proposing work. If the user references files, state which files you will read (or have read) and base your summary on them; do not invent content or new projects. If you see grounded snippets, use them first (cite path+chunk and agent with confidence) and blend in your own knowledge. If no snippets are present, answer directly unless more context would materially help—then call the tool. To call the tool, respond exactly with: TOOL:MEMORY_SEARCH {\"query\":\"<what to search>\",\"limit\":3} and nothing else. Delegate to a specialist only when the user requests it or when delegation clearly improves accuracy; otherwise stay front desk. Keep responses concise, actionable, and avoid persona switching."
    }

    fn compose_prompt(&self, request: &AgentRequest) -> String {
        format!(
            "{directive}\n\nUser request:\n{input}\n\nInstructions: if files are mentioned, acknowledge them explicitly before answering; if context is missing, emit TOOL:MEMORY_SEARCH as defined. Otherwise, reason briefly and outline next steps.",
            directive = self.system_directive(),
            input = request.input.trim()
        )
    }

    async fn handle_control(
        &self,
        request: &AgentRequest,
    ) -> anyhow::Result<Option<AgentResponse>> {
        let rag = match self.rag_agent.as_ref() {
            Some(rag) => rag,
            None => return Ok(None),
        };

        let raw = request.input.trim();
        let lower = raw.to_lowercase();

        if let Some(target_id) = Self::extract_forget_id(&lower, raw) {
            if target_id.is_empty() {
                return Ok(Some(AgentResponse::new(
                    "Tell me which memory id to forget (e.g., forget chunk-123).",
                )));
            }

            let delete_req = MemoryDeleteRequest {
                id: target_id.to_string(),
            };

            match rag
                .handle(MemoryRequest::Delete(delete_req))
                .await
                .map(|resp| resp.memory_ids)
            {
                Ok(ids) if !ids.is_empty() => {
                    let msg = format!("Deleted memory id={}", ids[0]);
                    return Ok(Some(AgentResponse::new(msg)));
                }
                Ok(_) => {
                    return Ok(Some(AgentResponse::new(
                        "Delete attempted, but no id was confirmed. Check the id and try again.",
                    )));
                }
                Err(err) => {
                    warn!(?err, "Delete request failed");
                    return Ok(Some(AgentResponse::new(
                        "I could not delete that memory. Verify the id and try again.",
                    )));
                }
            }
        }

        Ok(None)
    }

    fn extract_save_body<'a>(lower: &str, raw: &'a str) -> Option<(&'a str, String)> {
        // Ignore question-like requests asking about existing saves.
        if Self::looks_like_save_query(lower, raw) {
            return None;
        }

        // Guard against past-tense mentions like "saved" / "have saved" which are usually queries, not commands.
        if lower.contains(" saved") || lower.contains("have saved") {
            return None;
        }

        // Heuristic 1: explicit save/remember/store/log/note/keep keywords anywhere
        const KEYWORDS: &[&str] = &[
            "save",
            "remember",
            "store",
            "log",
            "note",
            "write down",
            "jot",
            "keep this",
            "record",
        ];
        for kw in KEYWORDS {
            if let Some(idx) = lower.find(kw) {
                if !Self::is_keyword_boundary(lower, idx, kw.len()) {
                    continue;
                }

                let tail = raw[idx + kw.len()..]
                    .trim_start_matches(|c: char| c == ':' || c.is_whitespace());
                if !tail.is_empty() {
                    return Some((tail, format!("keyword:{kw}")));
                }
            }
        }

        // Heuristic 2: colon pattern ("it goes like this: ...")
        if let Some(idx) = raw.find(':') {
            let tail = raw[idx + 1..].trim();
            if !tail.is_empty()
                && (lower.contains("goes like")
                    || lower.contains("it is")
                    || lower.contains("here it is")
                    || lower.contains("the text")
                    || lower.contains("idea")
                    || lower.contains("this:"))
            {
                return Some((tail, "colon_tail".to_string()));
            }
        }

        None
    }

    fn is_keyword_boundary(text: &str, start: usize, len: usize) -> bool {
        let bytes = text.as_bytes();

        let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();

        let after_idx = start + len;
        let after_ok = after_idx >= bytes.len() || !bytes[after_idx].is_ascii_alphanumeric();

        before_ok && after_ok
    }

    fn looks_like_save_query(lower: &str, raw: &str) -> bool {
        raw.contains('?')
            || lower.starts_with("do i have")
            || lower.starts_with("do you have")
            || lower.starts_with("did we save")
            || lower.starts_with("did you save")
            || lower.starts_with("have we saved")
            || lower.contains("anything saved")
            || lower.contains("any saves")
            || lower.contains("any saved")
            || lower.contains("recall any")
            || lower.contains("remember any")
    }

    async fn infer_topics_from_text(
        &self,
        raw: &str,
    ) -> anyhow::Result<Option<Vec<(String, Value)>>> {
        let prompt = format!(
            "You are a concise classifier. Given text, propose at most 5 topic objects in JSON array form. Each object: name (slug-like, lowercase with dots), description, parent (optional), status=active. If no new topics are needed, return an empty JSON array.\n\nText:\n{raw}\n\nRespond with JSON only."
        );

        let output = self.llm_client.complete(&prompt).await?;
        let parsed: Value = match serde_json::from_str(output.trim()) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };

        let arr = match parsed.as_array() {
            Some(a) => a,
            None => return Ok(None),
        };

        let mut seeds = Vec::new();
        for entry in arr {
            let obj = match entry.as_object() {
                Some(o) => o,
                None => continue,
            };

            let name = match obj.get("name").and_then(|v| v.as_str()) {
                Some(n) if !n.trim().is_empty() => n.trim(),
                _ => continue,
            };

            let mut meta = Value::Object(obj.clone());
            if let Some(map) = meta.as_object_mut() {
                map.remove("name");
                if map.is_empty() {
                    *map = Map::new();
                }
            }

            seeds.push((name.to_string(), meta));
        }

        if seeds.is_empty() {
            Ok(None)
        } else {
            Ok(Some(seeds))
        }
    }

    fn extract_topic_seeds(raw: &str) -> Option<Vec<(String, Value)>> {
        let parsed: Value = serde_json::from_str(raw).ok()?;
        let entries = parsed.as_array()?;
        let mut seeds = Vec::new();

        for entry in entries {
            let obj = match entry.as_object() {
                Some(o) => o,
                None => continue,
            };

            let name = match obj.get("name").and_then(|v| v.as_str()) {
                Some(n) if !n.trim().is_empty() => n.trim(),
                _ => continue,
            };

            let mut meta = Value::Object(obj.clone());
            if let Some(map) = meta.as_object_mut() {
                map.remove("name");
                if map.is_empty() {
                    *map = Map::new();
                }
            }

            seeds.push((name.to_string(), meta));
        }

        if seeds.is_empty() {
            None
        } else {
            Some(seeds)
        }
    }

    fn extract_tags(raw: &str, lower: &str) -> Vec<String> {
        const TOKENS: &[&str] = &["tags=", "tags:", "tag=", "tag:"];
        for token in TOKENS {
            if let Some(idx) = lower.find(token) {
                let tail = raw[idx + token.len()..].trim();
                if tail.is_empty() {
                    return Vec::new();
                }
                // Split on comma or whitespace
                let tags = tail
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter_map(|t| {
                        let trimmed = t.trim();
                        (!trimmed.is_empty()).then_some(trimmed.to_lowercase())
                    })
                    .collect::<Vec<_>>();
                return tags;
            }
        }
        Vec::new()
    }

    fn infer_category_topic(
        raw: &str,
        lower: &str,
        tags: &[String],
    ) -> (Vec<String>, String, String) {
        // If the user supplied topic=.../topic:..., honor it.
        for token in ["topic=", "topic:"] {
            if let Some(idx) = lower.find(token) {
                let tail = raw[idx + token.len()..].trim();
                if !tail.is_empty() {
                    let topic = tail
                        .split_whitespace()
                        .next()
                        .unwrap_or(tail)
                        .to_lowercase();
                    return (
                        vec!["unspecified".to_string()],
                        topic,
                        format!("user:{token}"),
                    );
                }
            }
        }

        // If tags are provided, prefer the first tag as topic.
        if let Some(first_tag) = tags.first() {
            return (
                vec!["unspecified".to_string()],
                first_tag.clone(),
                "tags:first".to_string(),
            );
        }

        let (categories, topic) = if lower.contains("comedy")
            || lower.contains("standup")
            || lower.contains("joke")
            || lower.contains("bit")
        {
            (
                vec!["hobby".to_string(), "comedy".to_string()],
                "standup_comedy".to_string(),
            )
        } else if lower.contains("fightstick")
            || lower.contains("arcade")
            || lower.contains("joystick")
            || lower.contains("sanwa")
            || lower.contains("happ")
            || lower.contains("brook")
            || lower.contains("buttons")
            || lower.contains("pcb")
        {
            (
                vec![
                    "hardware".to_string(),
                    "build".to_string(),
                    "arcade".to_string(),
                ],
                "hardware.build.fightstick".to_string(),
            )
        } else if lower.contains("shopping")
            || lower.contains("list")
            || lower.contains("buy")
            || lower.contains("purchase")
            || lower.contains("parts")
        {
            (
                vec!["task".to_string(), "list".to_string()],
                "task.list".to_string(),
            )
        } else if lower.contains("client")
            || lower.contains("proposal")
            || lower.contains("roadmap")
            || lower.contains("market")
            || lower.contains("product")
        {
            (vec!["business".to_string()], "business.idea".to_string())
        } else if lower.contains("project") {
            (vec!["project".to_string()], "project.note".to_string())
        } else {
            (vec!["personal".to_string()], "personal.note".to_string())
        };

        (categories, topic, "inferred".to_string())
    }

    fn extract_forget_id<'a>(lower: &str, raw: &'a str) -> Option<&'a str> {
        const PREFIXES: &[&str] = &["forget", "delete memory", "remove memory"];
        for prefix in PREFIXES {
            if lower.starts_with(prefix) {
                if let Some((_, tail)) = raw.split_once(char::is_whitespace) {
                    return Some(tail.trim());
                } else {
                    return Some("");
                }
            }
        }
        None
    }

    fn extract_save_plan(lower: &str, raw: &str) -> Option<SavePlan> {
        let (body, save_reason) = Self::extract_save_body(lower, raw)?;
        let tags = Self::extract_tags(raw, lower);
        let (categories, topic, topic_source) = Self::infer_category_topic(raw, lower, &tags);
        let trimmed = body.trim();
        let mode = if trimmed.len() < 12 {
            SaveMode::Confirm
        } else {
            SaveMode::Immediate
        };

        Some(SavePlan {
            mode,
            raw_input: raw.to_string(),
            tags,
            categories,
            topic,
            topic_source,
            save_reason,
            body: trimmed.to_string(),
        })
    }

    async fn persist_save_plan(
        &self,
        plan: &SavePlan,
        body_override: Option<&str>,
    ) -> anyhow::Result<Option<String>> {
        let rag = match self.rag_agent.as_ref() {
            Some(rag) => rag,
            None => {
                return Ok(Some(
                    "I can save this when memory is enabled. Right now RAG is disabled."
                        .to_string(),
                ))
            }
        };

        let final_body = body_override.unwrap_or(plan.body.as_str()).trim();
        if final_body.len() < 4 {
            return Ok(Some(
                "Tell me what to save (e.g., a list or a sentence), and I'll store it with tags."
                    .to_string(),
            ));
        }

        let summary: String = final_body.chars().take(200).collect();
        let record = MemoryRecord {
            id: None,
            agent_name: "Agent".to_string(),
            topic: plan.topic.clone(),
            project: None,
            conversation_id: None,
            timestamp: Utc::now(),
            summary: summary.clone(),
            full_content: final_body.to_string(),
            confidence: 0.4,
            open_questions: Vec::new(),
            perspectives: Vec::new(),
            messages: Vec::new(),
            artifacts: Vec::new(),
            tool_calls: Vec::new(),
            metadata: Some(serde_json::json!({
                "source": "agent.save",
                "raw_input": plan.raw_input,
                "categories": plan.categories,
                "topic_source": plan.topic_source,
                "tags": plan.tags,
                "body": final_body,
                "save_reason": plan.save_reason,
            })),
        };

        let response = rag
            .handle(MemoryRequest::Write(MemoryWriteRequest { record }))
            .await?;

        let memory_id = response
            .memory_ids
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let cats = plan.categories.join(", ");
        let tag_line = if plan.tags.is_empty() {
            String::new()
        } else {
            format!(" tags: {}.", plan.tags.join(", "))
        };
        let preview: String = final_body.chars().take(200).collect();
        let msg = format!(
            "Saved. id={memory_id} topic={topic}. Categories: {cats}.{tag_line} Stored: \"{preview}\". Ask later: 'remind me <topic/tags>'. To remove, say 'forget {memory_id}'.",
            topic = plan.topic
        );

        Ok(Some(msg))
    }

    #[instrument(skip_all, fields(raw_output_len = raw_output.len()))]
    #[allow(dead_code)]
    async fn maybe_tool_search(
        &self,
        request: &AgentRequest,
        raw_output: &str,
    ) -> anyhow::Result<Option<String>> {
        let rag = match self.rag_agent.as_ref() {
            Some(rag) => rag,
            None => return Ok(None),
        };

        const PREFIX: &str = "TOOL:MEMORY_SEARCH";
        let trimmed = raw_output.trim();
        let idx = match trimmed.find(PREFIX) {
            Some(i) => i,
            None => return Ok(None),
        };

        let json_part = trimmed[idx + PREFIX.len()..].trim();
        let search: serde_json::Value = serde_json::from_str(json_part)?;

        let query = search
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or(request.input.as_str())
            .to_string();
        let limit = search
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(3)
            .clamp(1, 10) as usize;

        let memory_query = MemoryQuery {
            query,
            filters: MemoryFilters::default(),
            limit,
        };

        info!(limit, query = %memory_query.query, "Memory tool request parsed; querying RAG");
        let results = rag.handle(MemoryRequest::Retrieve(memory_query)).await?;

        if results.records.is_empty() {
            warn!("Memory tool returned no matches");
            return Ok(Some(String::from(
                "No memories found in Helix. Answer from your own knowledge, and if prior context is needed, state that no stored memory matched.",
            )));
        }

        info!(
            count = results.records.len(),
            "Memory tool returned matches"
        );
        let context = results
            .records
            .iter()
            .take(limit)
            .map(|r| {
                let path = r
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let chunk_id = r
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("chunk_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(r.id.as_deref().unwrap_or(""));
                format!(
                    "- [{}] path={} chunk={} agent={} topic={} confidence={:.2} :: {}\n{}",
                    r.timestamp.to_rfc3339(),
                    path,
                    chunk_id,
                    r.agent_name,
                    r.topic,
                    r.confidence,
                    r.summary,
                    r.full_content
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let follow_up = format!(
            "Relevant memories found. Cite path+chunk and agent with confidence when you use them. Blend in your own knowledge to fill gaps, and if you add anything not in the snippets, say it is general knowledge.\n{context}\n\nUser request:\n{}",
            request.input.trim()
        );

        Ok(Some(follow_up))
    }
    async fn default_grounding(
        &self,
        request: &AgentRequest,
        rag: &SharedRagAgent,
    ) -> anyhow::Result<Option<String>> {
        let query = MemoryQuery {
            query: request.input.clone(),
            filters: MemoryFilters::default(),
            limit: 5,
        };

        info!(limit = query.limit, query = %query.query, "Running default memory search");
        let results = rag.handle(MemoryRequest::Retrieve(query.clone())).await;

        let results = match results {
            Ok(res) => res,
            Err(err) => {
                warn!(?err, "Default memory search failed");
                return Ok(None);
            }
        };

        if results.records.is_empty() {
            return Ok(None);
        }

        let context = results
            .records
            .iter()
            .take(query.limit())
            .map(|r| {
                let path = r
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let chunk_id = r
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("chunk_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(r.id.as_deref().unwrap_or(""));
                format!(
                    "- [{}] path={} chunk={} agent={} topic={} confidence={:.2} :: {}\n{}",
                    r.timestamp.to_rfc3339(),
                    path,
                    chunk_id,
                    r.agent_name,
                    r.topic,
                    r.confidence,
                    r.summary,
                    r.full_content
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let follow_up = format!(
            "Retrieved memories. Use them when relevant, cite path+chunk and agent with confidence, and blend with your own knowledge. If you add anything not in the snippets, say it is general knowledge.\n{context}\n\nUser request:\n{}",
            request.input.trim()
        );

        Ok(Some(follow_up))
    }
}

#[async_trait]
impl AgentBehavior for Agent {
    #[instrument(skip_all, fields(input = %request.input))]
    async fn handle(&self, request: AgentRequest) -> anyhow::Result<AgentResponse> {
        let raw = request.input.trim().to_string();
        let lower = raw.to_lowercase();
        if let Some(registry) = self.topic_registry.as_ref() {
            if let Some(seeds) = Self::extract_topic_seeds(&raw) {
                let ids = registry
                    .upsert_topics(&seeds)
                    .await
                    .map_err(|err| anyhow::anyhow!("Topic upsert failed: {err}"))?;
                let preview = ids.iter().take(10).cloned().collect::<Vec<_>>().join(", ");
                let msg = if ids.len() > 10 {
                    format!(
                        "Stored {} topics (showing first 10): {}",
                        ids.len(),
                        preview
                    )
                } else {
                    format!("Stored {} topics: {}", ids.len(), preview)
                };
                return Ok(AgentResponse::new(msg));
            } else if raw.len() > 20
                && (lower.contains("topic")
                    || lower.contains("category")
                    || lower.contains("categories")
                    || lower.contains("tag")
                    || lower.contains("classify")
                    || lower.contains("organize"))
            {
                if let Some(inferred) = self.infer_topics_from_text(&raw).await? {
                    let ids = registry
                        .upsert_topics(&inferred)
                        .await
                        .map_err(|err| anyhow::anyhow!("Topic upsert failed: {err}"))?;
                    let preview = ids.iter().take(10).cloned().collect::<Vec<_>>().join(", ");
                    let msg = if ids.len() > 10 {
                        format!(
                            "Inferred and stored {} topics (showing first 10): {}",
                            ids.len(),
                            preview
                        )
                    } else {
                        format!("Inferred and stored {} topics: {}", ids.len(), preview)
                    };
                    return Ok(AgentResponse::new(msg));
                }
            }
        }
        let save_plan = Self::extract_save_plan(&lower, &raw);

        if let Some(plan) = save_plan.as_ref() {
            if plan.mode == SaveMode::Confirm {
                let preview: String = plan.body.chars().take(80).collect();
                let msg = format!(
                    "I spotted a possible save request but your text is short/ambiguous: \"{}\". Do you want me to save it, or should I just check existing memories? Say 'save it' to store or 'just check' to search.",
                    preview
                );
                return Ok(AgentResponse::new(msg));
            }

            if plan.mode == SaveMode::Immediate {
                if let Some(msg) = self.persist_save_plan(plan, None).await? {
                    return Ok(AgentResponse::new(msg));
                }
            }
        }

        if let Some(controlled) = self.handle_control(&request).await? {
            return Ok(controlled);
        }

        let mut output: Option<String> = None;

        // Prefer a quick memory grounding when available to avoid hallucinations on rare/fictional terms.
        if let Some(rag) = self.rag_agent.as_ref() {
            if let Ok(Some(follow_up_prompt)) = self.default_grounding(&request, rag).await {
                let grounded = self.llm_client.complete(&follow_up_prompt).await?;
                output = Some(grounded);
            }
        }

        if output.is_none() {
            // Otherwise run once, and honor explicit TOOL:MEMORY_SEARCH directives if the model requests them.
            let prompt = self.compose_prompt(&request);
            let first = self.llm_client.complete(&prompt).await?;

            if self.rag_agent.is_some() {
                if let Some(follow_up_prompt) = self.maybe_tool_search(&request, &first).await? {
                    info!("Memory tool requested; rerunning with retrieved context");
                    let rerun = self.llm_client.complete(&follow_up_prompt).await?;
                    output = Some(rerun);
                }
            }

            if output.is_none() {
                output = Some(first);
            }
        }

        let mut final_output = output.unwrap_or_default();

        if let Some(plan) = save_plan.as_ref() {
            if plan.mode == SaveMode::AfterAnswer {
                if let Some(msg) = self
                    .persist_save_plan(plan, Some(final_output.as_str()))
                    .await?
                {
                    final_output = format!("{final_output}\n\n{msg}");
                }
            }
        }

        Ok(AgentResponse::new(final_output))
    }
}
