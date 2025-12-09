use std::sync::Arc;

use anyhow::Context;
use tracing::{instrument, warn};

use super::client::SharedRagClient;
use super::config::{HelixConfig, RagConfig};
use super::embed::OpenAiEmbeddingsClient;
use super::helix::{HelixClient, HelixQueryRagClient};
use super::mock::MockRagClient;
use super::types::{
    MemoryDeleteRequest, MemoryQuery, MemoryRequest, MemoryResponse, MemoryWriteRequest,
};

pub type SharedRagAgent = Arc<RagAgent>;

/// High-level interface responsible for validating and executing memory requests.
pub struct RagAgent {
    client: SharedRagClient,
}

impl RagAgent {
    pub fn new(client: SharedRagClient) -> Self {
        Self { client }
    }

    #[instrument(skip_all, name = "rag_agent_handle")]
    pub async fn handle(&self, request: MemoryRequest) -> anyhow::Result<MemoryResponse> {
        match request {
            MemoryRequest::Write(payload) => self.handle_write(payload).await,
            MemoryRequest::Retrieve(query) => self.handle_retrieve(query).await,
            MemoryRequest::Delete(payload) => self.handle_delete(payload).await,
        }
    }

    async fn handle_write(&self, request: MemoryWriteRequest) -> anyhow::Result<MemoryResponse> {
        let record = request.record;
        anyhow::ensure!(
            record.id.is_none(),
            "Memory writes should not include an id; backend assigns it"
        );

        let write_ack = self
            .client
            .write(MemoryWriteRequest { record })
            .await
            .context("RAG write failed")?;

        Ok(MemoryResponse {
            notes: format!("memory_id={} stored", write_ack.memory_id),
            records: Vec::new(),
            memory_ids: vec![write_ack.memory_id],
        })
    }

    async fn handle_retrieve(&self, query: MemoryQuery) -> anyhow::Result<MemoryResponse> {
        let records = self.client.query(query).await.context("RAG query failed")?;

        Ok(MemoryResponse {
            notes: format!("returned {} memories", records.len()),
            records,
            memory_ids: Vec::new(),
        })
    }

    async fn handle_delete(&self, request: MemoryDeleteRequest) -> anyhow::Result<MemoryResponse> {
        self.client
            .delete(request.clone())
            .await
            .context("RAG delete failed")?;

        Ok(MemoryResponse {
            notes: format!("deleted memory_id={}", request.id),
            records: Vec::new(),
            memory_ids: vec![request.id],
        })
    }
}

/// Attempt to build a RAG agent based on env configuration. Optionally falls back to a mock.
pub async fn build_rag_agent_from_env(
    default_to_mock: bool,
) -> anyhow::Result<Option<SharedRagAgent>> {
    let helix_config = match HelixConfig::from_env() {
        Ok(config) => config,
        Err(err) if default_to_mock => {
            warn!(?err, "Helix config missing; using in-memory mock RAG store");
            let client: SharedRagClient = Arc::new(MockRagClient::default());
            return Ok(Some(Arc::new(RagAgent::new(client))));
        }
        Err(_) => return Ok(None),
    };

    let embed_config = match RagConfig::from_env() {
        Ok(config) => config,
        Err(err) if default_to_mock => {
            warn!(
                ?err,
                "Embedding config missing; using in-memory mock RAG store"
            );
            let client: SharedRagClient = Arc::new(MockRagClient::default());
            return Ok(Some(Arc::new(RagAgent::new(client))));
        }
        Err(_) => return Ok(None),
    };

    let embedder = Arc::new(OpenAiEmbeddingsClient::from_config(&embed_config)?);
    let vector_dim = embed_config.vector_dim;
    let embedding_model = embed_config.embedding_model.clone();

    match HelixClient::new(helix_config.clone()) {
        Ok(helix_http) => {
            let client: SharedRagClient = Arc::new(HelixQueryRagClient::new(
                helix_http,
                embedder,
                embedding_model,
                vector_dim,
            ));
            Ok(Some(Arc::new(RagAgent::new(client))))
        }
        Err(err) if default_to_mock => {
            warn!(?err, "Helix client init failed; using mock RAG store");
            let client: SharedRagClient = Arc::new(MockRagClient::with_config(embed_config));
            Ok(Some(Arc::new(RagAgent::new(client))))
        }
        Err(err) => Err(err),
    }
}
