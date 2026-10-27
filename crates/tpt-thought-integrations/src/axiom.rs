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
//! `EpistemicStatus::Proven` comes only from [`verify_proven`]. It checks an Axiom proof claim with
//! a backend and a verifying key that the caller trusts. A claim carries no key of its own, so a
//! proof made under a key the verifier did not choose is rejected. The check also fails for a claim
//! made against a different circuit, because the claim's IR digest must match the circuit given.
//!
//! The adapter does not pick a backend. Callers pass one, so the adapter stays backend-neutral.

use tpt_axiom_core::{Confidence as AxiomConfidence, Fuzzy, Probability as AxiomProbability};
use tpt_axiom_ir::circuit::ExprId;
use tpt_axiom_ir::ConstraintSystem;
use tpt_axiom_verify::{check_comparison, normalize_all, Comparison, Mismatch};
use tpt_axiom_zk::{ProofClaim, ZkBackend};
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

/// Why a proof claim did not earn `EpistemicStatus::Proven`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProofError {
    /// The proof does not verify under the trusted key, or the claim was made against another
    /// circuit.
    Rejected,
    /// The backend failed while checking the proof. Holds the backend's message.
    Backend(String),
}

impl std::fmt::Display for ProofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected => write!(f, "proof does not verify against the trusted key"),
            Self::Backend(reason) => write!(f, "proof backend failed: {reason}"),
        }
    }
}

impl std::error::Error for ProofError {}

/// Checks `claim` against `ir` with `backend` and the trusted verifying key `vk`, and returns
/// `EpistemicStatus::Proven` when it holds.
///
/// `vk` must come from the verifier's own trusted setup for `ir`. The claim's proof is never checked
/// against a key taken from the claim.
///
/// # Errors
///
/// [`ProofError::Rejected`] when the proof fails or the claim names a different circuit, and
/// [`ProofError::Backend`] when the backend itself fails.
pub fn verify_proven<B: ZkBackend>(
    claim: &ProofClaim<B>,
    backend: &B,
    vk: &B::VerifyingKey,
    ir: &ConstraintSystem,
) -> Result<EpistemicStatus, ProofError> {
    match claim.verify_with(backend, vk, ir) {
        Ok(true) => Ok(EpistemicStatus::Proven),
        Ok(false) => Err(ProofError::Rejected),
        Err(error) => Err(ProofError::Backend(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_axiom_ir::{ConstraintSystemBuilder, Scalar};

    /// A stand-in backend for the adapter's tests. A proof verifies only under the key it was made
    /// with, and proof `0` is malformed. It checks the adapter's mapping, not any real circuit.
    struct KeyedBackend;

    const KEY: u64 = 7;

    impl ZkBackend for KeyedBackend {
        type Error = String;
        type Circuit = ();
        type ProvingKey = u64;
        type VerifyingKey = u64;
        type Proof = u64;

        fn name(&self) -> &'static str {
            "keyed-test"
        }

        fn compile(&self, _ir: &ConstraintSystem) -> Result<(), String> {
            Ok(())
        }

        fn generate_keys(
            &self,
            _ir: &ConstraintSystem,
            _params: &[u8],
        ) -> Result<(u64, u64), String> {
            Ok((KEY, KEY))
        }

        fn prove(
            &self,
            _circuit: &(),
            pk: &u64,
            _public: &[Scalar],
            _secret: &[Scalar],
        ) -> Result<u64, String> {
            Ok(*pk)
        }

        fn verify(&self, vk: &u64, _public: &[Scalar], proof: &u64) -> Result<bool, String> {
            if *proof == 0 {
                return Err("malformed proof".into());
            }
            Ok(proof == vk)
        }
    }

    fn at_least_system() -> ConstraintSystem {
        let mut builder = ConstraintSystemBuilder::new("at_least");
        let x = builder.secret_input("x");
        let y = builder.secret_input("y");
        let diff = builder.sub(x, y);
        builder.constrain_non_negative(diff);
        builder.build()
    }

    #[test]
    fn a_proof_under_the_trusted_key_is_proven() {
        let ir = at_least_system();
        let claim = ProofClaim::new("at_least", &ir, &[], KEY);
        assert_eq!(
            verify_proven(&claim, &KeyedBackend, &KEY, &ir),
            Ok(EpistemicStatus::Proven)
        );
    }

    #[test]
    fn a_proof_under_another_key_is_rejected() {
        let ir = at_least_system();
        let claim = ProofClaim::new("at_least", &ir, &[], 99);
        assert_eq!(
            verify_proven(&claim, &KeyedBackend, &KEY, &ir),
            Err(ProofError::Rejected)
        );
    }

    #[test]
    fn a_claim_for_another_circuit_is_rejected() {
        let ir = at_least_system();
        let claim = ProofClaim::new("at_least", &ir, &[], KEY);
        let mut other = ConstraintSystemBuilder::new("at_least");
        let x = other.secret_input("x");
        other.constrain_non_negative(x);
        assert_eq!(
            verify_proven(&claim, &KeyedBackend, &KEY, &other.build()),
            Err(ProofError::Rejected)
        );
    }

    #[test]
    fn a_backend_failure_is_reported_not_proven() {
        let ir = at_least_system();
        let claim = ProofClaim::new("at_least", &ir, &[], 0);
        assert_eq!(
            verify_proven(&claim, &KeyedBackend, &KEY, &ir),
            Err(ProofError::Backend("malformed proof".into()))
        );
    }

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
