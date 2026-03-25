use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// System-wide events broadcast via tokio channels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpcEvent {
    IssueCreated {
        issue_id: Uuid,
        company_id: Uuid,
    },
    IssueAssigned {
        issue_id: Uuid,
        agent_id: Uuid,
        company_id: Uuid,
    },
    IssueStatusChanged {
        issue_id: Uuid,
        company_id: Uuid,
        old_status: String,
        new_status: String,
    },
    ApprovalRequested {
        approval_id: Uuid,
        issue_id: Uuid,
        agent_id: Uuid,
        company_id: Uuid,
    },
    ApprovalResolved {
        approval_id: Uuid,
        issue_id: Uuid,
        company_id: Uuid,
        status: String,
    },
    AgentMentioned {
        agent_id: Uuid,
        issue_id: Uuid,
        comment_id: Uuid,
        company_id: Uuid,
    },
    CommentAdded {
        issue_id: Uuid,
        comment_id: Uuid,
        company_id: Uuid,
    },
    HeartbeatCompleted {
        run_id: Uuid,
        agent_id: Uuid,
        company_id: Uuid,
    },
    CostEvent {
        agent_id: Uuid,
        company_id: Uuid,
        cost_cents: i64,
    },
    ProjectApproved {
        project_id: Uuid,
        company_id: Uuid,
    },
}

/// Triggers that can start an agent heartbeat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HeartbeatTrigger {
    Schedule,
    Assignment,
    Mention,
    Manual,
    Approval,
    ChangesRequested,
}

impl HeartbeatTrigger {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Schedule => "schedule",
            Self::Assignment => "assignment",
            Self::Mention => "mention",
            Self::Manual => "manual",
            Self::Approval => "approval",
            Self::ChangesRequested => "changes_requested",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "schedule" => Some(Self::Schedule),
            "assignment" => Some(Self::Assignment),
            "mention" => Some(Self::Mention),
            "manual" => Some(Self::Manual),
            "approval" => Some(Self::Approval),
            "changes_requested" => Some(Self::ChangesRequested),
            _ => None,
        }
    }
}

/// Heartbeat execution record.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HeartbeatRun {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub issue_id: Option<Uuid>,
    pub trigger_type: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

/// Cost event for tracking token usage.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CostEvent {
    pub id: Uuid,
    pub company_id: Uuid,
    pub agent_id: Uuid,
    pub issue_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub heartbeat_run_id: Option<Uuid>,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_cents: i64,
    pub created_at: DateTime<Utc>,
}

/// Activity log entry for audit trail.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ActivityLogEntry {
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_type: String,
    pub actor_id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
