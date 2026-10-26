// SPDX-License-Identifier: MIT OR Apache-2.0
//! Acceptance criteria D and F, run against the real adapters in `tpt-thought-integrations`.
//! Both drive the `CognitiveLoop`. Only compiled with the `real-adapters` feature. The mock-backed
//! versions live in `acceptance.rs` and are kept unchanged.

#![cfg(feature = "real-adapters")]

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use common::*;
use tpt_thought_core::StructuredValue;
use tpt_thought_deliberation::{InvestigationPolicy, Termination};
use tpt_thought_integrations::eve::EveAdapter;
use tpt_thought_integrations::intellect::reference_provider;
use tpt_thought_loop::{CognitiveLoop, SlowStage};
use tpt_thought_providers::{
    Decision, IntelligenceProvider, IntelligenceRequest, ProviderError, ProviderId,
    ProviderProfile, ProviderRegistry,
};

/// The real Intellect provider, with a count of how many times the loop asked it to decide.
struct Counted<P> {
    inner: P,
    calls: Arc<AtomicUsize>,
}

impl<P: IntelligenceProvider> IntelligenceProvider for Counted<P> {
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
        self.inner.decide(request)
    }
}

/// D (real): the loop consults the real Intellect reference backend, and the decision it returns
/// is recorded with that backend as its source.
#[test]
fn real_d_intellect_is_consulted_and_recorded_as_source() {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Counted {
        inner: reference_provider(vec!["grasp".into()], profile()),
        calls: calls.clone(),
    };
    let expected = provider.id().as_str().to_owned();
    let mut registry = ProviderRegistry::new();
    registry.register(provider).expect("register");
    let mut lp = CognitiveLoop::new(
        reactive_config(9),
        registry,
        NullEngine,
        NullEstimator,
        Echo::default(),
    )
    .expect("loop");

    let report = lp.run(&grasp_input(50)).expect("cycle");

    assert!(
        calls.load(Ordering::SeqCst) >= 1,
        "the loop asked Intellect"
    );
    let decisions: Vec<_> = report
        .provenance
        .iter()
        .filter_map(|id| lp.provenance().get(id))
        .filter(|record| record.body.source == tpt_thought_provenance::ProvenanceSource::Decision)
        .collect();
    assert!(!decisions.is_empty(), "an accepted decision is recorded");
    for record in decisions {
        assert_eq!(record.body.provider.as_deref(), Some(expected.as_str()));
    }
}

/// F (real): the real Eve adapter implements `investigate`, so the loop's slow path runs an Eve
/// investigation. Eve holds no facts about the grasp, so it reports unknown confidence and maximal
/// risk, and the investigation policy stops on risk.
#[test]
fn real_f_eve_adapter_declines_investigation() {
    let mut config = reactive_config(6);
    config.accept_at = 0.8;
    config.investigation = InvestigationPolicy::new(prob(0.9), prob(0.5), 3);
    let mut lp = CognitiveLoop::new(
        config,
        registry_answering("intellect", 0.3),
        EveAdapter::new(6),
        NullEstimator,
        Echo::default(),
    )
    .expect("loop");

    let report = lp.run(&grasp_input(50)).expect("cycle");

    assert!(report.slow_stages.contains(&SlowStage::Eve));
    assert_eq!(report.investigation, Some(Termination::RiskPolicy));
}
