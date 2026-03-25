use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Active,
    Idle,
    Running,
    Error,
    Paused,
    Terminated,
}

impl AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Error => "error",
            Self::Paused => "paused",
            Self::Terminated => "terminated",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "idle" => Some(Self::Idle),
            "running" => Some(Self::Running),
            "error" => Some(Self::Error),
            "paused" => Some(Self::Paused),
            "terminated" => Some(Self::Terminated),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdapterType {
    Http,
    ClaudeCode,
    Process,
    OpenClaw,
}

impl AdapterType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::ClaudeCode => "claude_code",
            Self::Process => "process",
            Self::OpenClaw => "openclaw",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "http" => Some(Self::Http),
            "claude_code" => Some(Self::ClaudeCode),
            "process" => Some(Self::Process),
            "openclaw" => Some(Self::OpenClaw),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Agent {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub title: Option<String>,
    pub role: Option<String>,
    pub capabilities: Option<String>,
    pub adapter_type: String,
    pub adapter_config: serde_json::Value,
    pub monthly_budget_cents: i64,
    pub current_month_spent_cents: i64,
    pub status: String,
    pub manager_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgent {
    pub company_id: Uuid,
    pub name: String,
    pub title: Option<String>,
    pub role: Option<String>,
    pub capabilities: Option<String>,
    pub adapter_type: String,
    pub adapter_config: serde_json::Value,
    pub monthly_budget_cents: Option<i64>,
    pub manager_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgent {
    pub name: Option<String>,
    pub title: Option<String>,
    pub role: Option<String>,
    pub capabilities: Option<String>,
    pub adapter_type: Option<String>,
    pub adapter_config: Option<serde_json::Value>,
    pub monthly_budget_cents: Option<i64>,
    pub manager_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentApiKey {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub company_id: Uuid,
    pub key_hash: String,
    pub key_prefix: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Company {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub mission: Option<String>,
    pub monthly_budget_cents: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BoardUser {
    pub id: Uuid,
    pub company_id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}
