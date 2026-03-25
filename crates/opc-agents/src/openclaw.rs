use crate::adapter::{
    AgentAdapter, AgentResponse, AgentResponseStatus, AgentRunStatus, AgentTaskContext,
};
use anyhow::{bail, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for the OpenClaw adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawConfig {
    /// OpenClaw webhook URL (e.g. "http://127.0.0.1:18789/hooks/agent").
    pub webhook_url: String,
    /// Bearer token for OpenClaw authentication.
    pub token: String,
    /// OPC API key for the agent, so OpenClaw can call back to submit results.
    /// Generate one via POST /api/agents/{id}/keys and paste the opc_... key here.
    pub opc_api_key: String,
    /// Timeout in seconds (maps to OpenClaw's timeoutSeconds).
    pub timeout_secs: Option<u64>,
    /// Whether OpenClaw should also deliver the result to a messaging channel.
    pub deliver: Option<bool>,
    /// Target messaging channel (e.g. "slack", "telegram").
    pub channel: Option<String>,
    /// Recipient identifier (e.g. "#general", phone number).
    pub to: Option<String>,
    /// Model override (e.g. "anthropic/claude-sonnet-4-6").
    pub model: Option<String>,
}

/// Payload sent to OpenClaw's /hooks/agent endpoint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenClawPayload {
    message: String,
    name: String,
    deliver: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_seconds: Option<u64>,
}

pub struct OpenClawAdapter {
    config: OpenClawConfig,
    client: Client,
    running: Arc<Mutex<bool>>,
}

impl OpenClawAdapter {
    pub fn new(config: OpenClawConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(
                config.timeout_secs.unwrap_or(300),
            ))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            client,
            running: Arc::new(Mutex::new(false)),
        }
    }

    fn build_prompt(context: &AgentTaskContext, config: &OpenClawConfig) -> String {
        let mut prompt = format!(
            "You are agent '{}' working on a task in the OPC (One Person Company) system.\n\n",
            context.agent.name
        );

        prompt.push_str(&format!("## Task: {}\n\n", context.issue.title));

        if let Some(desc) = &context.issue.description {
            prompt.push_str(&format!("## Description\n{}\n\n", desc));
        }

        if let Some(repo_url) = &context.issue.repo_url {
            let id_short = &context.issue.id.to_string()[..8];
            prompt.push_str(&format!(
                "## Git Repository\n\nClone the repository, create a new branch for this task, do your work, commit, and push the branch.\n\nRepository: {}\n\nSteps:\n1. `git clone {}`\n2. `git checkout -b task/{}`\n3. Do the work\n4. `git add . && git commit -m \"<summary>\" && git push origin task/{}`\n\n",
                repo_url, repo_url, id_short, id_short
            ));
        }

        if !context.parent_chain.is_empty() {
            prompt.push_str("## Parent task context\n");
            for parent in &context.parent_chain {
                prompt.push_str(&format!(
                    "- {}: {}\n",
                    parent.title,
                    parent.description.as_deref().unwrap_or("")
                ));
            }
            prompt.push('\n');
        }

        if !context.comments.is_empty() {
            prompt.push_str("## Conversation history\n");
            for comment in &context.comments {
                prompt.push_str(&format!(
                    "[{}] {}: {}\n",
                    comment.author_type, comment.author_name, comment.body
                ));
            }
            prompt.push('\n');
        }

        if context.trigger == "changes_requested" {
            prompt.push_str("**IMPORTANT: The human reviewer has requested changes. Review the conversation above and address their feedback.**\n\n");
        }

        prompt.push_str("## Instructions\n\n");
        prompt.push_str("1. Complete the task described above.\n");
        prompt.push_str("2. Write a clear summary of what you did.\n");
        prompt.push_str("3. Submit your results back to the OPC system by running the following curl command. Replace SUMMARY with your actual summary (keep it concise, escape quotes):\n\n");

        prompt.push_str(&format!(
            "```bash\ncurl -X POST {}/api/agent/issues/{}/submit \\\n  -H 'Authorization: Bearer {}' \\\n  -H 'Content-Type: application/json' \\\n  -d '{{\"summary\": \"SUMMARY\", \"artifacts\": null}}'\n```\n\n",
            context.api_base_url, context.issue.id, config.opc_api_key
        ));

        prompt.push_str("You MUST run this curl command after completing the task. This submits your work for human review. Without this step, the task remains incomplete in the system.\n");

        prompt
    }
}

#[async_trait]
impl AgentAdapter for OpenClawAdapter {
    async fn invoke(&self, context: AgentTaskContext) -> Result<AgentResponse> {
        *self.running.lock().await = true;

        let prompt = Self::build_prompt(&context, &self.config);
        let issue_title = context.issue.title.clone();

        let payload = OpenClawPayload {
            message: prompt,
            name: issue_title.clone(),
            deliver: self.config.deliver.unwrap_or(false),
            channel: self.config.channel.clone(),
            to: self.config.to.clone(),
            model: self.config.model.clone(),
            timeout_seconds: self.config.timeout_secs,
        };

        let response = self
            .client
            .post(&self.config.webhook_url)
            .header("Authorization", format!("Bearer {}", self.config.token))
            .json(&payload)
            .send()
            .await;

        *self.running.lock().await = false;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    Ok(AgentResponse {
                        status: AgentResponseStatus::Dispatched,
                        summary: format!(
                            "Task '{}' dispatched to OpenClaw. Waiting for OpenClaw to process and submit results back.",
                            issue_title
                        ),
                        artifacts: vec![],
                        cost: None,
                    })
                } else {
                    bail!(
                        "OpenClaw returned error: {} {}",
                        resp.status(),
                        resp.text().await.unwrap_or_default()
                    );
                }
            }
            Err(e) => bail!("Failed to call OpenClaw webhook: {}", e),
        }
    }

    async fn status(&self) -> Result<AgentRunStatus> {
        if *self.running.lock().await {
            Ok(AgentRunStatus::Running)
        } else {
            Ok(AgentRunStatus::Idle)
        }
    }

    async fn cancel(&self) -> Result<()> {
        *self.running.lock().await = false;
        Ok(())
    }
}
