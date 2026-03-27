use crate::error::AppError;
use crate::state::AppState;
use askama::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use opc_core::domain::{
    Agent, ApprovalRequest, BoardUser, Issue, IssueComment, Project, ProjectUpdate,
};
use opc_db::queries;
use uuid::Uuid;

// --- Templates ---

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub user: BoardUser,
    pub agent_count: i64,
    pub active_issues: i64,
    pub pending_approvals: i64,
    pub recent_activity: Vec<opc_core::domain::ActivityLogEntry>,
}

#[derive(Template)]
#[template(path = "agents/list.html")]
pub struct AgentListTemplate {
    pub user: BoardUser,
    pub agents: Vec<Agent>,
}

#[derive(Template)]
#[template(path = "agents/detail.html")]
pub struct AgentDetailTemplate {
    pub user: BoardUser,
    pub agent: Agent,
    pub issues: Vec<Issue>,
    pub heartbeats: Vec<opc_core::domain::HeartbeatRun>,
}

#[derive(Template)]
#[template(path = "agents/new.html")]
pub struct AgentNewTemplate {
    pub user: BoardUser,
}

#[derive(Template)]
#[template(path = "issues/list.html")]
pub struct IssueListTemplate {
    pub user: BoardUser,
    pub issues: Vec<Issue>,
    pub agents: Vec<Agent>,
    pub filter_status: String,
}

#[derive(Template)]
#[template(path = "issues/detail.html")]
pub struct IssueDetailTemplate {
    pub user: BoardUser,
    pub issue: Issue,
    pub comments: Vec<IssueComment>,
    pub blocked_by: Vec<Issue>,
    pub blocks: Vec<Issue>,
    pub agents: Vec<Agent>,
    pub assignee: Option<Agent>,
    pub approval: Option<ApprovalRequest>,
}

#[derive(Template)]
#[template(path = "issues/new.html")]
pub struct IssueNewTemplate {
    pub user: BoardUser,
    pub agents: Vec<Agent>,
    pub projects: Vec<Project>,
}

#[derive(Template)]
#[template(path = "approvals/list.html")]
pub struct ApprovalListTemplate {
    pub user: BoardUser,
    pub approvals: Vec<ApprovalWithContext>,
}

pub struct ApprovalWithContext {
    pub approval: ApprovalRequest,
    pub issue: Option<Issue>,
    pub agent: Option<Agent>,
}

#[derive(Template)]
#[template(path = "approvals/detail.html")]
pub struct ApprovalDetailTemplate {
    pub user: BoardUser,
    pub approval: ApprovalRequest,
    pub issue: Issue,
    pub agent: Agent,
    pub comments: Vec<IssueComment>,
}

#[derive(Template)]
#[template(path = "projects/list.html")]
pub struct ProjectListTemplate {
    pub user: BoardUser,
    pub projects: Vec<Project>,
}

#[derive(Template)]
#[template(path = "projects/detail.html")]
pub struct ProjectDetailTemplate {
    pub user: BoardUser,
    pub project: Project,
    pub issues: Vec<Issue>,
    pub agents: Vec<Agent>,
    pub updates: Vec<ProjectUpdate>,
}

// --- Page Handlers ---

pub async fn login_page() -> Html<String> {
    let template = LoginTemplate { error: None };
    Html(template.render().unwrap_or_default())
}

pub async fn dashboard(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
) -> Result<Html<String>, AppError> {
    let agents = queries::agents::list_agents(&state.pool, state.company_id).await?;
    let issues =
        queries::issues::list_issues(&state.pool, state.company_id, None, None, None).await?;
    let pending = queries::approvals::count_pending(&state.pool, state.company_id).await?;
    let activity = queries::activity_log::list_activity(&state.pool, state.company_id, 20).await?;

    let active_count = issues
        .iter()
        .filter(|i| {
            matches!(
                i.status.as_str(),
                "todo" | "in_progress" | "awaiting_approval" | "changes_requested"
            )
        })
        .count() as i64;

    let template = DashboardTemplate {
        user: user.0,
        agent_count: agents.len() as i64,
        active_issues: active_count,
        pending_approvals: pending,
        recent_activity: activity,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn agents_page(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
) -> Result<Html<String>, AppError> {
    let agents = queries::agents::list_agents(&state.pool, state.company_id).await?;
    let template = AgentListTemplate {
        user: user.0,
        agents,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn agent_detail_page(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, AppError> {
    let agent = queries::agents::get_agent(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))?;
    let issues = queries::issues::get_agent_assignments(&state.pool, id).await?;
    let heartbeats = queries::heartbeats::list_heartbeat_runs(&state.pool, id, 20).await?;

    let template = AgentDetailTemplate {
        user: user.0,
        agent,
        issues,
        heartbeats,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn agent_new_page(user: axum::Extension<BoardUser>) -> Html<String> {
    let template = AgentNewTemplate { user: user.0 };
    Html(template.render().unwrap_or_default())
}

pub async fn issues_page(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
    axum::extract::Query(params): axum::extract::Query<super::issues::ListIssuesQuery>,
) -> Result<Html<String>, AppError> {
    let issues = queries::issues::list_issues(
        &state.pool,
        state.company_id,
        params.status.as_deref(),
        params.assignee_id,
        params.project_id,
    )
    .await?;
    let agents = queries::agents::list_agents(&state.pool, state.company_id).await?;

    let template = IssueListTemplate {
        user: user.0,
        issues,
        agents,
        filter_status: params.status.unwrap_or_default(),
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn issue_detail_page(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, AppError> {
    let issue = queries::issues::get_issue(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Issue not found"))?;
    let comments = queries::comments::list_comments(&state.pool, id).await?;
    let blocked_by = queries::issues::get_dependencies(&state.pool, id).await?;
    let blocks = queries::issues::get_dependents(&state.pool, id).await?;
    let agents = queries::agents::list_agents(&state.pool, state.company_id).await?;

    let assignee = if let Some(aid) = issue.assignee_id {
        queries::agents::get_agent(&state.pool, aid).await?
    } else {
        None
    };

    // Get latest pending approval for this issue
    let approvals =
        queries::approvals::list_pending_approvals(&state.pool, state.company_id).await?;
    let approval = approvals.into_iter().find(|a| a.issue_id == id);

    let template = IssueDetailTemplate {
        user: user.0,
        issue,
        comments,
        blocked_by,
        blocks,
        agents,
        assignee,
        approval,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn issue_new_page(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
) -> Result<Html<String>, AppError> {
    let agents = queries::agents::list_agents(&state.pool, state.company_id).await?;
    let projects = queries::projects::list_projects(&state.pool, state.company_id).await?;

    let template = IssueNewTemplate {
        user: user.0,
        agents,
        projects,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn approvals_page(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
) -> Result<Html<String>, AppError> {
    let approvals =
        queries::approvals::list_pending_approvals(&state.pool, state.company_id).await?;

    let mut approvals_with_context = Vec::new();
    for approval in approvals {
        let issue = queries::issues::get_issue(&state.pool, approval.issue_id).await?;
        let agent = queries::agents::get_agent(&state.pool, approval.agent_id).await?;
        approvals_with_context.push(ApprovalWithContext {
            approval,
            issue,
            agent,
        });
    }

    let template = ApprovalListTemplate {
        user: user.0,
        approvals: approvals_with_context,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn approval_detail_page(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, AppError> {
    let approval = queries::approvals::get_approval(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Approval not found"))?;
    let issue = queries::issues::get_issue(&state.pool, approval.issue_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Issue not found"))?;
    let agent = queries::agents::get_agent(&state.pool, approval.agent_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))?;
    let comments = queries::comments::list_comments(&state.pool, approval.issue_id).await?;

    let template = ApprovalDetailTemplate {
        user: user.0,
        approval,
        issue,
        agent,
        comments,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn project_detail_page(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, AppError> {
    let project = queries::projects::get_project(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Project not found"))?;
    let issues =
        queries::issues::list_issues(&state.pool, state.company_id, None, None, Some(id)).await?;
    let agents = queries::agents::list_agents(&state.pool, state.company_id).await?;
    let updates = queries::project_updates::list_project_updates(&state.pool, id, 50).await?;

    let template = ProjectDetailTemplate {
        user: user.0,
        project,
        issues,
        agents,
        updates,
    };
    Ok(Html(template.render().unwrap_or_default()))
}

pub async fn projects_page(
    State(state): State<AppState>,
    user: axum::Extension<BoardUser>,
) -> Result<Html<String>, AppError> {
    let projects = queries::projects::list_projects(&state.pool, state.company_id).await?;
    let template = ProjectListTemplate {
        user: user.0,
        projects,
    };
    Ok(Html(template.render().unwrap_or_default()))
}
