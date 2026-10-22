// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from `tpt-axiom` probabilistic types to Thought confidence and uncertainty
//! (spec sections 30, 50, 67; todo.md Phase 10).
//!
//! Axiom's `Probability` and `Confidence` both validate to `0..=1`, so they map to Thought's raw
//! probability without loss. Axiom's `Fuzzy` gives a mean and a variance. The variance is the
//! uncertainty in the estimate of the mean, so it is recorded as epistemic uncertainty. Axiom
//! records no calibration provenance, so its confidence is treated as raw (§67).
//!
//! Axiom's ZK proof types are not mapped. Verification markers belong to a separate
//! verification layer, and this adapter does not claim proofs it has not checked.

use tpt_axiom_core::{Confidence as AxiomConfidence, Fuzzy, Probability as AxiomProbability};
use tpt_thought_core::confidence::InvalidProbability;
use tpt_thought_core::{Confidence, Probability, Uncertainty};

/// A Thought probability with the same value as an Axiom probability.
pub fn probability(p: AxiomProbability) -> Result<Probability, InvalidProbability> {
    Probability::new(p.value())
}

/// A raw Thought confidence from an Axiom confidence.
pub fn confidence(c: AxiomConfidence) -> Result<Confidence, InvalidProbability> {
    probability(AxiomProbability::new(c.value()).map_err(|e| InvalidProbability(e.0))?)
        .map(Confidence::from_raw)
}

/// The uncertainty carried by a fuzzy estimate: its variance, recorded as epistemic uncertainty.
/// Axiom validates the variance when the value is built, so it is always a usable number here.
pub fn uncertainty(fuzzy: &Fuzzy<f64>) -> Uncertainty {
    Uncertainty {
        epistemic: Some(fuzzy.variance()),
        ..Uncertainty::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probabilities_keep_their_value() {
        let p = AxiomProbability::new(0.25).expect("valid");
        assert_eq!(probability(p).expect("valid").get(), 0.25);
    }

    #[test]
    fn axiom_rejects_out_of_range_before_adapting() {
        assert!(AxiomProbability::new(1.5).is_err());
        assert!(AxiomConfidence::new(-0.1).is_err());
    }

    #[test]
    fn confidence_is_recorded_as_raw() {
        let c = AxiomConfidence::new(0.8).expect("valid");
        let adapted = confidence(c).expect("valid");
        assert_eq!(adapted.raw.map(Probability::get), Some(0.8));
    }

    #[test]
    fn fuzzy_variance_becomes_epistemic_uncertainty() {
        let fuzzy = Fuzzy::new(0.7_f64, 0.04_f64);
        let uncertainty = uncertainty(&fuzzy);
        assert_eq!(uncertainty.epistemic, Some(0.04));
    }

    #[test]
    fn fuzzy_interval_brackets_the_mean() {
        let fuzzy = Fuzzy::from_std_dev(10.0_f64, 2.0_f64);
        let (low, high) = fuzzy.confidence_interval(0.95);
        assert!(low < 10.0 && 10.0 < high);
    }
}
