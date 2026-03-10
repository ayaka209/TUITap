use async_trait::async_trait;
use reqwest::Client;

use crate::{
    channel::{Channel, InboundMessage, MessagePriority, OutboundMessage},
    error::PushError,
};

// ntfy priority mapping
fn ntfy_priority(p: MessagePriority) -> &'static str {
    match p {
        MessagePriority::Low => "low",
        MessagePriority::Normal => "default",
        MessagePriority::High => "high",
        MessagePriority::Urgent => "urgent",
    }
}

/// Configuration for the ntfy channel.
#[derive(Debug, Clone)]
pub struct NtfyConfig {
    /// ntfy server base URL (default: `https://ntfy.sh`).
    pub server: String,
    /// Topic name.
    pub topic: String,
    /// Optional Bearer token for authenticated ntfy instances.
    pub token: Option<String>,
}

impl NtfyConfig {
    pub fn new(topic: impl Into<String>) -> Self {
        NtfyConfig {
            server: "https://ntfy.sh".into(),
            topic: topic.into(),
            token: None,
        }
    }

    pub fn with_server(mut self, server: impl Into<String>) -> Self {
        self.server = server.into();
        self
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }
}

/// ntfy push channel — supports both outbound notifications and inbound
/// messages via the ntfy JSON polling endpoint.
pub struct NtfyChannel {
    config: NtfyConfig,
    client: Client,
    /// Monotonically increasing ntfy message ID used as a poll cursor.
    last_id: Option<String>,
}

impl NtfyChannel {
    pub fn new(config: NtfyConfig) -> Self {
        NtfyChannel {
            config,
            client: Client::new(),
            last_id: None,
        }
    }

    fn topic_url(&self) -> String {
        format!("{}/{}", self.config.server.trim_end_matches('/'), self.config.topic)
    }

    fn add_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.config.token {
            req.bearer_auth(token)
        } else {
            req
        }
    }
}

#[async_trait]
impl Channel for NtfyChannel {
    fn name(&self) -> &str {
        "ntfy"
    }

    async fn send(&self, message: &OutboundMessage) -> Result<(), PushError> {
        let url = self.topic_url();
        let mut req = self.client.post(&url).body(message.body.clone());

        if let Some(title) = &message.title {
            req = req.header("Title", title.as_str());
        }
        req = req.header("Priority", ntfy_priority(message.priority));
        req = self.add_auth(req);

        let resp = req.send().await?;
        resp.error_for_status()?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<Option<InboundMessage>, PushError> {
        // Poll the ntfy JSON endpoint for new messages since `last_id`.
        let url = format!("{}/json?poll=1", self.topic_url());
        let mut req = self.client.get(&url);
        if let Some(id) = &self.last_id {
            req = req.header("Last-Event-ID", id.as_str());
        }
        req = self.add_auth(req);

        let resp = req.send().await?;
        let text = resp.text().await?;

        // ntfy streams newline-delimited JSON objects; pick the last one.
        if let Some(last_line) = text.lines().filter(|l| !l.is_empty()).last() {
            let v: serde_json::Value = serde_json::from_str(last_line)?;
            if v["event"] == "message" {
                let id = v["id"].as_str().unwrap_or("").to_string();
                let body = v["message"].as_str().unwrap_or("").to_string();
                let from = v["topic"].as_str().unwrap_or("ntfy").to_string();
                self.last_id = Some(id.clone());
                if !body.is_empty() {
                    return Ok(Some(InboundMessage {
                        from,
                        body,
                        channel_ref: id,
                    }));
                }
            }
        }
        Ok(None)
    }
}

// (end of ntfy.rs)
