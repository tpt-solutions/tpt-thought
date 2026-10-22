// SPDX-License-Identifier: MIT OR Apache-2.0
//! Planning representation (spec section 84): a goal, the state it starts from, candidate plans
//! with predicted outcomes, constraints, cost and risk.

use serde::Serialize;
use std::fmt;
use tpt_thought_core::StructuredValue;

/// One way of reaching the goal.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct CandidatePlan {
    /// Name of the plan. A provider may choose a plan by answering with this name.
    pub name: String,
    /// What the plan is expected to produce.
    pub predicted_outcome: StructuredValue,
    /// Cost in the deployment's units. Must be finite and non-negative.
    pub cost: f64,
    /// Risk in `0.0..=1.0`.
    pub risk: f64,
}

/// A plan representation that failed validation.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanError {
    /// The goal was blank.
    EmptyGoal,
    /// No candidate plans were supplied.
    NoCandidates,
    /// Two candidates share a name.
    DuplicateCandidate(String),
    /// A candidate's cost was not finite or was negative.
    InvalidCost {
        /// The candidate.
        name: String,
        /// The rejected cost.
        cost: f64,
    },
    /// A value was outside `0.0..=1.0`.
    InvalidRisk {
        /// The candidate, or `"max_risk"` for the limit.
        name: String,
        /// The rejected risk.
        risk: f64,
    },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyGoal => write!(f, "plan goal is blank"),
            Self::NoCandidates => write!(f, "plan has no candidates"),
            Self::DuplicateCandidate(name) => write!(f, "duplicate candidate `{name}`"),
            Self::InvalidCost { name, cost } => {
                write!(f, "candidate `{name}` has invalid cost {cost}")
            }
            Self::InvalidRisk { name, risk } => {
                write!(f, "`{name}` has risk {risk}, expected 0.0..=1.0")
            }
        }
    }
}

impl std::error::Error for PlanError {}

/// A validated plan: a goal, its starting state and the candidates for reaching it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plan {
    goal: String,
    state: StructuredValue,
    candidates: Vec<CandidatePlan>,
    constraints: Vec<String>,
    max_risk: f64,
}

impl Plan {
    /// Validate and build a plan. `max_risk` is the largest risk a chosen candidate may carry.
    pub fn new(
        goal: impl Into<String>,
        state: StructuredValue,
        candidates: Vec<CandidatePlan>,
        constraints: Vec<String>,
        max_risk: f64,
    ) -> Result<Self, PlanError> {
        let goal = goal.into();
        if goal.trim().is_empty() {
            return Err(PlanError::EmptyGoal);
        }
        if !(0.0..=1.0).contains(&max_risk) {
            return Err(PlanError::InvalidRisk {
                name: "max_risk".into(),
                risk: max_risk,
            });
        }
        if candidates.is_empty() {
            return Err(PlanError::NoCandidates);
        }
        for (i, c) in candidates.iter().enumerate() {
            if !c.cost.is_finite() || c.cost < 0.0 {
                return Err(PlanError::InvalidCost {
                    name: c.name.clone(),
                    cost: c.cost,
                });
            }
            if !(0.0..=1.0).contains(&c.risk) {
                return Err(PlanError::InvalidRisk {
                    name: c.name.clone(),
                    risk: c.risk,
                });
            }
            if candidates[..i].iter().any(|other| other.name == c.name) {
                return Err(PlanError::DuplicateCandidate(c.name.clone()));
            }
        }
        Ok(Self {
            goal,
            state,
            candidates,
            constraints,
            max_risk,
        })
    }

    /// The goal.
    pub fn goal(&self) -> &str {
        &self.goal
    }

    /// The state the plan starts from.
    pub fn state(&self) -> &StructuredValue {
        &self.state
    }

    /// The candidate plans, in the order supplied.
    pub fn candidates(&self) -> &[CandidatePlan] {
        &self.candidates
    }

    /// Constraints the plan must respect.
    pub fn constraints(&self) -> &[String] {
        &self.constraints
    }

    /// The largest risk a chosen candidate may carry.
    pub fn max_risk(&self) -> f64 {
        self.max_risk
    }

    /// The candidate with this name, if any.
    pub fn named(&self, name: &str) -> Option<&CandidatePlan> {
        self.candidates.iter().find(|c| c.name == name)
    }

    /// The cheapest candidate within the risk limit. Ties are broken by name, so the choice is
    /// deterministic. `None` if every candidate exceeds the limit.
    pub fn best(&self) -> Option<&CandidatePlan> {
        self.candidates
            .iter()
            .filter(|c| c.risk <= self.max_risk)
            .min_by(|a, b| a.cost.total_cmp(&b.cost).then_with(|| a.name.cmp(&b.name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(name: &str, cost: f64, risk: f64) -> CandidatePlan {
        CandidatePlan {
            name: name.into(),
            predicted_outcome: StructuredValue::Text(name.into()),
            cost,
            risk,
        }
    }

    #[test]
    fn best_is_cheapest_within_risk() {
        let plan = Plan::new(
            "grasp",
            StructuredValue::Null,
            vec![
                candidate("fast", 1.0, 0.9),
                candidate("safe", 3.0, 0.1),
                candidate("cheap", 2.0, 0.4),
            ],
            vec![],
            0.5,
        )
        .unwrap();
        assert_eq!(plan.best().unwrap().name, "cheap");
    }

    #[test]
    fn no_candidate_within_risk_gives_none() {
        let plan = Plan::new(
            "grasp",
            StructuredValue::Null,
            vec![candidate("risky", 1.0, 0.9)],
            vec![],
            0.5,
        )
        .unwrap();
        assert!(plan.best().is_none());
    }

    #[test]
    fn ties_break_by_name() {
        let plan = Plan::new(
            "grasp",
            StructuredValue::Null,
            vec![candidate("b", 1.0, 0.1), candidate("a", 1.0, 0.1)],
            vec![],
            1.0,
        )
        .unwrap();
        assert_eq!(plan.best().unwrap().name, "a");
    }

    #[test]
    fn validation_rejects_bad_input() {
        let ok = vec![candidate("a", 1.0, 0.1)];
        assert_eq!(
            Plan::new(" ", StructuredValue::Null, ok.clone(), vec![], 0.5),
            Err(PlanError::EmptyGoal)
        );
        assert_eq!(
            Plan::new("g", StructuredValue::Null, vec![], vec![], 0.5),
            Err(PlanError::NoCandidates)
        );
        assert!(matches!(
            Plan::new(
                "g",
                StructuredValue::Null,
                vec![candidate("a", -1.0, 0.1)],
                vec![],
                0.5
            ),
            Err(PlanError::InvalidCost { .. })
        ));
        assert!(matches!(
            Plan::new(
                "g",
                StructuredValue::Null,
                vec![candidate("a", 1.0, 1.5)],
                vec![],
                0.5
            ),
            Err(PlanError::InvalidRisk { .. })
        ));
        assert!(matches!(
            Plan::new("g", StructuredValue::Null, ok.clone(), vec![], 2.0),
            Err(PlanError::InvalidRisk { .. })
        ));
        let dup = vec![candidate("a", 1.0, 0.1), candidate("a", 2.0, 0.1)];
        assert_eq!(
            Plan::new("g", StructuredValue::Null, dup, vec![], 0.5),
            Err(PlanError::DuplicateCandidate("a".into()))
        );
    }
}
