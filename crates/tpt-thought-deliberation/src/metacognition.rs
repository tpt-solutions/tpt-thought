// SPDX-License-Identifier: MIT OR Apache-2.0
//! Metacognition consumed from an engine (spec section 19): how certain the engine is, what it
//! does not know, and what it recommends doing next.

use serde::{Deserialize, Serialize};
use tpt_thought_core::Probability;

/// The engine's recommendation after reflecting on its own state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NextAction {
    /// Keep deliberating; the answer is not yet adequate.
    Continue,
    /// The answer is adequate; act on it.
    Act,
    /// Deliberation cannot settle the question; ask a human or another provider for help.
    AskForHelp,
}

/// Self-assessment returned by an engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metacognition {
    /// Overall certainty in the current answer, if the engine states one.
    pub certainty: Option<Probability>,
    /// Questions the engine could not answer.
    pub gaps: Vec<String>,
    /// Assumptions the current answer depends on.
    pub assumptions: Vec<String>,
    /// Evidence that would settle the question if it were available.
    pub missing_evidence: Vec<String>,
    /// The recommended next step.
    pub next_action: NextAction,
}

impl Metacognition {
    /// A self-assessment with no certainty, gaps, assumptions or missing evidence recorded.
    pub fn new(next_action: NextAction) -> Self {
        Self {
            certainty: None,
            gaps: Vec::new(),
            assumptions: Vec::new(),
            missing_evidence: Vec::new(),
            next_action,
        }
    }

    /// True if the engine recommends asking for help.
    pub fn needs_help(&self) -> bool {
        self.next_action == NextAction::AskForHelp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_serde() {
        let m = Metacognition {
            certainty: Probability::new(0.4).ok(),
            gaps: vec!["sensor drift".into()],
            assumptions: vec!["static scene".into()],
            missing_evidence: vec!["second reading".into()],
            next_action: NextAction::AskForHelp,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: Metacognition = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
        assert!(back.needs_help());
        assert!(json.contains("\"ask_for_help\""));
    }
}
