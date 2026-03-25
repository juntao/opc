use crate::adapter::{AgentAdapter, AgentTaskContext};
use crate::claude_code::{ClaudeCodeAdapter, ClaudeCodeConfig};
use crate::http_adapter::{HttpAdapter, HttpAdapterConfig};
use crate::openclaw::{OpenClawAdapter, OpenClawConfig};
use anyhow::{bail, Result};
use opc_core::domain::{Agent, CreateApprovalRequest, OpcEvent};
use opc_core::events::EventBus;
use opc_core::services::agent_service;
use opc_db::queries;
use sqlx::PgPool;
use tracing::{error, info};

/// Execute a heartbeat for an agent: pick work, invoke adapter, handle result.
pub async fn execute_heartbeat(
    pool: &PgPool,
    event_bus: &EventBus,
    agent: &Agent,
    trigger: &str,
    api_base_url: &str,
) -> Result<()> {
    // Check budget
    let budget_status =
        agent_service::check_budget(agent.current_month_spent_cents, agent.monthly_budget_cents);
    agent_service::validate_agent_invocable(&agent.status, &budget_status)?;

    // Update agent status to running
    queries::agents::update_agent_status(pool, agent.id, "running").await?;

    // Get assignments
    let assignments = queries::issues::get_agent_assignments(pool, agent.id).await?;

    if assignments.is_empty() {
        info!("Agent {} has no pending assignments", agent.name);
        queries::agents::update_agent_status(pool, agent.id, "idle").await?;
        return Ok(());
    }

    // Pick first available issue
    let issue = &assignments[0];

    // Create heartbeat run record
    let run =
        queries::heartbeats::create_heartbeat_run(pool, agent.id, Some(issue.id), trigger).await?;

    // Atomic checkout
    let checked_out = queries::issues::checkout_issue(pool, issue.id, agent.id).await?;
    if checked_out.is_none() {
        info!(
            "Agent {} could not checkout issue {} (already checked out)",
            agent.name, issue.id
        );
        queries::heartbeats::complete_heartbeat_run(
            pool,
            run.id,
            "cancelled",
            Some("Issue already checked out"),
        )
        .await?;
        queries::agents::update_agent_status(pool, agent.id, "idle").await?;
        return Ok(());
    }

    let issue = checked_out.unwrap();

    // Get context
    let comments = queries::comments::list_comments(pool, issue.id).await?;
    let parent_chain = queries::issues::get_parent_chain(pool, issue.id).await?;

    // Build adapter
    let adapter: Box<dyn AgentAdapter> = create_adapter(agent)?;

    let context = AgentTaskContext {
        agent: agent.clone(),
        issue: issue.clone(),
        comments,
        parent_chain,
        trigger: trigger.to_string(),
        api_base_url: api_base_url.to_string(),
        api_key: String::new(), // Agent uses its own key externally
    };

    // Invoke
    info!("Invoking agent {} on issue {}", agent.name, issue.title);
    let result = adapter.invoke(context).await;

    match result {
        Ok(response) => {
            // Post summary as comment
            let comment_input = opc_core::domain::CreateComment {
                issue_id: issue.id,
                author_type: "agent".to_string(),
                author_id: agent.id.to_string(),
                author_name: agent.name.clone(),
                body: response.summary.clone(),
            };
            let _ = queries::comments::create_comment(pool, &comment_input).await;

            // Handle cost reporting
            if let Some(cost) = &response.cost {
                let cost_input = opc_core::services::cost_service::CreateCostEvent {
                    company_id: agent.company_id,
                    agent_id: agent.id,
                    issue_id: Some(issue.id),
                    project_id: issue.project_id,
                    heartbeat_run_id: Some(run.id),
                    model: cost.model.clone(),
                    input_tokens: cost.input_tokens,
                    output_tokens: cost.output_tokens,
                    cost_cents: cost.cost_cents,
                };
                let _ = queries::cost_events::create_cost_event(pool, &cost_input).await;
                let _ = queries::agents::increment_agent_spending(pool, agent.id, cost.cost_cents)
                    .await;
            }

            if matches!(
                response.status,
                crate::adapter::AgentResponseStatus::Dispatched
            ) {
                // Async agent (e.g. OpenClaw) will call back to submit results.
                // Leave the issue checked out — the agent's callback will submit.
                queries::heartbeats::complete_heartbeat_run(pool, run.id, "completed", None)
                    .await?;
                info!(
                    "Agent {} dispatched task '{}' to async agent, awaiting callback",
                    agent.name, issue.title
                );
            } else {
                // Synchronous agent — auto-submit for approval
                let _ = queries::issues::submit_issue(pool, issue.id, agent.id).await;

                let artifacts_json = serde_json::to_value(&response.artifacts).unwrap_or_default();
                let approval_input = CreateApprovalRequest {
                    issue_id: issue.id,
                    company_id: agent.company_id,
                    agent_id: agent.id,
                    summary: response.summary,
                    artifacts: Some(artifacts_json),
                };
                let approval = queries::approvals::create_approval(pool, &approval_input).await?;

                event_bus.publish(OpcEvent::ApprovalRequested {
                    approval_id: approval.id,
                    issue_id: issue.id,
                    agent_id: agent.id,
                    company_id: agent.company_id,
                });

                queries::heartbeats::complete_heartbeat_run(pool, run.id, "completed", None)
                    .await?;
                info!(
                    "Agent {} completed work on issue {}, awaiting approval",
                    agent.name, issue.title
                );
            }
        }
        Err(e) => {
            error!(
                "Agent {} failed on issue {}: {}",
                agent.name, issue.title, e
            );

            // Release checkout
            let _ = queries::issues::checkin_issue(pool, issue.id, agent.id).await;

            queries::heartbeats::complete_heartbeat_run(
                pool,
                run.id,
                "failed",
                Some(&e.to_string()),
            )
            .await?;

            queries::agents::update_agent_status(pool, agent.id, "error").await?;
            return Err(e);
        }
    }

    queries::agents::update_agent_status(pool, agent.id, "idle").await?;
    Ok(())
}

/// Create the appropriate adapter based on agent config.
fn create_adapter(agent: &Agent) -> Result<Box<dyn AgentAdapter>> {
    match agent.adapter_type.as_str() {
        "http" => {
            let config: HttpAdapterConfig = serde_json::from_value(agent.adapter_config.clone())?;
            Ok(Box::new(HttpAdapter::new(config)))
        }
        "claude_code" => {
            let config: ClaudeCodeConfig = serde_json::from_value(agent.adapter_config.clone())?;
            Ok(Box::new(ClaudeCodeAdapter::new(config)))
        }
        "openclaw" => {
            let config: OpenClawConfig = serde_json::from_value(agent.adapter_config.clone())?;
            Ok(Box::new(OpenClawAdapter::new(config)))
        }
        other => bail!("Unsupported adapter type: {}", other),
    }
}
