// SPDX-License-Identifier: MIT OR Apache-2.0
//! Intention lifecycle (spec section 23). Retrying a failed intention is legal, so the table
//! contains a loop; `Achieved` and `Abandoned` are terminal.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Where an intention is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentionStatus {
    /// Committed to, not yet handed to an executor.
    Formed,
    /// Handed to an executor (Anima INP). Thought does not execute it itself.
    Dispatched,
    /// The executor reports that work has started.
    Executing,
    /// The goal was reached.
    Achieved,
    /// Execution did not succeed. It may be dispatched again.
    Failed,
    /// Given up. Terminal.
    Abandoned,
}

/// Every permitted `(from, to)` pair.
pub const EDGES: &[(IntentionStatus, IntentionStatus)] = {
    use IntentionStatus as S;
    &[
        (S::Formed, S::Dispatched),
        (S::Formed, S::Abandoned),
        (S::Dispatched, S::Executing),
        (S::Dispatched, S::Failed),
        (S::Dispatched, S::Abandoned),
        (S::Executing, S::Achieved),
        (S::Executing, S::Failed),
        (S::Executing, S::Abandoned),
        (S::Failed, S::Dispatched),
        (S::Failed, S::Abandoned),
    ]
};

impl IntentionStatus {
    /// True if no further transition is possible.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Achieved | Self::Abandoned)
    }

    /// True if the intention still needs work or attention.
    pub const fn is_active(self) -> bool {
        !self.is_terminal()
    }

    /// True if `self -> to` is a permitted transition.
    pub fn can_transition_to(self, to: Self) -> bool {
        EDGES.contains(&(self, to))
    }
}

/// A transition the lifecycle does not permit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalIntentionTransition {
    /// Status the intention was in.
    pub from: IntentionStatus,
    /// Status that was requested.
    pub to: IntentionStatus,
}

impl fmt::Display for IllegalIntentionTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "intention cannot move from {:?} to {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for IllegalIntentionTransition {}

#[cfg(test)]
mod tests {
    use super::*;
    use IntentionStatus as S;

    const ALL: [S; 6] = [
        S::Formed,
        S::Dispatched,
        S::Executing,
        S::Achieved,
        S::Failed,
        S::Abandoned,
    ];

    #[test]
    fn table_matches_permitted_pairs() {
        let mut permitted = 0;
        for from in ALL {
            for to in ALL {
                let listed = EDGES.contains(&(from, to));
                assert_eq!(from.can_transition_to(to), listed);
                permitted += usize::from(listed);
            }
        }
        assert_eq!(permitted, EDGES.len());
    }

    #[test]
    fn terminal_states_have_no_exits() {
        for from in [S::Achieved, S::Abandoned] {
            assert!(from.is_terminal());
            for to in ALL {
                assert!(!from.can_transition_to(to));
            }
        }
    }

    #[test]
    fn failed_can_be_retried() {
        assert!(S::Failed.can_transition_to(S::Dispatched));
        assert!(S::Failed.is_active());
    }

    #[test]
    fn cannot_skip_dispatch() {
        assert!(!S::Formed.can_transition_to(S::Executing));
        assert!(!S::Formed.can_transition_to(S::Achieved));
    }
}
