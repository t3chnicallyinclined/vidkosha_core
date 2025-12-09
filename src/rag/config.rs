use std::env;

#[derive(Debug, Clone)]
pub struct RagConfig {
    pub embedding_api_key: String,
    pub embedding_base_url: Option<String>,
    pub embedding_model: String,
    pub vector_dim: usize,
}

impl RagConfig {
    const EMBEDDING_KEY_VARS: [&'static str; 4] = [
        "RAG_EMBEDDING_API_KEY",
        "AIE_RAG_EMBEDDING_API_KEY",
        "OPENAI_API_KEY",
        "AIE_OPENAI_API_KEY",
    ];
    const EMBEDDING_BASE_URL_VARS: [&'static str; 4] = [
        "RAG_EMBEDDING_BASE_URL",
        "AIE_RAG_EMBEDDING_BASE_URL",
        "OPENAI_BASE_URL",
        "AIE_OPENAI_BASE_URL",
    ];
    const EMBEDDING_MODEL_VARS: [&'static str; 2] =
        ["RAG_EMBEDDING_MODEL", "AIE_RAG_EMBEDDING_MODEL"];
    const VECTOR_DIM_VARS: [&'static str; 2] = ["RAG_VECTOR_DIM", "AIE_RAG_VECTOR_DIM"];

    pub fn from_env() -> anyhow::Result<Self> {
        let embedding_api_key =
            Self::read_env(&Self::EMBEDDING_KEY_VARS).unwrap_or_else(|| "sk-local".to_string());
        let embedding_model =
            Self::read_env(&Self::EMBEDDING_MODEL_VARS).unwrap_or_else(|| "bge-m3".to_string());
        let vector_dim: usize = Self::read_env(&Self::VECTOR_DIM_VARS)
            .and_then(|value| value.parse().ok())
            .unwrap_or(1024);

        Ok(Self {
            embedding_api_key,
            embedding_base_url: Self::read_env(&Self::EMBEDDING_BASE_URL_VARS)
                .or_else(|| Some("http://127.0.0.1:9000/v1".to_string())),
            embedding_model,
            vector_dim,
        })
    }

    fn read_env(candidates: &[&'static str]) -> Option<String> {
        candidates.iter().find_map(|key| env::var(key).ok())
    }
}

#[derive(Debug, Clone)]
pub struct HelixConfig {
    pub base_url: String,
    pub api_token: Option<String>,
    pub namespace: String,
    pub http_timeout_ms: u64,
}

impl HelixConfig {
    const BASE_URL_VARS: [&'static str; 2] = ["HELIX_BASE_URL", "AIE_HELIX_BASE_URL"];
    const API_TOKEN_VARS: [&'static str; 2] = ["HELIX_API_TOKEN", "AIE_HELIX_API_TOKEN"];
    const NAMESPACE_VARS: [&'static str; 3] = [
        "HELIX_GRAPH_NAMESPACE",
        "HELIX_NAMESPACE",
        "AIE_HELIX_GRAPH_NAMESPACE",
    ];
    const TIMEOUT_VARS: [&'static str; 2] = ["HELIX_HTTP_TIMEOUT_MS", "AIE_HELIX_HTTP_TIMEOUT_MS"];

    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = RagConfig::read_env(&Self::BASE_URL_VARS)
            .unwrap_or_else(|| "http://127.0.0.1:6969".to_string());
        let namespace = RagConfig::read_env(&Self::NAMESPACE_VARS)
            .unwrap_or_else(|| "vidkosha_cortex".to_string());
        let http_timeout_ms = RagConfig::read_env(&Self::TIMEOUT_VARS)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(10_000);

        Ok(Self {
            base_url,
            api_token: RagConfig::read_env(&Self::API_TOKEN_VARS),
            namespace,
            http_timeout_ms,
        })
    }
}
