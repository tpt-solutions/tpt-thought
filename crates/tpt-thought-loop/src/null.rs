// SPDX-License-Identifier: MIT OR Apache-2.0
//! Null implementations of the loop's three pluggable boundaries.
//!
//! They exist so a loop can be built and run without a deliberation engine, an estimator or an
//! executor, and so tests and examples need not copy their own versions. Each one answers the
//! question "this capability is absent", which the loop already handles.

use crate::action::{Action, ActionError, ActionSink};
use crate::slow::{Estimate, Estimator};
use tpt_thought_core::StructuredValue;
use tpt_thought_deliberation::DeliberationEngine;

/// A deliberation engine with no capabilities. Every request is answered as unsupported, so the
/// slow path records the gap and moves on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NullEngine;

impl DeliberationEngine for NullEngine {}

/// An estimator that can speak to no question.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NullEstimator;

impl Estimator for NullEstimator {
    fn estimate(&self, _question: &str) -> Option<Estimate> {
        None
    }
}

/// An action executor that refuses every action. A loop built without a real executor fails
/// loudly the first time it tries to act, rather than pretending the action happened.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NullActionSink;

impl ActionSink for NullActionSink {
    fn execute(&mut self, _action: &Action) -> Result<StructuredValue, ActionError> {
        Err(ActionError::Refused(
            "no action executor configured".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_estimator_speaks_to_nothing() {
        assert_eq!(NullEstimator.estimate("anything"), None);
    }
}
