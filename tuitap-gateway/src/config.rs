use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::approval::ApprovalRule;

/// Top-level configuration for a [`Gateway`](crate::gateway::Gateway) instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// How long the dispatcher must wait between consecutive pushes, in
    /// milliseconds (default: 500).
    #[serde(default = "default_cooldown_ms")]
    pub cooldown_ms: u64,

    /// Terminal columns (default: 80).
    #[serde(default = "default_cols")]
    pub cols: u16,

    /// Terminal rows (default: 24).
    #[serde(default = "default_rows")]
    pub rows: u16,

    /// Rules that gate inbound input behind explicit approval.
    ///
    /// If the list is empty, all input is forwarded directly (no approval).
    #[serde(default, skip)]
    pub approval_rules: Vec<ApprovalRule>,
}

fn default_cooldown_ms() -> u64 {
    500
}
fn default_cols() -> u16 {
    80
}
fn default_rows() -> u16 {
    24
}

impl Default for GatewayConfig {
    fn default() -> Self {
        GatewayConfig {
            cooldown_ms: default_cooldown_ms(),
            cols: default_cols(),
            rows: default_rows(),
            approval_rules: Vec::new(),
        }
    }
}

impl GatewayConfig {
    pub fn cooldown(&self) -> Duration {
        Duration::from_millis(self.cooldown_ms)
    }
}
