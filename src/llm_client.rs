use std::env;
use std::sync::Arc;

use anyhow::Context;
use async_openai::types::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::{config::OpenAIConfig, Client as AsyncOpenAiClient};
use async_trait::async_trait;
use tracing::instrument;

pub type SharedLlmClient = Arc<dyn LlmClient>;

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String>;
}

/// Temporary stand-in until we wire a real LLM backend.
#[derive(Debug, Default, Clone)]
pub struct EchoLlmClient;

#[async_trait]
impl LlmClient for EchoLlmClient {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        Ok(format!(
            "[stubbed Agent response]\nI received: {prompt}\nNext step: connect to LLM backend."
        ))
    }
}

impl EchoLlmClient {
    pub fn shared() -> SharedLlmClient {
        Arc::new(Self)
    }
}

/// OpenAI-compatible client that can point at OpenAI, vLLM, or any HTTP-compatible backend.
pub struct OpenAiLlmClient {
    client: AsyncOpenAiClient<OpenAIConfig>,
    model: String,
    system_prompt: String,
}

impl OpenAiLlmClient {
    const DEFAULT_MODEL: &'static str = "llama-3-8b-instruct";
    const DEFAULT_SYSTEM_PROMPT: &'static str =
        "You are Agent, orchestrator of Vidkosha Cortex. Respond with crisp, actionable output.";

    pub fn shared_from_env() -> anyhow::Result<SharedLlmClient> {
        let client = Self::from_env()?;
        Ok(Arc::new(client))
    }

    fn from_env() -> anyhow::Result<Self> {
        let config = Self::build_config_from_env()?;
        let model =
            env::var("VK_CORTEX_LLM_MODEL").unwrap_or_else(|_| Self::DEFAULT_MODEL.to_string());
        let system_prompt = env::var("VK_CORTEX_SYSTEM_PROMPT")
            .unwrap_or_else(|_| Self::DEFAULT_SYSTEM_PROMPT.to_string());

        Ok(Self {
            client: AsyncOpenAiClient::with_config(config),
            model,
            system_prompt,
        })
    }

    fn build_config_from_env() -> anyhow::Result<OpenAIConfig> {
        let api_key = env::var("OPENAI_API_KEY")
            .or_else(|_| env::var("AIE_OPENAI_API_KEY"))
            .context("Set OPENAI_API_KEY (or AIE_OPENAI_API_KEY) to use the OpenAI client")?;

        let mut config = OpenAIConfig::new().with_api_key(api_key);

        if let Ok(base_url) =
            env::var("OPENAI_BASE_URL").or_else(|_| env::var("AIE_OPENAI_BASE_URL"))
        {
            config = config.with_api_base(base_url);
        }

        Ok(config)
    }

    #[instrument(level = "debug", skip_all)]
    async fn chat(&self, prompt: &str) -> anyhow::Result<String> {
        let system_message = ChatCompletionRequestSystemMessageArgs::default()
            .content(&self.system_prompt)
            .build()?;
        let user_message = ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()?;

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .temperature(0.2)
            .messages(vec![system_message.into(), user_message.into()])
            .build()?;

        let response = self.client.chat().create(request).await?;
        let choice = response
            .choices
            .first()
            .context("LLM response did not contain any choices")?;

        let output = choice
            .message
            .content
            .clone()
            .unwrap_or_else(|| String::from("[empty LLM response]"));

        Ok(output)
    }
}

#[async_trait]
impl LlmClient for OpenAiLlmClient {
    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        self.chat(prompt).await
    }
}

/// Attempt to build an OpenAI-compatible client, optionally falling back to the echo client.
pub fn build_llm_client_from_env(default_to_echo: bool) -> anyhow::Result<SharedLlmClient> {
    match OpenAiLlmClient::shared_from_env() {
        Ok(client) => Ok(client),
        Err(err) if default_to_echo => {
            tracing::warn!(?err, "Falling back to EchoLlmClient");
            Ok(EchoLlmClient::shared())
        }
        Err(err) => Err(err),
    }
}
