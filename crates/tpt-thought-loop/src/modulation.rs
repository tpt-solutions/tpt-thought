// SPDX-License-Identifier: MIT OR Apache-2.0
//! The modulatory state (spec section 80) and its mapping to cognitive modes (section 82).

use crate::mode::CognitiveMode;
use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_consolidation::ResourceSnapshot;

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

impl Modulation {
    /// The modulation a cycle sees under the resource state `snapshot` (spec sections 48.3 and 81).
    ///
    /// Only arousal changes. Its value is raised to the [`resource_pressure`] when that is higher,
    /// and never lowered. Resource strain then pushes an idle or deliberative cycle toward the
    /// cheaper modes that run on the fast path (Reactive), while urgency, risk, uncertainty and
    /// confidence still come from the thought. Uncertainty keeps its own rule, so a thought that
    /// is unsure still investigates under pressure.
    pub fn under_pressure(&self, snapshot: &ResourceSnapshot) -> Self {
        let mut fields = self.fields;
        fields.arousal = fields.arousal.max(resource_pressure(snapshot));
        Self { fields }
    }
}

/// The strain on resources, from `0.0` (none) to `1.0`: the largest of compute scarcity
/// (`1 - compute_free`), energy scarcity (`1 - energy_available`) and `memory_pressure`. A reading
/// that is not finite is no evidence of strain and is ignored.
pub fn resource_pressure(snapshot: &ResourceSnapshot) -> f64 {
    [
        1.0 - snapshot.compute_free,
        1.0 - snapshot.energy_available,
        snapshot.memory_pressure,
    ]
    .into_iter()
    .filter(|pressure| pressure.is_finite())
    .fold(0.0, |strain, pressure| strain.max(pressure.clamp(0.0, 1.0)))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn resources(
        compute_free: f64,
        memory_pressure: f64,
        energy_available: f64,
    ) -> ResourceSnapshot {
        ResourceSnapshot {
            compute_free,
            memory_pressure,
            energy_available,
        }
    }

    fn idle() -> Modulation {
        Modulation::new(ModulationFields {
            urgency: 0.1,
            risk: 0.1,
            novelty: 0.1,
            reward: 0.1,
            threat: 0.1,
            confidence: 0.5,
            uncertainty: 0.1,
            arousal: 0.1,
        })
        .expect("valid modulation")
    }

    #[test]
    fn pressure_is_the_largest_strain() {
        assert_eq!(resource_pressure(&resources(1.0, 0.0, 1.0)), 0.0);
        assert!((resource_pressure(&resources(0.25, 0.1, 0.9)) - 0.75).abs() < 1e-9);
        assert!((resource_pressure(&resources(0.9, 0.1, 0.4)) - 0.6).abs() < 1e-9);
        assert!((resource_pressure(&resources(1.0, 0.7, 1.0)) - 0.7).abs() < 1e-9);
    }

    #[test]
    fn out_of_range_and_non_finite_readings_are_bounded() {
        assert_eq!(resource_pressure(&resources(2.0, 0.0, 1.0)), 0.0);
        assert_eq!(resource_pressure(&resources(-1.0, 9.0, 1.0)), 1.0);
        assert_eq!(resource_pressure(&resources(f64::NAN, 0.0, 1.0)), 0.0);
        assert_eq!(resource_pressure(&resources(1.0, f64::INFINITY, 1.0)), 0.0);
    }

    #[test]
    fn full_resources_leave_the_modulation_unchanged() {
        let base = idle();
        assert_eq!(base.under_pressure(&resources(1.0, 0.0, 1.0)), base);
    }

    #[test]
    fn pressure_raises_arousal_and_nothing_else() {
        let pressed = idle().under_pressure(&resources(0.1, 0.0, 1.0));
        let before = idle().fields();
        let after = pressed.fields();
        assert!((after.arousal - 0.9).abs() < 1e-9);
        assert_eq!(
            ModulationFields {
                arousal: before.arousal,
                ..after
            },
            before,
            "only arousal may change"
        );
        let calm = Modulation::new(ModulationFields {
            arousal: 0.7,
            ..idle().fields()
        })
        .expect("valid modulation");
        assert_eq!(
            calm.under_pressure(&resources(0.9, 0.2, 1.0))
                .fields()
                .arousal,
            0.7,
            "pressure never lowers arousal"
        );
    }

    #[test]
    fn pressure_moves_an_idle_cycle_to_the_fast_path() {
        assert_eq!(idle().select_mode(), CognitiveMode::Consolidation);
        assert_eq!(
            idle()
                .under_pressure(&resources(0.1, 0.0, 1.0))
                .select_mode(),
            CognitiveMode::Reactive
        );
        assert_eq!(
            idle()
                .under_pressure(&resources(1.0, 0.0, 0.1))
                .select_mode(),
            CognitiveMode::Reactive,
            "energy scarcity counts as pressure"
        );
    }

    #[test]
    fn mild_pressure_leaves_an_idle_cycle_idle() {
        assert_eq!(
            idle()
                .under_pressure(&resources(1.0, 0.1, 1.0))
                .select_mode(),
            CognitiveMode::Consolidation,
            "arousal 0.1 is still below 0.2"
        );
        assert_eq!(
            idle()
                .under_pressure(&resources(1.0, 0.3, 1.0))
                .select_mode(),
            CognitiveMode::Deliberative,
            "arousal 0.3 is no longer below 0.2, so consolidation no longer applies"
        );
    }

    #[test]
    fn uncertainty_still_investigates_under_pressure() {
        let unsure = Modulation::new(ModulationFields {
            uncertainty: 0.8,
            novelty: 0.6,
            ..idle().fields()
        })
        .expect("valid modulation");
        assert_eq!(unsure.select_mode(), CognitiveMode::Investigative);
        assert_eq!(
            unsure
                .under_pressure(&resources(0.0, 1.0, 0.0))
                .select_mode(),
            CognitiveMode::Investigative,
            "resource strain does not override the uncertainty rule"
        );
    }
}
