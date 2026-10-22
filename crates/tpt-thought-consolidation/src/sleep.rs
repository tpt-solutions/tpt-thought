// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bounded, interruptible sleep (todo Phase 9).
//!
//! Anima CK grants sleep through a [`SleepScheduler`] when resources permit. A granted
//! [`SleepBudget`] bounds both the number of steps and the wall time. A high-priority
//! [`Interrupt`] wakes the system at the next step boundary; the unfinished steps are returned
//! so the cycle can resume later.

use crate::phase::{IllegalPhaseTransition, PhaseEvent, SystemPhase};
use crate::plan::{ConsolidationPlan, ConsolidationStep};
use crate::Consolidator;
use tpt_thought_core::Clock;

/// Limits on one sleep cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SleepBudget {
    /// Most steps to run.
    pub max_steps: usize,
    /// Most wall time to spend, in nanoseconds.
    pub max_duration_nanos: i64,
    /// Interrupts at or above this priority wake the system. Below it they are ignored.
    pub wake_priority: f64,
}

/// An event that may wake a sleeping system.
#[derive(Debug, Clone, PartialEq)]
pub struct Interrupt {
    /// Priority, `0.0..=1.0`.
    pub priority: f64,
    /// Why the interrupt was raised.
    pub reason: String,
}

/// Resources Anima reports when asked to grant sleep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceSnapshot {
    /// Free compute, `0.0..=1.0`.
    pub compute_free: f64,
    /// Memory pressure, `0.0..=1.0` (higher is worse).
    pub memory_pressure: f64,
    /// Energy available, `0.0..=1.0`.
    pub energy_available: f64,
}

/// What the thought layer needs from a sleep cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SleepRequest {
    /// Steps waiting to run.
    pub pending_steps: usize,
    /// Episodes waiting to be replayed.
    pub backlog_episodes: usize,
}

/// Anima CK's answer to a sleep request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SleepGrant {
    /// Sleep may proceed within this budget.
    Granted(SleepBudget),
    /// Resources are too tight; ask again after the delay.
    Deferred {
        /// Nanoseconds to wait before asking again.
        retry_after_nanos: i64,
    },
}

/// Decides when sleep is affordable. Implemented by Anima CK; this crate never schedules CPU.
pub trait SleepScheduler {
    /// Grant or defer a sleep cycle for `request` given `resources`.
    fn grant(&self, request: &SleepRequest, resources: &ResourceSnapshot) -> SleepGrant;
}

/// A simple threshold scheduler: sleep when compute is free, memory is not under pressure and
/// energy is available. The budget is the request, capped at `max_steps`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThresholdScheduler {
    /// Minimum free compute.
    pub min_compute_free: f64,
    /// Maximum tolerated memory pressure.
    pub max_memory_pressure: f64,
    /// Minimum energy available.
    pub min_energy: f64,
    /// Cap on steps per cycle.
    pub max_steps: usize,
    /// Cap on wall time per cycle, in nanoseconds.
    pub max_duration_nanos: i64,
    /// Wake priority to hand out in budgets.
    pub wake_priority: f64,
    /// Delay before asking again when deferred, in nanoseconds.
    pub retry_after_nanos: i64,
}

impl Default for ThresholdScheduler {
    fn default() -> Self {
        Self {
            min_compute_free: 0.3,
            max_memory_pressure: 0.8,
            min_energy: 0.2,
            max_steps: 64,
            max_duration_nanos: 60_000_000_000,
            wake_priority: 0.9,
            retry_after_nanos: 60_000_000_000,
        }
    }
}

impl SleepScheduler for ThresholdScheduler {
    fn grant(&self, request: &SleepRequest, resources: &ResourceSnapshot) -> SleepGrant {
        let affordable = resources.compute_free >= self.min_compute_free
            && resources.memory_pressure <= self.max_memory_pressure
            && resources.energy_available >= self.min_energy;
        if affordable {
            SleepGrant::Granted(SleepBudget {
                max_steps: request.pending_steps.min(self.max_steps),
                max_duration_nanos: self.max_duration_nanos,
                wake_priority: self.wake_priority,
            })
        } else {
            SleepGrant::Deferred {
                retry_after_nanos: self.retry_after_nanos,
            }
        }
    }
}

/// How a sleep cycle ended.
#[derive(Debug, Clone, PartialEq)]
pub enum SleepOutcome {
    /// Every step ran.
    Completed {
        /// Steps run.
        steps_run: usize,
    },
    /// The budget ran out before the plan finished.
    Exhausted {
        /// Steps run.
        steps_run: usize,
        /// Steps not yet run, in order.
        remaining: Vec<ConsolidationStep>,
    },
    /// A high-priority interrupt woke the system.
    Interrupted {
        /// Steps run.
        steps_run: usize,
        /// Steps not yet run, in order.
        remaining: Vec<ConsolidationStep>,
        /// The interrupt that woke the system.
        by: Interrupt,
    },
}

/// The result of running a sleep cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct SleepReport {
    /// How the cycle ended.
    pub outcome: SleepOutcome,
    /// The phase afterwards (always [`SystemPhase::Awake`]).
    pub phase: SystemPhase,
    /// Low-priority interrupts that arrived and were ignored.
    pub ignored_interrupts: Vec<Interrupt>,
}

/// One sleep cycle, from entering sleep to waking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SleepCycle {
    phase: SystemPhase,
    budget: SleepBudget,
}

impl SleepCycle {
    /// Enter sleep from `from` (normally [`SystemPhase::Idle`]) within `budget`.
    pub fn begin(from: SystemPhase, budget: SleepBudget) -> Result<Self, IllegalPhaseTransition> {
        Ok(Self {
            phase: from.transition(PhaseEvent::Sleep)?,
            budget,
        })
    }

    /// The current phase.
    pub fn phase(&self) -> SystemPhase {
        self.phase
    }

    /// Run `plan` against `consolidator`.
    ///
    /// `interrupts` is asked before each step with the number of steps run so far. Each step is
    /// applied atomically, so an interrupt never leaves the consolidator half-updated.
    pub fn run(
        mut self,
        consolidator: &mut Consolidator,
        plan: &ConsolidationPlan,
        clock: &dyn Clock,
        interrupts: &mut dyn FnMut(usize) -> Option<Interrupt>,
    ) -> SleepReport {
        let started = clock.now();
        let steps = plan.steps();
        let mut next = 0;
        let mut ignored = Vec::new();
        loop {
            if let Some(irq) = interrupts(next) {
                if irq.priority >= self.budget.wake_priority {
                    return self.wake(steps, next, SleepEnd::Interrupted(irq), ignored);
                }
                ignored.push(irq);
            }
            if next == steps.len() {
                return self.wake(steps, next, SleepEnd::Completed, ignored);
            }
            let elapsed = clock.now().0 - started.0;
            if next >= self.budget.max_steps || elapsed >= self.budget.max_duration_nanos {
                return self.wake(steps, next, SleepEnd::Exhausted, ignored);
            }
            if self.phase == SystemPhase::Sleep {
                self.advance(PhaseEvent::Consolidate);
            }
            consolidator.apply(steps[next], clock.now());
            next += 1;
        }
    }

    fn advance(&mut self, event: PhaseEvent) {
        // Only legal events are issued at each point, so an illegal transition here would be a
        // bug in this module; the phase is left unchanged rather than panicking mid-sleep.
        if let Ok(to) = self.phase.transition(event) {
            self.phase = to;
        }
    }

    fn wake(
        mut self,
        steps: &[ConsolidationStep],
        done: usize,
        end: SleepEnd,
        ignored: Vec<Interrupt>,
    ) -> SleepReport {
        self.advance(PhaseEvent::Wake);
        let remaining = steps[done..].to_vec();
        let outcome = match end {
            SleepEnd::Completed => SleepOutcome::Completed { steps_run: done },
            SleepEnd::Exhausted => SleepOutcome::Exhausted {
                steps_run: done,
                remaining,
            },
            SleepEnd::Interrupted(by) => SleepOutcome::Interrupted {
                steps_run: done,
                remaining,
                by,
            },
        };
        SleepReport {
            outcome,
            phase: self.phase,
            ignored_interrupts: ignored,
        }
    }
}

enum SleepEnd {
    Completed,
    Exhausted,
    Interrupted(Interrupt),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_scheduler_defers_under_pressure() {
        let s = ThresholdScheduler::default();
        let req = SleepRequest {
            pending_steps: 100,
            backlog_episodes: 3,
        };
        let busy = ResourceSnapshot {
            compute_free: 0.1,
            memory_pressure: 0.1,
            energy_available: 1.0,
        };
        assert!(matches!(s.grant(&req, &busy), SleepGrant::Deferred { .. }));
        let idle = ResourceSnapshot {
            compute_free: 0.9,
            memory_pressure: 0.1,
            energy_available: 1.0,
        };
        match s.grant(&req, &idle) {
            SleepGrant::Granted(b) => assert_eq!(b.max_steps, 64),
            other => panic!("expected a grant, got {other:?}"),
        }
    }
}
