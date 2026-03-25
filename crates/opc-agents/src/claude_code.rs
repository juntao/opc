use crate::adapter::{
    AgentAdapter, AgentResponse, AgentResponseStatus, AgentRunStatus, AgentTaskContext, Artifact,
};
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

/// Configuration for the Claude Code adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeConfig {
    /// Working directory for Claude Code to operate in.
    pub working_dir: Option<String>,
    /// Maximum turns for the Claude Code session.
    pub max_turns: Option<u32>,
    /// Model to use (e.g., "sonnet", "opus").
    pub model: Option<String>,
}

pub struct ClaudeCodeAdapter {
    config: ClaudeCodeConfig,
    running: Arc<Mutex<bool>>,
    child: Arc<Mutex<Option<tokio::process::Child>>>,
}

impl ClaudeCodeAdapter {
    pub fn new(config: ClaudeCodeConfig) -> Self {
        Self {
            config,
            running: Arc::new(Mutex::new(false)),
            child: Arc::new(Mutex::new(None)),
        }
    }

    fn build_prompt(&self, context: &AgentTaskContext) -> String {
        let mut prompt = format!(
            "You are agent '{}' working on task: {}\n\n",
            context.agent.name, context.issue.title
        );

        // Project context
        if let Some(project) = &context.project {
            prompt.push_str(&format!("Project context ({}):\n", project.name));
            if let Some(desc) = &project.description {
                prompt.push_str(&format!("{}\n\n", desc));
            } else {
                prompt.push('\n');
            }
        }

        if let Some(desc) = &context.issue.description {
            prompt.push_str(&format!("Task description:\n{}\n\n", desc));
        }

        if let Some(repo_url) = context.project.as_ref().and_then(|p| p.repo_url.as_ref()) {
            let id_short = &context.issue.id.to_string()[..8];
            prompt.push_str(&format!(
                "Git repository:\nClone the repository, create a new branch for this task, do your work, commit, and push the branch.\n\nRepository: {}\n\nSteps:\n1. git clone {}\n2. git checkout -b task/{}\n3. Do the work\n4. git add . && git commit -m \"<summary>\" && git push origin task/{}\n\n",
                repo_url, repo_url, id_short, id_short
            ));
        }

        if !context.parent_chain.is_empty() {
            prompt.push_str("Parent task context:\n");
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
            prompt.push_str("Comment thread:\n");
            for comment in &context.comments {
                prompt.push_str(&format!(
                    "[{}] {}: {}\n",
                    comment.author_type, comment.author_name, comment.body
                ));
            }
            prompt.push('\n');
        }

        // Check for human feedback (changes_requested trigger)
        if context.trigger == "changes_requested" {
            prompt.push_str("IMPORTANT: The human reviewer has requested changes. Please review the comments above and address their feedback.\n\n");
        }

        prompt.push_str("Complete the task and provide a summary of what you did.\n");

        prompt
    }
}

#[async_trait]
impl AgentAdapter for ClaudeCodeAdapter {
    async fn invoke(&self, context: AgentTaskContext) -> Result<AgentResponse> {
        *self.running.lock().await = true;

        let prompt = self.build_prompt(&context);

        let mut cmd = Command::new("claude");
        cmd.arg("--print");
        cmd.arg("--output-format").arg("text");

        if let Some(model) = &self.config.model {
            cmd.arg("--model").arg(model);
        }

        if let Some(max_turns) = self.config.max_turns {
            cmd.arg("--max-turns").arg(max_turns.to_string());
        }

        if let Some(dir) = &self.config.working_dir {
            cmd.current_dir(dir);
        }

        cmd.arg(&prompt);

        let output = cmd.output().await;
        *self.running.lock().await = false;

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    Ok(AgentResponse {
                        status: AgentResponseStatus::NeedsApproval,
                        summary: if stdout.len() > 2000 {
                            format!("{}...", &stdout[..2000])
                        } else {
                            stdout.clone()
                        },
                        artifacts: vec![Artifact {
                            name: "claude_output".to_string(),
                            artifact_type: "text".to_string(),
                            url: None,
                            content: Some(stdout),
                        }],
                        cost: None,
                    })
                } else {
                    bail!("Claude Code failed: {}", stderr);
                }
            }
            Err(e) => {
                bail!(
                    "Failed to spawn Claude Code: {}. Is 'claude' CLI installed?",
                    e
                );
            }
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
        if let Some(child) = self.child.lock().await.as_mut() {
            let _ = child.kill().await;
        }
        Ok(())
    }
}
