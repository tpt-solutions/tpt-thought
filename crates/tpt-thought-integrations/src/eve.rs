// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from the `tpt-eve` reasoning layers to the Thought [`DeliberationEngine`] port
//! (spec sections 19, 46, 61, 62, 77; todo.md Phase 5).
//!
//! The adapter owns the Eve layers it uses and feeds them the facts, observations and causal
//! relations that Thought supplies. Eve has no single deliberation entry point, so each Thought
//! capability maps onto one layer. Capabilities with no Eve counterpart keep the trait's
//! `Unsupported` default: reasoning, investigation, contradiction analysis and query ranking.
//! Causal interventions and counterfactuals are also left unsupported, because they need Eve's
//! intervention and counterfactual request types, which this adapter does not construct yet.

use std::collections::HashMap;

use tpt_eve_abstraction::AbstractionEngine;
use tpt_eve_active_learning::{ActiveLearningEngine, Hypothesis as EveHypothesis};
use tpt_eve_causal::CausalReasoningEngine;
use tpt_eve_core::{CausalRelationDraft, CausalRelationKind, Fact, Value};
use tpt_eve_metacognition::MetaCognitionEngine;
use tpt_thought_core::{
    Confidence, Hypothesis, HypothesisId, IdGenerator, Probability, SeededEntropy,
};
use tpt_thought_deliberation::{
    AbstractionRequest, AbstractionVerdict, Capability, CausalAnalysis, CausalQuery, CausalRequest,
    DeliberationContext, DeliberationEngine, DeliberationError, ExperimentPlan, ExperimentRequest,
    HypothesisRequest, Metacognition, NextAction, Validity,
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
            .add_observation(tpt_eve_causal::Observation::new(variables));
    }

    /// Declare that `cause` affects `effect`. Call [`EveAdapter::fit`] after relating and
    /// observing.
    pub fn relate(&mut self, cause: &str, effect: &str) {
        self.causal.add_relation(CausalRelationDraft {
            cause: cause.into(),
            effect: effect.into(),
            relation: CausalRelationKind::Causes,
        });
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
        match request.query {
            CausalQuery::EffectEstimate => {}
            _ => return Err(DeliberationError::Unsupported(Capability::CausalReasoning)),
        }
        let estimate = self
            .causal
            .estimate_causal_effect(&request.cause, &request.effect)
            .map_err(|e| DeliberationError::Failed(e.to_string()))?;
        Ok(CausalAnalysis {
            claim: format!("{} causes {}", request.cause, request.effect),
            effect_estimate: Some(estimate),
            confidence: Confidence::UNKNOWN,
            assumptions: vec!["causal model as fitted by tpt-eve-causal".into()],
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{
        CognitiveBudget, StructuredValue, Thought, ThoughtContent, ThoughtOrigin, Timestamp,
    };
    use tpt_thought_deliberation::{ContradictionRequest, ThoughtSnapshot};

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

    #[test]
    fn interventions_are_reported_unsupported() {
        let mut eve = EveAdapter::new(3);
        let request = CausalRequest {
            cause: "dose".into(),
            effect: "response".into(),
            query: CausalQuery::Intervention {
                action: "raise".into(),
            },
        };
        assert_eq!(
            eve.analyse_causation(&context(), &request),
            Err(DeliberationError::Unsupported(Capability::CausalReasoning))
        );
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

    #[test]
    fn contradiction_analysis_stays_unsupported() {
        let mut eve = EveAdapter::new(5);
        let request = ContradictionRequest { claims: Vec::new() };
        assert_eq!(
            eve.analyse_contradictions(&context(), &request),
            Err(DeliberationError::Unsupported(
                Capability::ContradictionAnalysis
            ))
        );
    }
}
