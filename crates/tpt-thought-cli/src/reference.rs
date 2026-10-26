// SPDX-License-Identifier: MIT OR Apache-2.0
//! The reference setup the CLI runs with: one provider, a simulated executor, and no
//! deliberation engine or estimator.
//!
//! A replay can only be checked against the setup that produced it, so the CLI always uses this
//! module. Programs with their own providers or executors should build their loop directly, as
//! the examples in the `tpt-thought` crate do.

use tpt_thought::cognitive_loop::{
    Action, ActionError, ActionSink, CandidatePlan, CognitiveLoop, Input, LoopConfig, LoopError,
    Modulation, NullEngine, NullEstimator, ReplayMetadata,
};
use tpt_thought::core::{Probability, StructuredValue, Timestamp};
use tpt_thought::providers::{
    Locality, Privacy, ProviderProfile, ProviderRegistry, ReferenceProvider,
};

/// The question the reference provider knows the answer to.
pub const QUESTION: &str = "can the gripper grasp the cup?";

/// Records what it was asked to do and reports the expected outcome, so prediction error is zero.
#[derive(Debug, Default)]
pub struct SimulatedArm;

impl ActionSink for SimulatedArm {
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError> {
        Ok(action.expected.clone())
    }
}

/// The reference provider, `intellect`, answers [`QUESTION`] with 95% reliability.
pub fn registry() -> ProviderRegistry {
    let profile = ProviderProfile::new(Locality::Local, Privacy::OnDevice);
    let provider = ReferenceProvider::new("intellect", profile).with_answer(
        QUESTION,
        StructuredValue::Text("grasp".into()),
        Probability::new(0.95).expect("valid probability"),
    );
    let mut registry = ProviderRegistry::new();
    registry
        .register(provider)
        .expect("register the reference provider");
    registry
}

/// A loop built from `config` with the reference setup.
pub fn build(
    config: &LoopConfig,
) -> Result<CognitiveLoop<NullEngine, NullEstimator, SimulatedArm>, LoopError> {
    CognitiveLoop::builder()
        .config(config.clone())
        .registry(registry())
        .sink(SimulatedArm)
        .build()
}

/// The metadata every reference run records.
pub fn metadata() -> ReplayMetadata {
    ReplayMetadata {
        providers: vec!["intellect".into()],
        ..ReplayMetadata::default()
    }
}

/// The default configuration: seed 1 with a neutral modulatory state.
pub fn default_config() -> LoopConfig {
    LoopConfig::new(1, Modulation::neutral())
}

/// One grasp decision for the reference provider, observed at `at`.
pub fn grasp_input(at: i64) -> Input {
    Input {
        at: Timestamp(at),
        source: "camera".into(),
        observation: StructuredValue::Text("cup within reach".into()),
        question: QUESTION.into(),
        goal: "pick up the cup".into(),
        constraints: vec![],
        memories: vec![],
        candidates: vec![CandidatePlan {
            name: "grasp".into(),
            predicted_outcome: StructuredValue::Text("cup held".into()),
            cost: 1.0,
            risk: 0.1,
        }],
    }
}
