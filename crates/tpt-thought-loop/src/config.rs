// SPDX-License-Identifier: MIT OR Apache-2.0
//! Loop configuration. Everything that affects a run is here, so a replay record can carry it.

use crate::modulation::Modulation;
use serde::{Deserialize, Serialize};
use tpt_thought_core::CognitiveBudget;
use tpt_thought_deliberation::InvestigationPolicy;
use tpt_thought_providers::DEFAULT_MIN_SAMPLES;
use tpt_thought_safety::BreakerConfig;

/// Settings for a [`crate::CognitiveLoop`]. Serialisable, so it can be recorded for replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoopConfig {
    /// Seed for identifier generation. Two runs with the same seed and inputs produce the same
    /// identifiers and therefore the same trajectory.
    pub seed: u64,
    /// The modulatory state, which selects the cognitive mode each cycle.
    pub modulation: Modulation,
    /// Reliability at which a provider's answer is accepted without further asking.
    pub accept_at: f64,
    /// Cap on providers asked in deliberative mode. `None` asks every admitted provider.
    pub deliberative_max_providers: Option<usize>,
    /// Drive at which curiosity fires.
    pub curiosity_threshold: f64,
    /// Prediction-error thresholds `[moderate, large, catastrophic]`, in ascending order.
    pub ladder: [f64; 3],
    /// Investigation thresholds and step cap for the slow path.
    pub investigation: InvestigationPolicy,
    /// Budget checked by the investigation loop.
    pub budget: CognitiveBudget,
    /// Largest risk a chosen plan may carry.
    pub max_risk: f64,
    /// Thresholds for the cognitive circuit breakers.
    #[serde(default)]
    pub breaker: BreakerConfig,
    /// Observations a provider needs before its calibration is trusted.
    #[serde(default = "default_min_samples")]
    pub calibration_min_samples: u64,
}

fn default_min_samples() -> u64 {
    DEFAULT_MIN_SAMPLES
}

impl Default for LoopConfig {
    /// Seed `0` with a neutral modulatory state and the conventional settings of [`LoopConfig::new`].
    fn default() -> Self {
        Self::new(0, Modulation::neutral())
    }
}

impl LoopConfig {
    /// A configuration with the given seed and modulatory state and conventional defaults for
    /// everything else.
    pub fn new(seed: u64, modulation: Modulation) -> Self {
        Self {
            seed,
            modulation,
            accept_at: 0.9,
            deliberative_max_providers: None,
            curiosity_threshold: 0.2,
            ladder: [0.2, 0.5, 0.8],
            investigation: InvestigationPolicy::new(
                tpt_thought_core::Probability::new(0.9)
                    .unwrap_or(tpt_thought_core::Probability::ONE),
                tpt_thought_core::Probability::new(0.5)
                    .unwrap_or(tpt_thought_core::Probability::ONE),
                4,
            ),
            budget: CognitiveBudget::default(),
            max_risk: 0.5,
            breaker: BreakerConfig::default(),
            calibration_min_samples: DEFAULT_MIN_SAMPLES,
        }
    }
}
