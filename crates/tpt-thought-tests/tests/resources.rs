// SPDX-License-Identifier: MIT OR Apache-2.0
//! Resource suite (spec section 95): cognition stays bounded under limits, budgets and breakers.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use common::*;
use tpt_thought_attention::{
    ForgettingState, WorkingMemory, WorkingMemoryConfig, WorkingMemoryItem,
};
use tpt_thought_core::{
    Confidence, ProvenanceId, StructuredValue, ThoughtContent, Timestamp, Uncertainty,
};
use tpt_thought_deliberation::mock::ScriptedEngine;
use tpt_thought_deliberation::{
    DeliberationContext, DeliberationEngine, DeliberationError, Investigation,
    InvestigationOutcome, InvestigationPolicy, Metacognition,
};
use tpt_thought_loop::CognitiveLoop;
use tpt_thought_safety::{
    BreakerConfig, BreakerKind, CapabilityDenied, CircuitBreakers, ResourceLimits, Sandbox, Usage,
};

/// Counts every investigation step the loop actually runs.
struct CountingEngine {
    inner: ScriptedEngine,
    steps: Arc<AtomicUsize>,
}

impl DeliberationEngine for CountingEngine {
    fn investigate(
        &mut self,
        ctx: &DeliberationContext,
        request: &Investigation,
    ) -> Result<InvestigationOutcome, DeliberationError> {
        self.steps.fetch_add(1, Ordering::SeqCst);
        self.inner.investigate(ctx, request)
    }

    fn reflect(&mut self, ctx: &DeliberationContext) -> Result<Metacognition, DeliberationError> {
        self.inner.reflect(ctx)
    }
}

fn inconclusive() -> InvestigationOutcome {
    InvestigationOutcome {
        findings: vec![],
        confidence: Confidence::from_raw(prob(0.2)),
        uncertainty: Uncertainty::unknown(),
        risk: prob(0.1),
        needs_human: false,
    }
}

#[test]
fn investigation_never_exceeds_its_step_cap() {
    let mut engine = ScriptedEngine::default();
    for _ in 0..10 {
        engine.investigations.push_back(Ok(inconclusive()));
    }
    let steps = Arc::new(AtomicUsize::new(0));
    let mut config = reactive_config(51);
    config.accept_at = 0.8;
    config.investigation = InvestigationPolicy::new(prob(0.9), prob(0.5), 2);
    let mut lp = CognitiveLoop::new(
        config,
        registry_answering("intellect", 0.3),
        CountingEngine {
            inner: engine,
            steps: steps.clone(),
        },
        NoEstimate,
        Echo::default(),
    )
    .expect("loop");
    let report = lp.run(&grasp_input(300)).expect("cycle");
    assert_eq!(
        steps.load(Ordering::SeqCst),
        2,
        "the policy caps Eve at two steps"
    );
    assert!(report.investigation.is_some());
}

#[test]
fn sandbox_denies_capabilities_by_default() {
    let mut sandbox = Sandbox::new(ResourceLimits::default());
    let required = vec!["move:arm".to_string()];
    assert!(
        sandbox.authorize(&required).is_err(),
        "nothing is granted by default"
    );
    sandbox.grant("move:arm");
    assert!(sandbox.authorize(&required).is_ok());
    sandbox.revoke("move:arm");
    assert_eq!(
        sandbox.check("move:arm").expect_err("revoked"),
        CapabilityDenied {
            capability: "move:arm".into()
        }
    );
}

#[test]
fn sandbox_enforces_structural_limits() {
    let sandbox = Sandbox::new(ResourceLimits {
        max_depth: Some(3),
        max_children: Some(2),
        max_total_compute: Some(10.0),
        max_duration_nanos: Some(1_000),
    });
    let within = Usage {
        depth: 3,
        children: 2,
        compute: 10.0,
        elapsed_nanos: 1_000,
    };
    assert!(sandbox.check_usage(&within).is_ok(), "limits are inclusive");

    let deep = Usage { depth: 4, ..within };
    assert!(sandbox.check_usage(&deep).is_err());

    let slow = Usage {
        elapsed_nanos: 1_001,
        ..within
    };
    assert!(sandbox.check_usage(&slow).is_err());
}

#[test]
fn compute_budget_exhaustion_trips_the_breaker() {
    let mut breakers = CircuitBreakers::new(BreakerConfig::default());
    let max = breakers.config().max_compute;
    breakers.charge(max / 2.0).expect("half the budget is fine");
    let trip = breakers
        .charge(max)
        .expect_err("spending past the budget must trip");
    assert_eq!(trip.kind(), BreakerKind::ResourceExhaustion);
}

#[test]
fn repetition_trips_but_varied_steps_do_not() {
    let mut varied = CircuitBreakers::new(BreakerConfig::default());
    for step in 0..50u64 {
        varied
            .record_step(step)
            .expect("distinct steps never repeat");
    }
    assert!(!varied.is_open());

    let mut repeated = CircuitBreakers::new(BreakerConfig::default());
    let mut tripped = false;
    for _ in 0..200 {
        if repeated.record_step(42).is_err() {
            tripped = true;
            break;
        }
    }
    assert!(tripped, "a repeating step must trip the repetition breaker");
}

#[test]
fn working_memory_never_exceeds_capacity() {
    let mut wm = WorkingMemory::new(WorkingMemoryConfig {
        capacity: 3,
        half_life_nanos: 1_000,
        expiry_threshold: 0.05,
    });
    let ids = ids(52);
    for i in 0..20u64 {
        wm.insert(WorkingMemoryItem {
            thought: ids.thought(),
            content: ThoughtContent::Structured(StructuredValue::Text(format!("item {i}"))),
            weight: 1.0,
            priority: i as f64,
            inserted_at: Timestamp(i as i64),
            touched_at: Timestamp(i as i64),
            provenance: Vec::<ProvenanceId>::new(),
            state: ForgettingState::Temporary,
        });
        assert!(wm.len() <= 3, "capacity is strict");
    }
}
