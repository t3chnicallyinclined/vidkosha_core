use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::Context;
use async_openai::{
    config::OpenAIConfig, types::CreateEmbeddingRequestArgs, Client as OpenAiClient,
};
use async_trait::async_trait;
use blake3;

use super::config::RagConfig;

#[async_trait]
pub trait EmbeddingsProvider: Send + Sync {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
}

pub struct OpenAiEmbeddingsClient {
    client: OpenAiClient<OpenAIConfig>,
    model: String,
    cache: Mutex<HashMap<String, Vec<f32>>>,
    cache_capacity: usize,
}

impl OpenAiEmbeddingsClient {
    pub fn from_config(config: &RagConfig) -> anyhow::Result<Self> {
        let mut openai_config = OpenAIConfig::new().with_api_key(config.embedding_api_key.clone());
        if let Some(base_url) = &config.embedding_base_url {
            openai_config = openai_config.with_api_base(base_url.clone());
        }

        Ok(Self {
            client: OpenAiClient::with_config(openai_config),
            model: config.embedding_model.clone(),
            cache: Mutex::new(HashMap::new()),
            cache_capacity: 512,
        })
    }
}

#[async_trait]
impl EmbeddingsProvider for OpenAiEmbeddingsClient {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let cache_key = blake3::hash(text.as_bytes()).to_hex().to_string();

        if let Some(hit) = self
            .cache
            .lock()
            .expect("embedding cache poisoned")
            .get(&cache_key)
            .cloned()
        {
            return Ok(hit);
        }

        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input(text)
            .build()?;

        let response = self.client.embeddings().create(request).await?;
        let embedding = response
            .data
            .first()
            .context("Embedding response missing data")?
            .embedding
            .clone();

        let mut cache = self.cache.lock().expect("embedding cache poisoned");

        if cache.len() >= self.cache_capacity {
            cache.clear();
        }
        cache.insert(cache_key, embedding.clone());

        Ok(embedding)
    }
}
