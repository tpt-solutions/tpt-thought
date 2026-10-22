// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cognitive modes (spec section 82).

use serde::{Deserialize, Serialize};

/// How the cognitive loop is working. The modulatory state selects a mode for each cycle (see
/// [`crate::Modulation::select_mode`]); the loop enters [`CognitiveMode::Learning`] itself after
/// a prediction error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CognitiveMode {
    /// Fast, habitual response.
    Reactive,
    /// Weighing providers' answers.
    Deliberative,
    /// Searching for missing information.
    Investigative,
    /// Generating novel options.
    Creative,
    /// Examining the model and its assumptions.
    Reflective,
    /// Goal-directed plan construction.
    Planning,
    /// Updating beliefs from a prediction error.
    Learning,
    /// Offline consolidation of experience.
    Consolidation,
    /// Immediate, risk-limited response to a threat or deadline.
    Emergency,
}

impl CognitiveMode {
    /// A stable name for telemetry and trajectories.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Reactive => "reactive",
            Self::Deliberative => "deliberative",
            Self::Investigative => "investigative",
            Self::Creative => "creative",
            Self::Reflective => "reflective",
            Self::Planning => "planning",
            Self::Learning => "learning",
            Self::Consolidation => "consolidation",
            Self::Emergency => "emergency",
        }
    }
}
