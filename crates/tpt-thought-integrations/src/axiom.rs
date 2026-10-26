// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from `tpt-axiom` probabilistic types to Thought confidence and uncertainty
//! (spec sections 30, 50, 67; todo.md Phase 10).
//!
//! Axiom's `Probability` and `Confidence` both validate to `0..=1`, so they map to Thought's raw
//! probability without loss. Axiom's `Fuzzy` gives a mean and a variance. The variance is the
//! uncertainty in the estimate of the mean, so it is recorded as epistemic uncertainty. Axiom
//! records no calibration provenance, so its confidence is treated as raw (§67).
//!
//! Values Axiom produces from probabilistic inference are `EpistemicStatus::Probabilistic`.
//! A constraint from an Axiom circuit earns `EpistemicStatus::Verified` only when
//! `tpt-axiom-verify` checks that it encodes the claimed comparison (see [`verify_comparison`]).
//!
//! Axiom's ZK proof types are not mapped, so nothing here yields `EpistemicStatus::Proven`.
//! Proof verification belongs to a separate verification layer, and this adapter does not claim
//! proofs it has not checked.

use tpt_axiom_core::{Confidence as AxiomConfidence, Fuzzy, Probability as AxiomProbability};
use tpt_axiom_ir::circuit::ExprId;
use tpt_axiom_ir::ConstraintSystem;
use tpt_axiom_verify::{check_comparison, normalize_all, Comparison, Mismatch};
use tpt_thought_core::confidence::InvalidProbability;
use tpt_thought_core::{Confidence, EpistemicStatus, Probability, Uncertainty};

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

/// Why a constraint could not be verified as a comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintError {
    /// The system has no constraint at this index.
    MissingConstraint(usize),
    /// One of the compared expressions is not in the system's expression table.
    MissingExpr(ExprId),
    /// The constraint does not encode the claimed comparison.
    Mismatch(Mismatch),
}

/// Checks that constraint `constraint` of `system` encodes `lhs <cmp> rhs`.
///
/// Returns `EpistemicStatus::Verified` when the check passes. A failing check is an error, not an
/// informal status, so the caller sees the polynomials that disagree.
///
/// # Errors
///
/// [`ConstraintError`] when the constraint or an expression is missing, or when the constraint
/// does not encode the claimed comparison.
pub fn verify_comparison(
    system: &ConstraintSystem,
    constraint: usize,
    cmp: Comparison,
    lhs: ExprId,
    rhs: ExprId,
) -> Result<EpistemicStatus, ConstraintError> {
    let found = *system
        .constraints
        .get(constraint)
        .ok_or(ConstraintError::MissingConstraint(constraint))?;
    for expr in [lhs, rhs] {
        if expr >= system.exprs.len() {
            return Err(ConstraintError::MissingExpr(expr));
        }
    }
    let table = normalize_all(system);
    check_comparison(&table, &found, cmp, lhs, rhs).map_err(ConstraintError::Mismatch)?;
    Ok(EpistemicStatus::Verified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_axiom_ir::ConstraintSystemBuilder;

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

    /// Builds `x >= y` as the non-negativity constraint `x - y >= 0`.
    fn at_least() -> (ConstraintSystem, ExprId, ExprId) {
        let mut builder = ConstraintSystemBuilder::new("at_least");
        let x = builder.secret_input("x");
        let y = builder.secret_input("y");
        let diff = builder.sub(x, y);
        builder.constrain_non_negative(diff);
        (builder.build(), x, y)
    }

    #[test]
    fn a_constraint_that_encodes_the_claim_is_verified() {
        let (system, x, y) = at_least();
        let status = verify_comparison(&system, 0, Comparison::Ge, x, y);
        assert_eq!(status, Ok(EpistemicStatus::Verified));
    }

    #[test]
    fn a_constraint_that_encodes_a_different_claim_is_a_mismatch() {
        let (system, x, y) = at_least();
        let status = verify_comparison(&system, 0, Comparison::Gt, x, y);
        assert!(matches!(status, Err(ConstraintError::Mismatch(_))));
    }

    #[test]
    fn a_missing_constraint_is_reported() {
        let (system, x, y) = at_least();
        let status = verify_comparison(&system, 1, Comparison::Ge, x, y);
        assert_eq!(status, Err(ConstraintError::MissingConstraint(1)));
    }

    #[test]
    fn a_missing_expression_is_reported() {
        let (system, x, _) = at_least();
        let status = verify_comparison(&system, 0, Comparison::Ge, x, 999);
        assert_eq!(status, Err(ConstraintError::MissingExpr(999)));
    }
}
