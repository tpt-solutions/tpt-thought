// SPDX-License-Identifier: MIT OR Apache-2.0
//! The golden demonstration (spec section 123) against the real Intellect reference backend and
//! the real Eve adapter. Only compiled with the `real-adapters` feature.
//!
//! Anima's memory and the executor stay as in `golden.rs`, and Augur stays a fixed estimate: no
//! real Augur adapter is wired into this scenario.
//!
//! The single-candidate scenario is answered at full confidence by the real Intellect on the
//! first pass, so it never escalates. The escalation tests therefore offer Intellect a second
//! candidate. The reference backend's choice confidence is the chosen candidate's share of the
//! distribution, so the answer is honestly below `accept_at` and the slow path runs. Eve has no
//! facts about the object, so its investigation reports that it needs a human.

#![cfg(feature = "real-adapters")]

mod common;

use common::*;
use tpt_thought_core::{MemoryRef, StructuredValue, Timestamp};
use tpt_thought_deliberation::{InvestigationPolicy, Termination};
use tpt_thought_integrations::eve::EveAdapter;
use tpt_thought_integrations::intellect::reference_provider;
use tpt_thought_loop::{
    replay, CognitiveLoop, CycleOutcome, Estimate, Input, LoopConfig, LoopError, ReplayMetadata,
    ReplayRecord, SlowStage,
};
use tpt_thought_providers::ProviderRegistry;
use tpt_thought_workspace::{MemoryIndex, MemoryStore};

const OBJECT: &str = "unfamiliar object on the table";

fn registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry
        .register(reference_provider(vec!["grasp".into()], profile()))
        .expect("register");
    registry
}

/// The reference provider offered a second candidate, so its first-pass choice confidence is
/// the share of the chosen candidate, below full confidence.
fn hesitant_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry
        .register(reference_provider(
            vec!["grasp".into(), "push".into()],
            profile(),
        ))
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

fn config(seed: u64) -> LoopConfig {
    let mut config = reactive_config(seed);
    config.accept_at = 0.8;
    config.investigation = InvestigationPolicy::new(prob(0.9), prob(0.5), 3);
    config
}

fn augur() -> Estimate {
    Estimate {
        mean: 0.6,
        residual_uncertainty: 0.15,
    }
}

fn build(config: &LoopConfig) -> Result<CognitiveLoop<EveAdapter, Fixed, Echo>, LoopError> {
    CognitiveLoop::new(
        config.clone(),
        registry(),
        EveAdapter::new(config.seed),
        Fixed(augur()),
        Echo::default(),
    )
}

#[test]
fn robot_meets_an_unfamiliar_object_with_real_adapters() {
    let echo = Echo::default();
    let executed = echo.calls.clone();
    let mut lp = CognitiveLoop::new(
        config(21),
        registry(),
        EveAdapter::new(21),
        Fixed(augur()),
        echo,
    )
    .expect("loop");

    let report = lp
        .run(&scenario_input(anima_memory()))
        .expect("cycle completes");

    // The real Intellect answers at full confidence on the first pass, so no slow path runs.
    assert!(report.slow_stages.is_empty());

    // The answer is acted on through the executor, and the outcome is compared.
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
fn robot_scenario_with_real_adapters_replays_exactly() {
    let record = ReplayRecord::capture(
        config(22),
        ReplayMetadata {
            providers: vec!["reference".into()],
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

#[test]
fn robot_slow_path_order_with_real_adapters() {
    let mut lp = CognitiveLoop::new(
        config(23),
        hesitant_registry(),
        EveAdapter::new(23),
        Fixed(augur()),
        Echo::default(),
    )
    .expect("loop");
    let report = lp.run(&scenario_input(anima_memory())).expect("cycle");
    assert_eq!(
        report.slow_stages,
        vec![
            SlowStage::Intellect,
            SlowStage::Eve,
            SlowStage::Augur,
            SlowStage::IntellectReevaluation,
        ]
    );
}

#[test]
fn robot_eve_investigation_with_real_adapters() {
    let mut lp = CognitiveLoop::new(
        config(24),
        hesitant_registry(),
        EveAdapter::new(24),
        Fixed(augur()),
        Echo::default(),
    )
    .expect("loop");
    let report = lp.run(&scenario_input(anima_memory())).expect("cycle");
    // Eve holds no facts about the object, so it reports unknown confidence and maximal risk,
    // and the investigation policy stops on risk before it can ask for a human.
    assert_eq!(report.investigation, Some(Termination::RiskPolicy));
}
