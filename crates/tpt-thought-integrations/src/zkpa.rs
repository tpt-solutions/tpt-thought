// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from `tpt-anima-zkpa` to Thought's identity and permission checks (spec section 40;
//! todo.md Phase 6).
//!
//! Two mappings are provided:
//!
//! - Sessions: a task-scoped, expiring credential for an agent, with a compute budget. Thought
//!   opens one per task and revokes it when the task ends. Sessions expire by time as well.
//! - Delegation: a parent agent grants child agents named permissions, and may only grant what it
//!   holds itself. Revoking a delegation takes effect at once.
//!
//! Not mapped: hardware attestation, zero-knowledge proofs and the agency ledger. Thought does not
//! yet produce attestation evidence or hold a ledger, so those stay with Anima.

use std::time::Duration;

use tpt_anima_zkpa::{DelegationManager, SessionManager, ZkpaError};
use uuid::Uuid;

/// Why an identity operation was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum IdentityError {
    /// The compute budget is not a finite, non-negative number.
    InvalidBudget,
    /// The session lifetime is zero, so the session would be expired on creation.
    ZeroLifetime,
    /// Anima refused the operation, such as an empty or over-reaching delegation. Holds Anima's
    /// message, since `ZkpaError` does not implement `PartialEq`.
    Refused(String),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBudget => write!(f, "compute budget must be finite and non-negative"),
            Self::ZeroLifetime => write!(f, "session lifetime must be greater than zero"),
            Self::Refused(reason) => write!(f, "{reason}"),
        }
    }
}

fn refused(error: ZkpaError) -> IdentityError {
    IdentityError::Refused(error.to_string())
}

impl std::error::Error for IdentityError {}

/// Task sessions and permission delegations, held by Anima's ZKPA managers.
#[derive(Default)]
pub struct AnimaIdentity {
    sessions: SessionManager,
    delegations: DelegationManager,
}

impl AnimaIdentity {
    /// No sessions and no delegations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a session for `agent`, scoped to `task`, with `budget` compute units, valid for `ttl`.
    /// Returns the session's identifier.
    pub fn open_session(
        &mut self,
        agent: Uuid,
        task: &str,
        budget: f64,
        ttl: Duration,
    ) -> Result<Uuid, IdentityError> {
        if !budget.is_finite() || budget < 0.0 {
            return Err(IdentityError::InvalidBudget);
        }
        if ttl.is_zero() {
            return Err(IdentityError::ZeroLifetime);
        }
        let session = self
            .sessions
            .create_session(agent, task.to_string(), budget, ttl, None);
        Ok(session.id)
    }

    /// Whether a session exists and has not expired.
    pub fn session_is_valid(&self, session: Uuid) -> bool {
        self.sessions.validate_session(session).is_ok()
    }

    /// Revoke a session. Returns `false` if it was not held.
    pub fn close_session(&mut self, session: Uuid) -> bool {
        self.sessions.revoke_session(session).is_ok()
    }

    /// Grant `child` the named `permissions` from `parent`. Anima refuses an empty list, and a
    /// parent that does not hold a permission cannot delegate it.
    pub fn delegate(
        &mut self,
        parent: Uuid,
        child: Uuid,
        permissions: &[&str],
    ) -> Result<(), IdentityError> {
        let permissions = permissions.iter().map(|p| (*p).to_string()).collect();
        self.delegations
            .delegate(parent, child, permissions, None)
            .map(|_| ())
            .map_err(refused)
    }

    /// Whether `child` currently holds `permission` from `parent`. Revoked delegations do not count.
    pub fn is_authorized(&self, parent: Uuid, child: Uuid, permission: &str) -> bool {
        self.delegations.is_authorized(parent, child, permission)
    }

    /// Revoke the delegation from `parent` to `child`.
    pub fn revoke_delegation(&mut self, parent: Uuid, child: Uuid) -> Result<(), IdentityError> {
        self.delegations.revoke(parent, child).map_err(refused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn sessions_are_valid_until_closed() {
        let mut identity = AnimaIdentity::new();
        let session = identity
            .open_session(agent(1), "summarise", 5.0, Duration::from_secs(60))
            .unwrap();
        assert!(identity.session_is_valid(session));
        assert!(identity.close_session(session));
        assert!(!identity.session_is_valid(session));
        assert!(!identity.close_session(session), "already closed");
    }

    #[test]
    fn sessions_with_bad_budgets_or_zero_lifetimes_are_refused() {
        let mut identity = AnimaIdentity::new();
        let ttl = Duration::from_secs(60);
        assert_eq!(
            identity.open_session(agent(1), "t", f64::NAN, ttl),
            Err(IdentityError::InvalidBudget)
        );
        assert_eq!(
            identity.open_session(agent(1), "t", -1.0, ttl),
            Err(IdentityError::InvalidBudget)
        );
        assert_eq!(
            identity.open_session(agent(1), "t", 1.0, Duration::ZERO),
            Err(IdentityError::ZeroLifetime)
        );
    }

    #[test]
    fn delegated_permissions_hold_until_revoked() {
        let mut identity = AnimaIdentity::new();
        let (root, child) = (agent(1), agent(2));
        identity.delegate(root, child, &["read", "write"]).unwrap();
        assert!(identity.is_authorized(root, child, "read"));
        assert!(!identity.is_authorized(root, child, "delete"));
        identity.revoke_delegation(root, child).unwrap();
        assert!(!identity.is_authorized(root, child, "read"));
    }

    #[test]
    fn empty_delegations_are_refused() {
        let mut identity = AnimaIdentity::new();
        assert!(matches!(
            identity.delegate(agent(1), agent(2), &[]),
            Err(IdentityError::Refused(reason)) if reason.contains("empty permissions")
        ));
    }

    #[test]
    fn a_child_cannot_pass_on_more_than_it_was_granted() {
        let mut identity = AnimaIdentity::new();
        let (root, middle, leaf) = (agent(1), agent(2), agent(3));
        identity.delegate(root, middle, &["read"]).unwrap();
        assert!(identity.delegate(middle, leaf, &["write"]).is_err());
        assert!(identity.delegate(middle, leaf, &["read"]).is_ok());
    }
}
