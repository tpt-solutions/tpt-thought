// SPDX-License-Identifier: MIT OR Apache-2.0
use serde::{Deserialize, Serialize};
use tpt_thought_core::ThoughtId;

/// A raw signal that something may deserve reflection. The engine turns signals into a
/// [`ReflectionTrigger`] using its policy thresholds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSignal {
    /// A step failed or raised an error.
    Error {
        /// The thought in which the error occurred.
        thought: ThoughtId,
    },
    /// A thought's uncertainty, in `0.0..=1.0`.
    Uncertainty {
        /// The uncertain thought.
        thought: ThoughtId,
        /// How uncertain it is.
        uncertainty: f64,
    },
    /// Two thoughts disagree.
    Contradiction {
        /// One side of the contradiction.
        a: ThoughtId,
        /// The other side.
        b: ThoughtId,
    },
    /// The goal behind a thought changed.
    GoalChange {
        /// The thought whose goal changed.
        thought: ThoughtId,
    },
    /// An observed outcome differed from the prediction.
    OutcomeMismatch {
        /// The thought that made the prediction.
        thought: ThoughtId,
        /// Magnitude of the prediction error, in `0.0..=1.0`.
        error: f64,
    },
}

/// Why a reflection session was started, and what it reflects on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReflectionTrigger {
    /// An error occurred.
    Error {
        /// Thoughts to reflect on.
        targets: Vec<ThoughtId>,
    },
    /// Uncertainty was above threshold.
    Uncertainty {
        /// Thoughts to reflect on.
        targets: Vec<ThoughtId>,
        /// The highest uncertainty that fired the trigger.
        uncertainty: f64,
    },
    /// Thoughts contradict each other.
    Contradiction {
        /// The two sides.
        targets: Vec<ThoughtId>,
    },
    /// A goal changed.
    GoalChange {
        /// Thoughts to reflect on.
        targets: Vec<ThoughtId>,
    },
    /// An outcome mismatched its prediction.
    OutcomeMismatch {
        /// Thoughts to reflect on.
        targets: Vec<ThoughtId>,
        /// The error magnitude that fired the trigger.
        error: f64,
    },
    /// The periodic schedule came due.
    Periodic {
        /// Recent thoughts to reflect on.
        targets: Vec<ThoughtId>,
    },
}

impl ReflectionTrigger {
    /// The thoughts this reflection covers, in review order.
    pub fn targets(&self) -> &[ThoughtId] {
        match self {
            Self::Error { targets }
            | Self::Uncertainty { targets, .. }
            | Self::Contradiction { targets }
            | Self::GoalChange { targets }
            | Self::OutcomeMismatch { targets, .. }
            | Self::Periodic { targets } => targets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    #[test]
    fn targets_are_exposed_for_every_variant() {
        let ids = IdGenerator::new(SeededEntropy::new(7));
        let t = ids.thought();
        let trigger = ReflectionTrigger::Uncertainty {
            targets: vec![t],
            uncertainty: 0.9,
        };
        assert_eq!(trigger.targets(), &[t]);
    }
}
