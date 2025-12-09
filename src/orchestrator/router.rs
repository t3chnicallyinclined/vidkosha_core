use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::agents::{Agent, AgentBehavior, AgentRequest, AgentResponse};
use crate::orchestrator::routing::SemanticRouter;
use crate::rag::{MemoryRecord, MemoryRequest, MemoryResponse, MemoryWriteRequest, SharedRagAgent};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{instrument, warn};

type SpecialistHandle = Arc<dyn AgentBehavior>;

/// Wrapper around the primary Agent; later this will select specialist agents or pipelines.
pub struct OrchestratorRouter {
    front_desk: Agent,
    specialists: HashMap<String, SpecialistHandle>,
    rag_agent: Option<SharedRagAgent>,
    semantic_router: Option<SemanticRouter>,
}

impl OrchestratorRouter {
    pub fn new(front_desk: Agent) -> Self {
        Self {
            front_desk,
            specialists: HashMap::new(),
            rag_agent: None,
            semantic_router: None,
        }
    }

    pub fn with_specialist<A>(mut self, name: impl Into<String>, agent: A) -> Self
    where
        A: AgentBehavior + 'static,
    {
        self.specialists.insert(name.into(), Arc::new(agent));
        self
    }

    pub fn with_rag_agent(mut self, rag_agent: SharedRagAgent) -> Self {
        self.rag_agent = Some(rag_agent);
        self
    }

    pub fn with_semantic_router(mut self, router: SemanticRouter) -> Self {
        self.semantic_router = Some(router);
        self
    }

    #[instrument(skip_all, fields(input = %request.input))]
    pub async fn dispatch(&self, request: AgentRequest) -> anyhow::Result<RoutedAgentResponse> {
        let decision = self.classify_intent(&request);
        let (mut response, executed_agent) =
            self.route_to_agent(&decision, request.clone()).await?;
        response.metadata = Some(
            self.build_metadata(&request, &response, &decision, &executed_agent, None)
                .await,
        );

        Ok(RoutedAgentResponse {
            response,
            decision,
            executed_agent,
        })
    }

    fn classify_intent(&self, request: &AgentRequest) -> RoutingDecision {
        let normalized = request.input.to_lowercase();

        if let Some(explicit) = Self::explicit_specialist(&normalized) {
            return RoutingDecision::new(
                RouterIntent::GeneralSupport,
                0.95,
                format!("User explicitly requested {}", explicit),
                &explicit,
            );
        }

        for rule in ROUTING_RULES {
            if let Some(decision) = rule.evaluate(&normalized) {
                return decision;
            }
        }

        if let Some(semantic) = &self.semantic_router {
            if let Some(decision) = semantic.classify(&normalized) {
                return decision;
            }
        }

        RoutingDecision::general_default()
    }

    fn explicit_specialist(normalized_input: &str) -> Option<String> {
        // Allow users to target a specialist directly, e.g., "@ctoagent" or "specialist:researcher".
        const TOKENS: &[(&str, &str)] = &[
            ("@ctoagent", "CTOAgent"),
            ("specialist:cto", "CTOAgent"),
            ("@seniorengineeragent", "SeniorEngineerAgent"),
            ("specialist:seniorengineer", "SeniorEngineerAgent"),
            ("@researcheragent", "ResearcherAgent"),
            ("specialist:researcher", "ResearcherAgent"),
            ("@opscostagent", "OpsChainAgent"),
            ("specialist:opscost", "OpsChainAgent"),
            ("@opschainagent", "OpsChainAgent"),
            ("specialist:opschain", "OpsChainAgent"),
            ("@ragagent", "RagAgent"),
            ("specialist:rag", "RagAgent"),
        ];

        TOKENS
            .iter()
            .find(|(token, _)| normalized_input.contains(*token))
            .map(|(_, agent)| agent.to_string())
    }

    async fn route_to_agent(
        &self,
        decision: &RoutingDecision,
        request: AgentRequest,
    ) -> anyhow::Result<(AgentResponse, String)> {
        if let Some(agent) = self.specialists.get(decision.suggested_agent.as_str()) {
            let response = agent.handle(request).await?;
            return Ok((response, decision.suggested_agent.clone()));
        }

        let response = self.front_desk.handle(request).await?;
        Ok((response, String::from("Agent")))
    }

    async fn build_metadata(
        &self,
        request: &AgentRequest,
        response: &AgentResponse,
        decision: &RoutingDecision,
        executed_agent: &str,
        prefill_meta: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let router_meta = decision.metadata_payload(executed_agent);
        let memory_meta = self
            .capture_transcript(request, response, decision, executed_agent)
            .await;

        match (prefill_meta, memory_meta) {
            (Some(prefill), Some(memory)) => json!({
                "router": router_meta,
                "memory_prefill": prefill,
                "memory": memory,
            }),
            (Some(prefill), None) => json!({
                "router": router_meta,
                "memory_prefill": prefill,
            }),
            (None, Some(memory)) => json!({
                "router": router_meta,
                "memory": memory,
            }),
            (None, None) => json!({ "router": router_meta }),
        }
    }

    async fn capture_transcript(
        &self,
        request: &AgentRequest,
        response: &AgentResponse,
        decision: &RoutingDecision,
        executed_agent: &str,
    ) -> Option<serde_json::Value> {
        let rag_agent = self.rag_agent.as_ref()?;
        let summary = response
            .output
            .lines()
            .next()
            .unwrap_or_default()
            .to_string();
        let full_content = format!(
            "User:\n{}\n\nAgent:\n{}",
            request.input.trim(),
            response.output.trim()
        );

        let record = MemoryRecord {
            id: None,
            agent_name: executed_agent.to_string(),
            topic: format!("router.{}", decision.intent),
            project: None,
            conversation_id: None,
            timestamp: Utc::now(),
            summary,
            full_content,
            confidence: decision.confidence,
            open_questions: Vec::new(),
            perspectives: Vec::new(),
            messages: Vec::new(),
            artifacts: Vec::new(),
            tool_calls: Vec::new(),
            metadata: Some(json!({
                "rationale": decision.rationale,
                "suggested_agent": decision.suggested_agent,
                "executed_agent": executed_agent,
            })),
        };

        let request = MemoryRequest::Write(MemoryWriteRequest { record });

        match rag_agent.handle(request).await {
            Ok(MemoryResponse { notes, .. }) => Some(json!({
                "status": "stored",
                "notes": notes,
            })),
            Err(err) => {
                warn!(?err, "Failed to write transcript to RAG");
                Some(json!({
                    "status": "error",
                    "message": err.to_string(),
                }))
            }
        }
    }
}

pub struct RoutedAgentResponse {
    response: AgentResponse,
    decision: RoutingDecision,
    executed_agent: String,
}

impl RoutedAgentResponse {
    pub fn into_output(self) -> AgentResponse {
        tracing::debug!(
            intent = %self.decision.intent,
            confidence = self.decision.confidence,
            executed_agent = %self.executed_agent,
            "Returning agent output",
        );
        self.response
    }

    #[allow(dead_code)]
    pub fn decision(&self) -> &RoutingDecision {
        &self.decision
    }

    #[allow(dead_code)]
    pub fn executed_agent(&self) -> &str {
        &self.executed_agent
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouterIntent {
    GeneralSupport,
    Engineering,
    Research,
    Operations,
    Memory,
}

impl fmt::Display for RouterIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            RouterIntent::GeneralSupport => "general_support",
            RouterIntent::Engineering => "engineering",
            RouterIntent::Research => "research",
            RouterIntent::Operations => "operations",
            RouterIntent::Memory => "memory",
        };

        write!(f, "{}", label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub intent: RouterIntent,
    pub confidence: f32,
    pub rationale: String,
    pub suggested_agent: String,
}

impl RoutingDecision {
    pub(crate) fn new(
        intent: RouterIntent,
        confidence: f32,
        rationale: String,
        suggested_agent: &str,
    ) -> Self {
        Self {
            intent,
            confidence,
            rationale,
            suggested_agent: suggested_agent.to_string(),
        }
    }

    fn general_default() -> Self {
        Self {
            intent: RouterIntent::GeneralSupport,
            confidence: 0.35,
            rationale: String::from(
                "Defaulting to Agent until specialist routing rules match the request.",
            ),
            suggested_agent: "Agent".to_string(),
        }
    }

    fn metadata_payload(&self, executed_agent: &str) -> serde_json::Value {
        json!({
            "router_intent": self.intent.to_string(),
            "confidence": self.confidence,
            "rationale": self.rationale,
            "suggested_agent": self.suggested_agent,
            "executed_agent": executed_agent,
        })
    }
}

#[derive(Debug)]
struct RoutingRule {
    intent: RouterIntent,
    keywords: &'static [&'static str],
    rationale: &'static str,
    suggested_agent: &'static str,
    confidence: f32,
}

impl RoutingRule {
    const fn new(
        intent: RouterIntent,
        keywords: &'static [&'static str],
        rationale: &'static str,
        suggested_agent: &'static str,
        confidence: f32,
    ) -> Self {
        Self {
            intent,
            keywords,
            rationale,
            suggested_agent,
            confidence,
        }
    }

    fn evaluate(&self, normalized_input: &str) -> Option<RoutingDecision> {
        self.keywords
            .iter()
            .copied()
            .find(|keyword| normalized_input.contains(keyword))
            .map(|keyword| {
                let rationale = format!("{} (matched '{}')", self.rationale, keyword);
                RoutingDecision::new(
                    self.intent,
                    self.confidence,
                    rationale,
                    self.suggested_agent,
                )
            })
    }
}

const ROUTING_RULES: &[RoutingRule] = &[
    RoutingRule::new(
        RouterIntent::Engineering,
        &[
            "architecture",
            "architect",
            "system design",
            "design doc",
            "roadmap",
            "diagram",
            "blueprint",
        ],
        "Request focuses on systems architecture or roadmapping",
        "CTOAgent",
        0.8,
    ),
    RoutingRule::new(
        RouterIntent::Engineering,
        &[
            "rust",
            "code",
            "implement",
            "function",
            "struct",
            "compile",
            "bug",
            "refactor",
        ],
        "Request mentions engineering or code-level work",
        "SeniorEngineerAgent",
        0.82,
    ),
    RoutingRule::new(
        RouterIntent::Research,
        &[
            "research",
            "summarize",
            "compare",
            "investigate",
            "source",
            "paper",
            "article",
            "report",
        ],
        "Request leans toward research or synthesis",
        "ResearcherAgent",
        0.78,
    ),
    RoutingRule::new(
        RouterIntent::Operations,
        &[
            "deploy",
            "infrastructure",
            "infra",
            "cost",
            "budget",
            "capacity",
            "pipeline",
            "gpu",
            "cluster",
        ],
        "Request references operational planning or cost work",
        "OpsChainAgent",
        0.74,
    ),
    RoutingRule::new(
        RouterIntent::Memory,
        &[
            "memory",
            "remember",
            "recall",
            "save",
            "helix",
            "rag",
            "vector",
            "embedding",
            "knowledge base",
        ],
        "Request mentions memory/Helix/RAG operations",
        "RagAgent",
        0.78,
    ),
];
