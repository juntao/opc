use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use opc_core::domain::{CreateIssue, Issue, OpcEvent, UpdateIssue};
use opc_core::services::issue_service;
use opc_db::queries;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ListIssuesQuery {
    pub status: Option<String>,
    pub assignee_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
}

pub async fn api_list(
    State(state): State<AppState>,
    Query(params): Query<ListIssuesQuery>,
) -> Result<Json<Vec<Issue>>, AppError> {
    let issues = queries::issues::list_issues(
        &state.pool,
        state.company_id,
        params.status.as_deref(),
        params.assignee_id,
        params.project_id,
    )
    .await?;
    Ok(Json(issues))
}

pub async fn api_get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let issue = queries::issues::get_issue(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Issue not found"))?;
    let comments = queries::comments::list_comments(&state.pool, id).await?;
    let children = queries::issues::get_children(&state.pool, id).await?;

    Ok(Json(serde_json::json!({
        "issue": issue,
        "comments": comments,
        "children": children,
    })))
}

pub async fn api_create(
    State(state): State<AppState>,
    Json(mut input): Json<CreateIssue>,
) -> Result<Json<Issue>, AppError> {
    input.company_id = state.company_id;
    let issue = queries::issues::create_issue(&state.pool, &input).await?;

    state.event_bus.publish(OpcEvent::IssueCreated {
        issue_id: issue.id,
        company_id: state.company_id,
    });

    if let Some(assignee_id) = issue.assignee_id {
        state.event_bus.publish(OpcEvent::IssueAssigned {
            issue_id: issue.id,
            agent_id: assignee_id,
            company_id: state.company_id,
        });
    }

    Ok(Json(issue))
}

pub async fn api_update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateIssue>,
) -> Result<Json<Option<Issue>>, AppError> {
    // Validate status transition if status is being changed
    if let Some(new_status) = &input.status {
        let current = queries::issues::get_issue(&state.pool, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Issue not found"))?;
        issue_service::validate_status_transition(&current.status, new_status)?;
    }

    let issue = queries::issues::update_issue(&state.pool, id, &input).await?;
    Ok(Json(issue))
}

#[derive(Deserialize)]
pub struct AssignInput {
    pub assignee_id: Uuid,
}

pub async fn api_assign(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<AssignInput>,
) -> Result<Json<Option<Issue>>, AppError> {
    let update = UpdateIssue {
        title: None,
        description: None,
        status: Some("todo".to_string()),
        priority: None,
        assignee_id: Some(input.assignee_id),
        project_id: None,
    };
    let issue = queries::issues::update_issue(&state.pool, id, &update).await?;

    state.event_bus.publish(OpcEvent::IssueAssigned {
        issue_id: id,
        agent_id: input.assignee_id,
        company_id: state.company_id,
    });

    Ok(Json(issue))
}
