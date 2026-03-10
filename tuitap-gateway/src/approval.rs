use std::collections::HashMap;

use tokio::sync::oneshot;
use uuid::Uuid;

// ── PendingApproval ───────────────────────────────────────────────────────────

/// A terminal action that is awaiting explicit approval before being forwarded
/// to PTY stdin.
pub struct PendingApproval {
    /// Unique request identifier.
    pub id: String,
    /// The raw bytes to write to stdin if approved.
    pub payload: Vec<u8>,
    /// Identity that submitted the action.
    pub requested_by: String,
    /// Human-readable description of the action.
    pub description: String,
    /// Fulfil the pending `receiver` with `true` (approve) or `false` (reject).
    responder: oneshot::Sender<bool>,
}

// ── ApprovalGate ─────────────────────────────────────────────────────────────

/// Manages in-flight approval requests.
///
/// When an inbound action requires approval (based on [`ApprovalRule`]
/// matching), the gateway calls [`ApprovalGate::request`] to create a pending
/// entry and receives a [`oneshot::Receiver<bool>`] that resolves when an
/// `Approve`-level identity calls [`ApprovalGate::respond`].
#[derive(Default)]
pub struct ApprovalGate {
    pending: HashMap<String, PendingApproval>,
}

impl ApprovalGate {
    pub fn new() -> Self {
        ApprovalGate::default()
    }

    /// Create a new pending approval and return `(request_id, receiver)`.
    ///
    /// The caller should `.await` the receiver to learn whether the action was
    /// approved (`true`) or rejected (`false`).
    pub fn request(
        &mut self,
        payload: Vec<u8>,
        requested_by: impl Into<String>,
        description: impl Into<String>,
    ) -> (String, oneshot::Receiver<bool>) {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.insert(
            id.clone(),
            PendingApproval {
                id: id.clone(),
                payload,
                requested_by: requested_by.into(),
                description: description.into(),
                responder: tx,
            },
        );
        (id, rx)
    }

    /// Resolve a pending approval with `approved = true` or `false`.
    ///
    /// Returns the associated payload if the approval existed, or `None`
    /// if the `id` was not found.
    pub fn respond(&mut self, id: &str, approved: bool) -> Option<Vec<u8>> {
        if let Some(pending) = self.pending.remove(id) {
            let payload = if approved {
                Some(pending.payload)
            } else {
                None
            };
            // Ignore send errors — receiver may have been dropped on timeout.
            let _ = pending.responder.send(approved);
            return payload;
        }
        None
    }

    /// IDs of all currently pending approvals.
    pub fn pending_ids(&self) -> Vec<String> {
        self.pending.keys().cloned().collect()
    }

    /// Description of a pending approval, if it exists.
    pub fn describe(&self, id: &str) -> Option<&str> {
        self.pending.get(id).map(|p| p.description.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }
}

// ── ApprovalRule ─────────────────────────────────────────────────────────────

/// Decides which inbound input strings require an approval round-trip.
#[derive(Debug, Clone)]
pub enum ApprovalRule {
    /// Every inbound action requires approval.
    Always,
    /// Require approval when the input exactly matches the given string.
    ExactMatch(String),
    /// Require approval when the input contains the given substring.
    Contains(String),
    /// Require approval when the input matches the given regex pattern.
    Regex(String),
}

impl ApprovalRule {
    /// Return `true` if `input` should be gated behind approval.
    pub fn matches(&self, input: &str) -> bool {
        match self {
            ApprovalRule::Always => true,
            ApprovalRule::ExactMatch(s) => input == s,
            ApprovalRule::Contains(s) => input.contains(s.as_str()),
            ApprovalRule::Regex(pattern) => regex::Regex::new(pattern)
                .map(|re: regex::Regex| re.is_match(input))
                .unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_and_approve() {
        let mut gate = ApprovalGate::new();
        let (id, mut rx) = gate.request(b"rm -rf /".to_vec(), "alice", "delete everything");

        assert_eq!(gate.len(), 1);
        assert!(gate.describe(&id).is_some());

        let payload = gate.respond(&id, true);
        assert_eq!(payload, Some(b"rm -rf /".to_vec()));
        assert!(gate.is_empty());

        // Receiver should have been notified
        assert_eq!(rx.try_recv().ok(), Some(true));
    }

    #[test]
    fn test_request_and_reject() {
        let mut gate = ApprovalGate::new();
        let (id, mut rx) = gate.request(b"dangerous".to_vec(), "bob", "risky action");

        let payload = gate.respond(&id, false);
        assert_eq!(payload, None); // rejected → no payload forwarded
        assert_eq!(rx.try_recv().ok(), Some(false));
    }

    #[test]
    fn test_unknown_id_returns_none() {
        let mut gate = ApprovalGate::new();
        assert!(gate.respond("nonexistent", true).is_none());
    }

    #[test]
    fn test_approval_rule_always() {
        let rule = ApprovalRule::Always;
        assert!(rule.matches("anything"));
        assert!(rule.matches(""));
    }

    #[test]
    fn test_approval_rule_contains() {
        let rule = ApprovalRule::Contains("rm".into());
        assert!(rule.matches("sudo rm -rf /tmp"));
        assert!(!rule.matches("ls -la"));
    }

    #[test]
    fn test_approval_rule_regex() {
        let rule = ApprovalRule::Regex(r"^(rm|dd|mkfs)\b".into());
        assert!(rule.matches("rm -rf /"));
        assert!(!rule.matches("echo hello"));
    }
}
