// SPDX-License-Identifier: MIT OR Apache-2.0
//! Decisions, the policy context, and composition of policies.

use crate::budget::{Dimension, ResourceBudget, ResourceUsage};
use crate::level::EscalationLevel;
use crate::value::CognitionValue;

/// Why cognition stopped without an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// A resource limit was exceeded.
    BudgetExhausted(Dimension),
    /// More branches are open than policy allows; they must collapse first (spec section 64).
    TooManyBranches {
        /// Branches open.
        branches: u32,
        /// Permitted maximum.
        limit: u32,
    },
}

/// What a policy wants done next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// No objection: the current answer stands.
    Accept,
    /// Move to this level for deeper cognition.
    Escalate(EscalationLevel),
    /// Stop without an answer.
    Terminate(Termination),
}

impl Decision {
    /// Combine two decisions. Termination overrides everything, the first termination wins;
    /// otherwise the higher escalation wins; otherwise the result is [`Decision::Accept`].
    pub fn combine(self, other: Decision) -> Decision {
        match (self, other) {
            (Decision::Terminate(_), _) => self,
            (_, Decision::Terminate(_)) => other,
            (Decision::Escalate(a), Decision::Escalate(b)) => Decision::Escalate(a.max(b)),
            (Decision::Escalate(_), Decision::Accept) => self,
            (Decision::Accept, Decision::Escalate(_)) => other,
            (Decision::Accept, Decision::Accept) => Decision::Accept,
        }
    }
}

/// Everything a policy may look at. Plain data, so policies stay pure and replayable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolicyContext {
    /// The level the thought is currently at.
    pub level: EscalationLevel,
    /// Confidence in the current answer, `0.0..=1.0`.
    pub confidence: f64,
    /// Risk of acting on the current answer, `0.0..=1.0`.
    pub risk: f64,
    /// Branches currently open.
    pub branches: u32,
    /// Escalations that did not settle the question.
    pub failed_escalations: u32,
    /// Estimated value of more cognition, if the caller has one.
    pub value: Option<CognitionValue>,
    /// Resources consumed so far.
    pub usage: ResourceUsage,
    /// Resource limits in force.
    pub budget: ResourceBudget,
}

/// A rule that inspects a [`PolicyContext`] and returns a [`Decision`].
pub trait Policy {
    /// Decide what should happen next.
    fn decide(&self, ctx: &PolicyContext) -> Decision;
}

impl<F: Fn(&PolicyContext) -> Decision> Policy for F {
    fn decide(&self, ctx: &PolicyContext) -> Decision {
        self(ctx)
    }
}

/// An ordered collection of policies, combined with [`Decision::combine`].
#[derive(Default)]
pub struct PolicySet {
    policies: Vec<Box<dyn Policy>>,
}

impl PolicySet {
    /// An empty set. An empty set always returns [`Decision::Accept`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a policy.
    pub fn push(&mut self, policy: impl Policy + 'static) -> &mut Self {
        self.policies.push(Box::new(policy));
        self
    }

    /// Combined decision of every policy in the set.
    pub fn decide(&self, ctx: &PolicyContext) -> Decision {
        self.policies
            .iter()
            .fold(Decision::Accept, |acc, p| acc.combine(p.decide(ctx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn termination_beats_escalation_and_acceptance() {
        let stop = Decision::Terminate(Termination::BudgetExhausted(Dimension::Money));
        let up = Decision::Escalate(EscalationLevel::Human);
        assert_eq!(stop.combine(up), stop);
        assert_eq!(up.combine(stop), stop);
        assert_eq!(Decision::Accept.combine(stop), stop);
    }

    #[test]
    fn escalations_take_the_higher_level() {
        let low = Decision::Escalate(EscalationLevel::Reflex);
        let high = Decision::Escalate(EscalationLevel::DeliberateReasoning);
        assert_eq!(low.combine(high), high);
        assert_eq!(Decision::Accept.combine(low), low);
        assert_eq!(low.combine(Decision::Accept), low);
    }

    #[test]
    fn empty_set_accepts() {
        let ctx = PolicyContext {
            level: EscalationLevel::Reflex,
            confidence: 0.0,
            risk: 0.0,
            branches: 0,
            failed_escalations: 0,
            value: None,
            usage: ResourceUsage::default(),
            budget: ResourceBudget::UNLIMITED,
        };
        assert_eq!(PolicySet::new().decide(&ctx), Decision::Accept);
    }
}
