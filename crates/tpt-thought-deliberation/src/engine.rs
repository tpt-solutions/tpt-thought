// SPDX-License-Identifier: MIT OR Apache-2.0
//! The [`DeliberationEngine`] trait and the request and result shapes of its capabilities.

use crate::investigation::{Investigation, InvestigationOutcome};
use crate::metacognition::Metacognition;
use crate::snapshot::DeliberationContext;
use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_core::{Confidence, Hypothesis, HypothesisId, Uncertainty};

/// An operation a [`DeliberationEngine`] may support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Capability {
    /// Produce a conclusion with confidence and uncertainty.
    Reason,
    /// Run one step of investigation.
    Investigate,
    /// Assess the engine's own state.
    Reflect,
    /// Generate candidate hypotheses.
    HypothesisGeneration,
    /// Analyse conflicts between claims.
    ContradictionAnalysis,
    /// Causal reasoning: interventions, counterfactuals and effect estimation.
    CausalReasoning,
    /// Validate abstractions and analogies (spec section 77).
    AbstractionValidation,
    /// Design an experiment to test a hypothesis.
    ExperimentDesign,
    /// Rank candidate queries by expected information gain.
    ActiveLearning,
}

/// Why a deliberation request did not produce a result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DeliberationError {
    /// The engine does not implement this capability.
    Unsupported(Capability),
    /// The engine attempted the capability and failed.
    Failed(String),
}

impl fmt::Display for DeliberationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(cap) => write!(f, "engine does not support {cap:?}"),
            Self::Failed(reason) => write!(f, "deliberation failed: {reason}"),
        }
    }
}

impl std::error::Error for DeliberationError {}

/// The result of [`DeliberationEngine::reason`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reasoning {
    /// Candidate explanations considered.
    pub hypotheses: Vec<Hypothesis>,
    /// Confidence in the conclusion.
    pub confidence: Confidence,
    /// Uncertainty in the conclusion.
    pub uncertainty: Uncertainty,
    /// The engine's self-assessment.
    pub metacognition: Metacognition,
}

/// Ask for candidate hypotheses about a question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HypothesisRequest {
    /// The question to explain.
    pub question: String,
    /// Upper bound on the number of hypotheses to return.
    pub max_hypotheses: u32,
}

/// A labelled claim submitted for contradiction analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// Caller-chosen label, echoed back in [`Conflict`].
    pub label: String,
    /// The claim itself.
    pub statement: String,
}

/// Claims to check pairwise for contradiction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionRequest {
    /// Claims to analyse.
    pub claims: Vec<Claim>,
}

/// How a contradiction may be resolved (spec section 29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Resolution {
    /// New evidence would decide between the claims.
    NewEvidence,
    /// The claims hold in different contexts.
    ContextSeparation,
    /// Each claim holds only under stated conditions.
    ConditionalValidity,
    /// The engine cannot resolve the conflict.
    Unresolved,
}

/// A contradiction between two labelled claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// Label of the first claim.
    pub left: String,
    /// Label of the second claim.
    pub right: String,
    /// Why the claims conflict.
    pub explanation: String,
    /// Suggested resolution.
    pub resolution: Resolution,
}

/// The result of [`DeliberationEngine::analyse_contradictions`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionAnalysis {
    /// Conflicts found, if any.
    pub conflicts: Vec<Conflict>,
}

/// The kind of causal question asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CausalQuery {
    /// What happens to the effect if the cause is set by intervention.
    Intervention {
        /// The intervention applied.
        action: String,
    },
    /// What would have happened under a different condition.
    Counterfactual {
        /// The counterfactual condition.
        condition: String,
    },
    /// How strongly the cause affects the effect.
    EffectEstimate,
}

/// A causal question about a cause and an effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalRequest {
    /// The putative cause.
    pub cause: String,
    /// The putative effect.
    pub effect: String,
    /// What to ask about the pair.
    pub query: CausalQuery,
}

/// The result of [`DeliberationEngine::analyse_causation`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausalAnalysis {
    /// The causal claim the engine arrived at.
    pub claim: String,
    /// Estimated effect size, if the engine computed one.
    pub effect_estimate: Option<f64>,
    /// Confidence in the claim.
    pub confidence: Confidence,
    /// Assumptions the claim rests on.
    pub assumptions: Vec<String>,
}

/// Whether an abstraction or analogy holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Validity {
    /// The abstraction is supported by the evidence.
    Valid,
    /// The abstraction is contradicted by the evidence.
    Invalid,
    /// The abstraction holds only under conditions.
    Conditional,
}

/// An abstraction or analogy to validate (spec section 77).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbstractionRequest {
    /// The abstraction or analogy proposed.
    pub abstraction: String,
    /// Episodes or observations it was drawn from.
    pub evidence: Vec<String>,
    /// The target the analogy is applied to, if it is an analogy.
    pub analogy_target: Option<String>,
}

/// The result of [`DeliberationEngine::validate_abstraction`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbstractionVerdict {
    /// Whether the abstraction holds.
    pub validity: Validity,
    /// Why.
    pub rationale: String,
}

/// A request for an experiment that would test a hypothesis or question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentRequest {
    /// The question the experiment should answer.
    pub question: String,
    /// The hypothesis under test, if any.
    pub hypothesis: Option<HypothesisId>,
    /// Maximum acceptable cost, in the budget's cost units.
    pub max_cost: Option<f64>,
}

/// A proposed experiment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentPlan {
    /// Ordered steps.
    pub steps: Vec<String>,
    /// Expected reduction in uncertainty from running it.
    pub expected_information_gain: f64,
    /// Estimated cost, in the budget's cost units.
    pub estimated_cost: f64,
}

/// Candidate queries to rank for active learning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLearningRequest {
    /// Candidate queries.
    pub candidates: Vec<String>,
}

/// A query with its expected information gain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryCandidate {
    /// The query.
    pub query: String,
    /// Expected information gain from asking it.
    pub expected_information_gain: f64,
}

/// A reasoning engine that deliberation can drive (spec section 46).
///
/// Methods take `&mut self` because engines may keep state between calls, such as caches or
/// scripted responses. Every method defaults to [`DeliberationError::Unsupported`], so an
/// implementation overrides only the capabilities it has.
pub trait DeliberationEngine {
    /// Produce a conclusion for the thought in `ctx`.
    fn reason(&mut self, _ctx: &DeliberationContext) -> Result<Reasoning, DeliberationError> {
        Err(DeliberationError::Unsupported(Capability::Reason))
    }

    /// Run one step of investigation. Driven repeatedly by [`crate::run_investigation`].
    fn investigate(
        &mut self,
        _ctx: &DeliberationContext,
        _request: &Investigation,
    ) -> Result<InvestigationOutcome, DeliberationError> {
        Err(DeliberationError::Unsupported(Capability::Investigate))
    }

    /// Assess the engine's own state for the thought in `ctx`.
    fn reflect(&mut self, _ctx: &DeliberationContext) -> Result<Metacognition, DeliberationError> {
        Err(DeliberationError::Unsupported(Capability::Reflect))
    }

    /// Generate candidate hypotheses.
    fn generate_hypotheses(
        &mut self,
        _ctx: &DeliberationContext,
        _request: &HypothesisRequest,
    ) -> Result<Vec<Hypothesis>, DeliberationError> {
        Err(DeliberationError::Unsupported(
            Capability::HypothesisGeneration,
        ))
    }

    /// Analyse conflicts between claims.
    fn analyse_contradictions(
        &mut self,
        _ctx: &DeliberationContext,
        _request: &ContradictionRequest,
    ) -> Result<ContradictionAnalysis, DeliberationError> {
        Err(DeliberationError::Unsupported(
            Capability::ContradictionAnalysis,
        ))
    }

    /// Answer a causal question.
    fn analyse_causation(
        &mut self,
        _ctx: &DeliberationContext,
        _request: &CausalRequest,
    ) -> Result<CausalAnalysis, DeliberationError> {
        Err(DeliberationError::Unsupported(Capability::CausalReasoning))
    }

    /// Validate an abstraction or analogy.
    fn validate_abstraction(
        &mut self,
        _ctx: &DeliberationContext,
        _request: &AbstractionRequest,
    ) -> Result<AbstractionVerdict, DeliberationError> {
        Err(DeliberationError::Unsupported(
            Capability::AbstractionValidation,
        ))
    }

    /// Design an experiment.
    fn design_experiment(
        &mut self,
        _ctx: &DeliberationContext,
        _request: &ExperimentRequest,
    ) -> Result<ExperimentPlan, DeliberationError> {
        Err(DeliberationError::Unsupported(Capability::ExperimentDesign))
    }

    /// Rank candidate queries by expected information gain.
    fn rank_queries(
        &mut self,
        _ctx: &DeliberationContext,
        _request: &ActiveLearningRequest,
    ) -> Result<Vec<QueryCandidate>, DeliberationError> {
        Err(DeliberationError::Unsupported(Capability::ActiveLearning))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::ScriptedEngine;
    use crate::snapshot::ThoughtSnapshot;
    use tpt_thought_core::{
        CognitiveBudget, IdGenerator, SeededEntropy, StructuredValue, Thought, ThoughtContent,
        ThoughtOrigin, Timestamp,
    };

    /// An engine that implements nothing, so every call reports `Unsupported`.
    struct Empty;
    impl DeliberationEngine for Empty {}

    fn ctx() -> DeliberationContext {
        let ids = IdGenerator::new(SeededEntropy::new(3));
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
    fn unimplemented_capabilities_report_unsupported() {
        let mut e = Empty;
        let c = ctx();
        assert_eq!(
            e.reason(&c).unwrap_err(),
            DeliberationError::Unsupported(Capability::Reason)
        );
        assert_eq!(
            e.reflect(&c).unwrap_err(),
            DeliberationError::Unsupported(Capability::Reflect)
        );
        let req = ContradictionRequest { claims: vec![] };
        assert_eq!(
            e.analyse_contradictions(&c, &req).unwrap_err(),
            DeliberationError::Unsupported(Capability::ContradictionAnalysis)
        );
        let req = ActiveLearningRequest { candidates: vec![] };
        assert_eq!(
            e.rank_queries(&c, &req).unwrap_err(),
            DeliberationError::Unsupported(Capability::ActiveLearning)
        );
    }

    #[test]
    fn scripted_engine_implements_only_its_subset() {
        let mut e = ScriptedEngine::default();
        let c = ctx();
        let req = HypothesisRequest {
            question: "why".into(),
            max_hypotheses: 2,
        };
        assert_eq!(
            e.generate_hypotheses(&c, &req).unwrap_err(),
            DeliberationError::Unsupported(Capability::HypothesisGeneration)
        );
        assert!(e.calls.is_empty());
    }

    #[test]
    fn display_names_the_failure() {
        let msg = DeliberationError::Unsupported(Capability::CausalReasoning).to_string();
        assert!(msg.contains("CausalReasoning"));
    }
}
