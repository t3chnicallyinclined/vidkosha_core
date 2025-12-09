use std::sync::Arc;

use async_trait::async_trait;

use super::types::{
    MemoryDeleteRequest, MemoryQuery, MemoryRecord, MemoryWriteRequest, MemoryWriteResponse,
};

#[async_trait]
pub trait RagClient: Send + Sync {
    async fn write(&self, request: MemoryWriteRequest) -> anyhow::Result<MemoryWriteResponse>;
    async fn query(&self, query: MemoryQuery) -> anyhow::Result<Vec<MemoryRecord>>;
    async fn delete(&self, request: MemoryDeleteRequest) -> anyhow::Result<()>;
}

pub type SharedRagClient = Arc<dyn RagClient>;
