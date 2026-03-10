use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    channel::{Channel, InboundMessage, OutboundMessage},
    error::PushError,
};

/// Configuration for the Telegram Bot channel.
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    /// Bot token from BotFather.
    pub bot_token: String,
    /// The chat ID to send messages to (user, group, or channel).
    pub chat_id: i64,
}

impl TelegramConfig {
    pub fn new(bot_token: impl Into<String>, chat_id: i64) -> Self {
        TelegramConfig {
            bot_token: bot_token.into(),
            chat_id,
        }
    }
}

/// Telegram Bot channel — supports both outbound messages and inbound commands
/// via long-polling (`getUpdates`).
pub struct TelegramChannel {
    config: TelegramConfig,
    client: Client,
    /// Update offset: only retrieve updates with `update_id > offset`.
    offset: i64,
}

impl TelegramChannel {
    pub fn new(config: TelegramConfig) -> Self {
        TelegramChannel {
            config,
            client: Client::new(),
            offset: 0,
        }
    }

    fn api_url(&self, method: &str) -> String {
        format!(
            "https://api.telegram.org/bot{}/{}",
            self.config.bot_token, method
        )
    }
}

// ── Telegram API types ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    chat_id: i64,
    text: &'a str,
}

#[derive(Deserialize)]
struct TelegramResponse<T> {
    ok: bool,
    result: Option<T>,
}

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    message: Option<TelegramMessage>,
}

#[derive(Deserialize)]
struct TelegramMessage {
    message_id: i64,
    text: Option<String>,
    from: Option<TelegramUser>,
}

#[derive(Deserialize)]
struct TelegramUser {
    id: i64,
    username: Option<String>,
}

// ── Channel impl ──────────────────────────────────────────────────────────────

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn send(&self, message: &OutboundMessage) -> Result<(), PushError> {
        let text = match &message.title {
            Some(t) => format!("**{}**\n{}", t, message.body),
            None => message.body.clone(),
        };
        let req = SendMessageRequest {
            chat_id: self.config.chat_id,
            text: &text,
        };
        let resp = self
            .client
            .post(self.api_url("sendMessage"))
            .json(&req)
            .send()
            .await?;
        let tg: TelegramResponse<serde_json::Value> = resp.json().await?;
        if !tg.ok {
            return Err(PushError::Channel("Telegram sendMessage failed".into()));
        }
        Ok(())
    }

    async fn receive(&mut self) -> Result<Option<InboundMessage>, PushError> {
        #[derive(Serialize)]
        struct GetUpdatesRequest {
            offset: i64,
            limit: u8,
            timeout: u8,
        }

        let req = GetUpdatesRequest {
            offset: self.offset,
            limit: 10,
            timeout: 0, // non-blocking
        };

        let resp = self
            .client
            .post(self.api_url("getUpdates"))
            .json(&req)
            .send()
            .await?;
        let tg: TelegramResponse<Vec<Update>> = resp.json().await?;
        if !tg.ok {
            return Ok(None);
        }

        let updates = match tg.result {
            Some(u) => u,
            None => return Ok(None),
        };

        // Advance the offset past all updates we've seen.
        if let Some(last) = updates.last() {
            self.offset = last.update_id + 1;
        }

        // Return the first message that has text content.
        for update in updates {
            if let Some(msg) = update.message {
                if let Some(text) = msg.text {
                    let from = msg
                        .from
                        .map(|u| {
                            u.username
                                .unwrap_or_else(|| u.id.to_string())
                        })
                        .unwrap_or_else(|| "unknown".into());
                    return Ok(Some(InboundMessage {
                        from,
                        body: text,
                        channel_ref: msg.message_id.to_string(),
                    }));
                }
            }
        }
        Ok(None)
    }
}
