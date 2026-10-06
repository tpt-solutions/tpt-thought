// SPDX-License-Identifier: MIT OR Apache-2.0
//! Uncertainty is never a single generic number (spec section 30) and epistemic status is explicit
//! (spec section 50).

use crate::confidence::Probability;
use serde::{Deserialize, Serialize};

/// Distinct kinds of uncertainty. Every component is optional: unknown is not zero.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Uncertainty {
    /// Subjective confidence.
    pub confidence: Option<Probability>,
    /// Probability of the claim.
    pub probability: Option<Probability>,
    /// Reducible uncertainty from lack of knowledge.
    pub epistemic: Option<f64>,
    /// Irreducible uncertainty from inherent randomness.
    pub aleatoric: Option<f64>,
    /// The answer is genuinely unknown.
    pub unknown: bool,
    /// Conflicting evidence or thoughts exist.
    pub contradiction: bool,
    /// The provider abstained from answering.
    pub abstained: bool,
}

impl Uncertainty {
    /// Wholly unknown.
    pub fn unknown() -> Self {
        Self {
            unknown: true,
            ..Self::default()
        }
    }

    /// True if any flag indicates the claim should not be taken at face value.
    pub fn is_unresolved(&self) -> bool {
        self.unknown || self.contradiction || self.abstained
    }
}

/// How well-founded a claim is. Ordered from weakest to strongest.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicStatus {
    /// Heuristic or informal.
    #[default]
    Informal,
    /// Supported by probabilistic inference.
    Probabilistic,
    /// Checked by a verifier.
    Verified,
    /// Formally proven.
    Proven,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_ordered() {
        assert!(EpistemicStatus::Informal < EpistemicStatus::Probabilistic);
        assert!(EpistemicStatus::Verified < EpistemicStatus::Proven);
    }

    #[test]
    fn unresolved_flags() {
        assert!(Uncertainty::unknown().is_unresolved());
        assert!(!Uncertainty::default().is_unresolved());
    }
}
