use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── Permission levels ─────────────────────────────────────────────────────────

/// Permission level granted to a trusted identity.
///
/// Levels are ordered: `ReadOnly` < `SendInput` < `Approve`.
/// A check for level N succeeds when the identity holds level ≥ N.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    /// May only receive outbound notifications; cannot write to the terminal.
    ReadOnly = 0,
    /// May send arbitrary input to the terminal's stdin.
    SendInput = 1,
    /// May send input **and** approve or reject pending approval requests.
    Approve = 2,
}

// ── TrustedIdentity ───────────────────────────────────────────────────────────

/// A single entry in the trust store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedIdentity {
    /// Opaque identifier — device token, Telegram user ID, etc.
    pub id: String,
    /// Granted permission level.
    pub permission: PermissionLevel,
    /// Optional human-readable label.
    pub label: Option<String>,
}

// ── TrustStore ────────────────────────────────────────────────────────────────

/// Manages a configurable allowlist of trusted identities.
///
/// All inbound actions are verified against this store before they reach the
/// terminal.
#[derive(Debug, Default)]
pub struct TrustStore {
    identities: HashMap<String, TrustedIdentity>,
}

impl TrustStore {
    pub fn new() -> Self {
        TrustStore::default()
    }

    /// Register or update a trusted identity.
    pub fn add(
        &mut self,
        id: impl Into<String>,
        permission: PermissionLevel,
        label: Option<String>,
    ) {
        let id = id.into();
        self.identities.insert(
            id.clone(),
            TrustedIdentity {
                id,
                permission,
                label,
            },
        );
    }

    /// Remove an identity from the trust store.
    pub fn remove(&mut self, id: &str) -> bool {
        self.identities.remove(id).is_some()
    }

    /// Return the permission level of `id`, or `None` if unknown.
    pub fn permission(&self, id: &str) -> Option<PermissionLevel> {
        self.identities.get(id).map(|i| i.permission)
    }

    /// Return `true` iff identity `id` holds at least `required`.
    pub fn has_permission(&self, id: &str, required: PermissionLevel) -> bool {
        self.permission(id)
            .map(|p| p >= required)
            .unwrap_or(false)
    }

    /// Iterate over all trusted identities.
    pub fn identities(&self) -> impl Iterator<Item = &TrustedIdentity> {
        self.identities.values()
    }

    /// Number of registered identities.
    pub fn len(&self) -> usize {
        self.identities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_permission() {
        let mut store = TrustStore::new();
        store.add("alice", PermissionLevel::Approve, Some("Alice".into()));
        store.add("bob", PermissionLevel::ReadOnly, None);

        assert_eq!(store.permission("alice"), Some(PermissionLevel::Approve));
        assert_eq!(store.permission("bob"), Some(PermissionLevel::ReadOnly));
        assert_eq!(store.permission("charlie"), None);
    }

    #[test]
    fn test_has_permission_ordering() {
        let mut store = TrustStore::new();
        store.add("user", PermissionLevel::SendInput, None);

        assert!(store.has_permission("user", PermissionLevel::ReadOnly));
        assert!(store.has_permission("user", PermissionLevel::SendInput));
        assert!(!store.has_permission("user", PermissionLevel::Approve));
    }

    #[test]
    fn test_remove() {
        let mut store = TrustStore::new();
        store.add("x", PermissionLevel::ReadOnly, None);
        assert!(store.remove("x"));
        assert!(!store.remove("x")); // second remove returns false
        assert_eq!(store.permission("x"), None);
    }
}
