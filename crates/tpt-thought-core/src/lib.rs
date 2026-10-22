// SPDX-License-Identifier: MIT OR Apache-2.0
//! Core types for TPT Thought.
//!
//! This crate has no TPT runtime dependencies and no backend dependencies. It defines the
//! vocabulary of cognition (thoughts, observations, hypotheses, beliefs, predictions,
//! intentions, outcomes, confidence and uncertainty); orchestration lives elsewhere.

pub mod budget;
pub mod confidence;
pub mod content;
pub mod id;
pub mod ownership;
pub mod random;
pub mod state;
pub mod thought;
pub mod time;
pub mod trust;
pub mod uncertainty;
pub mod value;
pub mod wire;

pub use budget::CognitiveBudget;
pub use confidence::{Confidence, Probability};
pub use content::*;
pub use id::*;
pub use ownership::ThoughtOwnership;
pub use random::{RandomnessMetadata, TransitionKind};
pub use state::ThoughtState;
pub use thought::*;
pub use time::{Clock, ManualClock, SystemClock, Temporal, TemporalEvent, Timestamp};
pub use trust::SourcePolicy;
pub use uncertainty::{EpistemicStatus, Uncertainty};
pub use value::ValueAssessment;
