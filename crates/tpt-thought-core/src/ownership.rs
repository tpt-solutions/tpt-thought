// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thought ownership (spec section 40). Identity and permissions are delegated to Anima ZKPA; this
//! crate records the state, the rules for moving between states, and the agency ledger.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::ThoughtId;
use crate::time::Timestamp;

/// Who may see or act on a thought.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThoughtOwnership {
    /// Visible only to the owning agent.
    #[default]
    Private,
    /// Shared with other agents.
    Shared,
    /// Responsibility handed to another agent.
    Delegated {
        /// Opaque identity of the delegate (resolved by the identity provider).
        to: String,
    },
    /// Published for general consumption.
    Published,
    /// Retained read-only.
    Archived,
}

impl ThoughtOwnership {
    /// Whether ownership may move from this state to `next`.
    ///
    /// - Any state may move to `Archived`.
    /// - `Archived` is terminal.
    /// - `Published` may only move to `Archived`: a published thought cannot be withdrawn.
    /// - Any other move is allowed, including handing a thought to a different delegate.
    /// - A move to the same state is not a change, so it is refused.
    pub fn can_transition_to(&self, next: &Self) -> bool {
        match (self, next) {
            (Self::Archived, _) => false,
            (_, Self::Archived) => true,
            (Self::Published, _) => false,
            (current, next) => current != next,
        }
    }
}

/// One change of ownership state, recorded by the agency ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgencyEntry {
    /// The thought whose ownership changed.
    pub thought: ThoughtId,
    /// The state before the change. Must match the state the ledger holds.
    pub from: ThoughtOwnership,
    /// The state after the change.
    pub to: ThoughtOwnership,
    /// Opaque identity of the agent that made the change.
    pub actor: String,
    /// Why the actor was allowed to make the change, as a verification marker (for example a
    /// ZKPA `zkpa:...` marker). Empty when the caller did not check authority.
    pub basis: String,
    /// When the change was made.
    pub at: Timestamp,
}

/// Why the agency ledger refused a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// The thought has already been opened.
    AlreadyOpened(ThoughtId),
    /// The thought has not been opened, so it has no owner to record a change against.
    NotOpened(ThoughtId),
    /// The entry's `from` state is not the state the ledger holds.
    StaleState {
        /// The state the ledger holds.
        held: ThoughtOwnership,
        /// The state the entry claims to start from.
        claimed: ThoughtOwnership,
    },
    /// The move is not allowed by [`ThoughtOwnership::can_transition_to`].
    IllegalTransition {
        /// The state before the change.
        from: ThoughtOwnership,
        /// The refused state after the change.
        to: ThoughtOwnership,
    },
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyOpened(id) => write!(f, "thought {id:?} is already in the agency ledger"),
            Self::NotOpened(id) => write!(f, "thought {id:?} is not in the agency ledger"),
            Self::StaleState { held, claimed } => {
                write!(
                    f,
                    "ledger holds {held:?} but the change starts from {claimed:?}"
                )
            }
            Self::IllegalTransition { from, to } => {
                write!(f, "ownership cannot move from {from:?} to {to:?}")
            }
        }
    }
}

impl std::error::Error for LedgerError {}

/// Append-only record of who owns each thought and every change of ownership state.
///
/// The ledger does not check authority: callers decide who may act (for example through
/// `tpt-thought-integrations`' ZKPA adapter) and record the basis with each entry. The ledger
/// checks only that the history is consistent: each thought is opened once, each change starts from
/// the state the ledger holds, and each move is allowed by [`ThoughtOwnership::can_transition_to`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgencyLedger {
    owners: BTreeMap<ThoughtId, String>,
    entries: Vec<AgencyEntry>,
}

impl AgencyLedger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open `thought` with `owner` as its creating agent. The thought starts `Private`.
    pub fn open(
        &mut self,
        thought: ThoughtId,
        owner: impl Into<String>,
    ) -> Result<(), LedgerError> {
        if self.owners.contains_key(&thought) {
            return Err(LedgerError::AlreadyOpened(thought));
        }
        self.owners.insert(thought, owner.into());
        Ok(())
    }

    /// The agent that opened `thought`, if it is in the ledger.
    pub fn owner(&self, thought: ThoughtId) -> Option<&str> {
        self.owners.get(&thought).map(String::as_str)
    }

    /// The state the ledger holds for `thought`. An opened thought with no changes is `Private`.
    pub fn current(&self, thought: ThoughtId) -> Option<ThoughtOwnership> {
        if !self.owners.contains_key(&thought) {
            return None;
        }
        Some(
            self.last_entry(thought)
                .map(|entry| entry.to.clone())
                .unwrap_or_default(),
        )
    }

    /// The most recent change to `thought`, if any.
    pub fn last_entry(&self, thought: ThoughtId) -> Option<&AgencyEntry> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.thought == thought)
    }

    /// Every change to `thought`, oldest first.
    pub fn history(&self, thought: ThoughtId) -> impl Iterator<Item = &AgencyEntry> {
        self.entries
            .iter()
            .filter(move |entry| entry.thought == thought)
    }

    /// Append a change of ownership state.
    ///
    /// # Errors
    ///
    /// [`LedgerError`] when the thought is not open, the entry does not start from the held state,
    /// or the move is not allowed. A refused entry is not recorded.
    pub fn record(&mut self, entry: AgencyEntry) -> Result<(), LedgerError> {
        let held = self
            .current(entry.thought)
            .ok_or(LedgerError::NotOpened(entry.thought))?;
        if entry.from != held {
            return Err(LedgerError::StaleState {
                held,
                claimed: entry.from,
            });
        }
        if !entry.from.can_transition_to(&entry.to) {
            return Err(LedgerError::IllegalTransition {
                from: entry.from,
                to: entry.to,
            });
        }
        self.entries.push(entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{IdGenerator, SeededEntropy};

    fn thought() -> ThoughtId {
        IdGenerator::new(SeededEntropy::new(7)).thought()
    }

    fn delegated(to: &str) -> ThoughtOwnership {
        ThoughtOwnership::Delegated { to: to.into() }
    }

    fn change(
        thought: ThoughtId,
        from: ThoughtOwnership,
        to: ThoughtOwnership,
        actor: &str,
    ) -> AgencyEntry {
        AgencyEntry {
            thought,
            from,
            to,
            actor: actor.into(),
            basis: "test".into(),
            at: Timestamp(1),
        }
    }

    #[test]
    fn archived_is_terminal_and_published_only_archives() {
        use ThoughtOwnership::*;
        assert!(!Archived.can_transition_to(&Private));
        assert!(!Published.can_transition_to(&Shared));
        assert!(!Published.can_transition_to(&Private));
        assert!(Published.can_transition_to(&Archived));
        assert!(Private.can_transition_to(&Archived));
        assert!(delegated("a").can_transition_to(&delegated("b")));
    }

    #[test]
    fn a_move_to_the_same_state_is_refused() {
        assert!(!ThoughtOwnership::Shared.can_transition_to(&ThoughtOwnership::Shared));
        assert!(!delegated("a").can_transition_to(&delegated("a")));
    }

    #[test]
    fn an_opened_thought_starts_private_and_remembers_its_owner() {
        let mut ledger = AgencyLedger::new();
        let id = thought();
        ledger.open(id, "agent-1").unwrap();
        assert_eq!(ledger.owner(id), Some("agent-1"));
        assert_eq!(ledger.current(id), Some(ThoughtOwnership::Private));
        assert_eq!(
            ledger.open(id, "agent-2"),
            Err(LedgerError::AlreadyOpened(id))
        );
    }

    #[test]
    fn unopened_thoughts_cannot_change() {
        let mut ledger = AgencyLedger::new();
        let id = thought();
        assert_eq!(ledger.current(id), None);
        let entry = change(id, ThoughtOwnership::Private, ThoughtOwnership::Shared, "a");
        assert_eq!(ledger.record(entry), Err(LedgerError::NotOpened(id)));
    }

    #[test]
    fn changes_are_recorded_in_order_and_chain_from_the_held_state() {
        let mut ledger = AgencyLedger::new();
        let id = thought();
        ledger.open(id, "agent-1").unwrap();
        ledger
            .record(change(
                id,
                ThoughtOwnership::Private,
                delegated("agent-2"),
                "agent-1",
            ))
            .unwrap();
        ledger
            .record(change(
                id,
                delegated("agent-2"),
                ThoughtOwnership::Published,
                "agent-2",
            ))
            .unwrap();
        assert_eq!(ledger.current(id), Some(ThoughtOwnership::Published));
        assert_eq!(ledger.history(id).count(), 2);
        assert_eq!(
            ledger.last_entry(id).map(|e| e.actor.as_str()),
            Some("agent-2")
        );
    }

    #[test]
    fn a_stale_or_illegal_change_is_refused_and_not_recorded() {
        let mut ledger = AgencyLedger::new();
        let id = thought();
        ledger.open(id, "agent-1").unwrap();

        let stale = change(
            id,
            ThoughtOwnership::Shared,
            ThoughtOwnership::Published,
            "a",
        );
        assert!(matches!(
            ledger.record(stale),
            Err(LedgerError::StaleState { .. })
        ));

        ledger
            .record(change(
                id,
                ThoughtOwnership::Private,
                ThoughtOwnership::Published,
                "a",
            ))
            .unwrap();
        let illegal = change(
            id,
            ThoughtOwnership::Published,
            ThoughtOwnership::Private,
            "a",
        );
        assert!(matches!(
            ledger.record(illegal),
            Err(LedgerError::IllegalTransition { .. })
        ));
        assert_eq!(ledger.history(id).count(), 1);
    }
}
