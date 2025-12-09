use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use anyhow::anyhow;

use super::client::RagClient;
use super::config::RagConfig;
use super::types::{
    MemoryDeleteRequest, MemoryFilters, MemoryQuery, MemoryRecord, MemoryWriteRequest,
    MemoryWriteResponse,
};

#[derive(Default)]
pub struct MockRagClient {
    records: Mutex<Vec<MemoryRecord>>,
    id_counter: AtomicU64,
    #[allow(dead_code)]
    config: Option<RagConfig>,
}

impl MockRagClient {
    pub fn with_config(config: RagConfig) -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            id_counter: AtomicU64::new(0),
            config: Some(config),
        }
    }

    fn next_id(&self) -> String {
        let id = self.id_counter.fetch_add(1, Ordering::Relaxed) + 1;
        format!("mock-memory-{id}")
    }

    fn apply_filters<'a>(
        filters: &MemoryFilters,
        records: impl Iterator<Item = &'a MemoryRecord>,
    ) -> Vec<MemoryRecord>
    where
        MemoryRecord: Clone,
    {
        records
            .filter(|record| filters.matches(record))
            .cloned()
            .collect()
    }
}

#[async_trait::async_trait]
impl RagClient for MockRagClient {
    async fn write(&self, mut request: MemoryWriteRequest) -> anyhow::Result<MemoryWriteResponse> {
        let mut records = self.records.lock().expect("lock poisoned");
        let id = self.next_id();
        request.record.id = Some(id.clone());
        records.push(request.record);
        Ok(MemoryWriteResponse { memory_id: id })
    }

    async fn query(&self, query: MemoryQuery) -> anyhow::Result<Vec<MemoryRecord>> {
        let records = self
            .records
            .lock()
            .map_err(|_| anyhow!("mock rag client lock poisoned"))?;
        let filtered = Self::apply_filters(&query.filters, records.iter());
        Ok(filtered.into_iter().take(query.limit()).collect())
    }

    async fn delete(&self, request: MemoryDeleteRequest) -> anyhow::Result<()> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| anyhow!("mock rag client lock poisoned"))?;
        let before = records.len();
        records.retain(|r| r.id.as_deref() != Some(request.id.as_str()));
        let after = records.len();
        if before == after {
            return Err(anyhow!("memory_id not found"));
        }
        Ok(())
    }
}
