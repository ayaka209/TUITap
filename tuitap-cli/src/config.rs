use serde::{Deserialize, Serialize};
use tuitap_gateway::{GatewayConfig, PermissionLevel};
use tuitap_push::{BarkConfig, NtfyConfig, PushoverConfig, TelegramConfig};

/// Top-level configuration file structure (`tuitap.toml`).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default)]
    pub gateway: GatewayConfig,
    #[serde(default)]
    pub channels: ChannelsConfig,
    #[serde(default)]
    pub trust: Vec<TrustEntry>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ChannelsConfig {
    pub ntfy: Option<NtfyChannelConfig>,
    pub bark: Option<BarkChannelConfig>,
    pub telegram: Option<TelegramChannelConfig>,
    pub pushover: Option<PushoverChannelConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NtfyChannelConfig {
    pub topic: String,
    #[serde(default = "default_ntfy_server")]
    pub server: String,
    pub token: Option<String>,
}

fn default_ntfy_server() -> String {
    "https://ntfy.sh".into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BarkChannelConfig {
    pub device_key: String,
    #[serde(default = "default_bark_server")]
    pub server: String,
}

fn default_bark_server() -> String {
    "https://api.day.app".into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TelegramChannelConfig {
    pub bot_token: String,
    pub chat_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushoverChannelConfig {
    pub api_token: String,
    pub user_key: String,
    pub device: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrustEntry {
    pub id: String,
    pub permission: PermissionLevel,
    pub label: Option<String>,
}

// ── builders ──────────────────────────────────────────────────────────────────

impl ChannelsConfig {
    pub fn build_channels(&self) -> Vec<Box<dyn tuitap_push::Channel>> {
        let mut channels: Vec<Box<dyn tuitap_push::Channel>> = Vec::new();

        if let Some(c) = &self.ntfy {
            let cfg = NtfyConfig::new(c.topic.clone())
                .with_server(c.server.clone());
            let cfg = if let Some(t) = &c.token {
                cfg.with_token(t.clone())
            } else {
                cfg
            };
            channels.push(Box::new(tuitap_push::NtfyChannel::new(cfg)));
        }

        if let Some(c) = &self.bark {
            let cfg = BarkConfig::new(c.device_key.clone())
                .with_server(c.server.clone());
            channels.push(Box::new(tuitap_push::BarkChannel::new(cfg)));
        }

        if let Some(c) = &self.telegram {
            let cfg = TelegramConfig::new(c.bot_token.clone(), c.chat_id);
            channels.push(Box::new(tuitap_push::TelegramChannel::new(cfg)));
        }

        if let Some(c) = &self.pushover {
            let mut cfg = PushoverConfig::new(c.api_token.clone(), c.user_key.clone());
            if let Some(d) = &c.device {
                cfg = cfg.with_device(d.clone());
            }
            channels.push(Box::new(tuitap_push::PushoverChannel::new(cfg)));
        }

        channels
    }
}
