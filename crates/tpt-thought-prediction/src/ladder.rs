// SPDX-License-Identifier: MIT OR Apache-2.0
//! The configurable error response ladder (spec section 18).

use crate::error::PredictionError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// How large a prediction error is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorLevel {
    /// Within tolerance: continue as planned.
    Small,
    /// Noticeable: update the belief the prediction rested on.
    Moderate,
    /// Large: raise attention and investigate.
    Large,
    /// Catastrophic: suspend the model and re-evaluate its assumptions.
    Catastrophic,
}

/// A response the cognitive system takes to a prediction error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseAction {
    /// Carry on unchanged.
    Continue,
    /// Revise the belief the prediction was based on.
    UpdateBelief,
    /// Raise attention on the surprising input.
    RaiseAttention,
    /// Start an investigation.
    Investigate,
    /// Stop relying on the model that produced the prediction.
    SuspendModel,
    /// Re-examine the assumptions the prediction rested on.
    ReevaluateAssumptions,
}

/// The thresholds were not ordered within `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LadderError {
    /// The rejected thresholds, as `(moderate, large, catastrophic)`.
    pub thresholds: (f64, f64, f64),
}

impl fmt::Display for LadderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "thresholds must satisfy 0 <= moderate <= large <= catastrophic <= 1, got {:?}",
            self.thresholds
        )
    }
}

impl std::error::Error for LadderError {}

/// Maps error magnitudes to levels and levels to responses.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ResponseLadder {
    moderate: f64,
    large: f64,
    catastrophic: f64,
}

impl Default for ResponseLadder {
    fn default() -> Self {
        Self {
            moderate: 0.1,
            large: 0.4,
            catastrophic: 0.8,
        }
    }
}

impl ResponseLadder {
    /// Build a ladder. A magnitude is `Small` below `moderate`, and so on up to `Catastrophic`
    /// at or above `catastrophic`.
    pub fn new(moderate: f64, large: f64, catastrophic: f64) -> Result<Self, LadderError> {
        let ok = [moderate, large, catastrophic]
            .iter()
            .all(|t| t.is_finite() && (0.0..=1.0).contains(t))
            && moderate <= large
            && large <= catastrophic;
        if ok {
            Ok(Self {
                moderate,
                large,
                catastrophic,
            })
        } else {
            Err(LadderError {
                thresholds: (moderate, large, catastrophic),
            })
        }
    }

    /// The level for a magnitude in `0.0..=1.0`.
    pub fn classify(&self, magnitude: f64) -> ErrorLevel {
        if magnitude >= self.catastrophic {
            ErrorLevel::Catastrophic
        } else if magnitude >= self.large {
            ErrorLevel::Large
        } else if magnitude >= self.moderate {
            ErrorLevel::Moderate
        } else {
            ErrorLevel::Small
        }
    }

    /// The response plan for an error.
    pub fn plan(&self, error: &PredictionError) -> ResponsePlan {
        let level = self.classify(error.magnitude());
        let actions = match level {
            ErrorLevel::Small => vec![ResponseAction::Continue],
            ErrorLevel::Moderate => vec![ResponseAction::UpdateBelief],
            ErrorLevel::Large => vec![ResponseAction::RaiseAttention, ResponseAction::Investigate],
            ErrorLevel::Catastrophic => vec![
                ResponseAction::SuspendModel,
                ResponseAction::ReevaluateAssumptions,
            ],
        };
        ResponsePlan { level, actions }
    }
}

/// The level reached and the actions it calls for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponsePlan {
    /// The error level.
    pub level: ErrorLevel,
    /// Actions to take, in order.
    pub actions: Vec<ResponseAction>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::PredictionError;
    use tpt_thought_core::{IdGenerator, SeededEntropy, Timestamp};

    fn error_with(magnitude: f64) -> PredictionError {
        let ids = IdGenerator::new(SeededEntropy::new(3));
        PredictionError {
            prediction: ids.prediction(),
            observation: ids.observation(),
            source_thought: ids.thought(),
            content_error: magnitude,
            timing_error: 0.0,
            timing_offset_nanos: None,
            observed_at: Timestamp(0),
        }
    }

    #[test]
    fn thresholds_must_be_ordered_and_in_range() {
        assert!(ResponseLadder::new(0.1, 0.4, 0.8).is_ok());
        assert!(ResponseLadder::new(0.5, 0.4, 0.8).is_err());
        assert!(ResponseLadder::new(0.1, 0.4, 1.2).is_err());
        assert!(ResponseLadder::new(f64::NAN, 0.4, 0.8).is_err());
    }

    #[test]
    fn boundaries_belong_to_the_higher_level() {
        let l = ResponseLadder::new(0.1, 0.4, 0.8).unwrap();
        assert_eq!(l.classify(0.0), ErrorLevel::Small);
        assert_eq!(l.classify(0.099), ErrorLevel::Small);
        assert_eq!(l.classify(0.1), ErrorLevel::Moderate);
        assert_eq!(l.classify(0.4), ErrorLevel::Large);
        assert_eq!(l.classify(0.8), ErrorLevel::Catastrophic);
        assert_eq!(l.classify(1.0), ErrorLevel::Catastrophic);
    }

    #[test]
    fn each_level_maps_to_its_response() {
        let l = ResponseLadder::default();
        assert_eq!(
            l.plan(&error_with(0.0)).actions,
            vec![ResponseAction::Continue]
        );
        assert_eq!(
            l.plan(&error_with(0.2)).actions,
            vec![ResponseAction::UpdateBelief]
        );
        assert_eq!(
            l.plan(&error_with(0.5)).actions,
            vec![ResponseAction::RaiseAttention, ResponseAction::Investigate]
        );
        let cat = l.plan(&error_with(0.9));
        assert_eq!(cat.level, ErrorLevel::Catastrophic);
        assert_eq!(
            cat.actions,
            vec![
                ResponseAction::SuspendModel,
                ResponseAction::ReevaluateAssumptions
            ]
        );
    }

    #[test]
    fn custom_thresholds_shift_the_ladder() {
        let strict = ResponseLadder::new(0.0, 0.0, 0.5).unwrap();
        assert_eq!(strict.classify(0.01), ErrorLevel::Large);
        assert_eq!(strict.classify(0.5), ErrorLevel::Catastrophic);
    }
}
