// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adversarial suite (spec section 95): contradictions, false observations, memory contamination,
//! provider failure and disagreement, and recursive thought.

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

use common::*;
use tpt_thought_core::{
    IdGenerator, Observation, ProvenanceId, SourceTrust, StructuredValue, Thought, ThoughtContent,
    ThoughtId, ThoughtOrigin, Timestamp,
};
use tpt_thought_graph::{SpawnError, SpawnLimits, ThoughtTree};
use tpt_thought_hypothesis::{Contradiction, ContradictionError};
use tpt_thought_loop::{CognitiveLoop, CycleOutcome};
use tpt_thought_provenance::{ProvenanceBody, ProvenanceSource, ProvenanceStore};
use tpt_thought_providers::{FailingProvider, ProviderErrorKind, ProviderRegistry};
use tpt_thought_safety::{
    find_parent_cycle, verify_provenance, BreakerConfig, BreakerKind, CircuitBreakers,
    IntegrityFault, ObservationGate,
};

fn untrusted_observation(ids: &IdGenerator) -> Observation {
    Observation {
        id: ids.observation(),
        content: Box::new(ThoughtContent::Structured(StructuredValue::Text(
            "a forged sensor reading".into(),
        ))),
        source: "web-form".into(),
        trust: SourceTrust::Untrusted,
        observed_at: Timestamp(5),
    }
}

/// Contradictions: a pair that keeps contradicting itself trips the contradiction-loop breaker,
/// and only resolving the pair clears it.
#[test]
fn repeated_contradiction_trips_the_loop_breaker() {
    let mut breakers = CircuitBreakers::new(BreakerConfig::default());
    let pair = 0xC0FFEE;
    let mut tripped = None;
    for _ in 0..100 {
        if let Err(trip) = breakers.record_contradiction(pair) {
            tripped = Some(trip);
            break;
        }
    }
    let trip = tripped.expect("a contradiction loop must trip the breaker");
    assert_eq!(trip.kind(), BreakerKind::ContradictionLoop);
    assert!(breakers.is_open());

    breakers.reset(BreakerKind::ContradictionLoop);
    assert!(breakers.check().is_ok(), "reset clears the breaker");
}

#[test]
fn a_hypothesis_cannot_contradict_itself() {
    let ids = ids(31);
    let h = ids.hypothesis();
    assert_eq!(
        Contradiction::new(h, h).expect_err("self-contradiction is rejected"),
        ContradictionError::SelfContradiction
    );
}

/// False observations: an untrusted observation is refused by a trusted-only gate, and a trusted
/// one is admitted.
#[test]
fn false_observations_are_refused_by_the_trust_gate() {
    let ids = ids(32);
    let gate = ObservationGate::new(SourceTrust::Trusted);
    assert!(gate.admit(&untrusted_observation(&ids)).is_err());

    let mut trusted = untrusted_observation(&ids);
    trusted.trust = SourceTrust::Trusted;
    assert!(gate.admit(&trusted).is_ok());
}

/// Memory contamination: a thought that cites provenance the store does not hold, or that cites
/// a record explaining some other thought, fails verification.
#[test]
fn contaminated_provenance_fails_verification() {
    let ids = ids(33);
    let thought_id = ids.thought();
    let other_id = ids.thought();
    let mut store = ProvenanceStore::new();
    let record = ids.provenance();
    store
        .insert(ProvenanceBody {
            id: record,
            source: ProvenanceSource::Memory,
            thought: Some(other_id),
            parents: vec![],
            evidence: vec![],
            memory: vec![],
            provider: None,
            model: None,
            version: None,
            reasoning_path: vec![],
            verification: None,
            timestamp: Timestamp(1),
        })
        .expect("record");

    let mut thought = fresh_thought(thought_id);
    thought.provenance.records = vec![ids.provenance()];
    assert!(matches!(
        verify_provenance(&store, &thought),
        Err(IntegrityFault::MissingRecord(_))
    ));

    thought.provenance.records = vec![record];
    assert!(matches!(
        verify_provenance(&store, &thought),
        Err(IntegrityFault::ForeignRecord { .. })
    ));
}

/// A derivation cannot cite a parent that does not exist, so a forged chain cannot be inserted.
#[test]
fn forged_parent_is_rejected_on_insert() {
    let ids = ids(34);
    let mut store = ProvenanceStore::new();
    let ghost: ProvenanceId = ids.provenance();
    let result = store.insert(ProvenanceBody {
        id: ids.provenance(),
        source: ProvenanceSource::Inference,
        thought: Some(ids.thought()),
        parents: vec![ghost],
        evidence: vec![],
        memory: vec![],
        provider: None,
        model: None,
        version: None,
        reasoning_path: vec![],
        verification: None,
        timestamp: Timestamp(1),
    });
    assert!(result.is_err());
    assert!(store.is_empty());
}

/// Provider failure: when the only provider fails, the loop stops without acting and reports why.
#[test]
fn provider_failure_stops_the_cycle_without_acting() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(FailingProvider::new(
            "intellect",
            profile(),
            ProviderErrorKind::Unavailable,
        ))
        .expect("register");
    let echo = Echo::default();
    let executed = echo.calls.clone();
    let mut lp =
        CognitiveLoop::new(neutral_config(41), registry, NoEngine, NoEstimate, echo).expect("loop");
    let report = lp
        .run(&grasp_input(200))
        .expect("the loop itself must not fail");
    assert!(matches!(report.outcome, CycleOutcome::Unresolved { .. }));
    assert_eq!(
        executed.load(Ordering::SeqCst),
        0,
        "no action without an answer"
    );
}

/// Disagreement: two providers that give different answers do not produce an action, because the
/// loop never executes a disputed answer.
#[test]
fn disagreement_does_not_produce_an_action() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(answering("alpha", "grasp", 0.95))
        .expect("register");
    registry
        .register(answering("beta", "push", 0.95))
        .expect("register");
    let echo = Echo::default();
    let executed = echo.calls.clone();
    let mut lp =
        CognitiveLoop::new(neutral_config(42), registry, NoEngine, NoEstimate, echo).expect("loop");
    let report = lp.run(&grasp_input(210)).expect("cycle");
    assert_eq!(
        executed.load(Ordering::SeqCst),
        0,
        "a disputed answer is not executed"
    );
    assert!(matches!(report.outcome, CycleOutcome::Unresolved { .. }));
}

/// Recursive thought: a spawn chain stops at the depth limit, and a cycle in parent links is found.
#[test]
fn recursive_thought_is_bounded() {
    let ids = ids(35);
    let root = ids.thought();
    let limits = SpawnLimits {
        max_depth: Some(2),
        max_children: None,
        max_total_compute: None,
        max_duration_nanos: None,
    };
    let mut tree = ThoughtTree::new(root, limits, Timestamp(0));
    let child = ids.thought();
    tree.spawn(&root, child, 1.0, Timestamp(1))
        .expect("depth 1");
    let grandchild = ids.thought();
    tree.spawn(&child, grandchild, 1.0, Timestamp(2))
        .expect("depth 2");
    let too_deep = ids.thought();
    assert_eq!(
        tree.spawn(&grandchild, too_deep, 1.0, Timestamp(3)),
        Err(SpawnError::DepthExceeded)
    );

    let (a, b) = (ids.thought(), ids.thought());
    let mut parents: BTreeMap<ThoughtId, ThoughtId> = BTreeMap::new();
    parents.insert(a, b);
    parents.insert(b, a);
    assert!(
        find_parent_cycle(&parents).is_some(),
        "a parent loop must be detected"
    );
}

/// Recursive escalation: the breaker trips when escalation depth exceeds the configured level.
#[test]
fn recursive_escalation_trips_the_breaker() {
    let mut breakers = CircuitBreakers::new(BreakerConfig::default());
    let limit = breakers.config().max_escalation_level;
    let trip = breakers
        .observe_escalation(limit + 1)
        .expect_err("escalation past the limit must trip");
    assert_eq!(trip.kind(), BreakerKind::RecursiveEscalation);
}

/// A loop whose confidence oscillates is caught by the oscillation breaker.
#[test]
fn oscillating_confidence_trips_the_breaker() {
    let mut breakers = CircuitBreakers::new(BreakerConfig::default());
    let mut tripped = false;
    for i in 0..100 {
        let value = if i % 2 == 0 { 0.1 } else { 0.9 };
        if breakers.observe_confidence(value).is_err() {
            tripped = true;
            break;
        }
    }
    assert!(
        tripped,
        "flipping confidence must trip the oscillation breaker"
    );
}

fn fresh_thought(id: ThoughtId) -> Thought {
    Thought::new(
        id,
        ThoughtOrigin::Observation,
        ThoughtContent::Structured(StructuredValue::Null),
        Timestamp(0),
    )
}

/// A reference provider that answers [`QUESTION`] with `answer`.
fn answering(id: &str, answer: &str, p: f64) -> tpt_thought_providers::ReferenceProvider {
    tpt_thought_providers::ReferenceProvider::new(id, profile()).with_answer(
        QUESTION,
        StructuredValue::Text(answer.into()),
        prob(p),
    )
}

/// Contaminated replay: a record whose recorded trajectory was altered is detected, and the
/// divergence is reported at the step that was changed.
#[test]
fn tampered_replay_record_is_detected() {
    use tpt_thought_loop::{replay, ReplayError, ReplayMetadata, ReplayRecord};

    let build = |c: &tpt_thought_loop::LoopConfig| {
        CognitiveLoop::new(
            c.clone(),
            registry_answering("intellect", 0.95),
            NoEngine,
            NoEstimate,
            Echo::default(),
        )
    };
    let mut record = ReplayRecord::capture(
        neutral_config(43),
        ReplayMetadata::default(),
        vec![grasp_input(220)],
        build,
    )
    .expect("capture");

    replay(&record, build).expect("untampered record replays");

    let target = 2.min(record.trajectory.len() - 1);
    record.trajectory[target].detail.push_str(" (edited)");
    match replay(&record, build) {
        Err(ReplayError::Diverged { index, .. }) => assert_eq!(index, target),
        other => panic!("expected divergence at step {target}, got {other:?}"),
    }
}
