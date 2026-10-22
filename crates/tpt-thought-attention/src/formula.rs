// SPDX-License-Identifier: MIT OR Apache-2.0
use serde::{Deserialize, Serialize};
use tpt_thought_core::Thought;

/// The inputs to attention scoring (spec section 11). Each factor is nominally `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct AttentionFactors {
    /// How much it stands out.
    pub salience: f64,
    /// How new it is.
    pub novelty: f64,
    /// Time pressure.
    pub urgency: f64,
    /// Relevance to current goals.
    pub goal_relevance: f64,
    /// Risk if ignored.
    pub risk: f64,
    /// How uncertain it is.
    pub uncertainty: f64,
    /// Expected uncertainty reduction from attending.
    pub information_gain: f64,
    /// Emotional or value relevance.
    pub value_relevance: f64,
    /// How recent it is.
    pub recency: f64,
    /// Resource cost of attending (subtracts from the score).
    pub resource_cost: f64,
}

/// A pluggable attention scoring function.
pub trait AttentionFormula {
    /// Combine `factors` into a single score. Higher means more attention.
    fn score(&self, factors: &AttentionFactors) -> f64;
}

/// The default formula: a weighted sum with resource cost subtracted. Weights are
/// configurable; the default weights every factor 1.0 as in the spec's conceptual formula.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeightedSum {
    /// Per-factor weights. `resource_cost`'s weight is applied negatively.
    pub weights: AttentionFactors,
}

impl Default for WeightedSum {
    fn default() -> Self {
        Self {
            weights: AttentionFactors {
                salience: 1.0,
                novelty: 1.0,
                urgency: 1.0,
                goal_relevance: 1.0,
                risk: 1.0,
                uncertainty: 1.0,
                information_gain: 1.0,
                value_relevance: 1.0,
                recency: 1.0,
                resource_cost: 1.0,
            },
        }
    }
}

impl AttentionFormula for WeightedSum {
    fn score(&self, f: &AttentionFactors) -> f64 {
        let w = &self.weights;
        w.salience * f.salience
            + w.novelty * f.novelty
            + w.urgency * f.urgency
            + w.goal_relevance * f.goal_relevance
            + w.risk * f.risk
            + w.uncertainty * f.uncertainty
            + w.information_gain * f.information_gain
            + w.value_relevance * f.value_relevance
            + w.recency * f.recency
            - w.resource_cost * f.resource_cost
    }
}

impl<F: Fn(&AttentionFactors) -> f64> AttentionFormula for F {
    fn score(&self, factors: &AttentionFactors) -> f64 {
        self(factors)
    }
}

/// Score `factors` and store the result, plus salience and novelty, on the thought.
pub fn apply_attention(
    thought: &mut Thought,
    factors: &AttentionFactors,
    formula: &impl AttentionFormula,
) -> f64 {
    let score = formula.score(factors);
    thought.attention.score = score;
    thought.salience = factors.salience;
    thought.novelty = factors.novelty;
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_formula_adds_and_subtracts_cost() {
        let f = AttentionFactors {
            salience: 0.5,
            urgency: 0.25,
            resource_cost: 0.25,
            ..Default::default()
        };
        assert_eq!(WeightedSum::default().score(&f), 0.5);
    }

    #[test]
    fn closures_are_formulas() {
        let f = AttentionFactors {
            risk: 0.9,
            ..Default::default()
        };
        assert_eq!((|f: &AttentionFactors| f.risk * 2.0).score(&f), 1.8);
    }

    #[test]
    fn weights_are_configurable() {
        let mut w = WeightedSum::default();
        w.weights.risk = 10.0;
        let f = AttentionFactors {
            risk: 0.5,
            ..Default::default()
        };
        assert_eq!(w.score(&f), 5.0);
    }
}
