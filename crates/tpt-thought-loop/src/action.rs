// SPDX-License-Identifier: MIT OR Apache-2.0
//! The action boundary (spec sections 24, 25).
//!
//! Thought never executes an action itself. It hands an [`Action`] to an [`ActionSink`], which is
//! where Anima's INP or any other executor lives, and receives back the actual outcome.

use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_core::{IntentionId, StructuredValue};

/// An action Thought asks an executor to perform on behalf of an intention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    /// The intention this action serves.
    pub intention: IntentionId,
    /// The chosen plan's name.
    pub name: String,
    /// The expected outcome, against which the actual outcome is compared.
    pub expected: StructuredValue,
}

/// An executor refused or failed to perform an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    /// The executor declined the action.
    Refused(String),
    /// The action was attempted and failed.
    Failed(String),
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(reason) => write!(f, "action refused: {reason}"),
            Self::Failed(reason) => write!(f, "action failed: {reason}"),
        }
    }
}

impl std::error::Error for ActionError {}

/// Performs actions for Thought and reports what actually happened.
pub trait ActionSink {
    /// Perform `action` and return the observed outcome.
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError>;
}
