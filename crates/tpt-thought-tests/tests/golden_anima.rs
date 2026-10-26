// SPDX-License-Identifier: MIT OR Apache-2.0
//! Acceptance criterion H (todo.md Phase 7, spec section 24): the loop executes an intention, and
//! Thought never runs the action itself. Only compiled with the `real-anima` feature.
//!
//! The loop's executor is `AnimaActionSink`. Anima's INP round chooses the provider, and that
//! provider's executor performs the action. Intellect, Eve and Augur stay as mocks, as in
//! `golden.rs`, so the only real adapter in this scenario is the action boundary.

#![cfg(feature = "real-anima")]

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::*;
use tpt_thought_core::{IntentionId, StructuredValue};
use tpt_thought_integrations::anima::{AnimaActionSink, Claim};
use tpt_thought_intention::IntentionStatus;
use tpt_thought_loop::{Action, ActionError, ActionSink, CognitiveLoop, CycleOutcome, LoopError};

/// A claim for a grasp executor: fast, accurate and confident.
fn grasp_claim() -> Claim {
    Claim {
        estimated_cost: 1.0,
        estimated_latency: Duration::from_millis(50),
        estimated_accuracy: 0.95,
        confidence: 0.9,
    }
}

/// Forwards every action to the Anima sink and keeps the intention identifiers it was asked
/// for, so the test can look the intentions up after the cycle.
struct Recording {
    inner: AnimaActionSink,
    intentions: Vec<IntentionId>,
}

impl ActionSink for Recording {
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError> {
        self.intentions.push(action.intention);
        self.inner.execute(action)
    }
}

/// The status of the intention the loop dispatched for the last action.
fn last_status(lp: &CognitiveLoop<NullEngine, NullEstimator, Recording>) -> IntentionStatus {
    let id = lp
        .sink()
        .intentions
        .last()
        .expect("the loop dispatched an action");
    lp.intentions().get(id).expect("intention is kept").status()
}

#[test]
fn acceptance_h_an_intention_is_executed_through_anima() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let mut inner = AnimaActionSink::new();
    inner.register("grasp", grasp_claim(), move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(StructuredValue::Text("held".into()))
    });
    let mut lp = CognitiveLoop::new(
        neutral_config(31),
        registry_answering("intellect", 0.95),
        NullEngine,
        NullEstimator,
        Recording {
            inner,
            intentions: Vec::new(),
        },
    )
    .expect("loop");

    let report = lp.run(&grasp_input(40)).expect("cycle completes");

    // The provider ran the action once, and the outcome it returned matches the prediction.
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    match report.outcome {
        CycleOutcome::Completed { action, error, .. } => {
            assert_eq!(action, "grasp");
            assert_eq!(error, 0.0, "the executed outcome matches the prediction");
        }
        other => panic!("expected completion, got {other:?}"),
    }
    // The intention went through Dispatched and Executing to Achieved.
    assert_eq!(last_status(&lp), IntentionStatus::Achieved);
}

#[test]
fn acceptance_h_an_intention_with_no_provider_fails_without_executing() {
    let mut lp = CognitiveLoop::new(
        neutral_config(32),
        registry_answering("intellect", 0.95),
        NullEngine,
        NullEstimator,
        Recording {
            inner: AnimaActionSink::new(),
            intentions: Vec::new(),
        },
    )
    .expect("loop");

    let error = lp
        .run(&grasp_input(50))
        .expect_err("no provider, so the action cannot run");
    assert!(
        matches!(error, LoopError::Action(ActionError::Refused(_))),
        "expected a refusal, got {error:?}"
    );
    assert_eq!(last_status(&lp), IntentionStatus::Failed);
}
