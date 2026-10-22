// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::formula::{AttentionFactors, AttentionFormula};
use serde::{Deserialize, Serialize};
use tpt_thought_core::{Thought, Timestamp};

/// What a thought asks of the scheduler (spec section 35). Anima CK turns this into actual
/// compute scheduling; Thought never schedules CPU itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveDemand {
    /// Attention score used as scheduling priority.
    pub priority: f64,
    /// Earliest deadline among the thought's intentions.
    pub deadline: Option<Timestamp>,
    /// Estimated cost of the cognition required.
    pub estimated_cost: f64,
    /// Capabilities the thought's intentions require.
    pub required_capabilities: Vec<String>,
    /// Risk.
    pub risk: f64,
    /// Expected information gain.
    pub expected_information_gain: f64,
}

/// Compute the cognitive demand of `thought` given its attention `factors`.
pub fn demand(
    thought: &Thought,
    factors: &AttentionFactors,
    formula: &impl AttentionFormula,
) -> CognitiveDemand {
    let deadline = thought.intentions.iter().filter_map(|i| i.deadline).min();
    let mut required_capabilities: Vec<String> = thought
        .intentions
        .iter()
        .flat_map(|i| i.required_capabilities.iter().cloned())
        .collect();
    required_capabilities.sort();
    required_capabilities.dedup();
    CognitiveDemand {
        priority: formula.score(factors),
        deadline,
        estimated_cost: factors.resource_cost,
        required_capabilities,
        risk: factors.risk,
        expected_information_gain: factors.information_gain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WeightedSum;
    use tpt_thought_core::*;

    #[test]
    fn demand_collects_deadline_and_capabilities() {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let mut t = Thought::new(
            ids.thought(),
            ThoughtOrigin::Internal,
            ThoughtContent::Structured(StructuredValue::Null),
            Timestamp(0),
        );
        for (deadline, cap) in [
            (50, "sense:thermal"),
            (20, "move:arm"),
            (30, "sense:thermal"),
        ] {
            t.intentions.push(Intention {
                id: ids.intention(),
                goal: "g".into(),
                desired_outcome: None,
                constraints: vec![],
                risk_tolerance: None,
                priority: 1.0,
                required_capabilities: vec![cap.into()],
                deadline: Some(Timestamp(deadline)),
            });
        }
        let f = AttentionFactors {
            urgency: 1.0,
            risk: 0.5,
            resource_cost: 0.25,
            ..Default::default()
        };
        let d = demand(&t, &f, &WeightedSum::default());
        assert_eq!(d.deadline, Some(Timestamp(20)));
        assert_eq!(d.required_capabilities, vec!["move:arm", "sense:thermal"]);
        assert_eq!(d.priority, 1.25);
        assert_eq!(d.risk, 0.5);
    }
}
