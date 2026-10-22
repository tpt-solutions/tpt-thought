// SPDX-License-Identifier: MIT OR Apache-2.0
//! Acceptance criteria A to M (spec section 122), one test per criterion.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::*;
use tpt_thought_core::{
    Confidence, IdGenerator, MemoryRef, Observation, Prediction, ProvenanceId, SourceTrust,
    StructuredValue, Thought, ThoughtContent, ThoughtState, Timestamp, Uncertainty,
};
use tpt_thought_deliberation::mock::ScriptedEngine;
use tpt_thought_deliberation::{InvestigationOutcome, InvestigationPolicy};
use tpt_thought_loop::{
    replay, CandidatePlan, CognitiveLoop, CognitiveMode, CycleOutcome, CyclePhase, Estimate,
    LoopConfig, LoopError, Modulation, ReplayMetadata, ReplayRecord, SlowStage,
};
use tpt_thought_prediction::{compare, MatchConfig};

use tpt_thought_providers::{ProviderRegistry, ReferenceProvider};
use tpt_thought_workspace::{MemoryIndex, MemoryStore};

fn observation(ids: &IdGenerator, text: &str, at: i64) -> Observation {
    Observation {
        id: ids.observation(),
        content: Box::new(ThoughtContent::Structured(StructuredValue::Text(
            text.into(),
        ))),
        source: "camera".into(),
        trust: SourceTrust::Trusted,
        observed_at: Timestamp(at),
    }
}

/// A scripted Eve that reports one investigation with the given confidence.
fn eve_reports(confidence: f64) -> ScriptedEngine {
    let mut engine = ScriptedEngine::default();
    engine.investigations.push_back(Ok(InvestigationOutcome {
        findings: vec!["the handle is within reach".into()],
        confidence: Confidence::from_raw(prob(confidence)),
        uncertainty: Uncertainty::unknown(),
        risk: prob(0.1),
        needs_human: false,
    }));
    engine
}

fn build_with(
    config: &LoopConfig,
    registry: ProviderRegistry,
) -> Result<CognitiveLoop<NoEngine, NoEstimate, Echo>, LoopError> {
    CognitiveLoop::new(
        config.clone(),
        registry,
        NoEngine,
        NoEstimate,
        Echo::default(),
    )
}

/// A: an observation arrives, the thought is created from it, and the loop observes it first.
#[test]
fn a_thought_receives_an_observation() {
    let ids = ids(1);
    let obs = observation(&ids, "object in reach", 10);
    let t = Thought::from_observation(ids.thought(), obs, Timestamp(10));
    assert_eq!(
        t.state,
        ThoughtState::Created,
        "from_observation does not receive; the loop does"
    );
    assert_eq!(
        tpt_thought_state::next(t.state, tpt_thought_state::ThoughtEvent::Receive)
            .expect("receive"),
        ThoughtState::Received
    );
    assert_eq!(t.observations.len(), 1);

    let mut lp =
        build_with(&neutral_config(1), registry_answering("intellect", 0.95)).expect("loop");
    lp.run(&grasp_input(10)).expect("cycle");
    assert_eq!(lp.trajectory()[0].phase, CyclePhase::Observe);
}

/// B: relevant memory is retrieved by cue, the closest match ranks first, and the loop uses it.
#[test]
fn b_relevant_memory_is_retrieved() {
    let mut index = MemoryIndex::new();
    let mem = |id: &str| MemoryRef {
        store: "anima-sse".into(),
        id: id.into(),
    };
    index.insert(
        mem("gripper-notes"),
        ["gripper".to_string(), "grasp".to_string()],
    );
    index.insert(mem("kettle"), ["kettle".to_string()]);

    let cue = vec!["gripper".to_string(), "grasp".to_string()];
    let hits = index.retrieve(&cue, 5);
    assert_eq!(hits.first().map(|m| m.id.as_str()), Some("gripper-notes"));
    assert!(
        !hits.iter().any(|m| m.id == "kettle"),
        "unrelated memory must not match"
    );

    // The loop itself asks the store, so the input carries no memories of its own.
    let mut lp = build_with(&neutral_config(2), registry_answering("intellect", 0.95))
        .expect("loop")
        .with_memory(index);
    let report = lp.run(&grasp_input(20)).expect("cycle");
    assert!(matches!(report.outcome, CycleOutcome::Completed { .. }));
    assert!(
        report.memories.iter().any(|m| m.id == "gripper-notes"),
        "the relevant memory must reach the decision: {:?}",
        report.memories
    );
    assert!(!report.memories.iter().any(|m| m.id == "kettle"));

    let decision = lp
        .provenance()
        .get(&report.provenance[0])
        .expect("decision record");
    assert!(
        decision.body.memory.iter().any(|m| m.id == "gripper-notes"),
        "the decision's provenance must cite the memory it used"
    );
}

/// C: the loop holds the question and one hypothesis per candidate plan, in a bounded branch set.
#[test]
fn c_multiple_hypotheses_are_created() {
    let mut input = grasp_input(30);
    input.candidates.push(CandidatePlan {
        name: "push".into(),
        predicted_outcome: StructuredValue::Text("moved".into()),
        cost: 2.0,
        risk: 0.2,
    });
    let mut lp =
        build_with(&neutral_config(3), registry_answering("intellect", 0.95)).expect("loop");
    let report = lp.run(&input).expect("cycle");
    assert_eq!(
        report.hypotheses, 3,
        "the question plus one hypothesis per candidate plan"
    );
    assert!(matches!(report.outcome, CycleOutcome::Completed { .. }));
}

/// D: structured intelligence is requested from a provider with the question attached.
#[test]
fn d_structured_intelligence_is_requested() {
    let calls = Arc::new(AtomicUsize::new(0));
    let questions = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ProviderRegistry::new();
    registry
        .register(Counting {
            inner: ReferenceProvider::new("intellect", profile()).with_answer(
                QUESTION,
                StructuredValue::Text("grasp".into()),
                prob(0.95),
            ),
            calls: calls.clone(),
            questions: questions.clone(),
        })
        .expect("register");

    let mut lp = build_with(&neutral_config(4), registry).expect("loop");
    lp.run(&grasp_input(30)).expect("cycle");
    assert!(calls.load(Ordering::SeqCst) >= 1);
    assert_eq!(
        questions.lock().expect("log").first().map(String::as_str),
        Some(QUESTION)
    );
}

/// E: a low-confidence answer is not accepted, so the decision escalates through the slow path.
#[test]
fn e_uncertain_decision_escalates() {
    let mut config = reactive_config(5);
    config.accept_at = 0.8;
    let mut lp = CognitiveLoop::new(
        config,
        registry_answering("intellect", 0.3),
        NoEngine,
        Fixed(Estimate {
            mean: 0.5,
            residual_uncertainty: 0.2,
        }),
        Echo::default(),
    )
    .expect("loop");
    let report = lp.run(&grasp_input(40)).expect("cycle");
    assert!(report.slow_stages.contains(&SlowStage::Intellect));
    assert!(report.slow_stages.contains(&SlowStage::Augur));
    assert!(report
        .slow_stages
        .contains(&SlowStage::IntellectReevaluation));
}

/// F: Eve runs a bounded deliberation when the first pass escalates.
#[test]
fn f_eve_deliberation_is_invoked() {
    let mut config = reactive_config(6);
    config.accept_at = 0.8;
    config.investigation = InvestigationPolicy::new(prob(0.9), prob(0.5), 3);
    let mut lp = CognitiveLoop::new(
        config,
        registry_answering("intellect", 0.3),
        eve_reports(0.9),
        NoEstimate,
        Echo::default(),
    )
    .expect("loop");
    let report = lp.run(&grasp_input(50)).expect("cycle");
    assert!(report.slow_stages.contains(&SlowStage::Eve));
    assert!(report.investigation.is_some());
}

/// G: the loop predicts the action's outcome, tracks the prediction, and resolves it against the
/// observed outcome, which here differs, so the error is positive.
#[test]
fn g_prediction_is_created() {
    let mut lp = CognitiveLoop::new(
        neutral_config(7),
        registry_answering("intellect", 0.95),
        NoEngine,
        NoEstimate,
        Returns {
            value: StructuredValue::Text("dropped".into()),
            calls: Arc::new(AtomicUsize::new(0)),
        },
    )
    .expect("loop");
    let report = lp.run(&grasp_input(100)).expect("cycle");
    assert!(report.prediction.is_some(), "the action was predicted");
    assert_eq!(
        lp.predictions().open_count(),
        0,
        "the prediction must be resolved, not left open"
    );
    match report.outcome {
        CycleOutcome::Completed { error, .. } => {
            assert!(error > 0.0, "the mismatch must be scored");
        }
        other => panic!("expected completion, got {other:?}"),
    }
    assert!(lp.trajectory().iter().any(|s| s.phase == CyclePhase::Learn));
}

/// H: an intention is executed through the action sink, and only through it.
#[test]
fn h_intention_is_executed() {
    let echo = Echo::default();
    let calls = echo.calls.clone();
    let mut lp = CognitiveLoop::new(
        neutral_config(8),
        registry_answering("intellect", 0.95),
        NoEngine,
        NoEstimate,
        echo,
    )
    .expect("loop");
    let report = lp.run(&grasp_input(60)).expect("cycle");
    assert!(matches!(report.outcome, CycleOutcome::Completed { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// I: the predicted outcome is compared with the actual outcome, and the error is reported.
#[test]
fn i_predicted_and_actual_outcomes_are_compared() {
    let mut lp = CognitiveLoop::new(
        neutral_config(9),
        registry_answering("intellect", 0.95),
        NoEngine,
        NoEstimate,
        Returns {
            value: StructuredValue::Text("dropped".into()),
            calls: Arc::new(AtomicUsize::new(0)),
        },
    )
    .expect("loop");
    let report = lp.run(&grasp_input(70)).expect("cycle");
    match report.outcome {
        CycleOutcome::Completed { error, .. } => assert!(error > 0.0, "a mismatch must score"),
        other => panic!("expected completion, got {other:?}"),
    }

    let ids = ids(9);
    let prediction = Prediction {
        id: ids.prediction(),
        predicted_state: StructuredValue::Text("held".into()),
        expected_at: None,
        confidence: Confidence::from_raw(prob(0.9)),
        assumptions: vec![],
        source_thought: ids.thought(),
    };
    let err = compare(
        &prediction,
        &observation(&ids, "dropped", 1),
        &MatchConfig::default(),
    )
    .expect("comparable");
    assert!(err.magnitude() > 0.0);
}

/// J: the cognitive state moves through several stages, and each recorded state is reachable from
/// the one before it. Checkpoints can skip intermediate states, such as Attending, so this is a
/// reachability check rather than a single-edge check.
#[test]
fn j_cognitive_state_is_updated() {
    let mut lp =
        build_with(&neutral_config(10), registry_answering("intellect", 0.95)).expect("loop");
    lp.run(&grasp_input(80)).expect("cycle");
    let states: Vec<ThoughtState> = lp.trajectory().iter().map(|s| s.state).collect();
    let mut distinct = states.clone();
    distinct.dedup();
    assert!(
        distinct.len() >= 4,
        "the state must move through several stages: {distinct:?}"
    );
    for pair in states.windows(2) {
        assert!(
            reachable(pair[0], pair[1]),
            "{:?} cannot reach {:?} under the table",
            pair[0],
            pair[1]
        );
    }
}

/// K: the loop writes a provenance record for its decision and one for its learning step, attaches
/// both to the thought, and the derivation chain verifies.
#[test]
fn k_provenance_is_preserved() {
    let mut lp =
        build_with(&neutral_config(11), registry_answering("intellect", 0.95)).expect("loop");
    let report = lp.run(&grasp_input(110)).expect("cycle");

    assert_eq!(
        report.provenance.len(),
        2,
        "one record for the decision, one for the learning step"
    );
    for id in &report.provenance {
        assert!(lp.provenance().get(id).is_some(), "record {id:?} is stored");
    }
    lp.provenance().verify().expect("chain verifies");

    let explained: Vec<ProvenanceId> = lp
        .provenance()
        .explain(&report.thought)
        .iter()
        .map(|r| r.body.id)
        .collect();
    assert_eq!(explained, report.provenance, "oldest first");
    let learning = lp
        .provenance()
        .get(&report.provenance[1])
        .expect("learning record");
    assert_eq!(learning.body.parents, vec![report.provenance[0]]);
}

/// L: an entire episode is recorded and replays to an identical trajectory.
#[test]
fn l_entire_episode_can_be_replayed() {
    let record = ReplayRecord::capture(
        neutral_config(12),
        ReplayMetadata {
            providers: vec!["intellect".into()],
            ..ReplayMetadata::default()
        },
        vec![grasp_input(90), grasp_input(91)],
        |c| build_with(c, registry_answering("intellect", 0.95)),
    )
    .expect("capture");
    let reports = replay(&record, |c| {
        build_with(c, registry_answering("intellect", 0.95))
    })
    .expect("trajectory reproduced");
    assert_eq!(reports.len(), 2);
}

/// M: the same architecture runs unchanged with two different providers.
#[test]
fn m_same_architecture_runs_with_two_providers() {
    let run = |id: &str, confidence: f64| {
        let mut lp =
            build_with(&neutral_config(13), registry_answering(id, confidence)).expect("loop");
        let report = lp.run(&grasp_input(100)).expect("cycle");
        let phases: Vec<CyclePhase> = lp.trajectory().iter().map(|s| s.phase).collect();
        (phases, report.outcome)
    };
    let (phases_a, outcome_a) = run("alpha", 0.95);
    let (phases_b, outcome_b) = run("beta", 0.90);
    assert_eq!(
        phases_a, phases_b,
        "the cognitive cycle must not depend on the provider"
    );
    assert!(matches!(outcome_a, CycleOutcome::Completed { .. }));
    assert!(matches!(outcome_b, CycleOutcome::Completed { .. }));
    assert_eq!(
        Modulation::neutral().select_mode(),
        CognitiveMode::Deliberative
    );
}

/// Whether `to` can be reached from `from` by a sequence of legal transitions.
fn reachable(from: ThoughtState, to: ThoughtState) -> bool {
    let mut seen = vec![from];
    let mut frontier = vec![from];
    while let Some(state) = frontier.pop() {
        if state == to {
            return true;
        }
        for event in tpt_thought_state::permitted_events(state) {
            let next = tpt_thought_state::next(state, event).expect("permitted event");
            if !seen.contains(&next) {
                seen.push(next);
                frontier.push(next);
            }
        }
    }
    false
}

/// The control for E: a confident answer on the same reactive path is accepted without escalating.
#[test]
fn e_confident_answer_does_not_escalate() {
    let mut config = reactive_config(14);
    config.accept_at = 0.8;
    let mut lp = CognitiveLoop::new(
        config,
        registry_answering("intellect", 0.95),
        NoEngine,
        NoEstimate,
        Echo::default(),
    )
    .expect("loop");
    let report = lp.run(&grasp_input(45)).expect("cycle");
    assert!(report.slow_stages.is_empty());
    assert!(matches!(report.outcome, CycleOutcome::Completed { .. }));
}
