use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Report from an agent about token usage during a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_cents: i64,
}

/// Create a cost event record input.
#[derive(Debug, Clone)]
pub struct CreateCostEvent {
    pub company_id: Uuid,
    pub agent_id: Uuid,
    pub issue_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub heartbeat_run_id: Option<Uuid>,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_cents: i64,
}
