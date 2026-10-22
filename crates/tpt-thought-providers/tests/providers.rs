// SPDX-License-Identifier: MIT OR Apache-2.0
//! Behavioural tests for the provider-neutral layer.

use std::cell::Cell;
use std::rc::Rc;
use tpt_thought_core::{Confidence, Probability, StructuredValue, Timestamp};
use tpt_thought_providers::*;

fn p(x: f64) -> Probability {
    Probability::new(x).expect("test probability in range")
}

fn text(s: &str) -> StructuredValue {
    StructuredValue::Text(s.into())
}

fn local(accuracy: f64) -> ProviderProfile {
    ProviderProfile::new(Locality::Local, Privacy::OnDevice).with_accuracy(p(accuracy))
}

fn request(question: &str) -> IntelligenceRequest {
    IntelligenceRequest::new(question, Timestamp(100))
}

/// Counts how often it is asked, so tests can prove a provider was or was not consulted.
struct Counting {
    inner: ReferenceProvider,
    asked: Rc<Cell<u32>>,
}

impl IntelligenceProvider for Counting {
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
        self.asked.set(self.asked.get() + 1);
        self.inner.decide(request)
    }
}

fn counting(inner: ReferenceProvider) -> (Counting, Rc<Cell<u32>>) {
    let asked = Rc::new(Cell::new(0));
    let provider = Counting {
        inner,
        asked: Rc::clone(&asked),
    };
    (provider, asked)
}

// --- fast path -------------------------------------------------------------------------------

#[test]
fn fast_path_accepts_first_confident_answer_without_escalating() {
    let mut registry = ProviderRegistry::new();
    let (quick, quick_asked) = counting(ReferenceProvider::new("quick", local(0.95)).with_answer(
        "q",
        text("yes"),
        p(0.95),
    ));
    let (slow, slow_asked) =
        counting(ReferenceProvider::new("slow", local(0.5)).with_answer("q", text("no"), p(0.5)));
    registry.register(quick).unwrap();
    registry.register(slow).unwrap();

    let step =
        FastPath::default().respond(&request("q"), &registry, &CalibrationTracker::default());
    let Step::Accept(decision) = step else {
        panic!("expected acceptance, got {step:?}");
    };
    assert_eq!(decision.value(), Some(&text("yes")));
    assert_eq!(quick_asked.get(), 1);
    // The slow provider must not have been consulted: no unnecessary escalation.
    assert_eq!(slow_asked.get(), 0);
}

#[test]
fn fast_path_escalates_when_no_answer_is_confident_enough() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new("a", local(0.8)).with_answer("q", text("maybe"), p(0.5)))
        .unwrap();
    let step =
        FastPath::default().respond(&request("q"), &registry, &CalibrationTracker::default());
    let Step::Escalate(escalation) = step else {
        panic!("expected escalation");
    };
    assert_eq!(escalation.decisions.len(), 1);
    assert!(escalation.failures.is_empty());
    assert!(escalation.disagreement.is_none());
}

// --- abstention ------------------------------------------------------------------------------

#[test]
fn abstention_is_a_result_not_an_error() {
    let provider = ReferenceProvider::new("empty", local(0.9));
    let decision = provider
        .decide(&request("unknown"))
        .expect("abstention is Ok");
    assert!(decision.is_abstention());
    assert!(decision.uncertainty.abstained);
    assert_eq!(decision.reliability(), None);
    assert!(!Constraints::default().accepts(&decision));
}

// --- failure handling ------------------------------------------------------------------------

#[test]
fn provider_failure_is_recorded_and_failover_still_answers() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(FailingProvider::new(
            "down",
            local(0.99),
            ProviderErrorKind::Timeout,
        ))
        .unwrap();
    registry
        .register(ReferenceProvider::new("backup", local(0.7)).with_answer(
            "q",
            text("ok"),
            p(0.95),
        ))
        .unwrap();
    let step =
        FastPath::default().respond(&request("q"), &registry, &CalibrationTracker::default());
    let Step::Accept(decision) = step else {
        panic!("expected failover to answer");
    };
    assert_eq!(decision.value(), Some(&text("ok")));
}

#[test]
fn failures_are_reported_when_nothing_answers() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(FailingProvider::new(
            "down",
            local(0.9),
            ProviderErrorKind::Unavailable,
        ))
        .unwrap();
    let step =
        Deliberative::default().respond(&request("q"), &registry, &CalibrationTracker::default());
    let Step::Escalate(escalation) = step else {
        panic!("expected escalation");
    };
    assert_eq!(escalation.failures.len(), 1);
    assert_eq!(escalation.failures[0].kind, ProviderErrorKind::Unavailable);
    assert_eq!(escalation.reason, "no provider answered");
}

// --- deliberation and disagreement -----------------------------------------------------------

#[test]
fn deliberative_mode_accepts_agreement() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new("a", local(0.9)).with_answer("q", text("x"), p(0.9)))
        .unwrap();
    registry
        .register(ReferenceProvider::new("b", local(0.8)).with_answer("q", text("x"), p(0.8)))
        .unwrap();
    let step =
        Deliberative::default().respond(&request("q"), &registry, &CalibrationTracker::default());
    let Step::Accept(decision) = step else {
        panic!("expected agreement to be accepted");
    };
    assert_eq!(decision.value(), Some(&text("x")));
    // The most reliable agreeing decision is returned.
    assert_eq!(decision.reliability(), Some(p(0.9)));
}

#[test]
fn disagreement_is_a_cognitive_event_with_probes() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new("a", local(0.9)).with_answer("q", text("x"), p(0.9)))
        .unwrap();
    registry
        .register(ReferenceProvider::new("b", local(0.8)).with_answer("q", text("y"), p(0.8)))
        .unwrap();
    let step =
        Deliberative::default().respond(&request("q"), &registry, &CalibrationTracker::default());
    let Step::Escalate(escalation) = step else {
        panic!("disagreement must escalate");
    };
    let disagreement = escalation.disagreement.expect("disagreement recorded");
    assert_eq!(disagreement.positions.len(), 2);
    assert_eq!(disagreement.probes, Probe::ALL.to_vec());
    assert_eq!(escalation.reason, "providers disagree");
}

#[test]
fn combine_ignores_abstentions() {
    let source = DecisionSource::Provider(ProviderId::new("a"));
    let decisions = vec![
        Decision::answered(source.clone(), text("x"), Confidence::UNKNOWN, Timestamp(0)),
        Decision::abstained(source, "no idea", Timestamp(0)),
    ];
    assert_eq!(combine(&decisions), Consensus::Agreed { value: text("x") });
    assert_eq!(combine(&[]), Consensus::Insufficient);
}

#[test]
fn deliberative_limit_bounds_how_many_providers_are_asked() {
    let mut registry = ProviderRegistry::new();
    let (first, first_asked) =
        counting(ReferenceProvider::new("first", local(0.9)).with_answer("q", text("x"), p(0.9)));
    let (second, second_asked) =
        counting(ReferenceProvider::new("second", local(0.5)).with_answer("q", text("y"), p(0.5)));
    registry.register(first).unwrap();
    registry.register(second).unwrap();
    let mode = Deliberative {
        max_providers: Some(1),
    };
    let step = mode.respond(&request("q"), &registry, &CalibrationTracker::default());
    let Step::Accept(decision) = step else {
        panic!("single provider agrees with itself");
    };
    assert_eq!(decision.value(), Some(&text("x")));
    assert_eq!(first_asked.get(), 1);
    assert_eq!(second_asked.get(), 0);
}

// --- constraints -----------------------------------------------------------------------------

#[test]
fn constraints_gate_acceptance_and_admission() {
    let registry_profile = ProviderProfile::new(Locality::Remote, Privacy::External)
        .with_capability("reason:causal")
        .with_cost(10.0)
        .with_latency_nanos(5_000);
    let mut registry = ProviderRegistry::new();
    registry
        .register(
            ReferenceProvider::new("remote", registry_profile).with_answer("q", text("x"), p(0.95)),
        )
        .unwrap();
    let calibration = CalibrationTracker::default();

    let too_costly = Constraints {
        max_cost: Some(5.0),
        ..Constraints::default()
    };
    assert!(registry.candidates(&too_costly, &calibration).is_empty());

    let needs_capability = Constraints {
        required_capabilities: [Capability::new("estimate:cost")].into_iter().collect(),
        ..Constraints::default()
    };
    assert!(registry
        .candidates(&needs_capability, &calibration)
        .is_empty());

    let ok = Constraints {
        required_capabilities: [Capability::new("reason:causal")].into_iter().collect(),
        max_cost: Some(10.0),
        max_latency_nanos: Some(5_000),
        ..Constraints::default()
    };
    assert_eq!(registry.candidates(&ok, &calibration).len(), 1);

    let strict = Constraints {
        min_confidence: Some(p(0.99)),
        ..Constraints::default()
    };
    let decision = registry
        .get(&ProviderId::new("remote"))
        .unwrap()
        .decide(&request("q"))
        .unwrap();
    assert!(!strict.accepts(&decision));

    let risk = Constraints {
        risk_tolerance: Some(p(0.1)),
        ..Constraints::default()
    };
    assert!(risk.accepts(&decision));
}

#[test]
fn unknown_profile_values_never_exclude_a_provider() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new(
            "bare",
            ProviderProfile::new(Locality::Local, Privacy::OnDevice),
        ))
        .unwrap();
    let constraints = Constraints {
        max_cost: Some(1.0),
        max_latency_nanos: Some(1),
        ..Constraints::default()
    };
    assert_eq!(
        registry
            .candidates(&constraints, &CalibrationTracker::default())
            .len(),
        1
    );
}

// --- registry --------------------------------------------------------------------------------

#[test]
fn duplicate_identities_are_rejected() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new("same", local(0.5)))
        .unwrap();
    let err = registry
        .register(ReferenceProvider::new("same", local(0.6)))
        .unwrap_err();
    assert_eq!(err, RegistryError::Duplicate(ProviderId::new("same")));
}

#[test]
fn ranking_is_by_reliability_then_cost_then_identity() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new("mid", local(0.7)))
        .unwrap();
    registry
        .register(ReferenceProvider::new("best", local(0.9)))
        .unwrap();
    registry
        .register(ReferenceProvider::new(
            "unknown",
            ProviderProfile::new(Locality::Local, Privacy::OnDevice),
        ))
        .unwrap();
    let ids: Vec<&str> = registry
        .candidates(&Constraints::default(), &CalibrationTracker::default())
        .iter()
        .map(|c| c.provider.id().as_str())
        .collect();
    assert_eq!(ids, vec!["best", "mid", "unknown"]);
}

#[test]
fn acceptable_providers_restricts_the_pool() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new("a", local(0.9)))
        .unwrap();
    registry
        .register(ReferenceProvider::new("b", local(0.8)))
        .unwrap();
    let constraints = Constraints {
        acceptable_providers: Some([ProviderId::new("b")].into_iter().collect()),
        ..Constraints::default()
    };
    let candidates = registry.candidates(&constraints, &CalibrationTracker::default());
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].provider.id().as_str(), "b");
}

// --- calibration -----------------------------------------------------------------------------

#[test]
fn calibration_tracks_outcomes_and_annotates_decisions() {
    let id = ProviderId::new("a");
    let mut tracker = CalibrationTracker::new(5);
    // Ten claims at 0.9 confidence, half of which hold.
    for i in 0..10 {
        tracker.record(&id, p(0.9), i % 2 == 0);
    }
    assert_eq!(tracker.sample_count(&id), 10);
    assert_eq!(tracker.empirical_reliability(&id), Some(p(0.5)));
    assert_eq!(tracker.calibrated(&id, p(0.95)), Some(p(0.5)));
    // Mean of (0.9 - outcome)^2 over five hits and five misses: (5 * 0.01 + 5 * 0.81) / 10.
    assert!((tracker.brier_score(&id).unwrap() - 0.41).abs() < 1e-9);

    let mut decision = Decision::answered(
        DecisionSource::Provider(id.clone()),
        text("x"),
        Confidence::from_raw(p(0.9)),
        Timestamp(0),
    );
    tracker.annotate(&mut decision);
    assert_eq!(
        decision.confidence.raw,
        Some(p(0.9)),
        "raw is never overwritten"
    );
    assert_eq!(decision.confidence.calibrated, Some(p(0.5)));
    assert_eq!(
        decision.reliability(),
        Some(p(0.5)),
        "empirical wins over calibrated"
    );
}

#[test]
fn calibration_withholds_values_until_enough_samples() {
    let id = ProviderId::new("new");
    let mut tracker = CalibrationTracker::default();
    tracker.record(&id, p(0.9), true);
    assert_eq!(tracker.empirical_reliability(&id), None);
    assert_eq!(tracker.calibrated(&id, p(0.9)), None);
}

#[test]
fn record_decision_skips_abstentions() {
    let mut tracker = CalibrationTracker::new(1);
    let source = DecisionSource::Provider(ProviderId::new("a"));
    let abstained: Decision<StructuredValue> =
        Decision::abstained(source.clone(), "no", Timestamp(0));
    assert!(!tracker.record_decision(&abstained, true));
    let answered = Decision::answered(
        source,
        text("x"),
        Confidence::from_raw(p(0.8)),
        Timestamp(0),
    );
    assert!(tracker.record_decision(&answered, true));
    assert_eq!(tracker.sample_count(&ProviderId::new("a")), 1);
}

#[test]
fn track_record_reorders_candidates() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new("claims_high", local(0.95)))
        .unwrap();
    registry
        .register(ReferenceProvider::new("proves_reliable", local(0.6)))
        .unwrap();
    let mut tracker = CalibrationTracker::new(3);
    let reliable = ProviderId::new("proves_reliable");
    for _ in 0..3 {
        tracker.record(&reliable, p(0.9), true);
    }
    let order: Vec<String> = registry
        .candidates(&Constraints::default(), &tracker)
        .iter()
        .map(|c| c.provider.id().as_str().to_string())
        .collect();
    assert_eq!(order, vec!["proves_reliable", "claims_high"]);
}

// --- human provider --------------------------------------------------------------------------

#[test]
fn human_provider_keeps_identity_context_and_confidence() {
    let human = HumanProvider::new("ops-lead", |req: &IntelligenceRequest| {
        (req.question == "q").then(|| HumanAnswer {
            value: text("approve"),
            confidence: p(0.8),
            context: "seen the site survey".into(),
            answered_at: Timestamp(250),
        })
    });
    assert_eq!(human.id().as_str(), "human:ops-lead");
    let decision = human.decide(&request("q")).unwrap();
    assert_eq!(
        decision.source,
        DecisionSource::Human {
            identity: "ops-lead".into()
        }
    );
    assert_eq!(decision.value(), Some(&text("approve")));
    assert_eq!(decision.confidence.raw, Some(p(0.8)));
    assert_eq!(decision.context.as_deref(), Some("seen the site survey"));
    assert_eq!(decision.decided_at, Timestamp(250));

    let silent = human.decide(&request("other")).unwrap();
    assert!(silent.is_abstention());
}

// --- backend evolution (spec section 90) -----------------------------------------------------

/// A backend with a different internal design: it derives answers by rule rather than by table.
/// Nothing in the cognitive modes knows about it.
struct RuleBackend {
    id: ProviderId,
    profile: ProviderProfile,
}

impl IntelligenceProvider for RuleBackend {
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
        let value = StructuredValue::Int(request.question.len() as i64);
        Ok(Decision::answered(
            source,
            value,
            Confidence::from_raw(p(0.97)),
            request.at,
        ))
    }
}

#[test]
fn new_backend_is_added_without_changing_the_cognitive_model() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(RuleBackend {
            id: ProviderId::new("rules"),
            profile: local(0.9),
        })
        .unwrap();
    // The same mode values that run against the reference backend run unchanged here.
    let modes: [&dyn CognitionMode; 2] = [&FastPath::default(), &Deliberative::default()];
    for mode in modes {
        let step = mode.respond(&request("abc"), &registry, &CalibrationTracker::default());
        let decision = match step {
            Step::Accept(d) => d,
            Step::Escalate(e) => e.decisions.into_iter().next().expect("one decision"),
        };
        assert_eq!(
            decision.value(),
            Some(&StructuredValue::Int(3)),
            "mode {}",
            mode.name()
        );
    }
}

#[test]
fn mode_names_are_distinct() {
    assert_eq!(FastPath::default().name(), "fast");
    assert_eq!(Deliberative::default().name(), "deliberative");
}
