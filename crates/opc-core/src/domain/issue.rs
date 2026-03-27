use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Backlog,
    Todo,
    InProgress,
    AwaitingApproval,
    Approved,
    ChangesRequested,
    InReview,
    Done,
    Blocked,
    Cancelled,
}

impl IssueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Approved => "approved",
            Self::ChangesRequested => "changes_requested",
            Self::InReview => "in_review",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "backlog" => Some(Self::Backlog),
            "todo" => Some(Self::Todo),
            "in_progress" => Some(Self::InProgress),
            "awaiting_approval" => Some(Self::AwaitingApproval),
            "approved" => Some(Self::Approved),
            "changes_requested" => Some(Self::ChangesRequested),
            "in_review" => Some(Self::InReview),
            "done" => Some(Self::Done),
            "blocked" => Some(Self::Blocked),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Returns true if an agent is allowed to pick up issues in this status.
    pub fn is_agent_pickable(&self) -> bool {
        matches!(self, Self::Todo | Self::Approved | Self::ChangesRequested)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IssuePriority {
    Urgent,
    High,
    Medium,
    Low,
}

impl IssuePriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Urgent => "urgent",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "urgent" => Some(Self::Urgent),
            "high" => Some(Self::High),
            "medium" => Some(Self::Medium),
            "low" => Some(Self::Low),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Issue {
    pub id: Uuid,
    pub company_id: Uuid,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub assignee_id: Option<Uuid>,
    pub checked_out_by: Option<Uuid>,
    pub checked_out_at: Option<DateTime<Utc>>,
    pub approved_by: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIssue {
    pub company_id: Uuid,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub assignee_id: Option<Uuid>,
    #[serde(default)]
    pub blocked_by: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIssue {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IssueComment {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub author_type: String,
    pub author_id: String,
    pub author_name: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateComment {
    pub issue_id: Uuid,
    pub author_type: String,
    pub author_id: String,
    pub author_name: String,
    pub body: String,
}

/// A completed prerequisite issue with its comments, for agent context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedDependency {
    pub issue: Issue,
    pub comments: Vec<IssueComment>,
}
