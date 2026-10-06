// SPDX-License-Identifier: MIT OR Apache-2.0
//! Determinism markers (spec section 33) and randomness metadata (spec section 34).

use serde::{Deserialize, Serialize};

/// Whether a transition can be reproduced exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    /// Same input always yields the same transition.
    Deterministic,
    /// Outcome depends on randomness or a non-deterministic backend.
    Probabilistic,
}

/// Replay-relevant description of a probabilistic operation. All fields optional: record
/// what is available.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RandomnessMetadata {
    /// RNG seed.
    pub seed: Option<u64>,
    /// Origin of the randomness (e.g. a provider or model identifier).
    pub source: Option<String>,
    /// Sampling distribution.
    pub distribution: Option<String>,
    /// Sampling method.
    pub sampling_method: Option<String>,
}
