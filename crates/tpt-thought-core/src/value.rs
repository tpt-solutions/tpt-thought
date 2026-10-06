// SPDX-License-Identifier: MIT OR Apache-2.0
//! Value assessment (spec section 22). Thought only supplies the interface; deeper value modelling
//! is a candidate for a future `tpt-value` crate, hence `#[non_exhaustive]`.

use serde::{Deserialize, Serialize};

/// A thought's value assessment.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ValueAssessment {
    /// Expected utility.
    pub utility: f64,
    /// Cost of pursuing it.
    pub cost: f64,
    /// Risk involved.
    pub risk: f64,
    /// Time pressure.
    pub urgency: f64,
    /// How novel it is.
    pub novelty: f64,
    /// Expected reduction in uncertainty.
    pub expected_information_gain: f64,
}
