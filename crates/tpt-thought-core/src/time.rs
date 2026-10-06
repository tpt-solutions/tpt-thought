// SPDX-License-Identifier: MIT OR Apache-2.0
//! Temporal types and an injectable clock for deterministic replay (spec section 74).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nanoseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub i64);

impl Timestamp {
    /// The Unix epoch.
    pub const EPOCH: Timestamp = Timestamp(0);

    /// Nanoseconds since the Unix epoch.
    pub const fn as_nanos(self) -> i64 {
        self.0
    }
}

/// A source of time. Inject a [`ManualClock`] for deterministic replay.
pub trait Clock: Send + Sync {
    /// The current time.
    fn now(&self) -> Timestamp;
}

/// Wall-clock time.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        Timestamp(nanos)
    }
}

/// A clock that only moves when told to.
#[derive(Debug, Default)]
pub struct ManualClock(AtomicI64);

impl ManualClock {
    /// Start at `start`.
    pub fn new(start: Timestamp) -> Self {
        Self(AtomicI64::new(start.0))
    }

    /// Advance by `nanos`.
    pub fn advance(&self, nanos: i64) {
        self.0.fetch_add(nanos, Ordering::SeqCst);
    }

    /// Set the absolute time.
    pub fn set(&self, t: Timestamp) {
        self.0.store(t.0, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        Timestamp(self.0.load(Ordering::SeqCst))
    }
}

/// The temporal facets of a thought.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Temporal {
    /// When the thought was created.
    pub created_at: Option<Timestamp>,
    /// When it was last modified.
    pub updated_at: Option<Timestamp>,
    /// When the underlying observation occurred.
    pub observed_at: Option<Timestamp>,
    /// When a prediction was made.
    pub predicted_at: Option<Timestamp>,
    /// When the predicted state is expected to hold.
    pub expected_at: Option<Timestamp>,
    /// When an expectation was resolved against reality.
    pub resolved_at: Option<Timestamp>,
}

impl Temporal {
    /// Temporal facets for a thought created at `now`.
    pub fn created(now: Timestamp) -> Self {
        Self {
            created_at: Some(now),
            updated_at: Some(now),
            ..Self::default()
        }
    }
}

/// Kinds of temporal event. Events are first-class: relationships are not inferred solely
/// from timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum EventKind {
    /// Something was observed.
    Observed,
    /// A prediction was made.
    Predicted,
    /// A prediction or expectation resolved.
    Resolved,
    /// An action began.
    ActionStarted,
    /// An action finished.
    ActionCompleted,
    /// A state transition occurred.
    Transition,
}

/// A first-class temporal event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalEvent {
    /// What happened.
    pub kind: EventKind,
    /// When.
    pub at: Timestamp,
    /// Monotonic sequence number within its stream; orders events with equal timestamps.
    pub sequence: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_is_controlled() {
        let c = ManualClock::new(Timestamp(10));
        assert_eq!(c.now(), Timestamp(10));
        c.advance(5);
        assert_eq!(c.now(), Timestamp(15));
    }

    #[test]
    fn system_clock_is_after_epoch() {
        assert!(SystemClock.now() > Timestamp::EPOCH);
    }
}
