// SPDX-License-Identifier: MIT OR Apache-2.0
//! Behavioural tests for the closed loop, driven through the public API only.

use tpt_thought_core::{
    Confidence, Probability, StructuredValue, ThoughtState, Timestamp, TransitionKind, Uncertainty,
};
use tpt_thought_deliberation::mock::ScriptedEngine;
use tpt_thought_deliberation::{InvestigationOutcome, InvestigationPolicy, Termination};
use tpt_thought_loop::{
    replay, Action, ActionError, ActionSink, CandidatePlan, CognitiveLoop, CognitiveMode,
    CycleOutcome, CyclePhase, Estimate, Estimator, Input, LoopConfig, Modulation, ModulationError,
    ModulationFields, NullEngine, NullEstimator, ReplayError, ReplayMetadata, ReplayRecord,
    SlowStage, StepProvenance,
};
use tpt_thought_observatory::EventKind;
use tpt_thought_prediction::ErrorLevel;
use tpt_thought_provenance::{Hash256, IntegrityError, ProvenanceSource, ProvenanceStore};
use tpt_thought_providers::{
    Decision, DecisionSource, HumanAnswer, HumanProvider, IntelligenceProvider,
    IntelligenceRequest, Locality, Privacy, ProviderError, ProviderId, ProviderProfile,
    ProviderRegistry, ReferenceProvider,
};

const QUESTION: &str = "can the gripper grasp?";

// ---- Test doubles -------------------------------------------------------------------------------

/// Estimator that always answers with the same estimate.
struct Fixed(Estimate);

impl Estimator for Fixed {
    fn estimate(&self, _question: &str) -> Option<Estimate> {
        Some(self.0)
    }
}

/// Executor that returns the expected outcome, counting calls.
#[derive(Default)]
struct Echo {
    calls: usize,
}

impl ActionSink for Echo {
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError> {
        self.calls += 1;
        Ok(action.expected.clone())
    }
}

/// Executor that returns a fixed outcome, which may differ from the prediction.
struct Returns {
    value: StructuredValue,
}

impl ActionSink for Returns {
    fn execute(&mut self, _action: &Action) -> Result<StructuredValue, ActionError> {
        Ok(self.value.clone())
    }
}

/// A provider that abstains until the request carries an estimate in its context.
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
                Confidence::from_raw(prob(0.95)),
                request.at,
            ))
        } else {
            Ok(Decision::abstained(source, "needs an estimate", request.at))
        }
    }
}

// ---- Helpers ------------------------------------------------------------------------------------

fn prob(p: f64) -> Probability {
    Probability::new(p).expect("valid probability")
}

fn profile() -> ProviderProfile {
    ProviderProfile::new(Locality::Local, Privacy::OnDevice)
}

fn intellect_knows_grasp() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry
        .register(ReferenceProvider::new("intellect", profile()).with_answer(
            QUESTION,
            StructuredValue::Text("grasp".into()),
            prob(0.95),
        ))
        .expect("register");
    registry
}

fn grasp_input(at: i64) -> Input {
    Input {
        at: Timestamp(at),
        source: "camera".into(),
        observation: StructuredValue::Text("object in reach".into()),
        question: QUESTION.into(),
        goal: "pick up the object".into(),
        constraints: vec![],
        memories: vec![],
        candidates: vec![CandidatePlan {
            name: "grasp".into(),
            predicted_outcome: StructuredValue::Text("held".into()),
            cost: 1.0,
            risk: 0.1,
        }],
    }
}

fn neutral_config(seed: u64) -> LoopConfig {
    LoopConfig::new(seed, Modulation::neutral())
}

fn fields(overrides: &[(&str, f64)]) -> ModulationFields {
    let mut f = ModulationFields {
        urgency: 0.5,
        risk: 0.5,
        novelty: 0.5,
        reward: 0.5,
        threat: 0.5,
        confidence: 0.5,
        uncertainty: 0.5,
        arousal: 0.5,
    };
    for (name, value) in overrides {
        match *name {
            "urgency" => f.urgency = *value,
            "risk" => f.risk = *value,
            "novelty" => f.novelty = *value,
            "reward" => f.reward = *value,
            "threat" => f.threat = *value,
            "confidence" => f.confidence = *value,
            "uncertainty" => f.uncertainty = *value,
            "arousal" => f.arousal = *value,
            other => panic!("unknown field {other}"),
        }
    }
    f
}

fn mode_for(overrides: &[(&str, f64)]) -> CognitiveMode {
    Modulation::new(fields(overrides))
        .expect("valid modulation")
        .select_mode()
}

// ---- Cognitive cycle ----------------------------------------------------------------------------

#[test]
fn observe_think_act_learn_consolidate() {
    let mut lp = CognitiveLoop::new(
        neutral_config(7),
        intellect_knows_grasp(),
        NullEngine,
        NullEstimator,
        Echo::default(),
    )
    .expect("loop");

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert_eq!(report.mode, CognitiveMode::Deliberative);
    assert!(
        report.slow_stages.is_empty(),
        "first pass should be accepted"
    );
    match report.outcome {
        CycleOutcome::Completed {
            ref action,
            error,
            level,
        } => {
            assert_eq!(action, "grasp");
            assert_eq!(error, 0.0);
            assert_eq!(level, ErrorLevel::Small);
        }
        other => panic!("expected completion, got {other:?}"),
    }

    let phases: Vec<CyclePhase> = lp.trajectory().iter().map(|s| s.phase).collect();
    assert_eq!(
        phases,
        [
            CyclePhase::Observe,
            CyclePhase::Contextualise,
            CyclePhase::Associate,
            CyclePhase::Hypothesise,
            CyclePhase::Evaluate,
            CyclePhase::Decide,
            CyclePhase::Intend,
            CyclePhase::Act,
            CyclePhase::ObserveResult,
            CyclePhase::Learn,
            CyclePhase::Consolidate,
        ]
    );
    assert_eq!(
        lp.trajectory().last().map(|s| s.state),
        Some(ThoughtState::Complete)
    );
    assert_eq!(lp.sink().calls, 1);
    assert_eq!(lp.observatory().count(EventKind::ThoughtActionCompleted), 1);
    assert_eq!(lp.observatory().count(EventKind::ThoughtPredictionError), 1);
    assert_eq!(lp.intentions().len(), 1);
}

#[test]
fn mismatched_outcome_is_learned_from() {
    let mut lp = CognitiveLoop::new(
        neutral_config(7),
        intellect_knows_grasp(),
        NullEngine,
        NullEstimator,
        Returns {
            value: StructuredValue::Text("dropped".into()),
        },
    )
    .expect("loop");

    let report = lp.run(&grasp_input(100)).expect("cycle");

    match report.outcome {
        CycleOutcome::Completed { error, level, .. } => {
            assert!(error > 0.0, "a mismatch must produce a prediction error");
            assert_ne!(level, ErrorLevel::Small);
        }
        other => panic!("expected completion, got {other:?}"),
    }
}

#[test]
fn probabilistic_steps_are_flagged() {
    let mut lp = CognitiveLoop::new(
        neutral_config(7),
        intellect_knows_grasp(),
        NullEngine,
        NullEstimator,
        Echo::default(),
    )
    .expect("loop");
    lp.run(&grasp_input(100)).expect("cycle");

    let kind_of = |phase: CyclePhase| {
        lp.trajectory()
            .iter()
            .find(|s| s.phase == phase)
            .map(|s| s.transition)
            .expect("phase recorded")
    };
    assert_eq!(kind_of(CyclePhase::Observe), TransitionKind::Deterministic);
    assert_eq!(kind_of(CyclePhase::Decide), TransitionKind::Probabilistic);
    assert_eq!(kind_of(CyclePhase::Act), TransitionKind::Probabilistic);
    assert_eq!(kind_of(CyclePhase::Learn), TransitionKind::Deterministic);
}

#[test]
fn human_provider_answers_as_a_provider() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(HumanProvider::new(
            "alice",
            |request: &IntelligenceRequest| {
                Some(HumanAnswer {
                    value: StructuredValue::Text("grasp".into()),
                    confidence: prob(0.95),
                    context: "checked by hand".into(),
                    answered_at: request.at,
                })
            },
        ))
        .expect("register");
    let mut lp = CognitiveLoop::new(
        neutral_config(7),
        registry,
        NullEngine,
        NullEstimator,
        Echo::default(),
    )
    .expect("loop");

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert!(matches!(report.outcome, CycleOutcome::Completed { .. }));
    let decide = lp
        .trajectory()
        .iter()
        .find(|s| s.phase == CyclePhase::Decide)
        .expect("decide recorded");
    assert!(decide.detail.contains("human:alice"), "{}", decide.detail);
}

#[test]
fn unresolved_question_stops_without_acting() {
    let mut lp = CognitiveLoop::new(
        neutral_config(7),
        ProviderRegistry::new(),
        NullEngine,
        NullEstimator,
        Echo::default(),
    )
    .expect("loop");

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert!(matches!(report.outcome, CycleOutcome::Unresolved { .. }));
    assert_eq!(
        lp.sink().calls,
        0,
        "nothing may be executed when unresolved"
    );
    assert_eq!(
        lp.trajectory().last().map(|s| s.state),
        Some(ThoughtState::Uncertain)
    );
}

// ---- Slow path ----------------------------------------------------------------------------------

#[test]
fn slow_path_runs_in_order_and_resolves() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(NeedsEstimate {
            id: ProviderId::new("intellect"),
            profile: profile(),
        })
        .expect("register");

    let mut config = neutral_config(7);
    config.investigation = InvestigationPolicy::new(prob(0.9), prob(0.5), 1);

    let mut engine = ScriptedEngine::default();
    engine.investigations.push_back(Ok(InvestigationOutcome {
        findings: vec!["gripper pads are dry".into()],
        confidence: Confidence::from_raw(prob(0.1)),
        uncertainty: Uncertainty::unknown(),
        risk: prob(0.0),
        needs_human: false,
    }));

    let estimate = Fixed(Estimate {
        mean: 0.8,
        residual_uncertainty: 0.1,
    });
    let mut lp =
        CognitiveLoop::new(config, registry, engine, estimate, Echo::default()).expect("loop");

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert_eq!(
        report.slow_stages,
        [
            SlowStage::Intellect,
            SlowStage::Eve,
            SlowStage::Augur,
            SlowStage::IntellectReevaluation,
        ]
    );
    assert_eq!(report.investigation, Some(Termination::BudgetExhausted));
    assert!(matches!(report.outcome, CycleOutcome::Completed { .. }));

    let states: Vec<ThoughtState> = lp.trajectory().iter().map(|s| s.state).collect();
    assert!(states.contains(&ThoughtState::Investigating));
    assert!(states.contains(&ThoughtState::ReEvaluating));
    assert_eq!(lp.sink().calls, 1);
}

#[test]
fn augur_estimate_reaches_intellect_reevaluation() {
    // The estimate is only in context during re-evaluation, so the provider answers only then.
    let mut registry = ProviderRegistry::new();
    registry
        .register(NeedsEstimate {
            id: ProviderId::new("intellect"),
            profile: profile(),
        })
        .expect("register");
    let mut lp = CognitiveLoop::new(
        neutral_config(7),
        registry,
        NullEngine,
        Fixed(Estimate {
            mean: 0.8,
            residual_uncertainty: 0.1,
        }),
        Echo::default(),
    )
    .expect("loop");

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert_eq!(
        report.slow_stages,
        [
            SlowStage::Intellect,
            SlowStage::Augur,
            SlowStage::IntellectReevaluation,
        ],
        "Eve is skipped when the engine does not support investigation"
    );
    assert!(matches!(report.outcome, CycleOutcome::Completed { .. }));
}

// ---- Modes and modulation -----------------------------------------------------------------------

#[test]
fn neutral_modulation_is_deliberative() {
    assert_eq!(mode_for(&[]), CognitiveMode::Deliberative);
}

#[test]
fn mode_selection_follows_the_rules() {
    assert_eq!(mode_for(&[("threat", 0.95)]), CognitiveMode::Emergency);
    assert_eq!(
        mode_for(&[("urgency", 0.95), ("risk", 0.9)]),
        CognitiveMode::Emergency
    );
    assert_eq!(
        mode_for(&[("uncertainty", 0.8), ("novelty", 0.6)]),
        CognitiveMode::Investigative
    );
    assert_eq!(mode_for(&[("confidence", 0.1)]), CognitiveMode::Reflective);
    assert_eq!(
        mode_for(&[("novelty", 0.9), ("risk", 0.1)]),
        CognitiveMode::Creative
    );
    assert_eq!(
        mode_for(&[("arousal", 0.9), ("uncertainty", 0.1)]),
        CognitiveMode::Reactive
    );
    assert_eq!(
        mode_for(&[("reward", 0.8), ("urgency", 0.2)]),
        CognitiveMode::Planning
    );
    assert_eq!(
        mode_for(&[
            ("arousal", 0.1),
            ("urgency", 0.1),
            ("reward", 0.1),
            ("novelty", 0.1)
        ]),
        CognitiveMode::Consolidation
    );
}

#[test]
fn emergency_outranks_investigation() {
    // Threat is checked first, so an uncertain, novel, threatening situation is an emergency.
    assert_eq!(
        mode_for(&[("threat", 0.95), ("uncertainty", 0.9), ("novelty", 0.9)]),
        CognitiveMode::Emergency
    );
}

#[test]
fn modulation_rejects_out_of_range_values() {
    let err = Modulation::new(fields(&[("urgency", 1.5)])).unwrap_err();
    assert_eq!(
        err,
        ModulationError {
            field: "urgency",
            value: 1.5
        }
    );
    assert!(Modulation::new(fields(&[("arousal", f64::NAN)])).is_err());
    assert!(Modulation::new(fields(&[("risk", -0.1)])).is_err());
}

#[test]
fn modulation_deserialisation_is_validated() {
    let json = r#"{"urgency":2.0,"risk":0.5,"novelty":0.5,"reward":0.5,"threat":0.5,
        "confidence":0.5,"uncertainty":0.5,"arousal":0.5}"#;
    assert!(serde_json::from_str::<Modulation>(json).is_err());
}

// ---- Curiosity ----------------------------------------------------------------------------------

#[test]
fn surprise_raises_curiosity_when_novel() {
    let mut config = neutral_config(7);
    config.modulation = Modulation::new(fields(&[("novelty", 0.9)])).expect("valid");
    let mut lp = CognitiveLoop::new(
        config,
        intellect_knows_grasp(),
        NullEngine,
        NullEstimator,
        Returns {
            value: StructuredValue::Text("dropped".into()),
        },
    )
    .expect("loop");

    let report = lp.run(&grasp_input(100)).expect("cycle");
    let event = report.curiosity.expect("curiosity should fire");
    assert_eq!(event.question, QUESTION);
    assert!(event.drive >= 0.2);
}

#[test]
fn no_surprise_means_no_curiosity() {
    let mut config = neutral_config(7);
    config.modulation = Modulation::new(fields(&[("novelty", 0.9)])).expect("valid");
    let mut lp = CognitiveLoop::new(
        config,
        intellect_knows_grasp(),
        NullEngine,
        NullEstimator,
        Echo::default(),
    )
    .expect("loop");

    let report = lp.run(&grasp_input(100)).expect("cycle");
    assert!(report.curiosity.is_none());
}

// ---- Replay -------------------------------------------------------------------------------------

fn build(
    config: &LoopConfig,
) -> Result<CognitiveLoop<NullEngine, NullEstimator, Echo>, tpt_thought_loop::LoopError> {
    CognitiveLoop::new(
        config.clone(),
        intellect_knows_grasp(),
        NullEngine,
        NullEstimator,
        Echo::default(),
    )
}

fn captured() -> ReplayRecord {
    ReplayRecord::capture(
        neutral_config(42),
        ReplayMetadata {
            providers: vec!["intellect".into()],
            ..ReplayMetadata::default()
        },
        vec![grasp_input(100), grasp_input(200)],
        build,
    )
    .expect("capture")
}

#[test]
fn recorded_run_reproduces_its_trajectory() {
    let record = captured();
    let reports = replay(&record, build).expect("replay matches");
    assert_eq!(reports.len(), 2);
}

#[test]
fn record_survives_serialisation() {
    let record = captured();
    let json = serde_json::to_string(&record).expect("serialise");
    let restored: ReplayRecord = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(restored, record);
    replay(&restored, build).expect("replay from JSON matches");
}

#[test]
fn tampered_record_is_detected() {
    let mut record = captured();
    record.trajectory[3].detail = "tampered".into();
    match replay(&record, build) {
        Err(ReplayError::Diverged { index, .. }) => assert_eq!(index, 3),
        other => panic!("expected divergence at step 3, got {other:?}"),
    }
}

#[test]
fn truncated_record_is_detected() {
    let mut record = captured();
    record.trajectory.pop();
    assert!(matches!(
        replay(&record, build),
        Err(ReplayError::Length { .. })
    ));
}

/// The first decision record in a captured run: its step and its id.
fn first_decision(record: &mut ReplayRecord) -> &mut StepProvenance {
    record
        .provenance
        .as_mut()
        .expect("captured records carry provenance")
        .iter_mut()
        .find(|e| e.record.body.source == ProvenanceSource::Decision)
        .expect("a decision was recorded")
}

#[test]
fn captured_run_records_provenance_per_step() {
    let record = captured();
    let log = record.provenance.as_ref().expect("provenance captured");
    assert!(!log.is_empty());
    for entry in log {
        // Every provenance record is written by a step of the trajectory.
        assert!(entry.step < record.trajectory.len());
    }
    let decision = log
        .iter()
        .find(|e| e.record.body.source == ProvenanceSource::Decision)
        .expect("a decision was recorded");
    assert_eq!(decision.record.body.provider.as_deref(), Some("intellect"));
    // The recorded chain is intact as captured.
    let sealed = ProvenanceStore::restore(log.iter().map(|e| e.record.clone())).expect("chain");
    assert_eq!(sealed.verify(), Ok(()));
}

#[test]
fn tampered_provider_is_rejected_by_integrity_check() {
    let mut record = captured();
    let decision = first_decision(&mut record);
    let (step, id) = (decision.step, decision.record.body.id);
    decision.record.body.provider = Some("forged".into());
    match replay(&record, build) {
        Err(ReplayError::Integrity {
            step: at,
            error: IntegrityError::HashMismatch { record },
        }) => {
            assert_eq!(record, id);
            assert_eq!(at, step);
        }
        other => panic!("expected an integrity failure, got {other:?}"),
    }
}

#[test]
fn tampered_hash_is_rejected_by_integrity_check() {
    let mut record = captured();
    let decision = first_decision(&mut record);
    let (step, id) = (decision.step, decision.record.body.id);
    decision.record.hash = Hash256([0; 32]);
    match replay(&record, build) {
        Err(ReplayError::Integrity {
            step: at,
            error: IntegrityError::HashMismatch { record },
        }) => {
            assert_eq!(record, id);
            assert_eq!(at, step);
        }
        other => panic!("expected an integrity failure, got {other:?}"),
    }
}

#[test]
fn retagged_provenance_step_is_detected_on_rerun() {
    // The step tag is not part of the hash, so the chain still verifies and the re-run catches it.
    let mut record = captured();
    let decision = first_decision(&mut record);
    let (id, recorded_step) = (decision.record.body.id, decision.step);
    decision.step += 1;
    match replay(&record, build) {
        Err(ReplayError::Provenance {
            step,
            record,
            field,
            expected,
            actual,
        }) => {
            assert_eq!(step, recorded_step + 1);
            assert_eq!(record, id);
            assert_eq!(field, "step");
            assert_eq!(expected, (recorded_step + 1).to_string());
            assert_eq!(actual, recorded_step.to_string());
        }
        other => panic!("expected a provenance mismatch, got {other:?}"),
    }
}

#[test]
fn missing_provenance_records_are_detected() {
    let mut record = captured();
    record.provenance.as_mut().expect("provenance").pop();
    assert!(matches!(
        replay(&record, build),
        Err(ReplayError::ProvenanceLength { .. })
    ));
}

#[test]
fn record_without_provenance_replays_on_trajectory_alone() {
    // Records captured before provenance was recorded have no `provenance` key.
    let record = captured();
    let mut json: serde_json::Value = serde_json::to_value(&record).expect("serialise");
    json.as_object_mut().expect("object").remove("provenance");
    let legacy: ReplayRecord = serde_json::from_value(json).expect("deserialise legacy");
    assert_eq!(legacy.provenance, None);
    replay(&legacy, build).expect("legacy record replays");
}

#[test]
fn provenance_survives_serialisation() {
    let record = captured();
    let json = serde_json::to_string(&record).expect("serialise");
    assert!(json.contains("\"provenance\""));
    let restored: ReplayRecord = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(restored.provenance, record.provenance);
    replay(&restored, build).expect("replay with provenance from JSON matches");
}

// ---- Configuration ------------------------------------------------------------------------------

#[test]
fn bad_thresholds_are_rejected_at_construction() {
    let mut config = neutral_config(7);
    config.ladder = [0.8, 0.5, 0.2];
    assert!(CognitiveLoop::new(
        config,
        ProviderRegistry::new(),
        NullEngine,
        NullEstimator,
        Echo::default()
    )
    .is_err());
}

#[test]
fn accept_threshold_is_validated() {
    let mut config = neutral_config(7);
    config.accept_at = 1.5;
    assert!(CognitiveLoop::new(
        config,
        ProviderRegistry::new(),
        NullEngine,
        NullEstimator,
        Echo::default()
    )
    .is_err());
}

// ---- Policy gate -------------------------------------------------------------------------------

/// A loop whose fast answer is rejected, so every run reaches the policy gate.
fn escalating_loop(
    policy: tpt_thought_policy::PolicySet,
    estimate: Option<Estimate>,
) -> CognitiveLoop<NullEngine, Fixed, Echo> {
    let mut registry = ProviderRegistry::new();
    registry
        .register(NeedsEstimate {
            id: ProviderId::new("intellect"),
            profile: profile(),
        })
        .expect("register");
    let estimator = Fixed(estimate.unwrap_or(Estimate {
        mean: 0.8,
        residual_uncertainty: 0.1,
    }));
    CognitiveLoop::new(
        neutral_config(7),
        registry,
        NullEngine,
        estimator,
        Echo::default(),
    )
    .expect("loop")
    .with_policy(policy)
}

fn policy_returning(decision: tpt_thought_policy::Decision) -> tpt_thought_policy::PolicySet {
    let mut policy = tpt_thought_policy::PolicySet::new();
    policy.push(move |_: &tpt_thought_policy::PolicyContext| decision);
    policy
}

#[test]
fn policy_termination_stops_before_the_slow_path() {
    let policy = policy_returning(tpt_thought_policy::Decision::Terminate(
        tpt_thought_policy::Termination::TooManyBranches {
            branches: 9,
            limit: 4,
        },
    ));
    let mut lp = escalating_loop(policy, None);

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert!(report.slow_stages.is_empty(), "no slow stage may run");
    assert!(
        matches!(report.outcome, CycleOutcome::Unresolved { ref reason } if reason.contains("policy stopped"))
    );
    assert_eq!(lp.sink().calls, 0);
}

#[test]
fn policy_accept_holds_the_uncertain_answer() {
    let mut lp = escalating_loop(policy_returning(tpt_thought_policy::Decision::Accept), None);

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert!(report.slow_stages.is_empty());
    assert!(
        matches!(report.outcome, CycleOutcome::Unresolved { ref reason } if reason.contains("policy accepted"))
    );
    assert_eq!(lp.sink().calls, 0);
}

#[test]
fn low_escalation_level_runs_only_intellect() {
    let policy = policy_returning(tpt_thought_policy::Decision::Escalate(
        tpt_thought_policy::EscalationLevel::StructuredIntelligence,
    ));
    let mut lp = escalating_loop(policy, None);

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert_eq!(
        report.slow_stages,
        [SlowStage::Intellect, SlowStage::IntellectReevaluation],
        "L1 asks Intellect again without Augur or Eve"
    );
}

#[test]
fn probabilistic_level_adds_the_estimate_but_not_investigation() {
    let policy = policy_returning(tpt_thought_policy::Decision::Escalate(
        tpt_thought_policy::EscalationLevel::ProbabilisticInference,
    ));
    let mut lp = escalating_loop(policy, None);

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert_eq!(
        report.slow_stages,
        [
            SlowStage::Intellect,
            SlowStage::Augur,
            SlowStage::IntellectReevaluation,
        ],
        "L3 adds the estimate; investigation needs L4"
    );
    assert!(matches!(report.outcome, CycleOutcome::Completed { .. }));
}

#[test]
fn default_policy_runs_the_full_slow_path() {
    let mut lp = escalating_loop(tpt_thought_loop::default_policy(), None);

    let report = lp.run(&grasp_input(100)).expect("cycle");

    assert_eq!(
        report.slow_stages,
        [
            SlowStage::Intellect,
            SlowStage::Augur,
            SlowStage::IntellectReevaluation,
        ],
        "NullEngine skips Eve, but the full path is requested"
    );
}
