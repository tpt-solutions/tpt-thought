// SPDX-License-Identifier: MIT OR Apache-2.0
//! The modulatory state (spec section 80) and its mapping to cognitive modes (section 82).

use crate::mode::CognitiveMode;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Raw modulatory values as supplied or serialised. [`Modulation::new`] validates every field.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModulationFields {
    /// Time pressure.
    pub urgency: f64,
    /// Risk if the current course is wrong.
    pub risk: f64,
    /// How unfamiliar the situation is.
    pub novelty: f64,
    /// Expected reward of the current goal.
    pub reward: f64,
    /// Perceived threat.
    pub threat: f64,
    /// Confidence in the current model.
    pub confidence: f64,
    /// Residual uncertainty.
    pub uncertainty: f64,
    /// Overall arousal.
    pub arousal: f64,
}

/// A modulatory field was outside `0.0..=1.0` or not finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModulationError {
    /// The offending field.
    pub field: &'static str,
    /// Its value.
    pub value: f64,
}

impl fmt::Display for ModulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "modulation field `{}` is {}, expected 0.0..=1.0",
            self.field, self.value
        )
    }
}

impl std::error::Error for ModulationError {}

/// Validated modulatory state. Every field is in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ModulationFields", into = "ModulationFields")]
pub struct Modulation {
    fields: ModulationFields,
}

impl Modulation {
    /// Validate and wrap the fields.
    pub fn new(fields: ModulationFields) -> Result<Self, ModulationError> {
        let checks = [
            ("urgency", fields.urgency),
            ("risk", fields.risk),
            ("novelty", fields.novelty),
            ("reward", fields.reward),
            ("threat", fields.threat),
            ("confidence", fields.confidence),
            ("uncertainty", fields.uncertainty),
            ("arousal", fields.arousal),
        ];
        for (field, value) in checks {
            if !(0.0..=1.0).contains(&value) {
                return Err(ModulationError { field, value });
            }
        }
        Ok(Self { fields })
    }

    /// Every field at `0.5`.
    pub fn neutral() -> Self {
        Self {
            fields: ModulationFields {
                urgency: 0.5,
                risk: 0.5,
                novelty: 0.5,
                reward: 0.5,
                threat: 0.5,
                confidence: 0.5,
                uncertainty: 0.5,
                arousal: 0.5,
            },
        }
    }

    /// The raw fields.
    pub fn fields(&self) -> ModulationFields {
        self.fields
    }

    /// Choose the cognitive mode for a cycle. The rules are checked in order and the first match
    /// wins, so the mapping is deterministic.
    ///
    /// 1. Threat, or urgency with high risk: [`CognitiveMode::Emergency`].
    /// 2. High uncertainty with novelty: [`CognitiveMode::Investigative`].
    /// 3. Very low confidence: [`CognitiveMode::Reflective`].
    /// 4. High novelty with low risk: [`CognitiveMode::Creative`].
    /// 5. High arousal with low uncertainty: [`CognitiveMode::Reactive`].
    /// 6. Reward without urgency: [`CognitiveMode::Planning`].
    /// 7. Almost no stimulation or pressure: [`CognitiveMode::Consolidation`].
    /// 8. Otherwise: [`CognitiveMode::Deliberative`].
    pub fn select_mode(&self) -> CognitiveMode {
        let f = &self.fields;
        if f.threat >= 0.9 || (f.urgency >= 0.9 && f.risk >= 0.8) {
            CognitiveMode::Emergency
        } else if f.uncertainty >= 0.6 && f.novelty >= 0.5 {
            CognitiveMode::Investigative
        } else if f.confidence < 0.3 {
            CognitiveMode::Reflective
        } else if f.novelty >= 0.8 && f.risk < 0.3 {
            CognitiveMode::Creative
        } else if f.arousal >= 0.8 && f.uncertainty < 0.3 {
            CognitiveMode::Reactive
        } else if f.reward >= 0.5 && f.urgency < 0.5 {
            CognitiveMode::Planning
        } else if f.arousal < 0.2 && f.urgency < 0.2 {
            CognitiveMode::Consolidation
        } else {
            CognitiveMode::Deliberative
        }
    }
}

impl TryFrom<ModulationFields> for Modulation {
    type Error = ModulationError;

    fn try_from(fields: ModulationFields) -> Result<Self, Self::Error> {
        Self::new(fields)
    }
}

impl From<Modulation> for ModulationFields {
    fn from(m: Modulation) -> Self {
        m.fields
    }
}
