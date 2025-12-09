pub mod agent;
pub mod specialists;
pub mod traits;

pub use agent::Agent;
pub use specialists::{CTOAgent, OpsChainAgent, ResearcherAgent, SeniorEngineerAgent};
pub use traits::{AgentBehavior, AgentRequest, AgentResponse};
