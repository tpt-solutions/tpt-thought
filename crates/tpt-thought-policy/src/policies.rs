// SPDX-License-Identifier: MIT OR Apache-2.0
//! Built-in policies (spec sections 20, 36, 63, 64).

use crate::decision::{Decision, Policy, PolicyContext, Termination};
use crate::level::{EscalationLadder, EscalationLevel};

/// Escalates while confidence is short of `accept_confidence`, but only when more cognition is
/// worth its cost. Stops at the top of the ladder.
#[derive(Debug, Clone, PartialEq)]
pub struct EscalationGate {
    /// Confidence at or above which the current answer is accepted.
    pub accept_confidence: f64,
    /// Expected value of cognition at or below which escalation is not worth it.
    pub value_floor: f64,
    /// The levels escalation may visit.
    pub ladder: EscalationLadder,
}

impl Policy for EscalationGate {
    fn decide(&self, ctx: &PolicyContext) -> Decision {
        if ctx.confidence >= self.accept_confidence {
            return Decision::Accept;
        }
        if let Some(value) = ctx.value {
            if value.expected_value() <= self.value_floor {
                return Decision::Accept;
            }
        }
        match self.ladder.next_above(ctx.level) {
            Some(next) => Decision::Escalate(next),
            None => Decision::Accept,
        }
    }
}

/// Sends the thought to a human when risk exceeds `max_risk` (spec section 20, `L6`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiskCeiling {
    /// Highest risk the system may act on without a human.
    pub max_risk: f64,
}

impl Policy for RiskCeiling {
    fn decide(&self, ctx: &PolicyContext) -> Decision {
        if ctx.risk > self.max_risk && ctx.level != EscalationLevel::Human {
            Decision::Escalate(EscalationLevel::Human)
        } else {
            Decision::Accept
        }
    }
}

/// Sends the thought to a human once `after_failed_escalations` deeper attempts have failed to
/// settle the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HumanEscalation {
    /// Failed escalations tolerated before asking a human.
    pub after_failed_escalations: u32,
}

impl Policy for HumanEscalation {
    fn decide(&self, ctx: &PolicyContext) -> Decision {
        if ctx.failed_escalations >= self.after_failed_escalations
            && ctx.level != EscalationLevel::Human
        {
            Decision::Escalate(EscalationLevel::Human)
        } else {
            Decision::Accept
        }
    }
}

/// Terminates when more branches are open than `max_branches` allows, forcing collapse first
/// (spec section 64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchPolicy {
    /// Maximum simultaneous branches.
    pub max_branches: u32,
}

impl Policy for BranchPolicy {
    fn decide(&self, ctx: &PolicyContext) -> Decision {
        if ctx.branches > self.max_branches {
            Decision::Terminate(Termination::TooManyBranches {
                branches: ctx.branches,
                limit: self.max_branches,
            })
        } else {
            Decision::Accept
        }
    }
}

/// Terminates as soon as any resource dimension exceeds its limit (spec section 63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BudgetGuard;

impl Policy for BudgetGuard {
    fn decide(&self, ctx: &PolicyContext) -> Decision {
        match ctx.usage.check(&ctx.budget) {
            Ok(()) => Decision::Accept,
            Err(exhausted) => {
                Decision::Terminate(Termination::BudgetExhausted(exhausted.dimension))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{Dimension, ResourceBudget, ResourceUsage};
    use crate::value::CognitionValue;
    use crate::PolicySet;

    fn ctx(level: EscalationLevel, confidence: f64) -> PolicyContext {
        PolicyContext {
            level,
            confidence,
            risk: 0.0,
            branches: 1,
            failed_escalations: 0,
            value: None,
            usage: ResourceUsage::default(),
            budget: ResourceBudget::UNLIMITED,
        }
    }

    fn gate() -> EscalationGate {
        EscalationGate {
            accept_confidence: 0.8,
            value_floor: 0.0,
            ladder: EscalationLadder::default(),
        }
    }

    #[test]
    fn confident_answers_are_accepted() {
        let c = ctx(EscalationLevel::Reflex, 0.9);
        assert_eq!(gate().decide(&c), Decision::Accept);
    }

    #[test]
    fn low_confidence_escalates_one_rung() {
        let c = ctx(EscalationLevel::Reflex, 0.52);
        assert_eq!(
            gate().decide(&c),
            Decision::Escalate(EscalationLevel::StructuredIntelligence)
        );
    }

    #[test]
    fn escalation_stops_at_the_top_of_the_ladder() {
        let c = ctx(EscalationLevel::Human, 0.1);
        assert_eq!(gate().decide(&c), Decision::Accept);
    }

    #[test]
    fn cognition_not_worth_its_cost_is_not_escalated() {
        let mut c = ctx(EscalationLevel::Reflex, 0.4);
        c.value = Some(CognitionValue::new(0.1, 10.0, 2.0).unwrap());
        assert_eq!(gate().decide(&c), Decision::Accept);
        c.value = Some(CognitionValue::new(0.5, 10.0, 2.0).unwrap());
        assert_eq!(
            gate().decide(&c),
            Decision::Escalate(EscalationLevel::StructuredIntelligence)
        );
    }

    #[test]
    fn high_risk_goes_to_a_human() {
        let policy = RiskCeiling { max_risk: 0.5 };
        let mut c = ctx(EscalationLevel::SpecialistIntelligence, 0.9);
        c.risk = 0.6;
        assert_eq!(
            policy.decide(&c),
            Decision::Escalate(EscalationLevel::Human)
        );
        c.risk = 0.5;
        assert_eq!(policy.decide(&c), Decision::Accept);
    }

    #[test]
    fn repeated_failure_goes_to_a_human() {
        let policy = HumanEscalation {
            after_failed_escalations: 2,
        };
        let mut c = ctx(EscalationLevel::DeliberateReasoning, 0.3);
        c.failed_escalations = 1;
        assert_eq!(policy.decide(&c), Decision::Accept);
        c.failed_escalations = 2;
        assert_eq!(
            policy.decide(&c),
            Decision::Escalate(EscalationLevel::Human)
        );
    }

    #[test]
    fn too_many_branches_terminates() {
        let policy = BranchPolicy { max_branches: 3 };
        let mut c = ctx(EscalationLevel::Reflex, 0.1);
        c.branches = 4;
        assert_eq!(
            policy.decide(&c),
            Decision::Terminate(Termination::TooManyBranches {
                branches: 4,
                limit: 3,
            })
        );
    }

    #[test]
    fn exhausted_budget_terminates_even_when_escalation_is_wanted() {
        let mut c = ctx(EscalationLevel::Reflex, 0.1);
        c.budget = ResourceBudget {
            money: Some(1.0),
            ..ResourceBudget::UNLIMITED
        };
        c.usage = ResourceUsage {
            money: 1.5,
            ..Default::default()
        };
        let mut set = PolicySet::new();
        set.push(gate()).push(BudgetGuard);
        assert_eq!(
            set.decide(&c),
            Decision::Terminate(Termination::BudgetExhausted(Dimension::Money))
        );
    }

    #[test]
    fn composed_policies_escalate_to_the_highest_requirement() {
        let mut c = ctx(EscalationLevel::Reflex, 0.4);
        c.risk = 0.9;
        let mut set = PolicySet::new();
        set.push(gate())
            .push(RiskCeiling { max_risk: 0.5 })
            .push(BranchPolicy { max_branches: 5 });
        assert_eq!(set.decide(&c), Decision::Escalate(EscalationLevel::Human));
    }
}
