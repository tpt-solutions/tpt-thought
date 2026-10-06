// SPDX-License-Identifier: MIT OR Apache-2.0
//! Confidence types (spec section 67): raw, calibrated and empirical reliability are distinct.

use serde::{Deserialize, Serialize};

/// A probability in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Probability(f64);

/// A value outside `[0, 1]` (or NaN) was supplied as a probability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidProbability(pub f64);

impl std::fmt::Display for InvalidProbability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is not a probability in [0, 1]", self.0)
    }
}

impl std::error::Error for InvalidProbability {}

impl Probability {
    /// Certainly false.
    pub const ZERO: Probability = Probability(0.0);
    /// Certainly true.
    pub const ONE: Probability = Probability(1.0);

    /// Validate and wrap `p`.
    pub fn new(p: f64) -> Result<Self, InvalidProbability> {
        if (0.0..=1.0).contains(&p) {
            Ok(Self(p))
        } else {
            Err(InvalidProbability(p))
        }
    }

    /// The underlying value.
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Probability {
    type Error = InvalidProbability;
    fn try_from(p: f64) -> Result<Self, Self::Error> {
        Self::new(p)
    }
}

impl From<Probability> for f64 {
    fn from(p: Probability) -> f64 {
        p.0
    }
}

/// Confidence in a conclusion. A nominal 0.9 is not necessarily a reliable 90% probability,
/// so the three notions are kept apart.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Confidence {
    /// The provider's own stated confidence.
    pub raw: Option<Probability>,
    /// Confidence after provider calibration.
    pub calibrated: Option<Probability>,
    /// Observed hit rate of comparable past claims.
    pub empirical_reliability: Option<Probability>,
}

impl Confidence {
    /// No confidence information.
    pub const UNKNOWN: Confidence = Confidence {
        raw: None,
        calibrated: None,
        empirical_reliability: None,
    };

    /// Confidence with only a raw value.
    pub fn from_raw(raw: Probability) -> Self {
        Self {
            raw: Some(raw),
            ..Self::default()
        }
    }

    /// The most trustworthy available value: empirical, then calibrated, then raw.
    pub fn best(&self) -> Option<Probability> {
        self.empirical_reliability.or(self.calibrated).or(self.raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_out_of_range() {
        assert!(Probability::new(1.1).is_err());
        assert!(Probability::new(-0.1).is_err());
        assert!(Probability::new(f64::NAN).is_err());
        assert!(Probability::new(0.5).is_ok());
    }

    #[test]
    fn best_prefers_empirical() {
        let c = Confidence {
            raw: Probability::new(0.9).ok(),
            calibrated: Probability::new(0.7).ok(),
            empirical_reliability: Probability::new(0.6).ok(),
        };
        assert_eq!(c.best().unwrap().get(), 0.6);
        assert_eq!(Confidence::UNKNOWN.best(), None);
    }

    #[test]
    fn deserialize_validates() {
        assert!(serde_json::from_str::<Probability>("2.0").is_err());
    }
}
