// SPDX-License-Identifier: MIT OR Apache-2.0
//! Serialisable escalation configuration (spec section 20: "the escalation policy must be
//! configurable").

use crate::decision::PolicySet;
use crate::level::{EscalationLadder, EscalationLevel, LadderError};
use crate::policies::{BranchPolicy, BudgetGuard, EscalationGate, HumanEscalation, RiskCeiling};
use serde::{Deserialize, Serialize};

/// Tunable thresholds for the standard policy set. Load it from any serde format, then call
/// [`EscalationConfig::build`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EscalationConfig {
    /// Confidence at or above which an answer is accepted.
    pub accept_confidence: f64,
    /// Expected value of cognition at or below which escalation stops.
    pub value_floor: f64,
    /// Risk above which a human is asked.
    pub max_risk: f64,
    /// Failed escalations tolerated before a human is asked.
    pub human_after_failed_escalations: u32,
    /// Maximum simultaneous branches.
    pub max_branches: u32,
    /// Levels escalation may visit, lowest first.
    pub ladder: Vec<EscalationLevel>,
}

impl Default for EscalationConfig {
    fn default() -> Self {
        Self {
            accept_confidence: 0.8,
            value_floor: 0.0,
            max_risk: 0.7,
            human_after_failed_escalations: 3,
            max_branches: 8,
            ladder: EscalationLevel::ALL.to_vec(),
        }
    }
}

impl EscalationConfig {
    /// Build the standard policy set: escalation gate, risk ceiling, human escalation, branch
    /// limit, and budget guard. Fails only if the configured ladder is invalid.
    pub fn build(&self) -> Result<PolicySet, LadderError> {
        let ladder = EscalationLadder::new(self.ladder.clone())?;
        let mut set = PolicySet::new();
        set.push(EscalationGate {
            accept_confidence: self.accept_confidence,
            value_floor: self.value_floor,
            ladder,
        })
        .push(RiskCeiling {
            max_risk: self.max_risk,
        })
        .push(HumanEscalation {
            after_failed_escalations: self.human_after_failed_escalations,
        })
        .push(BranchPolicy {
            max_branches: self.max_branches,
        })
        .push(BudgetGuard);
        Ok(set)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{ResourceBudget, ResourceUsage};
    use crate::decision::{Decision, PolicyContext};

    #[test]
    fn default_config_builds_and_accepts_a_confident_answer() {
        let set = EscalationConfig::default().build().unwrap();
        let ctx = PolicyContext {
            level: EscalationLevel::Reflex,
            confidence: 0.95,
            risk: 0.1,
            branches: 1,
            failed_escalations: 0,
            value: None,
            usage: ResourceUsage::default(),
            budget: ResourceBudget::UNLIMITED,
        };
        assert_eq!(set.decide(&ctx), Decision::Accept);
    }

    #[test]
    fn invalid_ladder_fails_to_build() {
        let config = EscalationConfig {
            ladder: vec![EscalationLevel::Human, EscalationLevel::Reflex],
            ..Default::default()
        };
        assert!(config.build().is_err());
    }

    #[test]
    fn config_round_trips_through_json() {
        let config = EscalationConfig {
            max_risk: 0.4,
            ladder: vec![EscalationLevel::Reflex, EscalationLevel::Human],
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: EscalationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, config);
    }
}
