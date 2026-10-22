// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hypothesis lifecycle (spec section 16). Transitions are a fixed table, so the same
//! `(from, to)` pair always has the same answer.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Where a hypothesis stands in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisStatus {
    /// Just proposed.
    Generated,
    /// Proposed but not yet tested.
    Untested,
    /// Under test.
    Testing,
    /// Evidence so far supports it.
    Supported,
    /// Evidence so far counts against it.
    Weakened,
    /// Revised after new evidence or context.
    Updated,
    /// Accepted as the working explanation.
    Accepted,
    /// Rejected. Terminal.
    Rejected,
    /// Parked; may be resumed.
    Suspended,
}

use HypothesisStatus as S;

/// Every permitted `(from, to)` pair.
pub const EDGES: &[(HypothesisStatus, HypothesisStatus)] = &[
    (S::Generated, S::Untested),
    (S::Generated, S::Rejected),
    (S::Untested, S::Testing),
    (S::Untested, S::Suspended),
    (S::Untested, S::Rejected),
    (S::Testing, S::Supported),
    (S::Testing, S::Weakened),
    (S::Testing, S::Updated),
    (S::Testing, S::Suspended),
    (S::Testing, S::Rejected),
    (S::Supported, S::Testing),
    (S::Supported, S::Updated),
    (S::Supported, S::Accepted),
    (S::Supported, S::Weakened),
    (S::Weakened, S::Testing),
    (S::Weakened, S::Updated),
    (S::Weakened, S::Rejected),
    (S::Weakened, S::Suspended),
    (S::Updated, S::Testing),
    (S::Updated, S::Supported),
    (S::Updated, S::Weakened),
    (S::Updated, S::Accepted),
    (S::Updated, S::Rejected),
    (S::Updated, S::Suspended),
    (S::Suspended, S::Untested),
    (S::Suspended, S::Testing),
    (S::Suspended, S::Rejected),
    (S::Accepted, S::Testing),
    (S::Accepted, S::Updated),
];

/// A transition that the lifecycle table does not permit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalHypothesisTransition {
    /// Status the hypothesis was in.
    pub from: HypothesisStatus,
    /// Status that was requested.
    pub to: HypothesisStatus,
}

impl fmt::Display for IllegalHypothesisTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "hypothesis cannot move from {:?} to {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for IllegalHypothesisTransition {}

impl HypothesisStatus {
    /// True if the table permits moving from `self` to `to`.
    pub fn can_move_to(self, to: Self) -> bool {
        EDGES.contains(&(self, to))
    }

    /// Move to `to`, or report why the move is not permitted.
    pub fn transition(self, to: Self) -> Result<Self, IllegalHypothesisTransition> {
        if self.can_move_to(to) {
            Ok(to)
        } else {
            Err(IllegalHypothesisTransition { from: self, to })
        }
    }

    /// True for statuses that no transition leaves.
    pub fn is_terminal(self) -> bool {
        !EDGES.iter().any(|(from, _)| *from == self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_is_terminal() {
        assert!(S::Rejected.is_terminal());
        assert!(!S::Accepted.is_terminal());
    }

    #[test]
    fn illegal_moves_are_refused() {
        assert!(S::Generated.transition(S::Accepted).is_err());
        assert_eq!(S::Testing.transition(S::Supported), Ok(S::Supported));
    }

    #[test]
    fn table_has_no_duplicates() {
        for (i, a) in EDGES.iter().enumerate() {
            assert!(!EDGES[i + 1..].contains(a), "duplicate edge {a:?}");
        }
    }
}
