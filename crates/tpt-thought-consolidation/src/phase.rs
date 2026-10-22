// SPDX-License-Identifier: MIT OR Apache-2.0
//! System lifecycle: ACTIVE, IDLE, REFLECTION, SLEEP, CONSOLIDATION, AWAKE.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Where the system is in its day/night cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemPhase {
    /// Serving input.
    Active,
    /// No input; available for reflection or sleep.
    Idle,
    /// Reflecting on recent thoughts (see `tpt-thought-reflection`).
    Reflection,
    /// Sleeping; interruptible.
    Sleep,
    /// Running consolidation steps during sleep.
    Consolidation,
    /// Woken; transitions back to active work.
    Awake,
}

/// An event that moves the system between phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhaseEvent {
    /// Input ceased; the system became idle.
    Idle,
    /// Reflection began.
    Reflect,
    /// Sleep was granted.
    Sleep,
    /// Consolidation began.
    Consolidate,
    /// A wake-up, either from an interrupt or after consolidation finished.
    Wake,
    /// Input resumed.
    Activate,
}

/// The attempted phase transition is not permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalPhaseTransition {
    /// Phase the system was in.
    pub from: SystemPhase,
    /// Event that was rejected.
    pub event: PhaseEvent,
}

impl fmt::Display for IllegalPhaseTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "event {:?} is not permitted in phase {:?}",
            self.event, self.from
        )
    }
}

impl std::error::Error for IllegalPhaseTransition {}

impl SystemPhase {
    /// Apply `event`. Pure: the same input always yields the same output.
    pub fn transition(self, event: PhaseEvent) -> Result<SystemPhase, IllegalPhaseTransition> {
        use PhaseEvent as E;
        use SystemPhase as P;
        let to = match (self, event) {
            (P::Active, E::Idle) => P::Idle,
            (P::Idle, E::Activate) => P::Active,
            (P::Idle, E::Reflect) => P::Reflection,
            (P::Reflection, E::Idle) => P::Idle,
            (P::Idle, E::Sleep) => P::Sleep,
            (P::Sleep, E::Consolidate) => P::Consolidation,
            // Sleep and consolidation are both interruptible.
            (P::Sleep, E::Wake) | (P::Consolidation, E::Wake) => P::Awake,
            (P::Awake, E::Activate) => P::Active,
            _ => return Err(IllegalPhaseTransition { from: self, event }),
        };
        Ok(to)
    }

    /// True while the system is asleep (sleep or consolidation).
    pub const fn is_sleeping(self) -> bool {
        matches!(self, SystemPhase::Sleep | SystemPhase::Consolidation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_and_consolidation_are_interruptible() {
        let s = SystemPhase::Idle.transition(PhaseEvent::Sleep).unwrap();
        assert_eq!(s.transition(PhaseEvent::Wake), Ok(SystemPhase::Awake));
        let c = s.transition(PhaseEvent::Consolidate).unwrap();
        assert_eq!(c.transition(PhaseEvent::Wake), Ok(SystemPhase::Awake));
    }

    #[test]
    fn illegal_transitions_are_rejected() {
        assert!(SystemPhase::Active.transition(PhaseEvent::Sleep).is_err());
        assert!(SystemPhase::Active.transition(PhaseEvent::Wake).is_err());
        assert!(SystemPhase::Reflection
            .transition(PhaseEvent::Consolidate)
            .is_err());
    }
}
