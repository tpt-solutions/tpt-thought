// SPDX-License-Identifier: MIT OR Apache-2.0
//! Expected value of additional cognition (spec section 36).

use std::fmt;

/// The estimated worth of one more round of cognition.
///
/// `p_improve` is the probability that the extra cognition changes the decision for the better,
/// `gain` is the value of that improved decision, and `cost` is what the extra cognition costs,
/// in the same units as `gain`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CognitionValue {
    p_improve: f64,
    gain: f64,
    cost: f64,
}

/// A [`CognitionValue`] was built from a non-finite or out-of-range input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidCognitionValue;

impl fmt::Display for InvalidCognitionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cognition value needs finite inputs and a probability in 0..=1")
    }
}

impl std::error::Error for InvalidCognitionValue {}

impl CognitionValue {
    /// Build a value estimate. Rejects a probability outside `0..=1` and non-finite amounts.
    pub fn new(p_improve: f64, gain: f64, cost: f64) -> Result<Self, InvalidCognitionValue> {
        let p_ok = (0.0..=1.0).contains(&p_improve);
        if !p_ok || !gain.is_finite() || !cost.is_finite() || gain < 0.0 || cost < 0.0 {
            return Err(InvalidCognitionValue);
        }
        Ok(Self {
            p_improve,
            gain,
            cost,
        })
    }

    /// The probability that more cognition improves the decision.
    pub fn p_improve(&self) -> f64 {
        self.p_improve
    }

    /// The value of the improved decision.
    pub fn gain(&self) -> f64 {
        self.gain
    }

    /// The cost of the extra cognition.
    pub fn cost(&self) -> f64 {
        self.cost
    }

    /// Expected net value of cognition: `p_improve * gain - cost`. Positive means worth doing.
    pub fn expected_value(&self) -> f64 {
        self.p_improve * self.gain - self.cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_value_weighs_gain_against_cost() {
        let v = CognitionValue::new(0.5, 10.0, 2.0).unwrap();
        assert_eq!(v.expected_value(), 3.0);
        let not_worth = CognitionValue::new(0.1, 10.0, 2.0).unwrap();
        assert!(not_worth.expected_value() < 0.0);
    }

    #[test]
    fn rejects_bad_inputs() {
        assert!(CognitionValue::new(1.5, 1.0, 1.0).is_err());
        assert!(CognitionValue::new(0.5, f64::NAN, 1.0).is_err());
        assert!(CognitionValue::new(0.5, 1.0, -1.0).is_err());
    }
}
