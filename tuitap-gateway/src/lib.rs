//! tuitap-gateway — central hub for PTY ↔ remote channel communication.
//!
//! # Architecture
//!
//! ```text
//!  ┌────────────┐   raw bytes   ┌──────────────────┐
//!  │  PTY master│──────────────▶│  VTE processor   │
//!  └────────────┘               │  + noise filter  │
//!        ▲                      └────────┬─────────┘
//!        │ stdin                         │ clean snapshot
//!        │                               ▼
//!  ┌─────┴──────┐              ┌──────────────────┐
//!  │ TrustStore │◀────────────▶│   Dispatcher     │
//!  │ (identity  │              │  (diff + cooldown│
//!  │  allowlist)│              └────────┬─────────┘
//!  └─────┬──────┘                       │
//!        │                              │ OutboundMessage
//!  ┌─────▼──────┐              ┌────────▼─────────┐
//!  │ Approval   │              │  Push channels   │
//!  │   Gate     │◀────────────▶│ ntfy/Bark/TG/PO  │
//!  └────────────┘  InboundMsg  └──────────────────┘
//! ```

pub mod approval;
pub mod config;
pub mod dispatcher;
pub mod error;
pub mod gateway;
pub mod trust;

pub use approval::{ApprovalGate, ApprovalRule, PendingApproval};
pub use config::GatewayConfig;
pub use dispatcher::Dispatcher;
pub use error::GatewayError;
pub use gateway::Gateway;
pub use trust::{PermissionLevel, TrustStore, TrustedIdentity};
