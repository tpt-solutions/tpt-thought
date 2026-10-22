// SPDX-License-Identifier: MIT OR Apache-2.0
//! Attention calculation, cognitive demand and working memory (spec sections 11, 13, 28, 35).
//!
//! Thought calculates cognitive *demand*; Anima CK decides how that maps to scheduling.

mod demand;
mod formula;
mod working_memory;

pub use demand::{demand, CognitiveDemand};
pub use formula::{apply_attention, AttentionFactors, AttentionFormula, WeightedSum};
pub use working_memory::{
    ForgettingState, InsertOutcome, WorkingMemory, WorkingMemoryConfig, WorkingMemoryItem,
};
