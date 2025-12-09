use async_trait::async_trait;
use tracing::{instrument, warn};

use crate::llm_client::SharedLlmClient;
use crate::rag::{MemoryFilters, MemoryQuery, MemoryRecord, MemoryRequest, SharedRagAgent};

use super::traits::{AgentBehavior, AgentRequest, AgentResponse};

fn format_prompt(
    directive: &str,
    body_hint: &str,
    request: &AgentRequest,
    context: Option<&str>,
) -> String {
    let mut prompt = String::from(directive.trim());

    if let Some(ctx) = context.and_then(|raw| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    }) {
        prompt.push_str("\n\nContext snippets:\n");
        prompt.push_str(ctx);
    }

    prompt.push_str("\n\nUser brief:\n");
    prompt.push_str(request.input.trim());
    prompt.push_str("\n\nRespond with ");
    prompt.push_str(body_hint.trim());
    prompt.push('.');

    prompt
}

pub struct CTOAgent {
    llm_client: SharedLlmClient,
    rag_agent: Option<SharedRagAgent>,
}

impl CTOAgent {
    const DIRECTIVE: &'static str = "You are CTOAgent, the architecture strategist of Vidkosha Cortex. Restate constraints, articulate service boundaries, and surface trade-offs before recommending next steps.";
    const AGENT_NAME: &'static str = "CTOAgent";
    const DEFAULT_TOPIC: &'static str = "architecture";

    pub fn new(llm_client: SharedLlmClient) -> Self {
        Self {
            llm_client,
            rag_agent: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_rag(mut self, rag_agent: Option<SharedRagAgent>) -> Self {
        self.rag_agent = rag_agent;
        self
    }

    fn compose_prompt(&self, request: &AgentRequest, context: Option<&str>) -> String {
        format_prompt(
            Self::DIRECTIVE,
            "a structured brief with sections: ## Overview, ## Components, ## Trade-offs, ## Next Actions",
            request,
            context,
        )
    }

    async fn build_context(&self, request: &AgentRequest) -> Option<String> {
        fetch_recent_memories(
            &self.rag_agent,
            Self::AGENT_NAME,
            Some(Self::DEFAULT_TOPIC),
            &request.input,
        )
        .await
    }
}

#[async_trait]
impl AgentBehavior for CTOAgent {
    #[instrument(skip_all, fields(role = "CTOAgent", input = %request.input))]
    async fn handle(&self, request: AgentRequest) -> anyhow::Result<AgentResponse> {
        let context = self.build_context(&request).await;
        let prompt = self.compose_prompt(&request, context.as_deref());
        let output = self.llm_client.complete(&prompt).await?;
        Ok(AgentResponse::new(output))
    }
}

pub struct SeniorEngineerAgent {
    llm_client: SharedLlmClient,
    rag_agent: Option<SharedRagAgent>,
}

impl SeniorEngineerAgent {
    const DIRECTIVE: &'static str = "You are SeniorEngineerAgent, a pragmatic Rust engineer. Outline implementation steps, cite risks, and note verification commands.";
    const AGENT_NAME: &'static str = "SeniorEngineerAgent";
    const DEFAULT_TOPIC: &'static str = "engineering";

    pub fn new(llm_client: SharedLlmClient) -> Self {
        Self {
            llm_client,
            rag_agent: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_rag(mut self, rag_agent: Option<SharedRagAgent>) -> Self {
        self.rag_agent = rag_agent;
        self
    }

    fn compose_prompt(&self, request: &AgentRequest, context: Option<&str>) -> String {
        format_prompt(
            Self::DIRECTIVE,
            "## Changes (bullets), ## Verification (commands vs expectations), ## Next Actions",
            request,
            context,
        )
    }

    async fn build_context(&self, request: &AgentRequest) -> Option<String> {
        fetch_recent_memories(
            &self.rag_agent,
            Self::AGENT_NAME,
            Some(Self::DEFAULT_TOPIC),
            &request.input,
        )
        .await
    }
}

#[async_trait]
impl AgentBehavior for SeniorEngineerAgent {
    #[instrument(skip_all, fields(role = "SeniorEngineerAgent", input = %request.input))]
    async fn handle(&self, request: AgentRequest) -> anyhow::Result<AgentResponse> {
        let context = self.build_context(&request).await;
        let prompt = self.compose_prompt(&request, context.as_deref());
        let output = self.llm_client.complete(&prompt).await?;
        Ok(AgentResponse::new(output))
    }
}

pub struct ResearcherAgent {
    llm_client: SharedLlmClient,
    rag_agent: Option<SharedRagAgent>,
}

impl ResearcherAgent {
    const DIRECTIVE: &'static str = "You are ResearcherAgent. Surface the most relevant knowledge, cite every claim, and flag open questions.";
    const AGENT_NAME: &'static str = "ResearcherAgent";
    const DEFAULT_TOPIC: &'static str = "research";

    pub fn new(llm_client: SharedLlmClient) -> Self {
        Self {
            llm_client,
            rag_agent: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_rag(mut self, rag_agent: Option<SharedRagAgent>) -> Self {
        self.rag_agent = rag_agent;
        self
    }

    fn compose_prompt(&self, request: &AgentRequest, context: Option<&str>) -> String {
        format_prompt(
            Self::DIRECTIVE,
            "## Findings (bulleted, cite sources), ## Sources (list), ## Next Steps, Confidence: <0-1>, Open Questions: <list>",
            request,
            context,
        )
    }

    async fn build_context(&self, request: &AgentRequest) -> Option<String> {
        fetch_recent_memories(
            &self.rag_agent,
            Self::AGENT_NAME,
            Some(Self::DEFAULT_TOPIC),
            &request.input,
        )
        .await
    }
}

#[async_trait]
impl AgentBehavior for ResearcherAgent {
    #[instrument(skip_all, fields(role = "ResearcherAgent", input = %request.input))]
    async fn handle(&self, request: AgentRequest) -> anyhow::Result<AgentResponse> {
        let context = self.build_context(&request).await;
        let prompt = self.compose_prompt(&request, context.as_deref());
        let output = self.llm_client.complete(&prompt).await?;
        Ok(AgentResponse::new(output))
    }
}

pub struct OpsChainAgent {
    llm_client: SharedLlmClient,
    rag_agent: Option<SharedRagAgent>,
}

impl OpsChainAgent {
    const DIRECTIVE: &'static str = "You are OpsChainAgent. Model capacity, reliability, and ops/anchoring trade-offs with actionable next steps.";
    const AGENT_NAME: &'static str = "OpsChainAgent";
    const DEFAULT_TOPIC: &'static str = "operations";

    pub fn new(llm_client: SharedLlmClient) -> Self {
        Self {
            llm_client,
            rag_agent: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_rag(mut self, rag_agent: Option<SharedRagAgent>) -> Self {
        self.rag_agent = rag_agent;
        self
    }

    fn compose_prompt(&self, request: &AgentRequest, context: Option<&str>) -> String {
        format_prompt(
            Self::DIRECTIVE,
            "## Options (table or bullets with cost + capacity), ## Recommendation, ## Risks, ## Next Actions",
            request,
            context,
        )
    }

    async fn build_context(&self, request: &AgentRequest) -> Option<String> {
        fetch_recent_memories(
            &self.rag_agent,
            Self::AGENT_NAME,
            Some(Self::DEFAULT_TOPIC),
            &request.input,
        )
        .await
    }
}

#[async_trait]
impl AgentBehavior for OpsChainAgent {
    #[instrument(skip_all, fields(role = "OpsChainAgent", input = %request.input))]
    async fn handle(&self, request: AgentRequest) -> anyhow::Result<AgentResponse> {
        let context = self.build_context(&request).await;
        let prompt = self.compose_prompt(&request, context.as_deref());
        let output = self.llm_client.complete(&prompt).await?;
        Ok(AgentResponse::new(output))
    }
}

async fn fetch_recent_memories(
    rag_agent: &Option<SharedRagAgent>,
    agent_name: &str,
    topic_hint: Option<&str>,
    query_text: &str,
) -> Option<String> {
    let rag = rag_agent.as_ref()?;
    let trimmed_query = query_text.trim();
    let query_string = if trimmed_query.is_empty() {
        format!("latest {agent_name} context")
    } else {
        trimmed_query.to_string()
    };

    let filters = MemoryFilters {
        agent_name: Some(agent_name.to_string()),
        topic: topic_hint.map(|topic| topic.to_string()),
        ..MemoryFilters::default()
    };

    let query = MemoryQuery {
        query: query_string,
        filters,
        limit: 3,
    };

    match rag.handle(MemoryRequest::Retrieve(query)).await {
        Ok(response) if !response.records.is_empty() => {
            Some(render_memory_context(agent_name, &response.records))
        }
        Ok(_) => None,
        Err(err) => {
            warn!(
                ?err,
                agent = agent_name,
                "Failed to fetch RAG context for specialist"
            );
            None
        }
    }
}

fn render_memory_context(agent_name: &str, records: &[MemoryRecord]) -> String {
    let mut lines = Vec::with_capacity(records.len() + 1);
    lines.push(format!("Latest {agent_name} memos:"));

    for record in records {
        lines.push(format!(
            "- [{}] {} — {}",
            record.timestamp.to_rfc3339(),
            record.topic,
            record.summary
        ));
    }

    lines.join("\n")
}
