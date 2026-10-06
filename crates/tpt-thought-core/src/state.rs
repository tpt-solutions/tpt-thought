// SPDX-License-Identifier: MIT OR Apache-2.0
//! The thought lifecycle states (spec section 7). Legal transitions live in `tpt-thought-state`.

use serde::{Deserialize, Serialize};

/// Lifecycle state of a thought.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThoughtState {
    /// Constructed.
    Created,
    /// Input received.
    Received,
    /// Being attended to.
    Attending,
    /// Gathering context.
    Contextualising,
    /// Associating with memories.
    Associating,
    /// Forming hypotheses.
    Hypothesising,
    /// Evaluating alternatives.
    Evaluating,
    /// Conclusion accepted.
    Accepted,
    /// Conclusion uncertain.
    Uncertain,
    /// Gathering more evidence.
    Investigating,
    /// Re-evaluating after investigation.
    ReEvaluating,
    /// Forming an intention.
    Intending,
    /// Executing an action.
    Acting,
    /// Observing the result.
    Observing,
    /// Learning from the outcome.
    Learning,
    /// Consolidating into memory.
    Consolidating,
    /// Finished.
    Complete,
}

impl ThoughtState {
    /// Every state, in lifecycle order.
    pub const ALL: [ThoughtState; 17] = [
        Self::Created,
        Self::Received,
        Self::Attending,
        Self::Contextualising,
        Self::Associating,
        Self::Hypothesising,
        Self::Evaluating,
        Self::Accepted,
        Self::Uncertain,
        Self::Investigating,
        Self::ReEvaluating,
        Self::Intending,
        Self::Acting,
        Self::Observing,
        Self::Learning,
        Self::Consolidating,
        Self::Complete,
    ];

    /// True for the terminal state.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Complete)
    }
}
