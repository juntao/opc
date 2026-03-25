use anyhow::Result;
use async_trait::async_trait;
use opc_core::domain::{Agent, Issue, IssueComment};
use serde::{Deserialize, Serialize};

/// Context provided to an agent when invoked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskContext {
    pub agent: Agent,
    pub issue: Issue,
    pub comments: Vec<IssueComment>,
    pub parent_chain: Vec<Issue>,
    pub trigger: String,
    pub api_base_url: String,
    pub api_key: String,
}

/// Response from an agent after execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub status: AgentResponseStatus,
    pub summary: String,
    pub artifacts: Vec<Artifact>,
    pub cost: Option<CostReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentResponseStatus {
    Completed,
    Failed,
    NeedsApproval,
    /// Task was dispatched to an async agent (e.g. OpenClaw) that will call back
    /// to OPC's agent API to submit results. The heartbeat should NOT auto-submit.
    Dispatched,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub artifact_type: String,
    pub url: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentRunStatus {
    Running,
    Idle,
    Error(String),
}

/// Trait that all agent adapters must implement.
#[async_trait]
pub trait AgentAdapter: Send + Sync {
    /// Invoke the agent with a task context.
    async fn invoke(&self, context: AgentTaskContext) -> Result<AgentResponse>;

    /// Check if the agent is currently running.
    async fn status(&self) -> Result<AgentRunStatus>;

    /// Cancel a running agent invocation.
    async fn cancel(&self) -> Result<()>;
}
