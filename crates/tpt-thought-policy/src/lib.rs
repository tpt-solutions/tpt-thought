// SPDX-License-Identifier: MIT OR Apache-2.0
//! Escalation, branching, resource and termination policies (spec sections 20, 36, 63).
//!
//! Policies are pure functions of a [`PolicyContext`]. They never execute cognition; they only
//! decide whether to accept the current answer, escalate to a deeper level, or terminate.

mod budget;
mod config;
mod decision;
mod level;
mod policies;
mod value;

pub use budget::{BudgetExhausted, Dimension, ResourceBudget, ResourceUsage};
pub use config::EscalationConfig;
pub use decision::{Decision, Policy, PolicyContext, PolicySet, Termination};
pub use level::{EscalationLadder, EscalationLevel, LadderError};
pub use policies::{BranchPolicy, BudgetGuard, EscalationGate, HumanEscalation, RiskCeiling};
pub use value::{CognitionValue, InvalidCognitionValue};
