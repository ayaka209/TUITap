//! tuitap-push — pluggable outbound/inbound channel adapters.
//!
//! Each adapter implements the [`Channel`] trait which covers both `send` and
//! `receive` directions.  Send-only adapters (Bark, Pushover) return
//! `Ok(None)` from `receive`.
//!
//! # Built-in adapters
//!
//! | Adapter | Struct | Directions |
//! |---------|--------|-----------|
//! | ntfy | [`NtfyChannel`] | send + receive |
//! | Bark | [`BarkChannel`] | send only |
//! | Telegram Bot | [`TelegramChannel`] | send + receive |
//! | Pushover | [`PushoverChannel`] | send only |

pub mod bark;
pub mod channel;
pub mod error;
pub mod ntfy;
pub mod pushover;
pub mod telegram;

pub use bark::{BarkChannel, BarkConfig};
pub use channel::{Channel, InboundMessage, MessagePriority, OutboundMessage};
pub use error::PushError;
pub use ntfy::{NtfyChannel, NtfyConfig};
pub use pushover::{PushoverChannel, PushoverConfig};
pub use telegram::{TelegramChannel, TelegramConfig};
