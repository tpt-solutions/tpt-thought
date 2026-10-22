// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared fixtures for the test suites. Everything here is deterministic: IDs come from a seeded
//! entropy source, and no test reads the wall clock.
#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tpt_thought_core::{
    Confidence, IdGenerator, Probability, SeededEntropy, StructuredValue, Timestamp,
};
use tpt_thought_deliberation::DeliberationEngine;
use tpt_thought_loop::{
    Action, ActionError, ActionSink, CandidatePlan, Estimate, Estimator, Input, LoopConfig,
    Modulation, ModulationFields,
};
use tpt_thought_providers::{
    Decision, IntelligenceProvider, IntelligenceRequest, Locality, Privacy, ProviderError,
    ProviderId, ProviderProfile, ProviderRegistry, ReferenceProvider,
};

pub const QUESTION: &str = "can the gripper grasp?";

pub fn prob(p: f64) -> Probability {
    Probability::new(p).expect("valid probability")
}

pub fn ids(seed: u64) -> IdGenerator {
    IdGenerator::new(SeededEntropy::new(seed))
}

pub fn profile() -> ProviderProfile {
    ProviderProfile::new(Locality::Local, Privacy::OnDevice)
}

pub fn neutral_config(seed: u64) -> LoopConfig {
    LoopConfig::new(seed, Modulation::neutral())
}

pub fn grasp_candidates() -> Vec<CandidatePlan> {
    vec![CandidatePlan {
        name: "grasp".into(),
        predicted_outcome: StructuredValue::Text("held".into()),
        cost: 1.0,
        risk: 0.1,
    }]
}

pub fn grasp_input(at: i64) -> Input {
    Input {
        at: Timestamp(at),
        source: "camera".into(),
        observation: StructuredValue::Text("object in reach".into()),
        question: QUESTION.into(),
        goal: "pick up the object".into(),
        constraints: vec![],
        memories: vec![],
        candidates: grasp_candidates(),
    }
}

/// A registry holding one reference provider that answers [`QUESTION`] with `grasp`.
pub fn registry_answering(id: &str, confidence: f64) -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new(id, profile()).with_answer(
            QUESTION,
            StructuredValue::Text("grasp".into()),
            prob(confidence),
        ))
        .expect("register");
    registry
}

/// A deliberation engine that supports nothing, so Eve is skipped.
pub struct NoEngine;

impl DeliberationEngine for NoEngine {}

/// An estimator with no opinion, so Augur contributes nothing.
pub struct NoEstimate;

impl Estimator for NoEstimate {
    fn estimate(&self, _question: &str) -> Option<Estimate> {
        None
    }
}

/// An estimator that always answers with the same estimate.
pub struct Fixed(pub Estimate);

impl Estimator for Fixed {
    fn estimate(&self, _question: &str) -> Option<Estimate> {
        Some(self.0)
    }
}

/// An action sink that returns the expected outcome, so prediction error is zero. Counts calls.
#[derive(Clone, Default)]
pub struct Echo {
    pub calls: Arc<AtomicUsize>,
}

impl ActionSink for Echo {
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(action.expected.clone())
    }
}

/// An action sink that returns a fixed outcome, which may differ from the prediction.
pub struct Returns {
    pub value: StructuredValue,
    pub calls: Arc<AtomicUsize>,
}

impl ActionSink for Returns {
    fn execute(&mut self, _action: &Action) -> Result<StructuredValue, ActionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.value.clone())
    }
}

/// Wraps a provider and counts the requests it receives.
pub struct Counting<P> {
    pub inner: P,
    pub calls: Arc<AtomicUsize>,
    pub questions: Arc<std::sync::Mutex<Vec<String>>>,
}

impl<P: IntelligenceProvider> IntelligenceProvider for Counting<P> {
    fn id(&self) -> &ProviderId {
        self.inner.id()
    }

    fn profile(&self) -> &ProviderProfile {
        self.inner.profile()
    }

    fn decide(
        &self,
        request: &IntelligenceRequest,
    ) -> Result<Decision<StructuredValue>, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.questions
            .lock()
            .expect("question log")
            .push(request.question.clone());
        self.inner.decide(request)
    }
}

/// A confidence at the given raw probability.
pub fn raw_confidence(p: f64) -> Confidence {
    Confidence::from_raw(prob(p))
}

/// A loop configuration whose modulation selects the reactive mode, which answers from the fast
/// path and escalates when the answer is not reliable enough. Deliberative mode accepts any
/// consensus, so it never escalates on a single provider.
pub fn reactive_config(seed: u64) -> LoopConfig {
    let fields = ModulationFields {
        urgency: 0.5,
        risk: 0.5,
        novelty: 0.5,
        reward: 0.5,
        threat: 0.5,
        confidence: 0.5,
        uncertainty: 0.1,
        arousal: 0.9,
    };
    LoopConfig::new(seed, Modulation::new(fields).expect("valid modulation"))
}

/// Benchmark scenarios shared by `benches/pipeline.rs` and the `cargo test` smoke run.
pub mod scenarios;
