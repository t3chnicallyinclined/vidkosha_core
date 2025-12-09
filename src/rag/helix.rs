use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use blake3;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tracing::{info, warn};

use super::client::RagClient;
use super::config::HelixConfig;
use super::embed::EmbeddingsProvider;
use super::types::{
    ArtifactRef, MemoryDeleteRequest, MemoryFilters, MemoryQuery, MemoryRecord, MemoryWriteRequest,
    MemoryWriteResponse, MessageRecord, PayoutEvent, PerspectiveView, ToolCallRecord, UsageEvent,
};

/// Minimal HTTP client for HelixDB's REST surface.
pub struct HelixClient {
    http: reqwest::Client,
    config: HelixConfig,
}

impl HelixClient {
    pub fn new(config: HelixConfig) -> anyhow::Result<Self> {
        let timeout = Duration::from_millis(config.http_timeout_ms.max(1));
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .context("Failed to build Helix HTTP client")?;

        Ok(Self { http, config })
    }

    fn endpoint(&self, path: &str) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        format!("{base}/{path}")
    }

    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.config.api_token {
            builder.bearer_auth(token)
        } else {
            builder
        }
    }

    #[allow(dead_code)]
    /// Ping the `/health` endpoint to ensure the Helix node is reachable.
    pub async fn health_check(&self) -> anyhow::Result<()> {
        let url = self.endpoint("health");
        let response = self
            .apply_auth(self.http.post(url))
            .send()
            .await
            .context("Helix health request failed")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Helix health endpoint returned status {}",
                response.status()
            ))
        }
    }

    /// Check if HelixDB is running by calling the introspect endpoint
    pub async fn check_connectivity(&self) -> anyhow::Result<reqwest::StatusCode> {
        let url = format!("{}/introspect", self.config.base_url);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .context("Failed to reach HelixDB")?;
        Ok(response.status())
    }

    #[allow(dead_code)]
    /// Fetch lightweight namespace metadata; useful for smoke tests before writes.
    pub async fn fetch_namespace(&self) -> anyhow::Result<HelixNamespaceMeta> {
        let path = format!("api/v1/namespaces/{}", self.config.namespace);
        let response = self
            .apply_auth(self.http.post(self.endpoint(&path)))
            .send()
            .await
            .context("Helix namespace request failed")?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(anyhow!(
                "Helix namespace '{}' not found",
                self.config.namespace
            ));
        }

        let body = response
            .error_for_status()
            .context("Helix namespace request returned error status")?;

        body.json::<HelixNamespaceMeta>()
            .await
            .context("Failed to deserialize Helix namespace metadata response")
    }

    /// Call a HelixQL query (gateway expects POST /<query_name> with JSON body)
    pub async fn post_query<T: Serialize, R: DeserializeOwned>(
        &self,
        query_name: &str,
        payload: &T,
    ) -> anyhow::Result<R> {
        let url = self.endpoint(query_name);
        let response = self
            .apply_auth(self.http.post(&url))
            .json(payload)
            .send()
            .await
            .with_context(|| format!("Helix query '{query_name}' request failed"))?;

        let response = response
            .error_for_status()
            .with_context(|| format!("Helix query '{query_name}' returned error status"))?;

        response
            .json::<R>()
            .await
            .with_context(|| format!("Failed to deserialize Helix query '{query_name}' response"))
    }

    #[allow(dead_code)]
    pub fn namespace(&self) -> &str {
        &self.config.namespace
    }

    async fn create_node(&self, payload: &HelixNodeUpsertRequest) -> anyhow::Result<String> {
        let path = format!("api/v1/namespaces/{}/nodes", self.config.namespace);
        let response = self
            .apply_auth(self.http.post(self.endpoint(&path)))
            .json(payload)
            .send()
            .await
            .context("Helix node upsert request failed")?
            .error_for_status()
            .context("Helix node upsert returned error status")?;

        let body = response
            .json::<HelixNodeWriteResponse>()
            .await
            .context("Failed to deserialize Helix upsert response")?;

        Ok(body.node_id)
    }

    async fn create_edge(&self, payload: &HelixEdgeUpsertRequest) -> anyhow::Result<()> {
        let path = format!("api/v1/namespaces/{}/edges", self.config.namespace);
        self.apply_auth(self.http.post(self.endpoint(&path)))
            .json(payload)
            .send()
            .await
            .context("Helix edge upsert request failed")?
            .error_for_status()
            .context("Helix edge upsert returned error status")?;

        Ok(())
    }

    async fn search_nodes(
        &self,
        payload: &HelixSearchRequest,
    ) -> anyhow::Result<Vec<HelixSearchHit>> {
        let path = format!("api/v1/namespaces/{}/search", self.config.namespace);
        let response = self
            .apply_auth(self.http.post(self.endpoint(&path)))
            .json(payload)
            .send()
            .await
            .context("Helix search request failed")?
            .error_for_status()
            .context("Helix search returned error status")?;

        let body = response
            .json::<HelixSearchResponse>()
            .await
            .context("Failed to deserialize Helix search response")?;

        Ok(body.hits)
    }

    #[allow(dead_code)]
    async fn delete_node(&self, node_id: &str) -> anyhow::Result<()> {
        let path = format!(
            "api/v1/namespaces/{}/nodes/{}",
            self.config.namespace, node_id
        );

        let response = self
            .apply_auth(self.http.delete(self.endpoint(&path)))
            .send()
            .await
            .context("Helix delete request failed")?;

        if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
            return Ok(());
        }

        Err(anyhow!(
            "Helix delete returned status {}",
            response.status()
        ))
    }

    async fn fetch_neighbors(
        &self,
        node_id: &str,
        depth: usize,
    ) -> anyhow::Result<Vec<HelixNeighbor>> {
        let path = format!(
            "api/v1/namespaces/{}/nodes/{}/neighbors?depth={}",
            self.config.namespace, node_id, depth
        );
        let response = self
            .apply_auth(self.http.post(self.endpoint(&path)))
            .send()
            .await
            .context("Helix neighbor request failed")?
            .error_for_status()
            .context("Helix neighbor request returned error status")?;

        let body = response
            .json::<HelixNeighborResponse>()
            .await
            .context("Failed to deserialize Helix neighbor response")?;

        Ok(body.neighbors)
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct HelixNamespaceMeta {
    pub name: String,
    #[serde(default)]
    pub collections: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

const MEMORY_NODE_TYPE: &str = "memory_entry";
const PERSPECTIVE_NODE_TYPE: &str = "perspective_view";
const AGENT_NODE_TYPE: &str = "agent_profile";
const TOPIC_NODE_TYPE: &str = "topic";
const PROJECT_NODE_TYPE: &str = "project";
const CONVERSATION_NODE_TYPE: &str = "conversation";
const MESSAGE_NODE_TYPE: &str = "message";
const TOOL_CALL_NODE_TYPE: &str = "tool_call";
const ARTIFACT_NODE_TYPE: &str = "artifact";

const EDGE_RECORDED_BY: &str = "RECORDED_BY";
const EDGE_RELATES_TO_TOPIC: &str = "RELATES_TO_TOPIC";
const EDGE_PART_OF_PROJECT: &str = "PART_OF_PROJECT";
const EDGE_HAS_PERSPECTIVE: &str = "HAS_PERSPECTIVE";
const EDGE_IN_THREAD: &str = "IN_THREAD";
const EDGE_HAS_MESSAGE: &str = "HAS_MESSAGE";
const EDGE_REPLIES_TO: &str = "REPLIES_TO";
const EDGE_PRODUCED_MEMORY: &str = "PRODUCED_MEMORY";
const EDGE_REFERENCES_ARTIFACT: &str = "REFERENCES_ARTIFACT";

#[allow(dead_code)]
pub struct HelixGraphClient {
    helix: HelixClient,
    embedder: Arc<dyn EmbeddingsProvider>,
    embedding_model: String,
    vector_dim: usize,
}

/// Helix query-based client that matches the current MemoryChunk schema and
/// uses HelixQL endpoints (e.g., /SearchMemoryChunk, /InsertMemoryChunk).
pub struct HelixQueryRagClient {
    helix: HelixClient,
    embedder: Arc<dyn EmbeddingsProvider>,
    _embedding_model: String,
    vector_dim: usize,
    neighbor_depth: Option<usize>,
}

impl HelixQueryRagClient {
    const MIN_SCORE: f64 = 0.25;
    pub fn new(
        helix: HelixClient,
        embedder: Arc<dyn EmbeddingsProvider>,
        embedding_model: String,
        vector_dim: usize,
    ) -> Self {
        Self {
            helix,
            embedder,
            _embedding_model: embedding_model,
            vector_dim,
            neighbor_depth: neighbor_depth_from_env(),
        }
    }

    fn to_f64(vector: &[f32]) -> Vec<f64> {
        vector.iter().map(|v| *v as f64).collect()
    }
}

#[allow(dead_code)]
impl HelixGraphClient {
    #[allow(dead_code)]
    pub fn new(
        helix: HelixClient,
        embedder: Arc<dyn EmbeddingsProvider>,
        embedding_model: String,
        vector_dim: usize,
    ) -> Self {
        Self {
            helix,
            embedder,
            embedding_model,
            vector_dim,
        }
    }

    async fn upsert_memory(
        &self,
        record: &MemoryRecord,
        vector: Vec<f32>,
    ) -> anyhow::Result<String> {
        if vector.len() != self.vector_dim {
            warn!(
                expected = self.vector_dim,
                actual = vector.len(),
                "Embedding dimension mismatch during Helix write"
            );
        }

        let record_json = serde_json::to_string(record)?;
        let properties = HelixMemoryProperties::from_record(record, record_json);

        let request = HelixNodeUpsertRequest {
            node_type: MEMORY_NODE_TYPE.to_string(),
            properties: serde_json::to_value(properties)?,
            embedding: Some(HelixEmbeddingPayload {
                model: self.embedding_model.clone(),
                vector,
            }),
            external_id: None,
        };

        self.helix.create_node(&request).await
    }

    async fn search(
        &self,
        query: MemoryQuery,
        vector: Vec<f32>,
    ) -> anyhow::Result<(Vec<MemoryRecord>, bool)> {
        if vector.len() != self.vector_dim {
            warn!(
                expected = self.vector_dim,
                actual = vector.len(),
                "Embedding dimension mismatch during Helix search"
            );
        }

        let filters = Self::build_filters(&query.filters);
        let request = HelixSearchRequest {
            node_type: MEMORY_NODE_TYPE.to_string(),
            limit: query.limit(),
            filters,
            vector: HelixEmbeddingPayload {
                model: self.embedding_model.clone(),
                vector,
            },
        };

        let hits = self.helix.search_nodes(&request).await?;

        let mut any_missing_neighbors = false;
        let records = hits
            .into_iter()
            .filter_map(|hit| match Self::record_from_hit(hit) {
                Some((record, has_neighbors)) => {
                    if !has_neighbors {
                        any_missing_neighbors = true;
                    }
                    Some(record)
                }
                None => None,
            })
            .collect();

        Ok((records, !any_missing_neighbors))
    }

    fn build_filters(filters: &MemoryFilters) -> Vec<HelixPropertyFilter> {
        let mut helix_filters = Vec::new();
        if let Some(agent_name) = &filters.agent_name {
            helix_filters.push(HelixPropertyFilter::Equals {
                field: "agent_name".to_string(),
                value: agent_name.clone(),
            });
        }

        if let Some(topic) = &filters.topic {
            helix_filters.push(HelixPropertyFilter::Equals {
                field: "topic".to_string(),
                value: topic.clone(),
            });
        }

        if let Some(project) = &filters.project {
            helix_filters.push(HelixPropertyFilter::Equals {
                field: "project".to_string(),
                value: project.clone(),
            });
        }

        if let Some(conversation_id) = &filters.conversation_id {
            helix_filters.push(HelixPropertyFilter::Equals {
                field: "conversation_id".to_string(),
                value: conversation_id.clone(),
            });
        }

        if let Some(since) = &filters.since {
            helix_filters.push(HelixPropertyFilter::Gte {
                field: "timestamp".to_string(),
                value: since.to_rfc3339(),
            });
        }

        helix_filters
    }

    fn record_from_hit(hit: HelixSearchHit) -> Option<(MemoryRecord, bool)> {
        let mut record: MemoryRecord = match serde_json::from_str(&hit.properties.record_json) {
            Ok(record) => record,
            Err(err) => {
                warn!(?err, "Failed to deserialize Helix memory record");
                return None;
            }
        };
        if record.id.is_none() {
            record.id = Some(hit.node_id);
        }

        let mut has_neighbors = false;
        if let Some(neighbors) = hit.neighbors {
            if !neighbors.is_empty() {
                let summary = neighbors.into_iter().map(helix_neighbor_to_value).collect();
                insert_metadata_field(
                    &mut record.metadata,
                    "helix_neighbors",
                    Value::Array(summary),
                );
                has_neighbors = true;
            }
        }

        Some((record, has_neighbors))
    }

    async fn write_memory_context(
        &self,
        memory_node_id: &str,
        record: &MemoryRecord,
    ) -> anyhow::Result<()> {
        let agent_id = self.ensure_agent_profile(&record.agent_name).await?;
        self.link_nodes(EDGE_RECORDED_BY, memory_node_id, &agent_id, "recorded_by")
            .await?;

        let topic_id = self.ensure_topic_node(&record.topic).await?;
        self.link_nodes(
            EDGE_RELATES_TO_TOPIC,
            memory_node_id,
            &topic_id,
            "relates_to_topic",
        )
        .await?;

        if let Some(project) = &record.project {
            let project_id = self.ensure_project_node(project).await?;
            self.link_nodes(
                EDGE_PART_OF_PROJECT,
                memory_node_id,
                &project_id,
                "part_of_project",
            )
            .await?;
        }

        self.write_perspectives(memory_node_id, &record.perspectives)
            .await?;

        let conversation_node_id = if let Some(conversation_id) = &record.conversation_id {
            Some(self.ensure_conversation_node(conversation_id).await?)
        } else {
            None
        };

        self.write_messages(
            memory_node_id,
            conversation_node_id.as_deref(),
            record.conversation_id.as_deref(),
            &record.messages,
        )
        .await?;

        self.write_artifacts(memory_node_id, &record.artifacts)
            .await?;

        self.write_tool_calls(memory_node_id, &record.tool_calls)
            .await?;

        Ok(())
    }

    async fn ensure_agent_profile(&self, agent_name: &str) -> anyhow::Result<String> {
        let slug = slugify(agent_name);
        let properties = json!({
            "agent_name": agent_name,
            "role": agent_name,
            "mission": format!("Auto-generated profile for {agent_name}"),
        });

        self.helix
            .create_node(&HelixNodeUpsertRequest::metadata(
                AGENT_NODE_TYPE,
                properties,
                Some(format!("agent::{slug}")),
            ))
            .await
    }

    async fn ensure_topic_node(&self, topic: &str) -> anyhow::Result<String> {
        let slug = slugify(topic);
        let properties = json!({
            "slug": slug,
            "label": topic,
        });

        self.helix
            .create_node(&HelixNodeUpsertRequest::metadata(
                TOPIC_NODE_TYPE,
                properties,
                Some(format!("topic::{slug}")),
            ))
            .await
    }

    async fn ensure_project_node(&self, project: &str) -> anyhow::Result<String> {
        let slug = slugify(project);
        let properties = json!({
            "slug": slug,
            "title": project,
        });

        self.helix
            .create_node(&HelixNodeUpsertRequest::metadata(
                PROJECT_NODE_TYPE,
                properties,
                Some(format!("project::{slug}")),
            ))
            .await
    }

    async fn write_perspectives(
        &self,
        memory_node_id: &str,
        views: &[PerspectiveView],
    ) -> anyhow::Result<()> {
        if views.is_empty() {
            return Ok(());
        }

        for view in views {
            if view.role.trim().is_empty() {
                continue;
            }

            let slug = slugify(&format!("{}-{}", memory_node_id, view.role));
            let properties = json!({
                "memory_id": memory_node_id,
                "role": view.role,
                "summary": view.summary,
                "body": view.body,
                "risks": view.risks,
                "decisions": view.decisions,
                "actions": view.actions,
            });

            let node_id = self
                .helix
                .create_node(&HelixNodeUpsertRequest::metadata(
                    PERSPECTIVE_NODE_TYPE,
                    properties,
                    Some(format!("perspective::{slug}")),
                ))
                .await?;

            self.link_nodes(
                EDGE_HAS_PERSPECTIVE,
                memory_node_id,
                &node_id,
                "has_perspective",
            )
            .await?;
        }

        Ok(())
    }

    async fn ensure_conversation_node(&self, conversation_id: &str) -> anyhow::Result<String> {
        let slug = slugify(conversation_id);
        let properties = json!({
            "conversation_id": conversation_id,
            "title": conversation_id,
        });

        self.helix
            .create_node(&HelixNodeUpsertRequest::metadata(
                CONVERSATION_NODE_TYPE,
                properties,
                Some(format!("conversation::{slug}")),
            ))
            .await
    }

    async fn write_messages(
        &self,
        _memory_node_id: &str,
        conversation_node_id: Option<&str>,
        conversation_id: Option<&str>,
        messages: &[MessageRecord],
    ) -> anyhow::Result<()> {
        if messages.is_empty() {
            return Ok(());
        }

        let mut message_nodes: HashMap<String, String> = HashMap::new();

        for (idx, message) in messages.iter().enumerate() {
            if message.role.trim().is_empty() || message.content.trim().is_empty() {
                continue;
            }

            let id_for_slug = message
                .message_id
                .as_deref()
                .map(slugify)
                .unwrap_or_else(|| slugify(&format!("message-{idx}")));

            let created_at = message.created_at.unwrap_or_else(Utc::now).to_rfc3339();

            let conversation_value = message
                .conversation_id
                .as_deref()
                .or(conversation_id)
                .map(|s| s.to_string());

            let properties = json!({
                "message_id": message.message_id,
                "conversation_id": conversation_value,
                "role": message.role,
                "content": message.content,
                "created_at": created_at,
                "metadata": message.metadata.clone(),
            });

            let node_id = self
                .helix
                .create_node(&HelixNodeUpsertRequest::metadata(
                    MESSAGE_NODE_TYPE,
                    properties,
                    Some(format!("message::{id_for_slug}")),
                ))
                .await?;

            if let Some(conv_id) = conversation_node_id {
                self.link_nodes(EDGE_IN_THREAD, &node_id, conv_id, "in_thread")
                    .await?;
                self.link_nodes(EDGE_HAS_MESSAGE, conv_id, &node_id, "has_message")
                    .await?;
            }

            if let Some(msg_id) = &message.message_id {
                message_nodes.insert(msg_id.clone(), node_id);
            }
        }

        // Thread replies after all nodes exist
        for message in messages.iter() {
            let from_node = match &message.message_id {
                Some(mid) => message_nodes.get(mid),
                None => None,
            };
            let to_node = match &message.reply_to {
                Some(reply_to) => message_nodes.get(reply_to),
                None => None,
            };

            if let (Some(from), Some(to)) = (from_node, to_node) {
                self.link_nodes(EDGE_REPLIES_TO, from, to, "replies_to")
                    .await?;
            }
        }

        Ok(())
    }

    async fn write_artifacts(
        &self,
        memory_node_id: &str,
        artifacts: &[ArtifactRef],
    ) -> anyhow::Result<()> {
        if artifacts.is_empty() {
            return Ok(());
        }

        for (idx, artifact) in artifacts.iter().enumerate() {
            if artifact.uri.trim().is_empty() {
                continue;
            }

            let id_for_slug = artifact
                .checksum
                .clone()
                .unwrap_or_else(|| slugify(&format!("artifact-{idx}")));

            let properties = json!({
                "artifact_id": artifact.checksum.clone().unwrap_or_else(|| id_for_slug.clone()),
                "uri": artifact.uri,
                "kind": artifact.kind,
                "checksum": artifact.checksum,
                "size_bytes": artifact.size_bytes,
                "title": artifact.title,
                "metadata": artifact.metadata.clone(),
            });

            let node_id = self
                .helix
                .create_node(&HelixNodeUpsertRequest::metadata(
                    ARTIFACT_NODE_TYPE,
                    properties,
                    Some(format!("artifact::{id_for_slug}")),
                ))
                .await?;

            self.link_nodes(
                EDGE_REFERENCES_ARTIFACT,
                memory_node_id,
                &node_id,
                "references_artifact",
            )
            .await?;
        }

        Ok(())
    }

    async fn write_tool_calls(
        &self,
        memory_node_id: &str,
        tool_calls: &[ToolCallRecord],
    ) -> anyhow::Result<()> {
        if tool_calls.is_empty() {
            return Ok(());
        }

        for (idx, tool_call) in tool_calls.iter().enumerate() {
            if tool_call.tool_name.trim().is_empty() {
                continue;
            }

            let id_for_slug = tool_call
                .tool_call_id
                .as_deref()
                .map(slugify)
                .unwrap_or_else(|| slugify(&format!("toolcall-{idx}")));

            let created_at = tool_call.created_at.unwrap_or_else(Utc::now).to_rfc3339();

            let properties = json!({
                "tool_call_id": tool_call.tool_call_id.clone().unwrap_or_else(|| id_for_slug.clone()),
                "tool_name": tool_call.tool_name,
                "args_json": tool_call.args_json,
                "result_summary": tool_call.result_summary,
                "created_at": created_at,
                "metadata": tool_call.metadata.clone(),
            });

            let node_id = self
                .helix
                .create_node(&HelixNodeUpsertRequest::metadata(
                    TOOL_CALL_NODE_TYPE,
                    properties,
                    Some(format!("toolcall::{id_for_slug}")),
                ))
                .await?;

            self.link_nodes(
                EDGE_PRODUCED_MEMORY,
                &node_id,
                memory_node_id,
                "produced_memory",
            )
            .await?;
        }

        Ok(())
    }

    async fn link_nodes(
        &self,
        edge_type: &str,
        from: &str,
        to: &str,
        note: &str,
    ) -> anyhow::Result<()> {
        let metadata = json!({
            "note": note,
            "created_at": Utc::now().to_rfc3339(),
        });
        self.helix
            .create_edge(&HelixEdgeUpsertRequest::new(
                edge_type,
                from.to_string(),
                to.to_string(),
                Some(metadata),
            ))
            .await
    }

    async fn attach_neighbors(&self, records: &mut [MemoryRecord]) -> anyhow::Result<()> {
        for record in records.iter_mut() {
            let node_id = match &record.id {
                Some(id) => id.clone(),
                None => continue,
            };

            let neighbors = match self.helix.fetch_neighbors(&node_id, 1).await {
                Ok(list) => list,
                Err(err) => {
                    warn!(?err, %node_id, "Failed to fetch Helix neighbors");
                    continue;
                }
            };

            if neighbors.is_empty() {
                continue;
            }

            let summary: Vec<Value> = neighbors
                .into_iter()
                .map(|neighbor| {
                    json!({
                        "node_id": neighbor.node_id,
                        "type": neighbor.node_type,
                        "edge_type": neighbor.edge_type,
                        "properties": neighbor.properties,
                    })
                })
                .collect();

            insert_metadata_field(
                &mut record.metadata,
                "helix_neighbors",
                Value::Array(summary),
            );
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete_memory(&self, node_id: &str) -> anyhow::Result<()> {
        self.helix.delete_node(node_id).await
    }

    #[allow(dead_code)]
    /// Placeholder for future Helix-backed usage logging; currently just records via tracing.
    pub async fn log_usage_event(&self, event: &UsageEvent) -> anyhow::Result<()> {
        info!(
            request_id = %event.request_id,
            agent = event.agent_name,
            operator = ?event.operator_id,
            specialist = ?event.specialist_agent_id,
            tokens = event.tokens_consumed,
            tool = event.tool_name,
            "Usage event (Helix logging stub)"
        );
        Ok(())
    }

    #[allow(dead_code)]
    /// Placeholder for NCRX payout logging until Helix event schemas are finalized.
    pub async fn log_payout_event(&self, event: &PayoutEvent) -> anyhow::Result<()> {
        info!(
            request_id = %event.request_id,
            operator = %event.operator_id,
            specialist = %event.specialist_agent_id,
            tokens = event.tokens_settled,
            total_cost = event.total_cost,
            rating = ?event.rating,
            "Payout event (Helix logging stub)"
        );
        Ok(())
    }
}

#[async_trait]
impl RagClient for HelixGraphClient {
    async fn write(&self, mut request: MemoryWriteRequest) -> anyhow::Result<MemoryWriteResponse> {
        let vector = self
            .embedder
            .embed(&request.record.full_content)
            .await
            .context("Helix embedding failed")?;

        if request.record.id.is_some() {
            warn!("Helix backend will overwrite provided memory id");
            request.record.id = None;
        }

        let node_id = self.upsert_memory(&request.record, vector).await?;
        self.write_memory_context(&node_id, &request.record)
            .await
            .context("Failed to write Helix edges/perspectives")?;

        Ok(MemoryWriteResponse { memory_id: node_id })
    }

    async fn query(&self, query: MemoryQuery) -> anyhow::Result<Vec<MemoryRecord>> {
        let vector = self
            .embedder
            .embed(&query.query)
            .await
            .context("Helix embedding failed")?;

        let (mut records, neighbors_complete) = self.search(query, vector).await?;

        if !neighbors_complete {
            self.attach_neighbors(&mut records)
                .await
                .context("Failed to fetch Helix neighborhood metadata")?;
        }

        Ok(records)
    }

    async fn delete(&self, request: MemoryDeleteRequest) -> anyhow::Result<()> {
        // Attempt to delete the chunk/node by id. If not found, surface a clear error.
        self.helix
            .delete_node(&request.id)
            .await
            .with_context(|| format!("Helix delete failed for id {}", request.id))
    }
}

#[derive(Deserialize)]
struct WriteMemoryV2Response {
    memory_entry: HelixWriteNode,
    memory_chunk: InsertMemoryChunkNode,
}

#[derive(Deserialize)]
struct HelixWriteNode {
    id: String,
}

#[derive(Deserialize)]
struct InsertMemoryChunkNode {
    id: String,
    #[serde(default)]
    chunk_id: Option<String>,
}

#[derive(Deserialize)]
struct SearchMemoryChunkResponse {
    matches: Vec<MemoryChunkHit>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct MemoryChunkHit {
    id: String,
    agent_name: String,
    topic: String,
    #[serde(default)]
    project: Option<String>,
    summary: String,
    timestamp: String,
    #[serde(default)]
    open_questions: Vec<String>,
    #[serde(default)]
    metadata: Option<String>,
    #[serde(default)]
    score: Option<f64>,
    #[serde(default)]
    artifact_id: Option<String>,
    #[serde(default)]
    chunk_id: Option<String>,
    #[serde(default)]
    payload_hash: Option<String>,
}

#[async_trait]
impl RagClient for HelixQueryRagClient {
    async fn write(&self, request: MemoryWriteRequest) -> anyhow::Result<MemoryWriteResponse> {
        let record = request.record;
        // Embed combined summary + full_content to capture more semantics.
        let embed_text = format!("{}\n\n{}", record.summary, record.full_content);
        let vector = self
            .embedder
            .embed(&embed_text)
            .await
            .context("Helix embedding failed")?;

        if vector.len() != self.vector_dim {
            warn!(
                expected = self.vector_dim,
                actual = vector.len(),
                "Embedding dimension mismatch during HelixQL write"
            );
        }

        let timestamp = record.timestamp.to_rfc3339();
        let metadata_json = record
            .metadata
            .as_ref()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "{}".to_string());

        let chunk_id = record
            .id
            .clone()
            .unwrap_or_else(|| format!("chunk-{}", record.timestamp.timestamp_millis()));

        let artifact_id = record
            .project
            .clone()
            .unwrap_or_else(|| "artifact-auto".to_string());

        let payload_hash = format!("sha256:{}", blake3::hash(embed_text.as_bytes()).to_hex());

        let payload = json!({
            "vector": Self::to_f64(&vector),
            "agent_name": record.agent_name,
            "topic": record.topic,
            "project": record.project.clone().unwrap_or_default(),
            "summary": record.summary,
            "full_content": record.full_content,
            "timestamp": timestamp,
            "confidence": record.confidence,
            "open_questions": record.open_questions,
            "metadata": metadata_json,
            "payload_hash": payload_hash,
            "chunk_id": chunk_id,
            "artifact_id": artifact_id,
            "conversation_id": record.conversation_id.unwrap_or_default(),
        });

        let response: WriteMemoryV2Response = self
            .helix
            .post_query("write_memory_v2", &payload)
            .await
            .context("HelixQL write_memory_v2 failed")?;

        let _memory_entry_id = response.memory_entry.id;
        let memory_id = response
            .memory_chunk
            .chunk_id
            .unwrap_or(response.memory_chunk.id);

        Ok(MemoryWriteResponse { memory_id })
    }

    async fn query(&self, query: MemoryQuery) -> anyhow::Result<Vec<MemoryRecord>> {
        let vector = self
            .embedder
            .embed(&query.query)
            .await
            .context("Helix embedding failed")?;

        if vector.len() != self.vector_dim {
            warn!(
                expected = self.vector_dim,
                actual = vector.len(),
                "Embedding dimension mismatch during HelixQL search"
            );
        }

        let payload = json!({
            "vector": Self::to_f64(&vector),
            "limit": query.limit() as i64,
        });

        let response: SearchMemoryChunkResponse = self
            .helix
            .post_query("search_memory_v2", &payload)
            .await
            .context("HelixQL search_memory_v2 failed")?;

        let mut records = Vec::with_capacity(response.matches.len());
        for hit in response.matches {
            if let Some(score) = hit.score {
                if score < Self::MIN_SCORE {
                    continue;
                }
            }
            let ts = DateTime::parse_from_rfc3339(&hit.timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let metadata = hit
                .metadata
                .as_ref()
                .and_then(|m| serde_json::from_str(m).ok());

            let full_content = metadata
                .as_ref()
                .and_then(|m: &serde_json::Value| m.get("body"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| hit.summary.clone());

            let mut record = MemoryRecord {
                id: Some(hit.chunk_id.unwrap_or(hit.id)),
                agent_name: hit.agent_name,
                topic: hit.topic,
                project: hit.project.clone(),
                conversation_id: None,
                timestamp: ts,
                summary: hit.summary.clone(),
                full_content,
                confidence: hit.score.unwrap_or(0.5) as f32,
                open_questions: hit.open_questions,
                perspectives: Vec::new(),
                messages: Vec::new(),
                artifacts: Vec::new(),
                tool_calls: Vec::new(),
                metadata,
            };

            if let Some(depth) = self.neighbor_depth {
                if let Err(err) = self.enrich_from_neighbors(&mut record, depth).await {
                    warn!(?err, "Failed to enrich HelixQL hit with neighbors");
                }
            }

            records.push(record);
        }

        Ok(records)
    }

    async fn delete(&self, request: MemoryDeleteRequest) -> anyhow::Result<()> {
        self.helix
            .delete_node(&request.id)
            .await
            .with_context(|| format!("Helix delete failed for id {}", request.id))
    }
}

impl HelixQueryRagClient {
    async fn enrich_from_neighbors(
        &self,
        record: &mut MemoryRecord,
        depth: usize,
    ) -> anyhow::Result<()> {
        let node_id = match &record.id {
            Some(id) => id.clone(),
            None => return Ok(()),
        };

        let neighbors = match self.helix.fetch_neighbors(&node_id, depth).await {
            Ok(list) => list,
            Err(err) => {
                warn!(?err, %node_id, "Failed to fetch Helix neighbors (query client)");
                return Ok(());
            }
        };

        if neighbors.is_empty() {
            return Ok(());
        }

        for neighbor in &neighbors {
            if neighbor.node_type.eq_ignore_ascii_case("memoryentry")
                || neighbor.node_type.eq_ignore_ascii_case("memory_entry")
            {
                apply_memory_entry_overlay(record, neighbor);
            }
        }

        let summary: Vec<Value> = neighbors.into_iter().map(helix_neighbor_to_value).collect();

        insert_metadata_field(
            &mut record.metadata,
            "helix_neighbors",
            Value::Array(summary),
        );

        Ok(())
    }
}

fn neighbor_depth_from_env() -> Option<usize> {
    env::var("RAG_NEIGHBOR_DEPTH")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|d| *d > 0)
        .or(Some(1))
}

#[derive(Serialize)]
struct HelixNodeUpsertRequest {
    #[serde(rename = "type")]
    node_type: String,
    properties: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding: Option<HelixEmbeddingPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_id: Option<String>,
}

impl HelixNodeUpsertRequest {
    fn metadata(node_type: &str, properties: Value, external_id: Option<String>) -> Self {
        Self {
            node_type: node_type.to_string(),
            properties,
            embedding: None,
            external_id,
        }
    }
}

#[derive(Serialize)]
struct HelixEmbeddingPayload {
    model: String,
    vector: Vec<f32>,
}

#[derive(Deserialize)]
struct HelixNodeWriteResponse {
    node_id: String,
}

#[derive(Serialize)]
struct HelixEdgeUpsertRequest {
    #[serde(rename = "type")]
    edge_type: String,
    from: String,
    to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<Value>,
}

impl HelixEdgeUpsertRequest {
    fn new(edge_type: &str, from: String, to: String, properties: Option<Value>) -> Self {
        Self {
            edge_type: edge_type.to_string(),
            from,
            to,
            properties,
        }
    }
}

#[derive(Serialize)]
struct HelixSearchRequest {
    #[serde(rename = "type")]
    node_type: String,
    limit: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filters: Vec<HelixPropertyFilter>,
    vector: HelixEmbeddingPayload,
}

#[derive(Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum HelixPropertyFilter {
    Equals { field: String, value: String },
    Gte { field: String, value: String },
}

#[derive(Deserialize)]
struct HelixSearchResponse {
    hits: Vec<HelixSearchHit>,
}

#[derive(Deserialize)]
struct HelixSearchHit {
    node_id: String,
    #[allow(dead_code)]
    score: f32,
    properties: HelixMemoryProperties,
    #[serde(default)]
    neighbors: Option<Vec<HelixNeighbor>>,
}

#[derive(Deserialize)]
struct HelixNeighborResponse {
    neighbors: Vec<HelixNeighbor>,
}

#[derive(Deserialize)]
struct HelixNeighbor {
    node_id: String,
    #[serde(rename = "type")]
    node_type: String,
    edge_type: String,
    #[serde(default)]
    properties: Value,
}

fn helix_neighbor_to_value(neighbor: HelixNeighbor) -> Value {
    json!({
        "node_id": neighbor.node_id,
        "node_type": neighbor.node_type,
        "edge_type": neighbor.edge_type,
        "properties": neighbor.properties,
    })
}

fn apply_memory_entry_overlay(record: &mut MemoryRecord, neighbor: &HelixNeighbor) {
    let props = &neighbor.properties;

    if let Some(summary) = props.get("summary").and_then(|v| v.as_str()) {
        record.summary = summary.to_string();
    }

    if let Some(full_content) = props.get("full_content").and_then(|v| v.as_str()) {
        record.full_content = full_content.to_string();
    }

    if let Some(project) = props.get("project").and_then(|v| v.as_str()) {
        if !project.is_empty() {
            record.project = Some(project.to_string());
        }
    }

    if let Some(conversation_id) = props.get("conversation_id").and_then(|v| v.as_str()) {
        if !conversation_id.is_empty() {
            record.conversation_id = Some(conversation_id.to_string());
        }
    }

    if let Some(confidence) = props.get("confidence").and_then(|v| v.as_f64()) {
        record.confidence = confidence as f32;
    }

    if let Some(open_questions) = props.get("open_questions").and_then(|v| v.as_array()) {
        let questions: Vec<String> = open_questions
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        if !questions.is_empty() {
            record.open_questions = questions;
        }
    }

    if let Some(timestamp) = props.get("timestamp").and_then(|v| v.as_str()) {
        if let Ok(parsed) = DateTime::parse_from_rfc3339(timestamp) {
            record.timestamp = parsed.with_timezone(&Utc);
        }
    }

    if let Some(metadata) = props.get("metadata") {
        let parsed = normalize_metadata_value(metadata);
        record.metadata = merge_metadata(record.metadata.take(), parsed);
    }

    insert_metadata_field(
        &mut record.metadata,
        "memory_entry_id",
        Value::String(neighbor.node_id.clone()),
    );
}

fn normalize_metadata_value(value: &Value) -> Value {
    match value {
        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::String(s.clone())),
        other => other.clone(),
    }
}

fn merge_metadata(existing: Option<Value>, incoming: Value) -> Option<Value> {
    match (existing, incoming) {
        (Some(Value::Object(mut left)), Value::Object(right)) => {
            for (k, v) in right.into_iter() {
                left.insert(k, v);
            }
            Some(Value::Object(left))
        }
        (None, value) => Some(value),
        (Some(existing), value) => {
            let mut map = Map::new();
            map.insert("existing".to_string(), existing);
            map.insert("overlay".to_string(), value);
            Some(Value::Object(map))
        }
    }
}

#[derive(Serialize, Deserialize)]
struct HelixMemoryProperties {
    agent_name: String,
    topic: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conversation_id: Option<String>,
    summary: String,
    timestamp: String,
    record_json: String,
    confidence: f32,
    #[serde(default)]
    open_questions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    metadata: Option<Value>,
}

impl HelixMemoryProperties {
    fn from_record(record: &MemoryRecord, record_json: String) -> Self {
        Self {
            agent_name: record.agent_name.clone(),
            topic: record.topic.clone(),
            project: record.project.clone(),
            conversation_id: record.conversation_id.clone(),
            summary: record.summary.clone(),
            timestamp: record.timestamp.to_rfc3339(),
            record_json,
            confidence: record.confidence,
            open_questions: record.open_questions.clone(),
            metadata: record.metadata.clone(),
        }
    }
}

fn insert_metadata_field(metadata: &mut Option<Value>, key: &str, value: Value) {
    let mut map = match metadata.take() {
        Some(Value::Object(obj)) => obj,
        Some(other) => {
            let mut obj = Map::new();
            obj.insert("raw".to_string(), other);
            obj
        }
        None => Map::new(),
    };
    map.insert(key.to_string(), value);
    *metadata = Some(Value::Object(map));
}

fn slugify(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unnamed".to_string();
    }

    let mut slug: String = trimmed
        .chars()
        .map(|ch| match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' => ch,
            _ => '-',
        })
        .collect();

    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "unnamed".to_string()
    } else {
        slug
    }
}
