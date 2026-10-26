// SPDX-License-Identifier: MIT OR Apache-2.0
//! The resource gate between a cycle and the compute that backs it (spec sections 35, 48.3, 81).
//!
//! A cycle states what it needs as a [`CognitiveRequirement`]. The backend that owns compute
//! (Anima CK, through the integrations crate) decides whether it is admitted, and the loop
//! honours the answer. The loop never names a backend: it sees only this trait and the
//! [`ResourceSnapshot`] it reports.

use serde::{Deserialize, Serialize};
use tpt_thought_consolidation::ResourceSnapshot;
use tpt_thought_core::Timestamp;

/// What a cycle asks of the scheduler (spec section 35). The gate turns these fields into its own
/// terms; Thought states costs and the backend decides.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CognitiveRequirement {
    /// Cost of the cognition, in the gate's units (for Anima CK, flops). Must be finite and
    /// non-negative. A gate refuses anything else.
    pub estimated_cost: f64,
    /// Attention score used as scheduling priority, from the attention formula.
    pub priority: f64,
    /// Risk the cycle carries, from `0.0..=1.0`.
    pub risk: f64,
    /// Deadline of the cycle's intentions, if any.
    pub deadline: Option<Timestamp>,
}

/// The gate's answer to a requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// The compute is granted. The gate holds it until [`ResourceGate::release`].
    Admitted,
    /// The compute is refused. Nothing is held, and the reason is kept for the caller.
    Denied {
        /// Why the gate refused.
        reason: String,
    },
}

/// The backend that owns compute and says whether a cycle may run. Implemented by the
/// integrations crate over Anima CK, and by [`Unmetered`] when no budget applies.
pub trait ResourceGate {
    /// The resource state the next cycle is judged against. Read before admission, so that
    /// pressure can shape the cognitive mode.
    fn snapshot(&self) -> ResourceSnapshot;

    /// Ask whether `requirement` may run. An admitted requirement is held until it is released.
    fn admit(&mut self, requirement: &CognitiveRequirement) -> Admission;

    /// Return an admitted requirement once its cycle has ended, on every exit path.
    fn release(&mut self, requirement: &CognitiveRequirement);
}

/// A gate with no budget: every cycle is admitted and the resources are never strained. A loop
/// run without a gate behaves as if it were given this one.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unmetered;

impl ResourceGate for Unmetered {
    fn snapshot(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            compute_free: 1.0,
            memory_pressure: 0.0,
            energy_available: 1.0,
        }
    }

    fn admit(&mut self, _requirement: &CognitiveRequirement) -> Admission {
        Admission::Admitted
    }

    fn release(&mut self, _requirement: &CognitiveRequirement) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmetered_admits_everything_and_reports_no_strain() {
        let mut gate = Unmetered;
        let requirement = CognitiveRequirement {
            estimated_cost: f64::MAX,
            ..CognitiveRequirement::default()
        };
        assert_eq!(gate.admit(&requirement), Admission::Admitted);
        gate.release(&requirement);
        let snapshot = gate.snapshot();
        assert_eq!(snapshot.compute_free, 1.0);
        assert_eq!(snapshot.energy_available, 1.0);
        assert_eq!(snapshot.memory_pressure, 0.0);
    }
}
