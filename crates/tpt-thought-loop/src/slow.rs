// SPDX-License-Identifier: MIT OR Apache-2.0
//! Slow-path types (spec section 61). The orchestration itself lives in [`crate::CognitiveLoop`],
//! which owns the engine, providers and estimator that the stages call.

use serde::{Deserialize, Serialize};

/// A stage of the slow path, in the order it runs (spec section 61): Intellect, then Eve, then
/// Augur, then Intellect again with the estimate in context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlowStage {
    /// The first Intellect pass escalated.
    Intellect,
    /// Eve ran a bounded investigation.
    Eve,
    /// Augur produced an estimate.
    Augur,
    /// Intellect was asked again, with the estimate in context.
    IntellectReevaluation,
}

/// A quantitative estimate with its residual uncertainty.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    /// The central value.
    pub mean: f64,
    /// Residual uncertainty, as a standard deviation. Must be finite and non-negative.
    pub residual_uncertainty: f64,
}

/// Produces estimates for a question. Augur's adapter implements this in the integrations crate.
pub trait Estimator {
    /// An estimate for `question`, or `None` if the estimator cannot speak to it.
    fn estimate(&self, question: &str) -> Option<Estimate>;
}
