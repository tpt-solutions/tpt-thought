// SPDX-License-Identifier: MIT OR Apache-2.0
use serde::{Deserialize, Serialize};

/// The three ways a cognitive system runs. The rules differ, so a thought is always run in
/// exactly one mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CognitiveMode {
    /// Live thinking about the present input. Latency-bound. It considers and proposes, but a
    /// thought is never a belief merely because it was considered (spec section 15).
    Online,
    /// Bounded review of past thoughts, triggered by errors, uncertainty, contradictions,
    /// goal changes, outcome mismatches or a schedule. May propose durable revisions.
    Reflection,
    /// Offline consolidation, owned by `tpt-thought-consolidation`. Interruptible.
    Sleep,
}

/// What a mode is allowed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeRules {
    /// Must finish within a latency budget.
    pub latency_bound: bool,
    /// May be preempted by a high-priority event.
    pub interruptible: bool,
    /// May propose durable belief or memory changes.
    pub durable_updates: bool,
    /// May read thoughts that are no longer live.
    pub reads_past_thoughts: bool,
}

impl CognitiveMode {
    /// The rules that govern this mode.
    pub const fn rules(self) -> ModeRules {
        match self {
            Self::Online => ModeRules {
                latency_bound: true,
                interruptible: false,
                durable_updates: false,
                reads_past_thoughts: false,
            },
            Self::Reflection => ModeRules {
                latency_bound: false,
                interruptible: true,
                durable_updates: true,
                reads_past_thoughts: true,
            },
            Self::Sleep => ModeRules {
                latency_bound: false,
                interruptible: true,
                durable_updates: true,
                reads_past_thoughts: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_thought_cannot_update_durably() {
        assert!(!CognitiveMode::Online.rules().durable_updates);
        assert!(CognitiveMode::Online.rules().latency_bound);
    }

    #[test]
    fn reflection_reads_past_and_is_interruptible() {
        let r = CognitiveMode::Reflection.rules();
        assert!(r.reads_past_thoughts && r.interruptible && !r.latency_bound);
    }
}
