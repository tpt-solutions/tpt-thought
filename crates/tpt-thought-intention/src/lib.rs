// SPDX-License-Identifier: MIT OR Apache-2.0
//! Intentions (spec section 23): the goals a thought commits to, with their constraints, risk
//! tolerance, priority, deadline, required capabilities and provenance.
//!
//! The goal data is core's [`Intention`]. This crate adds a validated constructor, a lifecycle
//! ([`IntentionStatus`]), provenance, and a set type ([`IntentionSet`]) for priority and deadline
//! queries.
//!
//! Thought never executes actions (spec section 24). Nothing here runs anything: dispatch and
//! execution belong to Anima's INP, and their outcomes come back as status transitions.

mod set;
mod status;

pub use set::{IntentionSet, SetError};
pub use status::{IllegalIntentionTransition, IntentionStatus, EDGES};

use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_core::{Intention, IntentionId, ProvenanceId, Timestamp};

/// Why an intention was rejected at construction.
#[derive(Debug, Clone, PartialEq)]
pub enum InvalidIntention {
    /// The goal is empty or only whitespace.
    EmptyGoal,
    /// Priority is NaN or infinite.
    NonFinitePriority(f64),
    /// A constraint or capability name is empty or only whitespace.
    BlankEntry(&'static str),
}

impl fmt::Display for InvalidIntention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyGoal => write!(f, "intention goal must not be blank"),
            Self::NonFinitePriority(p) => write!(f, "intention priority must be finite, got {p}"),
            Self::BlankEntry(field) => write!(f, "intention {field} entries must not be blank"),
        }
    }
}

impl std::error::Error for InvalidIntention {}

/// Capabilities an intention needs that were not granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingCapabilities {
    /// The capabilities that were required but not granted, sorted.
    pub missing: Vec<String>,
}

impl fmt::Display for MissingCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "missing capabilities: {}", self.missing.join(", "))
    }
}

impl std::error::Error for MissingCapabilities {}

/// Check that every capability `intention` requires appears in `granted`.
///
/// Nothing is granted by default: an empty `granted` satisfies only an intention that requires
/// no capabilities.
pub fn require_capabilities<'a>(
    intention: &Intention,
    granted: impl IntoIterator<Item = &'a str>,
) -> Result<(), MissingCapabilities> {
    let granted: Vec<&str> = granted.into_iter().collect();
    let mut missing: Vec<String> = intention
        .required_capabilities
        .iter()
        .filter(|c| !granted.contains(&c.as_str()))
        .cloned()
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    missing.sort();
    missing.dedup();
    Err(MissingCapabilities { missing })
}

/// An intention with its lifecycle state, transition history and provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedIntention {
    intention: Intention,
    status: IntentionStatus,
    history: Vec<IntentionStatus>,
    provenance: Vec<ProvenanceId>,
}

impl ManagedIntention {
    /// Validate `intention` and start it in [`IntentionStatus::Formed`].
    ///
    /// The goal must already be non-blank; nothing is trimmed. Constraints and capabilities are
    /// sorted and de-duplicated, so two intentions with the same content compare equal.
    pub fn new(
        mut intention: Intention,
        provenance: Vec<ProvenanceId>,
    ) -> Result<Self, InvalidIntention> {
        if intention.goal.trim().is_empty() {
            return Err(InvalidIntention::EmptyGoal);
        }
        if !intention.priority.is_finite() {
            return Err(InvalidIntention::NonFinitePriority(intention.priority));
        }
        if intention.constraints.iter().any(|c| c.trim().is_empty()) {
            return Err(InvalidIntention::BlankEntry("constraint"));
        }
        if intention
            .required_capabilities
            .iter()
            .any(|c| c.trim().is_empty())
        {
            return Err(InvalidIntention::BlankEntry("capability"));
        }
        normalise(&mut intention.constraints);
        normalise(&mut intention.required_capabilities);
        Ok(Self {
            intention,
            status: IntentionStatus::Formed,
            history: vec![IntentionStatus::Formed],
            provenance,
        })
    }

    /// The identity of the underlying intention.
    pub fn id(&self) -> IntentionId {
        self.intention.id
    }

    /// The goal data.
    pub fn intention(&self) -> &Intention {
        &self.intention
    }

    /// Current lifecycle status.
    pub fn status(&self) -> IntentionStatus {
        self.status
    }

    /// Every status visited, oldest first, starting with `Formed`.
    pub fn history(&self) -> &[IntentionStatus] {
        &self.history
    }

    /// Records explaining how the intention arose, oldest first.
    pub fn provenance(&self) -> &[ProvenanceId] {
        &self.provenance
    }

    /// Append a provenance record.
    pub fn add_provenance(&mut self, record: ProvenanceId) {
        self.provenance.push(record);
    }

    /// Move to `to` if the lifecycle permits it. A rejected transition leaves the intention
    /// unchanged.
    pub fn advance(&mut self, to: IntentionStatus) -> Result<(), IllegalIntentionTransition> {
        if !self.status.can_transition_to(to) {
            return Err(IllegalIntentionTransition {
                from: self.status,
                to,
            });
        }
        self.status = to;
        self.history.push(to);
        Ok(())
    }

    /// True if the intention is past its deadline at `now`. Terminal intentions are never
    /// overdue.
    pub fn is_overdue(&self, now: Timestamp) -> bool {
        self.status.is_active() && self.intention.deadline.is_some_and(|d| d < now)
    }
}

fn normalise(items: &mut Vec<String>) {
    items.sort();
    items.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, Probability, SeededEntropy};

    pub(crate) fn ids() -> IdGenerator {
        IdGenerator::new(SeededEntropy::new(7))
    }

    pub(crate) fn intention(ids: &IdGenerator, goal: &str, priority: f64) -> Intention {
        Intention {
            id: ids.intention(),
            goal: goal.into(),
            desired_outcome: None,
            constraints: vec![],
            risk_tolerance: Probability::new(0.2).ok(),
            priority,
            required_capabilities: vec![],
            deadline: None,
        }
    }

    #[test]
    fn rejects_blank_goal_and_non_finite_priority() {
        let ids = ids();
        assert_eq!(
            ManagedIntention::new(intention(&ids, "  ", 1.0), vec![]),
            Err(InvalidIntention::EmptyGoal)
        );
        assert!(matches!(
            ManagedIntention::new(intention(&ids, "x", f64::NAN), vec![]),
            Err(InvalidIntention::NonFinitePriority(_))
        ));
    }

    #[test]
    fn rejects_blank_capability_and_constraint() {
        let ids = ids();
        let mut i = intention(&ids, "x", 1.0);
        i.required_capabilities = vec!["move:arm".into(), " ".into()];
        assert_eq!(
            ManagedIntention::new(i, vec![]),
            Err(InvalidIntention::BlankEntry("capability"))
        );
        let mut i = intention(&ids, "x", 1.0);
        i.constraints = vec!["".into()];
        assert_eq!(
            ManagedIntention::new(i, vec![]),
            Err(InvalidIntention::BlankEntry("constraint"))
        );
    }

    #[test]
    fn normalises_lists() {
        let ids = ids();
        let mut i = intention(&ids, "x", 1.0);
        i.required_capabilities = vec!["b".into(), "a".into(), "b".into()];
        let m = ManagedIntention::new(i, vec![]).unwrap();
        assert_eq!(m.intention().required_capabilities, vec!["a", "b"]);
    }

    #[test]
    fn lifecycle_follows_table_and_records_history() {
        let ids = ids();
        let mut m = ManagedIntention::new(intention(&ids, "x", 1.0), vec![]).unwrap();
        m.advance(IntentionStatus::Dispatched).unwrap();
        m.advance(IntentionStatus::Failed).unwrap();
        m.advance(IntentionStatus::Dispatched).unwrap();
        m.advance(IntentionStatus::Executing).unwrap();
        m.advance(IntentionStatus::Achieved).unwrap();
        assert_eq!(m.status(), IntentionStatus::Achieved);
        assert_eq!(m.history().len(), 6);
        assert_eq!(m.history()[0], IntentionStatus::Formed);
    }

    #[test]
    fn illegal_transition_leaves_state_unchanged() {
        let ids = ids();
        let mut m = ManagedIntention::new(intention(&ids, "x", 1.0), vec![]).unwrap();
        let err = m.advance(IntentionStatus::Achieved).unwrap_err();
        assert_eq!(err.from, IntentionStatus::Formed);
        assert_eq!(err.to, IntentionStatus::Achieved);
        assert_eq!(m.status(), IntentionStatus::Formed);
        assert_eq!(m.history(), &[IntentionStatus::Formed]);
    }

    #[test]
    fn terminal_intention_cannot_move() {
        let ids = ids();
        let mut m = ManagedIntention::new(intention(&ids, "x", 1.0), vec![]).unwrap();
        m.advance(IntentionStatus::Abandoned).unwrap();
        assert!(m.advance(IntentionStatus::Dispatched).is_err());
    }

    #[test]
    fn overdue_only_while_active() {
        let ids = ids();
        let mut i = intention(&ids, "x", 1.0);
        i.deadline = Some(Timestamp(10));
        let mut m = ManagedIntention::new(i, vec![]).unwrap();
        assert!(!m.is_overdue(Timestamp(10)));
        assert!(m.is_overdue(Timestamp(11)));
        m.advance(IntentionStatus::Abandoned).unwrap();
        assert!(!m.is_overdue(Timestamp(11)));
    }

    #[test]
    fn capabilities_are_never_granted_by_default() {
        let ids = ids();
        let mut i = intention(&ids, "x", 1.0);
        i.required_capabilities = vec!["move:arm".into(), "sense:thermal".into()];
        assert_eq!(
            require_capabilities(&i, []),
            Err(MissingCapabilities {
                missing: vec!["move:arm".into(), "sense:thermal".into()]
            })
        );
        assert!(require_capabilities(&i, ["move:arm"]).is_err());
        assert!(require_capabilities(&i, ["move:arm", "sense:thermal", "extra"]).is_ok());
    }

    #[test]
    fn intention_without_requirements_needs_no_grant() {
        let ids = ids();
        let i = intention(&ids, "x", 1.0);
        assert!(require_capabilities(&i, []).is_ok());
    }

    #[test]
    fn serde_round_trip_preserves_state_and_provenance() {
        let ids = ids();
        let mut m =
            ManagedIntention::new(intention(&ids, "x", 1.0), vec![ids.provenance()]).unwrap();
        m.advance(IntentionStatus::Dispatched).unwrap();
        let json = serde_json::to_string(&m).unwrap();
        let back: ManagedIntention = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.status(), IntentionStatus::Dispatched);
    }
}
