use crate::adapter::{
    AgentAdapter, AgentResponse, AgentResponseStatus, AgentRunStatus, AgentTaskContext,
};
use anyhow::{bail, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for the HTTP webhook adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpAdapterConfig {
    pub webhook_url: String,
    pub timeout_secs: Option<u64>,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

pub struct HttpAdapter {
    config: HttpAdapterConfig,
    client: Client,
    running: Arc<Mutex<bool>>,
}

impl HttpAdapter {
    pub fn new(config: HttpAdapterConfig) -> Self {
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
}

#[async_trait]
impl AgentAdapter for HttpAdapter {
    async fn invoke(&self, context: AgentTaskContext) -> Result<AgentResponse> {
        *self.running.lock().await = true;

        let mut request = self.client.post(&self.config.webhook_url).json(&context);

        if let Some(headers) = &self.config.headers {
            for (key, value) in headers {
                request = request.header(key.as_str(), value.as_str());
            }
        }

        let response = request.send().await;
        *self.running.lock().await = false;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<AgentResponse>().await {
                        Ok(agent_resp) => Ok(agent_resp),
                        Err(_) => Ok(AgentResponse {
                            status: AgentResponseStatus::Completed,
                            summary: "Agent completed (no structured response)".to_string(),
                            artifacts: vec![],
                            cost: None,
                        }),
                    }
                } else {
                    bail!(
                        "Webhook returned error status: {} {}",
                        resp.status(),
                        resp.text().await.unwrap_or_default()
                    );
                }
            }
            Err(e) => bail!("Failed to call webhook: {}", e),
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
