// SPDX-License-Identifier: MIT OR Apache-2.0
//! The golden demonstration (spec section 123): a robot meets an unfamiliar object.
//!
//! Every sibling system is a mock here: Anima supplies memory and executes the action, Intellect
//! abstains until an estimate is in context, Eve runs a scripted investigation, and Augur gives a
//! fixed estimate. The same scenario against the real sibling adapters is blocked, and is tracked
//! as unchecked in `todo.md`.

mod common;

use common::*;
use tpt_thought_core::{Confidence, MemoryRef, StructuredValue, Timestamp, Uncertainty};
use tpt_thought_deliberation::mock::ScriptedEngine;
use tpt_thought_deliberation::{InvestigationOutcome, InvestigationPolicy};
use tpt_thought_loop::{
    replay, CognitiveLoop, CycleOutcome, Estimate, Input, LoopConfig, LoopError, ReplayMetadata,
    ReplayRecord, SlowStage,
};
use tpt_thought_providers::{
    Decision, DecisionSource, IntelligenceProvider, IntelligenceRequest, ProviderError, ProviderId,
    ProviderProfile, ProviderRegistry,
};
use tpt_thought_workspace::{MemoryIndex, MemoryStore};

const OBJECT: &str = "unfamiliar object on the table";

/// Intellect in the scenario: it abstains until Augur's estimate is in the request context.
struct NeedsEstimate {
    id: ProviderId,
    profile: ProviderProfile,
}

impl IntelligenceProvider for NeedsEstimate {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn profile(&self) -> &ProviderProfile {
        &self.profile
    }

    fn decide(
        &self,
        request: &IntelligenceRequest,
    ) -> Result<Decision<StructuredValue>, ProviderError> {
        let source = DecisionSource::Provider(self.id.clone());
        if request.context.iter().any(|c| c.starts_with("estimate:")) {
            Ok(Decision::answered(
                source,
                StructuredValue::Text("grasp".into()),
                Confidence::from_raw(prob(0.92)),
                request.at,
            ))
        } else {
            Ok(Decision::abstained(source, "unfamiliar object", request.at))
        }
    }
}

fn registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry
        .register(NeedsEstimate {
            id: ProviderId::new("intellect"),
            profile: profile(),
        })
        .expect("register");
    registry
}

/// Anima's memory: an earlier encounter with a similar shape, retrieved by cue.
fn anima_memory() -> Vec<MemoryRef> {
    let mut index = MemoryIndex::new();
    index.insert(
        MemoryRef {
            store: "anima-sse".into(),
            id: "cup-encounter-7".into(),
        },
        ["object".to_string(), "table".to_string()],
    );
    index.insert(
        MemoryRef {
            store: "anima-sse".into(),
            id: "kettle-encounter-2".into(),
        },
        ["kitchen".to_string()],
    );
    index.retrieve(&["object".to_string(), "table".to_string()], 4)
}

fn scenario_input(memories: Vec<MemoryRef>) -> Input {
    Input {
        at: Timestamp(1_000),
        source: "depth-camera".into(),
        observation: StructuredValue::Text(OBJECT.into()),
        question: QUESTION.into(),
        goal: "pick up the object".into(),
        constraints: vec![],
        memories,
        candidates: grasp_candidates(),
    }
}

/// Eve's investigation: one step that reaches the confidence target.
fn eve() -> ScriptedEngine {
    let mut engine = ScriptedEngine::default();
    engine.investigations.push_back(Ok(InvestigationOutcome {
        findings: vec!["object is rigid and within reach".into()],
        confidence: Confidence::from_raw(prob(0.9)),
        uncertainty: Uncertainty::unknown(),
        risk: prob(0.1),
        needs_human: false,
    }));
    engine
}

fn config(seed: u64) -> LoopConfig {
    let mut config = reactive_config(seed);
    config.accept_at = 0.8;
    config.investigation = InvestigationPolicy::new(prob(0.9), prob(0.5), 3);
    config
}

fn build(config: &LoopConfig) -> Result<CognitiveLoop<ScriptedEngine, Fixed, Echo>, LoopError> {
    CognitiveLoop::new(
        config.clone(),
        registry(),
        eve(),
        Fixed(Estimate {
            mean: 0.6,
            residual_uncertainty: 0.15,
        }),
        Echo::default(),
    )
}

#[test]
fn robot_meets_an_unfamiliar_object() {
    let echo = Echo::default();
    let executed = echo.calls.clone();
    let mut lp = CognitiveLoop::new(
        config(21),
        registry(),
        eve(),
        Fixed(Estimate {
            mean: 0.6,
            residual_uncertainty: 0.15,
        }),
        echo,
    )
    .expect("loop");

    let report = lp
        .run(&scenario_input(anima_memory()))
        .expect("cycle completes");

    // The first pass cannot answer, so the slow path runs in its specified order.
    assert_eq!(
        report.slow_stages,
        vec![
            SlowStage::Intellect,
            SlowStage::Eve,
            SlowStage::Augur,
            SlowStage::IntellectReevaluation,
        ]
    );
    assert!(
        report.investigation.is_some(),
        "Eve ran a bounded investigation"
    );

    // The re-evaluated answer is acted on through the executor, and the outcome is compared.
    match report.outcome {
        CycleOutcome::Completed { action, error, .. } => {
            assert_eq!(action, "grasp");
            assert_eq!(error, 0.0, "the mock executor matches the prediction");
        }
        other => panic!("expected completion, got {other:?}"),
    }
    assert_eq!(executed.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn robot_scenario_replays_exactly() {
    let record = ReplayRecord::capture(
        config(22),
        ReplayMetadata {
            providers: vec!["intellect".into()],
            memory_refs: vec!["cup-encounter-7".into()],
            ..ReplayMetadata::default()
        },
        vec![scenario_input(anima_memory())],
        build,
    )
    .expect("capture");

    let reports = replay(&record, build).expect("episode replays step for step");
    assert_eq!(reports.len(), 1);
    assert!(!record.trajectory.is_empty());
}
