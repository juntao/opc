use crate::state::AppState;
use opc_core::domain::OpcEvent;
use opc_db::queries;
use tracing::info;

/// Event listener that triggers agent heartbeats based on system events.
pub async fn start_event_listener(state: AppState) {
    let mut rx = state.event_bus.subscribe();

    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, event).await {
                    tracing::error!("Error handling event: {}", e);
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("Event listener lagged by {} events", n);
            }
            Err(_) => break,
        }
    }
}

async fn handle_event(state: &AppState, event: OpcEvent) -> anyhow::Result<()> {
    match event {
        OpcEvent::IssueAssigned {
            issue_id, agent_id, ..
        } => {
            info!(
                "Issue {} assigned to agent {}, triggering heartbeat",
                issue_id, agent_id
            );
            trigger_agent_heartbeat(state, agent_id, "assignment").await?;
        }
        OpcEvent::ApprovalResolved {
            issue_id, status, ..
        } => {
            match status.as_str() {
                "approved" => {
                    // Check if there are child issues with assigned agents
                    let children = queries::issues::get_children(&state.pool, issue_id).await?;
                    for child in children {
                        if let Some(assignee_id) = child.assignee_id {
                            info!(
                                "Approval granted, triggering downstream agent {}",
                                assignee_id
                            );
                            trigger_agent_heartbeat(state, assignee_id, "approval").await?;
                        }
                    }
                    // Also mark the issue as done if it was the final step
                    let issue = queries::issues::get_issue(&state.pool, issue_id).await?;
                    if let Some(_issue) = issue {
                        let children = queries::issues::get_children(&state.pool, issue_id).await?;
                        if children.is_empty() {
                            // Leaf issue, mark done
                            queries::issues::update_issue_status(&state.pool, issue_id, "done")
                                .await?;
                        }
                    }
                }
                "changes_requested" => {
                    // Re-trigger the agent that worked on this issue
                    let issue = queries::issues::get_issue(&state.pool, issue_id).await?;
                    if let Some(issue) = issue {
                        if let Some(assignee_id) = issue.assignee_id {
                            info!("Changes requested, re-triggering agent {}", assignee_id);
                            trigger_agent_heartbeat(state, assignee_id, "changes_requested")
                                .await?;
                        }
                    }
                }
                _ => {}
            }
        }
        OpcEvent::ProjectApproved { project_id, .. } => {
            info!(
                "Project {} approved, activating root-level issues",
                project_id
            );
            let root_issues =
                queries::projects::get_root_issues_for_activation(&state.pool, project_id).await?;
            for issue in root_issues {
                queries::issues::update_issue_status(&state.pool, issue.id, "todo").await?;
                if let Some(assignee_id) = issue.assignee_id {
                    info!("Triggering agent {} for issue {}", assignee_id, issue.id);
                    trigger_agent_heartbeat(state, assignee_id, "assignment").await?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

async fn trigger_agent_heartbeat(
    state: &AppState,
    agent_id: uuid::Uuid,
    trigger: &str,
) -> anyhow::Result<()> {
    let agent = queries::agents::get_agent(&state.pool, agent_id).await?;
    if let Some(agent) = agent {
        let pool = state.pool.clone();
        let event_bus = state.event_bus.clone();
        let api_base_url = state.api_base_url.clone();
        let trigger = trigger.to_string();
        tokio::spawn(async move {
            if let Err(e) = opc_agents::heartbeat::execute_heartbeat(
                &pool,
                &event_bus,
                &agent,
                &trigger,
                &api_base_url,
            )
            .await
            {
                tracing::error!("Heartbeat failed for agent {}: {}", agent.name, e);
            }
        });
    }
    Ok(())
}
