// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from the `tpt-eve` reasoning layers to the Thought [`DeliberationEngine`] port
//! (spec sections 19, 46, 61, 62, 77; todo.md Phase 5).
//!
//! The adapter owns the Eve layers it uses and feeds them the facts, observations and causal
//! relations that Thought supplies. Eve has no single deliberation entry point, so each Thought
//! capability maps onto one layer. Capabilities with no Eve counterpart keep the trait's
//! `Unsupported` default: reasoning and query ranking.

use std::collections::{HashMap, HashSet};

use tpt_eve_abstraction::AbstractionEngine;
use tpt_eve_active_learning::{ActiveLearningEngine, Hypothesis as EveHypothesis};
use tpt_eve_causal::CausalReasoningEngine;
use tpt_eve_core::{CausalRelationDraft, CausalRelationKind, Fact, Value};
use tpt_eve_metacognition::MetaCognitionEngine;
use tpt_eve_symbolic::ConsistencyChecker;
use tpt_thought_core::{
    Confidence, Hypothesis, HypothesisId, IdGenerator, Probability, SeededEntropy, Uncertainty,
};
use tpt_thought_deliberation::{
    AbstractionRequest, AbstractionVerdict, Capability, CausalAnalysis, CausalQuery, CausalRequest,
    Claim, Conflict, ContradictionAnalysis, ContradictionRequest, DeliberationContext,
    DeliberationEngine, DeliberationError, ExperimentPlan, ExperimentRequest, HypothesisRequest,
    Investigation, InvestigationOutcome, Metacognition, NextAction, Resolution, Validity,
};

/// Knowledge gaps are reported for facts below this confidence.
const GAP_THRESHOLD: f32 = 0.5;

/// Eve-backed deliberation. Facts, observations and causal relations are supplied through the
/// `assert_*`, `observe` and `relate` methods before deliberation.
pub struct EveAdapter {
    ids: IdGenerator,
    learning: ActiveLearningEngine,
    metacognition: MetaCognitionEngine,
    causal: CausalReasoningEngine,
    abstraction: AbstractionEngine,
    hypotheses: Vec<(HypothesisId, EveHypothesis)>,
    baseline: HashMap<String, f64>,
    effects: HashSet<String>,
}

impl EveAdapter {
    /// An empty adapter. Identifiers are drawn from a seeded source, so runs are reproducible.
    pub fn new(seed: u64) -> Self {
        Self {
            ids: IdGenerator::new(SeededEntropy::new(seed)),
            learning: ActiveLearningEngine::new(),
            metacognition: MetaCognitionEngine::new(),
            causal: CausalReasoningEngine::new(),
            abstraction: AbstractionEngine::new(),
            hypotheses: Vec::new(),
            baseline: HashMap::new(),
            effects: HashSet::new(),
        }
    }

    /// Assert a subject-predicate-object fact with full confidence.
    pub fn assert_fact(&mut self, subject: &str, predicate: &str, object: &str) {
        let fact = Fact::asserted(
            Value::Symbol(subject.into()),
            Value::Symbol(predicate.into()),
            Value::Symbol(object.into()),
        );
        self.learning.add_fact(fact.clone());
        self.metacognition.add_fact(fact);
    }

    /// Record one observation of named numeric variables for causal fitting.
    pub fn observe(&mut self, variables: HashMap<String, f64>) {
        self.causal
            .add_observation(tpt_eve_causal::Observation::new(variables.clone()));
        self.baseline = variables;
    }

    /// Declare that `cause` affects `effect`. Call [`EveAdapter::fit`] after relating and
    /// observing.
    pub fn relate(&mut self, cause: &str, effect: &str) {
        self.causal.add_relation(CausalRelationDraft {
            cause: cause.into(),
            effect: effect.into(),
            relation: CausalRelationKind::Causes,
        });
        self.effects.insert(effect.to_string());
    }

    /// Fit the causal model to the relations and observations supplied so far.
    pub fn fit(&mut self) {
        self.causal.fit();
    }

    /// Add a worked example for abstraction mining: a named set of facts.
    pub fn add_example(&mut self, name: &str, facts: Vec<Fact>) {
        self.abstraction.add_example(name, facts);
    }

    /// The Thought view of an Eve hypothesis. A hypothesis already seen keeps its identifier, so
    /// repeated generation over the same facts does not grow the known set.
    fn core_hypothesis(&mut self, eve: EveHypothesis) -> Hypothesis {
        let probability = Probability::new(f64::from(eve.confidence).clamp(0.0, 1.0)).ok();
        let confidence = probability.map_or(Confidence::UNKNOWN, Confidence::from_raw);
        let known = self
            .hypotheses
            .iter()
            .find(|(_, known)| known.description == eve.description)
            .map(|(id, _)| *id);
        let id = match known {
            Some(id) => id,
            None => {
                let id = self.ids.hypothesis();
                self.hypotheses.push((id, eve.clone()));
                id
            }
        };
        Hypothesis {
            id,
            statement: eve.description,
            probability,
            confidence,
        }
    }
}

impl DeliberationEngine for EveAdapter {
    /// One investigation step reads the metacognition layer's view of the facts supplied so far.
    ///
    /// Eve cannot acquire evidence, so a step reports what it knows and nothing more:
    /// - `confidence` is the mean current confidence of the supplied facts, the best Eve has on
    ///   the question. With no facts it is unknown, not zero, since nothing was measured.
    /// - `uncertainty` is the residual gap `1 - confidence`, as epistemic uncertainty because it
    ///   comes from missing knowledge. With no facts the whole answer is marked unknown.
    /// - `risk` is the share of facts below [`GAP_THRESHOLD`], since acting on an answer that
    ///   rests on uncertain facts carries that share of risk. With no facts it is `1.0`.
    /// - `needs_human` is set when any knowledge gap remains. Eve closes gaps only from facts it
    ///   is given, so a remaining gap, or having no facts at all, needs a human source.
    ///
    /// The state does not change between steps, so the same outcome repeats. A run therefore
    /// either stops on its first step (by risk, human need or confidence) or runs to the step cap.
    fn investigate(
        &mut self,
        _ctx: &DeliberationContext,
        _request: &Investigation,
    ) -> Result<InvestigationOutcome, DeliberationError> {
        let records = self.metacognition.records();
        if records.is_empty() {
            return Ok(InvestigationOutcome {
                findings: vec!["no facts supplied to Eve for this question".into()],
                confidence: Confidence::UNKNOWN,
                uncertainty: Uncertainty::unknown(),
                risk: Probability::ONE,
                needs_human: true,
            });
        }
        let count = records.len() as f64;
        let mean = records
            .iter()
            .map(|record| f64::from(record.current()))
            .sum::<f64>()
            / count;
        let uncertain = records
            .iter()
            .filter(|record| record.current() < GAP_THRESHOLD)
            .count();
        let gaps = self.metacognition.detect_gaps(GAP_THRESHOLD);
        let needs_human = !gaps.is_empty();
        let findings = if needs_human {
            gaps.into_iter().map(|gap| gap.description).collect()
        } else {
            vec!["no knowledge gap above the threshold".into()]
        };
        Ok(InvestigationOutcome {
            findings,
            confidence: probability(mean).map_or(Confidence::UNKNOWN, Confidence::from_raw),
            uncertainty: Uncertainty {
                epistemic: probability(1.0 - mean).map(Probability::get),
                ..Uncertainty::default()
            },
            risk: probability(uncertain as f64 / count).unwrap_or(Probability::ONE),
            needs_human,
        })
    }

    fn reflect(&mut self, _ctx: &DeliberationContext) -> Result<Metacognition, DeliberationError> {
        let gaps: Vec<String> = self
            .metacognition
            .detect_gaps(GAP_THRESHOLD)
            .into_iter()
            .map(|gap| gap.description)
            .collect();
        let certainty = Probability::new(
            f64::from(1.0 - self.metacognition.calibration_error()).clamp(0.0, 1.0),
        )
        .ok();
        let next_action = if gaps.is_empty() {
            NextAction::Act
        } else {
            NextAction::Continue
        };
        Ok(Metacognition {
            certainty,
            gaps,
            assumptions: Vec::new(),
            missing_evidence: Vec::new(),
            next_action,
        })
    }

    fn generate_hypotheses(
        &mut self,
        _ctx: &DeliberationContext,
        request: &HypothesisRequest,
    ) -> Result<Vec<Hypothesis>, DeliberationError> {
        let generated = self.learning.generate_hypotheses();
        let limit = request.max_hypotheses as usize;
        Ok(generated
            .into_iter()
            .take(limit)
            .map(|eve| self.core_hypothesis(eve))
            .collect())
    }

    fn analyse_causation(
        &mut self,
        _ctx: &DeliberationContext,
        request: &CausalRequest,
    ) -> Result<CausalAnalysis, DeliberationError> {
        match &request.query {
            CausalQuery::EffectEstimate => {
                let estimate = self
                    .causal
                    .estimate_causal_effect(&request.cause, &request.effect)
                    .map_err(|e| DeliberationError::Failed(e.to_string()))?;
                Ok(CausalAnalysis {
                    claim: format!("{} causes {}", request.cause, request.effect),
                    effect_estimate: Some(estimate),
                    confidence: Confidence::UNKNOWN,
                    assumptions: fitted_model_assumptions(),
                })
            }
            CausalQuery::Intervention { action } => {
                self.require_baseline()?;
                let assignments = parse_assignments(action)?;
                let [(variable, value)] = assignments.as_slice() else {
                    return Err(DeliberationError::Failed(
                        "an intervention sets exactly one variable".into(),
                    ));
                };
                let prediction = self
                    .causal
                    .predict_intervention(
                        tpt_eve_causal::Intervention::new(variable.clone(), *value),
                        &request.effect,
                        &self.root_values(),
                    )
                    .map_err(|e| DeliberationError::Failed(e.to_string()))?;
                Ok(CausalAnalysis {
                    claim: format!(
                        "do({variable}={value}) gives {} = {:.4}",
                        request.effect, prediction.expected_value
                    ),
                    effect_estimate: Some(prediction.expected_value),
                    confidence: causal_confidence(prediction.confidence),
                    assumptions: fitted_model_assumptions(),
                })
            }
            CausalQuery::Counterfactual { condition } => {
                self.require_baseline()?;
                let changes: HashMap<String, f64> =
                    parse_assignments(condition)?.into_iter().collect();
                let simulation = self
                    .causal
                    .simulate_counterfactual(tpt_eve_causal::Counterfactual::new(
                        self.baseline.clone(),
                        changes,
                    ))
                    .map_err(|e| DeliberationError::Failed(e.to_string()))?;
                let effect = simulation
                    .counterfactual_world
                    .get(&request.effect)
                    .copied()
                    .ok_or_else(|| {
                        DeliberationError::Failed(format!(
                            "'{}' is not in the counterfactual world",
                            request.effect
                        ))
                    })?;
                Ok(CausalAnalysis {
                    claim: format!("if {condition}, then {} = {effect:.4}", request.effect),
                    effect_estimate: Some(effect),
                    confidence: Confidence::UNKNOWN,
                    assumptions: fitted_model_assumptions(),
                })
            }
            _ => Err(DeliberationError::Unsupported(Capability::CausalReasoning)),
        }
    }

    fn validate_abstraction(
        &mut self,
        _ctx: &DeliberationContext,
        request: &AbstractionRequest,
    ) -> Result<AbstractionVerdict, DeliberationError> {
        let support_needed = request.evidence.len().max(1);
        // Mine at minimum support 1 so partially supported principles stay visible and can be
        // reported as Conditional, rather than being filtered out as Invalid.
        let principles = self.abstraction.mine_principles(1);
        let found = principles.iter().find(|p| p.name == request.abstraction);
        let (validity, rationale) = match found {
            Some(p) if p.support >= support_needed => (
                Validity::Valid,
                format!("mined from {} examples", p.support),
            ),
            Some(p) => (
                Validity::Conditional,
                format!("only {} of {support_needed} examples support it", p.support),
            ),
            None => (
                Validity::Invalid,
                "no principle mined from the supplied examples".into(),
            ),
        };
        let rationale = match &request.analogy_target {
            Some(_) => format!("{rationale}; analogy not validated by this adapter"),
            None => rationale,
        };
        Ok(AbstractionVerdict {
            validity,
            rationale,
        })
    }

    fn analyse_contradictions(
        &mut self,
        _ctx: &DeliberationContext,
        request: &ContradictionRequest,
    ) -> Result<ContradictionAnalysis, DeliberationError> {
        let claims = request
            .claims
            .iter()
            .map(parse_claim)
            .collect::<Result<Vec<_>, _>>()?;
        let facts: Vec<Fact> = claims
            .iter()
            .map(|claim| {
                Fact::asserted(
                    Value::Symbol(claim.subject.clone()),
                    Value::Symbol(claim.predicate.clone()),
                    Value::Text(claim.object.clone()),
                )
            })
            .collect();
        // Eve reports a key with several facts, so identical repeats are filtered out below.
        let mut conflicts = Vec::new();
        for contradiction in ConsistencyChecker::new().check(&facts) {
            let subject = contradiction.subject.as_str();
            let predicate = contradiction.predicate.as_str();
            let group: Vec<&ParsedClaim> = claims
                .iter()
                .filter(|claim| claim.subject == subject && claim.predicate == predicate)
                .collect();
            for (i, left) in group.iter().enumerate() {
                for right in &group[i + 1..] {
                    if left.object != right.object {
                        conflicts.push(Conflict {
                            left: left.label.clone(),
                            right: right.label.clone(),
                            explanation: format!(
                                "{subject} {predicate} both \"{}\" and \"{}\"",
                                left.object, right.object
                            ),
                            resolution: Resolution::NewEvidence,
                        });
                    }
                }
            }
        }
        conflicts.sort_by(|a, b| (&a.left, &a.right).cmp(&(&b.left, &b.right)));
        Ok(ContradictionAnalysis { conflicts })
    }

    fn design_experiment(
        &mut self,
        _ctx: &DeliberationContext,
        request: &ExperimentRequest,
    ) -> Result<ExperimentPlan, DeliberationError> {
        if request.max_cost.is_some() {
            // Eve does not estimate experiment cost, so a cost cap cannot be checked here.
            return Err(DeliberationError::Failed(
                "Eve cannot estimate experiment cost, so max_cost cannot be honoured".into(),
            ));
        }
        let id = request
            .hypothesis
            .ok_or_else(|| DeliberationError::Failed("no hypothesis to test".into()))?;
        let (_, eve) = self
            .hypotheses
            .iter()
            .find(|(known, _)| *known == id)
            .ok_or_else(|| DeliberationError::Failed("unknown hypothesis".into()))?;
        let design = self.learning.design_experiment(eve);
        Ok(ExperimentPlan {
            steps: vec![design.name],
            expected_information_gain: f64::from(1.0 - eve.confidence).clamp(0.0, 1.0),
            estimated_cost: 0.0,
        })
    }
}

impl EveAdapter {
    /// The observed values of root variables only: those no relation declares as an effect.
    /// Eve seeds its propagation with the whole baseline, so an observed effect would be carried
    /// through unchanged instead of being predicted from the intervention.
    fn root_values(&self) -> HashMap<String, f64> {
        self.baseline
            .iter()
            .filter(|(name, _)| !self.effects.contains(*name))
            .map(|(name, value)| (name.clone(), *value))
            .collect()
    }

    fn require_baseline(&self) -> Result<(), DeliberationError> {
        if self.baseline.is_empty() {
            Err(DeliberationError::Failed(
                "no observation supplied; call observe() before asking about interventions or counterfactuals"
                    .into(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Parse `variable=value` pairs separated by commas, for example `dose=2, age=40`.
fn parse_assignments(text: &str) -> Result<Vec<(String, f64)>, DeliberationError> {
    text.split(',')
        .map(|part| {
            let (name, raw) = part.split_once('=').ok_or_else(|| {
                DeliberationError::Failed(format!("expected variable=value, got '{part}'"))
            })?;
            let value: f64 = raw.trim().parse().map_err(|_| {
                DeliberationError::Failed(format!("'{}' is not a number", raw.trim()))
            })?;
            Ok((name.trim().to_string(), value))
        })
        .collect()
}

/// A claim split into the `subject predicate object` form Eve's facts use. The subject and
/// predicate are the first two words, and the object is the rest of the statement.
struct ParsedClaim {
    label: String,
    subject: String,
    predicate: String,
    object: String,
}

fn parse_claim(claim: &Claim) -> Result<ParsedClaim, DeliberationError> {
    let malformed = || {
        DeliberationError::Failed(format!(
            "claim '{}' is not in `subject predicate object` form",
            claim.label
        ))
    };
    let mut words = claim.statement.split_whitespace();
    let (Some(subject), Some(predicate)) = (words.next(), words.next()) else {
        return Err(malformed());
    };
    let object = words.collect::<Vec<_>>().join(" ");
    if object.is_empty() {
        return Err(malformed());
    }
    Ok(ParsedClaim {
        label: claim.label.clone(),
        subject: subject.to_string(),
        predicate: predicate.to_string(),
        object,
    })
}

/// Eve's intervention confidence is a rough score in `0..=1`; anything outside that is unknown.
fn causal_confidence(value: f64) -> Confidence {
    probability(value).map_or(Confidence::UNKNOWN, Confidence::from_raw)
}

/// A probability from a value clamped to `0..=1`, or `None` for NaN.
fn probability(value: f64) -> Option<Probability> {
    Probability::new(value.clamp(0.0, 1.0)).ok()
}

fn fitted_model_assumptions() -> Vec<String> {
    vec![
        "linear structural causal model fitted by tpt-eve-causal".into(),
        "interventions use the root variables of the most recent observation".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{
        CognitiveBudget, StructuredValue, Thought, ThoughtContent, ThoughtOrigin, Timestamp,
    };
    use tpt_thought_deliberation::{
        run_investigation, ContradictionRequest, InvestigationPolicy, Termination, ThoughtSnapshot,
    };

    fn context() -> DeliberationContext {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let thought = Thought::new(
            ids.thought(),
            ThoughtOrigin::Internal,
            ThoughtContent::Structured(StructuredValue::Null),
            Timestamp(0),
        );
        DeliberationContext {
            thought: ThoughtSnapshot::from_thought(&thought),
            budget: CognitiveBudget::UNLIMITED,
        }
    }

    #[test]
    fn metacognition_reports_gaps_from_asserted_facts() {
        let mut eve = EveAdapter::new(1);
        eve.assert_fact("kettle", "is", "hot");
        let report = eve.reflect(&context()).unwrap();
        assert!(report.certainty.is_some());
    }

    fn investigate_once(eve: &mut EveAdapter) -> InvestigationOutcome {
        let request = Investigation {
            question: "can the gripper grasp?".into(),
            step: 0,
        };
        eve.investigate(&context(), &request).unwrap()
    }

    #[test]
    fn investigation_without_facts_needs_a_human() {
        let mut eve = EveAdapter::new(7);
        let outcome = investigate_once(&mut eve);
        assert_eq!(outcome.confidence, Confidence::UNKNOWN);
        assert!(outcome.uncertainty.unknown);
        assert_eq!(outcome.risk.get(), 1.0);
        assert!(outcome.needs_human);
    }

    #[test]
    fn investigation_reports_full_confidence_from_asserted_facts() {
        let mut eve = EveAdapter::new(8);
        // Each object is also a subject, so no entity is left unexplained.
        eve.assert_fact("gripper", "describes", "cup");
        eve.assert_fact("cup", "describes", "gripper");
        let outcome = investigate_once(&mut eve);
        assert_eq!(outcome.confidence.best().map(Probability::get), Some(1.0));
        assert_eq!(outcome.uncertainty.epistemic, Some(0.0));
        assert_eq!(outcome.risk.get(), 0.0);
        assert!(!outcome.needs_human);
    }

    #[test]
    fn low_confidence_facts_are_gaps_that_need_a_human() {
        let mut eve = EveAdapter::new(9);
        let mut fact = Fact::asserted(
            Value::Symbol("kettle".into()),
            Value::Symbol("is".into()),
            Value::Symbol("hot".into()),
        );
        fact.confidence = 0.2;
        eve.metacognition.add_fact(fact);
        let outcome = investigate_once(&mut eve);
        let confidence = outcome.confidence.best().expect("a confidence").get();
        assert!(
            (confidence - 0.2).abs() < 1e-6,
            "confidence was {confidence}"
        );
        let epistemic = outcome.uncertainty.epistemic.expect("a residual gap");
        assert!((epistemic - 0.8).abs() < 1e-6, "epistemic was {epistemic}");
        assert_eq!(outcome.risk.get(), 1.0);
        assert!(outcome.needs_human);
        assert!(!outcome.findings.is_empty());
    }

    #[test]
    fn investigation_without_facts_stops_on_risk() {
        let mut eve = EveAdapter::new(10);
        let policy = InvestigationPolicy::new(
            Probability::new(0.9).unwrap(),
            Probability::new(0.5).unwrap(),
            3,
        );
        let run =
            run_investigation(&mut eve, &context(), "can the gripper grasp?", &policy).unwrap();
        assert_eq!(run.termination, Termination::RiskPolicy);
        assert_eq!(run.outcomes.len(), 1);
    }

    #[test]
    fn causal_effect_is_estimated_from_observations() {
        let mut eve = EveAdapter::new(2);
        eve.relate("dose", "response");
        for i in 0..20 {
            let dose = f64::from(i);
            let mut vars = HashMap::new();
            vars.insert("dose".to_string(), dose);
            vars.insert("response".to_string(), 2.0 * dose);
            eve.observe(vars);
        }
        eve.fit();
        let request = CausalRequest {
            cause: "dose".into(),
            effect: "response".into(),
            query: CausalQuery::EffectEstimate,
        };
        let analysis = eve.analyse_causation(&context(), &request).unwrap();
        let estimate = analysis.effect_estimate.expect("an estimate");
        assert!((estimate - 2.0).abs() < 0.5, "estimate was {estimate}");
    }

    /// A linear dose-response model (response = 2 * dose) fitted from 20 observations, with the
    /// most recent observation at dose 10.
    fn fitted_dose_response(seed: u64) -> EveAdapter {
        let mut eve = EveAdapter::new(seed);
        eve.relate("dose", "response");
        for i in 0..20 {
            let dose = f64::from(i);
            let mut vars = HashMap::new();
            vars.insert("dose".to_string(), dose);
            vars.insert("response".to_string(), 2.0 * dose);
            eve.observe(vars);
        }
        let mut latest = HashMap::new();
        latest.insert("dose".to_string(), 10.0);
        latest.insert("response".to_string(), 20.0);
        eve.observe(latest);
        eve.fit();
        eve
    }

    fn causal(cause: &str, effect: &str, query: CausalQuery) -> CausalRequest {
        CausalRequest {
            cause: cause.into(),
            effect: effect.into(),
            query,
        }
    }

    #[test]
    fn intervention_changes_the_predicted_effect() {
        let mut eve = fitted_dose_response(3);
        let low = eve
            .analyse_causation(
                &context(),
                &causal(
                    "dose",
                    "response",
                    CausalQuery::Intervention {
                        action: "dose=0".into(),
                    },
                ),
            )
            .unwrap()
            .effect_estimate
            .expect("a prediction");
        let high = eve
            .analyse_causation(
                &context(),
                &causal(
                    "dose",
                    "response",
                    CausalQuery::Intervention {
                        action: "dose=5".into(),
                    },
                ),
            )
            .unwrap()
            .effect_estimate
            .expect("a prediction");
        assert!(low.abs() < 0.5, "do(dose=0) gave {low}");
        assert!((high - 10.0).abs() < 0.5, "do(dose=5) gave {high}");
        assert!(high > low + 5.0, "the intervention must change the effect");
    }

    #[test]
    fn counterfactual_changes_the_simulated_effect() {
        let mut eve = fitted_dose_response(4);
        let analysis = eve
            .analyse_causation(
                &context(),
                &causal(
                    "dose",
                    "response",
                    CausalQuery::Counterfactual {
                        condition: "dose=3".into(),
                    },
                ),
            )
            .unwrap();
        let effect = analysis.effect_estimate.expect("a simulated effect");
        assert!((effect - 6.0).abs() < 0.5, "counterfactual gave {effect}");
        assert!(analysis.claim.starts_with("if dose=3"));
    }

    #[test]
    fn interventions_need_an_observation_first() {
        let mut eve = EveAdapter::new(5);
        eve.relate("dose", "response");
        eve.fit();
        let result = eve.analyse_causation(
            &context(),
            &causal(
                "dose",
                "response",
                CausalQuery::Intervention {
                    action: "dose=1".into(),
                },
            ),
        );
        assert!(matches!(result, Err(DeliberationError::Failed(_))));
    }

    #[test]
    fn an_intervention_must_set_exactly_one_variable() {
        let mut eve = fitted_dose_response(6);
        for action in ["dose=1,age=2", "raise", "dose=high"] {
            let result = eve.analyse_causation(
                &context(),
                &causal(
                    "dose",
                    "response",
                    CausalQuery::Intervention {
                        action: action.into(),
                    },
                ),
            );
            assert!(
                matches!(result, Err(DeliberationError::Failed(_))),
                "'{action}' should be rejected"
            );
        }
    }

    #[test]
    fn hypotheses_from_facts_can_be_tested_by_experiment() {
        let mut eve = EveAdapter::new(6);
        eve.assert_fact("kettle", "is", "hot");
        eve.assert_fact("kettle", "is", "warm");
        let request = HypothesisRequest {
            question: "why is the kettle hot".into(),
            max_hypotheses: 5,
        };
        let hypotheses = eve.generate_hypotheses(&context(), &request).unwrap();
        assert!(hypotheses.len() <= 5);
        for hypothesis in &hypotheses {
            assert!(!hypothesis.statement.is_empty());
        }
        if let Some(first) = hypotheses.first() {
            let plan = eve
                .design_experiment(
                    &context(),
                    &ExperimentRequest {
                        question: request.question.clone(),
                        hypothesis: Some(first.id),
                        max_cost: None,
                    },
                )
                .unwrap();
            assert!(!plan.steps.is_empty());
        }
    }

    #[test]
    fn unknown_hypothesis_cannot_be_tested() {
        let mut eve = EveAdapter::new(4);
        let request = ExperimentRequest {
            question: "does it hold".into(),
            hypothesis: None,
            max_cost: None,
        };
        assert!(eve.design_experiment(&context(), &request).is_err());
    }

    #[test]
    fn regenerating_hypotheses_does_not_duplicate_them() {
        let mut eve = EveAdapter::new(7);
        eve.assert_fact("kettle", "is", "hot");
        eve.assert_fact("kettle", "is", "warm");
        let request = HypothesisRequest {
            question: "why is the kettle hot".into(),
            max_hypotheses: 5,
        };
        let first = eve.generate_hypotheses(&context(), &request).unwrap();
        let second = eve.generate_hypotheses(&context(), &request).unwrap();
        let ids: Vec<_> = first.iter().map(|h| h.id).collect();
        assert_eq!(ids, second.iter().map(|h| h.id).collect::<Vec<_>>());
        assert_eq!(eve.hypotheses.len(), first.len());
    }

    #[test]
    fn experiments_refuse_a_cost_cap_they_cannot_check() {
        let mut eve = EveAdapter::new(8);
        let request = ExperimentRequest {
            question: "does it hold".into(),
            hypothesis: None,
            max_cost: Some(10.0),
        };
        assert!(eve.design_experiment(&context(), &request).is_err());
    }

    fn claim(label: &str, statement: &str) -> Claim {
        Claim {
            label: label.into(),
            statement: statement.into(),
        }
    }

    #[test]
    fn exclusive_predicates_with_different_objects_conflict() {
        let mut eve = EveAdapter::new(5);
        let request = ContradictionRequest {
            claims: vec![
                claim("a", "TypeScript is a programming language"),
                claim("b", "TypeScript is a database"),
                claim("c", "Rust is a programming language"),
            ],
        };
        let analysis = eve.analyse_contradictions(&context(), &request).unwrap();
        assert_eq!(analysis.conflicts.len(), 1);
        let conflict = &analysis.conflicts[0];
        assert_eq!(
            (conflict.left.as_str(), conflict.right.as_str()),
            ("a", "b")
        );
        assert_eq!(conflict.resolution, Resolution::NewEvidence);
    }

    #[test]
    fn repeated_identical_claims_do_not_conflict() {
        let mut eve = EveAdapter::new(5);
        let request = ContradictionRequest {
            claims: vec![
                claim("a", "Rust is memory safe"),
                claim("b", "Rust is memory safe"),
            ],
        };
        let analysis = eve.analyse_contradictions(&context(), &request).unwrap();
        assert!(analysis.conflicts.is_empty());
    }

    #[test]
    fn non_exclusive_predicates_do_not_conflict() {
        let mut eve = EveAdapter::new(5);
        let request = ContradictionRequest {
            claims: vec![claim("a", "cat has tail"), claim("b", "cat has fur")],
        };
        let analysis = eve.analyse_contradictions(&context(), &request).unwrap();
        assert!(analysis.conflicts.is_empty());
    }

    #[test]
    fn malformed_claims_are_refused_rather_than_skipped() {
        let mut eve = EveAdapter::new(5);
        let request = ContradictionRequest {
            claims: vec![claim("a", "Rust is"), claim("b", "Rust is fast")],
        };
        assert!(matches!(
            eve.analyse_contradictions(&context(), &request),
            Err(DeliberationError::Failed(_))
        ));
    }
}
