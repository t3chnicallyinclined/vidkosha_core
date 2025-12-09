use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Structured payload for messages entering the Agent network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub input: String,
}

impl AgentRequest {
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
        }
    }
}

/// Standardized response wrapper so downstream tools can rely on metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub output: String,
    pub metadata: Option<serde_json::Value>,
}

impl AgentResponse {
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            metadata: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_metadata(output: impl Into<String>, metadata: serde_json::Value) -> Self {
        Self {
            output: output.into(),
            metadata: Some(metadata),
        }
    }
}

#[async_trait]
pub trait AgentBehavior: Send + Sync {
    async fn handle(&self, request: AgentRequest) -> anyhow::Result<AgentResponse>;
}
