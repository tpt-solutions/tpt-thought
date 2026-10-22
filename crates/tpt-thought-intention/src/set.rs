// SPDX-License-Identifier: MIT OR Apache-2.0
//! A collection of intentions with priority ordering and deadline queries.

use crate::{IllegalIntentionTransition, IntentionStatus, ManagedIntention};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use tpt_thought_core::{IntentionId, Timestamp};

/// Why a set operation failed.
#[derive(Debug, Clone, PartialEq)]
pub enum SetError {
    /// An intention with this id is already in the set.
    Duplicate(IntentionId),
    /// No intention with this id is in the set.
    Unknown(IntentionId),
    /// The requested lifecycle transition is not permitted.
    Transition(IllegalIntentionTransition),
}

impl fmt::Display for SetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(_) => write!(f, "intention is already in the set"),
            Self::Unknown(_) => write!(f, "no intention with that id is in the set"),
            Self::Transition(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for SetError {}

impl From<IllegalIntentionTransition> for SetError {
    fn from(e: IllegalIntentionTransition) -> Self {
        Self::Transition(e)
    }
}

/// Intentions keyed by id. Iteration order is by id; queries sort explicitly.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<ManagedIntention>", into = "Vec<ManagedIntention>")]
pub struct IntentionSet {
    items: BTreeMap<IntentionId, ManagedIntention>,
}

impl IntentionSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of intentions, in any status.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True if the set holds no intentions.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Add an intention. Ids must be unique within the set.
    pub fn insert(&mut self, item: ManagedIntention) -> Result<(), SetError> {
        let id = item.id();
        if self.items.contains_key(&id) {
            return Err(SetError::Duplicate(id));
        }
        self.items.insert(id, item);
        Ok(())
    }

    /// The intention with `id`, if present.
    pub fn get(&self, id: &IntentionId) -> Option<&ManagedIntention> {
        self.items.get(id)
    }

    /// Move the intention with `id` to `to`.
    pub fn advance(&mut self, id: &IntentionId, to: IntentionStatus) -> Result<(), SetError> {
        let item = self.items.get_mut(id).ok_or(SetError::Unknown(*id))?;
        item.advance(to)?;
        Ok(())
    }

    /// Intentions that are still active, highest priority first. Ties break by id.
    pub fn by_priority(&self) -> Vec<&ManagedIntention> {
        let mut out: Vec<&ManagedIntention> = self
            .items
            .values()
            .filter(|m| m.status().is_active())
            .collect();
        out.sort_by(|a, b| {
            b.intention()
                .priority
                .total_cmp(&a.intention().priority)
                .then_with(|| a.id().cmp(&b.id()))
        });
        out
    }

    /// Active intentions whose deadline is at or before `t`, earliest deadline first.
    pub fn due_before(&self, t: Timestamp) -> Vec<&ManagedIntention> {
        let mut out: Vec<&ManagedIntention> = self
            .items
            .values()
            .filter(|m| m.status().is_active())
            .filter(|m| m.intention().deadline.is_some_and(|d| d <= t))
            .collect();
        out.sort_by_key(|m| (m.intention().deadline, m.id()));
        out
    }

    /// Active intentions already past their deadline at `now`, earliest deadline first.
    pub fn overdue(&self, now: Timestamp) -> Vec<&ManagedIntention> {
        let mut out: Vec<&ManagedIntention> =
            self.items.values().filter(|m| m.is_overdue(now)).collect();
        out.sort_by_key(|m| (m.intention().deadline, m.id()));
        out
    }

    /// The earliest deadline among active intentions, if any.
    pub fn next_deadline(&self) -> Option<Timestamp> {
        self.items
            .values()
            .filter(|m| m.status().is_active())
            .filter_map(|m| m.intention().deadline)
            .min()
    }
}

impl From<IntentionSet> for Vec<ManagedIntention> {
    fn from(set: IntentionSet) -> Self {
        set.items.into_values().collect()
    }
}

impl TryFrom<Vec<ManagedIntention>> for IntentionSet {
    type Error = SetError;

    fn try_from(items: Vec<ManagedIntention>) -> Result<Self, Self::Error> {
        let mut set = Self::new();
        for item in items {
            set.insert(item)?;
        }
        Ok(set)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{ids, intention};
    use tpt_thought_core::{IdGenerator, Intention};

    fn managed(
        ids: &IdGenerator,
        goal: &str,
        priority: f64,
        deadline: Option<i64>,
    ) -> ManagedIntention {
        let mut i: Intention = intention(ids, goal, priority);
        i.deadline = deadline.map(Timestamp);
        ManagedIntention::new(i, vec![]).unwrap()
    }

    #[test]
    fn orders_by_priority_then_id() {
        let ids = ids();
        let mut set = IntentionSet::new();
        let low = managed(&ids, "low", 0.1, None);
        let high = managed(&ids, "high", 0.9, None);
        let mid_a = managed(&ids, "mid", 0.5, None);
        let mid_b = managed(&ids, "mid", 0.5, None);
        let (low_id, high_id) = (low.id(), high.id());
        for m in [low, high, mid_a, mid_b] {
            set.insert(m).unwrap();
        }
        let ordered = set.by_priority();
        assert_eq!(ordered[0].id(), high_id);
        assert_eq!(ordered[3].id(), low_id);
        let mids: Vec<IntentionId> = ordered[1..3].iter().map(|m| m.id()).collect();
        assert!(mids[0] < mids[1]);
    }

    #[test]
    fn priority_queries_skip_terminal_intentions() {
        let ids = ids();
        let mut set = IntentionSet::new();
        let m = managed(&ids, "done", 1.0, None);
        let id = m.id();
        set.insert(m).unwrap();
        set.advance(&id, IntentionStatus::Abandoned).unwrap();
        assert!(set.by_priority().is_empty());
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn duplicate_and_unknown_ids_are_errors() {
        let ids = ids();
        let mut set = IntentionSet::new();
        let m = managed(&ids, "x", 1.0, None);
        let id = m.id();
        set.insert(m.clone()).unwrap();
        assert_eq!(set.insert(m), Err(SetError::Duplicate(id)));
        let other = ids.intention();
        assert_eq!(
            set.advance(&other, IntentionStatus::Dispatched),
            Err(SetError::Unknown(other))
        );
    }

    #[test]
    fn illegal_transition_through_set_is_reported() {
        let ids = ids();
        let mut set = IntentionSet::new();
        let m = managed(&ids, "x", 1.0, None);
        let id = m.id();
        set.insert(m).unwrap();
        let err = set.advance(&id, IntentionStatus::Achieved).unwrap_err();
        assert!(matches!(err, SetError::Transition(_)));
        assert_eq!(set.get(&id).unwrap().status(), IntentionStatus::Formed);
    }

    #[test]
    fn deadline_queries() {
        let ids = ids();
        let mut set = IntentionSet::new();
        let late = managed(&ids, "late", 1.0, Some(300));
        let early = managed(&ids, "early", 1.0, Some(100));
        let none = managed(&ids, "none", 1.0, None);
        let done = managed(&ids, "done", 1.0, Some(50));
        let done_id = done.id();
        for m in [late, early, none, done] {
            set.insert(m).unwrap();
        }
        set.advance(&done_id, IntentionStatus::Abandoned).unwrap();

        assert_eq!(set.next_deadline(), Some(Timestamp(100)));

        let due: Vec<Option<Timestamp>> = set
            .due_before(Timestamp(200))
            .iter()
            .map(|m| m.intention().deadline)
            .collect();
        assert_eq!(due, vec![Some(Timestamp(100))]);

        let overdue: Vec<Option<Timestamp>> = set
            .overdue(Timestamp(301))
            .iter()
            .map(|m| m.intention().deadline)
            .collect();
        assert_eq!(overdue, vec![Some(Timestamp(100)), Some(Timestamp(300))]);
    }

    #[test]
    fn serde_round_trip_and_duplicate_rejection() {
        let ids = ids();
        let mut set = IntentionSet::new();
        set.insert(managed(&ids, "a", 1.0, Some(5))).unwrap();
        set.insert(managed(&ids, "b", 2.0, None)).unwrap();
        let json = serde_json::to_string(&set).unwrap();
        let back: IntentionSet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, set);

        let one = managed(&ids, "c", 1.0, None);
        let dup = serde_json::to_string(&vec![one.clone(), one]).unwrap();
        assert!(serde_json::from_str::<IntentionSet>(&dup).is_err());
    }
}
