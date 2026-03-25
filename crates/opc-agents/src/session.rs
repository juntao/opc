use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tracks an agent's working session on a specific task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskSession {
    pub agent_id: Uuid,
    pub issue_id: Uuid,
    pub working_dir: Option<String>,
    pub iteration: u32,
}
