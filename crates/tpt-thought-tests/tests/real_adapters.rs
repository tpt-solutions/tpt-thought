// SPDX-License-Identifier: MIT OR Apache-2.0
//! Acceptance criterion F, run against the real Eve adapter in `tpt-thought-integrations`.
//! Drives the `CognitiveLoop`. Only compiled with the `real-adapters` feature. The mock-backed
//! version of F lives in `acceptance.rs` and is kept unchanged.

#![cfg(feature = "real-adapters")]

mod common;

use common::*;
use tpt_thought_deliberation::InvestigationPolicy;
use tpt_thought_integrations::eve::EveAdapter;
use tpt_thought_integrations::intellect::reference_provider;
use tpt_thought_loop::{CognitiveLoop, CycleOutcome, SlowStage};
use tpt_thought_providers::ProviderRegistry;

/// D (real): the loop asks the real Intellect reference backend, and an accepted answer is
/// recorded as a provenance record. Whether the answer is accepted depends on the reference
/// backend's confidence, so the test asserts the record exists only when the decision is taken.
#[test]
fn real_d_intellect_answer_is_recorded_when_accepted() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(reference_provider(vec!["grasp".into()], profile()))
        .expect("register");
    let mut lp = CognitiveLoop::new(
        reactive_config(9),
        registry,
        NoEngine,
        NoEstimate,
        Echo::default(),
    )
    .expect("loop");

    let report = lp.run(&grasp_input(50)).expect("cycle");

    assert_eq!(
        report.provenance.is_empty(),
        !matches!(report.outcome, CycleOutcome::Completed { .. }),
        "a provenance record exists exactly when the cycle completed its action"
    );
}

/// F (real): the real Eve adapter does not implement `investigate`, so the loop's slow path must
/// skip Eve. This asserts that gap through the loop. It does not show Eve deliberating.
#[test]
fn real_f_eve_adapter_declines_investigation() {
    let mut config = reactive_config(6);
    config.accept_at = 0.8;
    config.investigation = InvestigationPolicy::new(prob(0.9), prob(0.5), 3);
    let mut lp = CognitiveLoop::new(
        config,
        registry_answering("intellect", 0.3),
        EveAdapter::new(6),
        NoEstimate,
        Echo::default(),
    )
    .expect("loop");

    let report = lp.run(&grasp_input(50)).expect("cycle");

    assert!(!report.slow_stages.contains(&SlowStage::Eve));
    assert!(report.investigation.is_none());
}
