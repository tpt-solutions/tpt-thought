// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cognitive budget (spec section 63). `None` means unbounded in that dimension.

use serde::{Deserialize, Serialize};

/// Limits on how much cognition a thought may consume.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct CognitiveBudget {
    /// Maximum wall-clock latency in nanoseconds.
    pub max_latency_nanos: Option<u64>,
    /// Maximum compute, in provider-defined units.
    pub max_compute: Option<f64>,
    /// Maximum monetary or abstract cost.
    pub max_cost: Option<f64>,
    /// Maximum simultaneous branches.
    pub max_branches: Option<u32>,
    /// Maximum loop iterations.
    pub max_iterations: Option<u32>,
    /// Maximum calls to external providers.
    pub max_external_calls: Option<u32>,
}

impl CognitiveBudget {
    /// No limits.
    pub const UNLIMITED: CognitiveBudget = CognitiveBudget {
        max_latency_nanos: None,
        max_compute: None,
        max_cost: None,
        max_branches: None,
        max_iterations: None,
        max_external_calls: None,
    };
}
