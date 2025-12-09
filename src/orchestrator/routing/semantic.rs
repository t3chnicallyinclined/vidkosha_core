use std::collections::HashSet;
use std::env;

use crate::orchestrator::router::{RouterIntent, RoutingDecision};

/// Lightweight semantic router scaffold using token overlap scoring.
/// This is a placeholder until we wire real embeddings; kept flag-gated.
pub struct SemanticRouter {
    prototypes: Vec<SemanticPrototype>,
    threshold: f32,
    enabled: bool,
}

#[derive(Clone, Debug)]
pub struct SemanticPrototype {
    pub agent_name: String,
    pub intent: RouterIntent,
    pub text: String,
}

impl SemanticRouter {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let enabled = env::var("ROUTING_SEMANTIC_ENABLED")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if !enabled {
            return Ok(None);
        }

        let threshold = env::var("ROUTING_SEMANTIC_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.35);

        let prototypes = default_prototypes();
        if prototypes.is_empty() {
            return Ok(None);
        }

        Ok(Some(Self {
            prototypes,
            threshold,
            enabled,
        }))
    }

    pub fn classify(&self, input: &str) -> Option<RoutingDecision> {
        if !self.enabled {
            return None;
        }

        let input_tokens = tokenize(input);
        if input_tokens.is_empty() {
            return None;
        }

        let mut best: Option<(f32, &SemanticPrototype)> = None;

        for proto in &self.prototypes {
            let score = overlap_score(&input_tokens, &tokenize(&proto.text));
            match best {
                Some((best_score, _)) if score <= best_score => {}
                _ => best = Some((score, proto)),
            }
        }

        let (score, proto) = best?;
        if score < self.threshold {
            return None;
        }

        Some(RoutingDecision::new(
            proto.intent,
            score,
            format!(
                "Semantic match for {} (score={:.2})",
                proto.agent_name, score
            ),
            &proto.agent_name,
        ))
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

fn overlap_score(input: &[String], proto: &[String]) -> f32 {
    if input.is_empty() || proto.is_empty() {
        return 0.0;
    }

    let input_set: HashSet<&String> = input.iter().collect();
    let proto_set: HashSet<&String> = proto.iter().collect();
    let intersection = input_set.intersection(&proto_set).count() as f32;
    let denom = ((input_set.len() * proto_set.len()) as f32).sqrt();
    if denom == 0.0 {
        0.0
    } else {
        intersection / denom
    }
}

fn default_prototypes() -> Vec<SemanticPrototype> {
    vec![
        SemanticPrototype {
            agent_name: "CTOAgent".to_string(),
            intent: RouterIntent::Engineering,
            text: "architecture system design roadmap blueprint diagram mvp".to_string(),
        },
        SemanticPrototype {
            agent_name: "SeniorEngineerAgent".to_string(),
            intent: RouterIntent::Engineering,
            text: "rust code implement bug compile refactor function struct tests".to_string(),
        },
        SemanticPrototype {
            agent_name: "ResearcherAgent".to_string(),
            intent: RouterIntent::Research,
            text: "research summarize compare investigate source paper article report".to_string(),
        },
        SemanticPrototype {
            agent_name: "OpsChainAgent".to_string(),
            intent: RouterIntent::Operations,
            text: "deploy infrastructure infra cost gpu cluster budget scaling reliability"
                .to_string(),
        },
        SemanticPrototype {
            agent_name: "RagAgent".to_string(),
            intent: RouterIntent::Memory,
            text: "rag memory store recall context vector embeddings helix".to_string(),
        },
    ]
}
