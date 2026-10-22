// SPDX-License-Identifier: MIT OR Apache-2.0
//! The bounded deep-investigation loop (spec section 62).
//!
//! The loop repeatedly asks an engine for one investigation step and stops on the first of four
//! termination reasons, or when the step cap is reached. The cap is the lower of
//! [`InvestigationPolicy::max_steps`] and the thought's `max_iterations` budget, so no engine
//! can keep a thought running indefinitely.

use crate::engine::{DeliberationEngine, DeliberationError};
use crate::snapshot::DeliberationContext;
use serde::{Deserialize, Serialize};
use tpt_thought_core::{Confidence, Probability, Uncertainty};

/// One investigation step requested from an engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Investigation {
    /// The question under investigation.
    pub question: String,
    /// Zero-based index of this step within the run.
    pub step: u32,
}

/// What one investigation step found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvestigationOutcome {
    /// Findings from this step.
    pub findings: Vec<String>,
    /// Confidence in the answer after this step.
    pub confidence: Confidence,
    /// Residual uncertainty after this step.
    pub uncertainty: Uncertainty,
    /// Risk if the current answer were acted on.
    pub risk: Probability,
    /// True if the engine cannot proceed without a human.
    pub needs_human: bool,
}

/// Thresholds that decide when an investigation stops.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InvestigationPolicy {
    /// Stop once the best available confidence reaches this value.
    pub target_confidence: Probability,
    /// Stop if an outcome's risk exceeds this value.
    pub max_risk: Probability,
    /// Hard cap on steps, independent of the budget.
    pub max_steps: u32,
}

impl InvestigationPolicy {
    /// A policy with the given thresholds.
    pub fn new(target_confidence: Probability, max_risk: Probability, max_steps: u32) -> Self {
        Self {
            target_confidence,
            max_risk,
            max_steps,
        }
    }
}

/// Why an investigation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Termination {
    /// Confidence reached the policy target.
    ConfidenceMet,
    /// The step cap or the thought's iteration budget was exhausted.
    BudgetExhausted,
    /// An outcome's risk exceeded the policy maximum.
    RiskPolicy,
    /// The engine asked for a human.
    HumanNeeded,
}

/// The record of a finished investigation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvestigationRun {
    /// Every step taken, in order.
    pub outcomes: Vec<InvestigationOutcome>,
    /// Why the run stopped.
    pub termination: Termination,
}

/// Drive `engine` through a bounded investigation of `question`.
///
/// The budget is checked before each step. After each step the checks run in this order: risk
/// policy, then human need, then confidence. An engine error aborts the run and is returned
/// unchanged.
pub fn run_investigation<E: DeliberationEngine>(
    engine: &mut E,
    ctx: &DeliberationContext,
    question: &str,
    policy: &InvestigationPolicy,
) -> Result<InvestigationRun, DeliberationError> {
    let cap = ctx
        .budget
        .max_iterations
        .unwrap_or(u32::MAX)
        .min(policy.max_steps);
    let mut outcomes = Vec::new();
    loop {
        let step = outcomes.len() as u32;
        if step >= cap {
            return Ok(InvestigationRun {
                outcomes,
                termination: Termination::BudgetExhausted,
            });
        }
        let request = Investigation {
            question: question.to_owned(),
            step,
        };
        let outcome = engine.investigate(ctx, &request)?;
        let termination = check(&outcome, policy);
        outcomes.push(outcome);
        if let Some(termination) = termination {
            return Ok(InvestigationRun {
                outcomes,
                termination,
            });
        }
    }
}

fn check(outcome: &InvestigationOutcome, policy: &InvestigationPolicy) -> Option<Termination> {
    if outcome.risk.get() > policy.max_risk.get() {
        Some(Termination::RiskPolicy)
    } else if outcome.needs_human {
        Some(Termination::HumanNeeded)
    } else if outcome
        .confidence
        .best()
        .is_some_and(|c| c.get() >= policy.target_confidence.get())
    {
        Some(Termination::ConfidenceMet)
    } else {
        None
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

    fn p(v: f64) -> Probability {
        Probability::new(v).unwrap()
    }

    fn ctx(budget: CognitiveBudget) -> DeliberationContext {
        let ids = IdGenerator::new(SeededEntropy::new(7));
        let thought = Thought::new(
            ids.thought(),
            ThoughtOrigin::Internal,
            ThoughtContent::Structured(StructuredValue::Null),
            Timestamp(0),
        );
        DeliberationContext {
            thought: ThoughtSnapshot::from_thought(&thought),
            budget,
        }
    }

    fn step(raw: f64, risk: f64, needs_human: bool) -> InvestigationOutcome {
        InvestigationOutcome {
            findings: vec![],
            confidence: Confidence::from_raw(p(raw)),
            uncertainty: Uncertainty::default(),
            risk: p(risk),
            needs_human,
        }
    }

    fn policy(max_steps: u32) -> InvestigationPolicy {
        InvestigationPolicy::new(p(0.9), p(0.5), max_steps)
    }

    #[test]
    fn stops_when_confidence_is_met() {
        let mut engine = ScriptedEngine::default();
        engine.investigations.push_back(Ok(step(0.3, 0.1, false)));
        engine.investigations.push_back(Ok(step(0.95, 0.1, false)));
        let run = run_investigation(
            &mut engine,
            &ctx(CognitiveBudget::UNLIMITED),
            "q",
            &policy(8),
        )
        .unwrap();
        assert_eq!(run.termination, Termination::ConfidenceMet);
        assert_eq!(run.outcomes.len(), 2);
    }

    #[test]
    fn stops_at_policy_step_cap() {
        let mut engine = ScriptedEngine::default();
        for _ in 0..5 {
            engine.investigations.push_back(Ok(step(0.1, 0.1, false)));
        }
        let run = run_investigation(
            &mut engine,
            &ctx(CognitiveBudget::UNLIMITED),
            "q",
            &policy(3),
        )
        .unwrap();
        assert_eq!(run.termination, Termination::BudgetExhausted);
        assert_eq!(run.outcomes.len(), 3);
        assert_eq!(engine.requests.len(), 3);
    }

    #[test]
    fn budget_iterations_cap_below_policy_cap() {
        let mut engine = ScriptedEngine::default();
        for _ in 0..5 {
            engine.investigations.push_back(Ok(step(0.1, 0.1, false)));
        }
        let budget = CognitiveBudget {
            max_iterations: Some(2),
            ..CognitiveBudget::UNLIMITED
        };
        let run = run_investigation(&mut engine, &ctx(budget), "q", &policy(8)).unwrap();
        assert_eq!(run.termination, Termination::BudgetExhausted);
        assert_eq!(run.outcomes.len(), 2);
    }

    #[test]
    fn stops_on_risk_policy() {
        let mut engine = ScriptedEngine::default();
        engine.investigations.push_back(Ok(step(0.1, 0.8, false)));
        let run = run_investigation(
            &mut engine,
            &ctx(CognitiveBudget::UNLIMITED),
            "q",
            &policy(8),
        )
        .unwrap();
        assert_eq!(run.termination, Termination::RiskPolicy);
        assert_eq!(run.outcomes.len(), 1);
    }

    #[test]
    fn stops_when_human_is_needed() {
        let mut engine = ScriptedEngine::default();
        engine.investigations.push_back(Ok(step(0.1, 0.1, true)));
        let run = run_investigation(
            &mut engine,
            &ctx(CognitiveBudget::UNLIMITED),
            "q",
            &policy(8),
        )
        .unwrap();
        assert_eq!(run.termination, Termination::HumanNeeded);
    }

    #[test]
    fn risk_takes_precedence_over_confidence() {
        let mut engine = ScriptedEngine::default();
        engine.investigations.push_back(Ok(step(0.99, 0.9, false)));
        let run = run_investigation(
            &mut engine,
            &ctx(CognitiveBudget::UNLIMITED),
            "q",
            &policy(8),
        )
        .unwrap();
        assert_eq!(run.termination, Termination::RiskPolicy);
    }

    #[test]
    fn engine_error_aborts_the_run() {
        let mut engine = ScriptedEngine::default();
        engine
            .investigations
            .push_back(Err(DeliberationError::Failed("timeout".into())));
        let err = run_investigation(
            &mut engine,
            &ctx(CognitiveBudget::UNLIMITED),
            "q",
            &policy(8),
        )
        .unwrap_err();
        assert_eq!(err, DeliberationError::Failed("timeout".into()));
    }

    #[test]
    fn steps_are_numbered_from_zero() {
        let mut engine = ScriptedEngine::default();
        for _ in 0..3 {
            engine.investigations.push_back(Ok(step(0.1, 0.1, false)));
        }
        run_investigation(
            &mut engine,
            &ctx(CognitiveBudget::UNLIMITED),
            "why",
            &policy(3),
        )
        .unwrap();
        let steps: Vec<u32> = engine.requests.iter().map(|r| r.step).collect();
        assert_eq!(steps, vec![0, 1, 2]);
        assert!(engine.requests.iter().all(|r| r.question == "why"));
    }
}
