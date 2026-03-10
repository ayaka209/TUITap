//! tuitap-core — PTY capture, VT parsing, screen buffer, noise filtering.

pub mod error;
pub mod noise_filter;
pub mod pty;
pub mod screen_buffer;
pub mod vte_processor;

pub use error::CoreError;
pub use noise_filter::{NoiseFilter, NoiseFilterConfig};
pub use pty::PtySession;
pub use screen_buffer::ScreenBuffer;
pub use vte_processor::VteProcessor;
