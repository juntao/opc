use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;
use opc_core::domain::ApprovalRequest;
use opc_core::services::approval_service;
use opc_db::queries;
use serde::Deserialize;
use uuid::Uuid;

pub async fn api_list_pending(
    State(state): State<AppState>,
) -> Result<Json<Vec<ApprovalRequest>>, AppError> {
    let approvals =
        queries::approvals::list_pending_approvals(&state.pool, state.company_id).await?;
    Ok(Json(approvals))
}

pub async fn api_get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let approval = queries::approvals::get_approval(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Approval not found"))?;

    let issue = queries::issues::get_issue(&state.pool, approval.issue_id).await?;
    let comments = queries::comments::list_comments(&state.pool, approval.issue_id).await?;
    let agent = queries::agents::get_agent(&state.pool, approval.agent_id).await?;

    Ok(Json(serde_json::json!({
        "approval": approval,
        "issue": issue,
        "comments": comments,
        "agent": agent,
    })))
}

#[derive(Deserialize)]
pub struct ResolveInput {
    pub comment: Option<String>,
}

pub async fn api_approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    user: axum::Extension<opc_core::domain::BoardUser>,
    Json(input): Json<ResolveInput>,
) -> Result<Json<ApprovalRequest>, AppError> {
    let approval = queries::approvals::resolve_approval(
        &state.pool,
        id,
        "approved",
        &user.username,
        input.comment.as_deref(),
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("Approval not found or already resolved"))?;

    // Move issue to approved
    queries::issues::approve_issue(&state.pool, approval.issue_id, &user.username).await?;

    // Post approval comment if provided
    if let Some(comment) = &input.comment {
        let create = opc_core::domain::CreateComment {
            issue_id: approval.issue_id,
            author_type: "human".to_string(),
            author_id: user.id.to_string(),
            author_name: user.username.clone(),
            body: format!("**Approved**: {}", comment),
        };
        queries::comments::create_comment(&state.pool, &create).await?;
    }

    // Emit approval event (triggers downstream agents)
    approval_service::emit_approval_resolved(
        &state.event_bus,
        approval.id,
        approval.issue_id,
        state.company_id,
        "approved",
    );

    Ok(Json(approval))
}

pub async fn api_request_changes(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    user: axum::Extension<opc_core::domain::BoardUser>,
    Json(input): Json<ResolveInput>,
) -> Result<Json<ApprovalRequest>, AppError> {
    let comment_text = input
        .comment
        .clone()
        .unwrap_or_else(|| "Changes requested".to_string());

    let approval = queries::approvals::resolve_approval(
        &state.pool,
        id,
        "changes_requested",
        &user.username,
        Some(&comment_text),
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("Approval not found or already resolved"))?;

    // Move issue to changes_requested
    queries::issues::update_issue_status(&state.pool, approval.issue_id, "changes_requested")
        .await?;

    // Post feedback as comment
    let create = opc_core::domain::CreateComment {
        issue_id: approval.issue_id,
        author_type: "human".to_string(),
        author_id: user.id.to_string(),
        author_name: user.username.clone(),
        body: format!("**Changes Requested**: {}", comment_text),
    };
    queries::comments::create_comment(&state.pool, &create).await?;

    // Emit event (agent will re-wake)
    approval_service::emit_approval_resolved(
        &state.event_bus,
        approval.id,
        approval.issue_id,
        state.company_id,
        "changes_requested",
    );

    Ok(Json(approval))
}

pub async fn api_reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    user: axum::Extension<opc_core::domain::BoardUser>,
    Json(input): Json<ResolveInput>,
) -> Result<Json<ApprovalRequest>, AppError> {
    let approval = queries::approvals::resolve_approval(
        &state.pool,
        id,
        "rejected",
        &user.username,
        input.comment.as_deref(),
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("Approval not found or already resolved"))?;

    // Cancel the issue
    queries::issues::update_issue_status(&state.pool, approval.issue_id, "cancelled").await?;

    approval_service::emit_approval_resolved(
        &state.event_bus,
        approval.id,
        approval.issue_id,
        state.company_id,
        "rejected",
    );

    Ok(Json(approval))
}
