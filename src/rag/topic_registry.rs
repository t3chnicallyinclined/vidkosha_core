use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Context;
use serde_json::Value;
use tokio::sync::Mutex;

use super::config::HelixConfig;
use super::helix::HelixClient;

pub type SharedTopicRegistry = Arc<TopicRegistry>;

/// Minimal helper to upsert topic/category nodes via the InsertTopic HelixQL query.
pub struct TopicRegistry {
    client: HelixClient,
    known: Mutex<HashSet<String>>,
}

impl TopicRegistry {
    pub const MAX_TOPICS: usize = 500;

    pub fn new(config: HelixConfig) -> anyhow::Result<Self> {
        let client = HelixClient::new(config)?;
        Ok(Self {
            client,
            known: Mutex::new(HashSet::new()),
        })
    }

    /// Upsert a single topic. Metadata is stored as JSON string because the query expects String.
    pub async fn upsert_topic(&self, name: &str, metadata: &Value) -> anyhow::Result<String> {
        let payload = serde_json::json!({
            "name": name,
            "metadata": serde_json::to_string(metadata).unwrap_or_else(|_| "{}".to_string()),
        });

        #[derive(serde::Deserialize)]
        struct InsertTopicResponse {
            topic: InsertedTopic,
        }

        #[derive(serde::Deserialize)]
        struct InsertedTopic {
            id: Option<String>,
        }

        let resp: InsertTopicResponse = self
            .client
            .post_query("InsertTopic", &payload)
            .await
            .context("InsertTopic call failed")?;

        Ok(resp.topic.id.unwrap_or_else(|| name.to_string()))
    }

    pub async fn upsert_topics(&self, seeds: &[(String, Value)]) -> anyhow::Result<Vec<String>> {
        let mut guard = self.known.lock().await;
        let unique_new = seeds
            .iter()
            .filter(|(name, _)| !guard.contains(name))
            .count();

        if guard.len() + unique_new > Self::MAX_TOPICS {
            anyhow::bail!(
                "Topic cap reached ({}). Requested {} new topics would exceed the limit.",
                Self::MAX_TOPICS,
                unique_new
            );
        }

        let mut ids = Vec::with_capacity(seeds.len());
        for (name, meta) in seeds {
            let id = self.upsert_topic(name, meta).await?;
            ids.push(id.clone());
            guard.insert(name.clone());
        }
        Ok(ids)
    }
}
