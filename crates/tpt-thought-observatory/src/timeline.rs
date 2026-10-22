// SPDX-License-Identifier: MIT OR Apache-2.0
//! Sleep and reflection cycle visualisation, as data (spec sections 9 and 99). A timeline records
//! which system mode was entered when, so a UI can draw it. Drawing is not done here.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tpt_thought_core::Timestamp;

/// System lifecycle modes (Phase 9): ACTIVE, IDLE, REFLECTION, SLEEP, CONSOLIDATION, AWAKE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SystemMode {
    /// Responding to input.
    Active,
    /// No input; available for reflection or sleep.
    Idle,
    /// Reviewing recent cognition.
    Reflection,
    /// Offline processing, interruptible.
    Sleep,
    /// Replaying and consolidating memory.
    Consolidation,
    /// Woken and back to active use.
    Awake,
}

/// One entry in a [`CycleTimeline`]: the mode entered at `at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    /// When the mode was entered.
    pub at: Timestamp,
    /// The mode entered.
    pub mode: SystemMode,
    /// Why it was entered, such as `"idle timeout"` or `"high-priority event"`.
    pub reason: Option<String>,
}

/// An ordered record of mode transitions. Only real changes are stored: entering the current
/// mode again is a no-op, and times must not go backwards.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleTimeline {
    entries: Vec<TimelineEntry>,
}

impl CycleTimeline {
    /// An empty timeline.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter `mode` at `at`. Returns `false`, and records nothing, if `mode` is the current mode
    /// or `at` is earlier than the last entry.
    pub fn enter(&mut self, at: Timestamp, mode: SystemMode, reason: Option<String>) -> bool {
        if let Some(last) = self.entries.last() {
            if last.mode == mode || at < last.at {
                return false;
            }
        }
        self.entries.push(TimelineEntry { at, mode, reason });
        true
    }

    /// The current mode, if any has been entered.
    pub fn current(&self) -> Option<SystemMode> {
        self.entries.last().map(|e| e.mode)
    }

    /// The recorded entries, oldest first.
    pub fn entries(&self) -> &[TimelineEntry] {
        &self.entries
    }

    /// Total nanoseconds spent in each mode, measuring the final entry up to `until`.
    pub fn time_in_modes(&self, until: Timestamp) -> BTreeMap<SystemMode, i64> {
        let mut totals = BTreeMap::new();
        for (i, entry) in self.entries.iter().enumerate() {
            let end = self.entries.get(i + 1).map_or(until, |next| next.at);
            let span = (end.0 - entry.at.0).max(0);
            *totals.entry(entry.mode).or_insert(0) += span;
        }
        totals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_only_real_changes() {
        let mut t = CycleTimeline::new();
        assert!(t.enter(Timestamp(0), SystemMode::Active, None));
        assert!(!t.enter(Timestamp(5), SystemMode::Active, None));
        assert!(t.enter(Timestamp(10), SystemMode::Idle, Some("quiet".into())));
        assert!(!t.enter(Timestamp(9), SystemMode::Sleep, None));
        assert_eq!(t.entries().len(), 2);
        assert_eq!(t.current(), Some(SystemMode::Idle));
    }

    #[test]
    fn time_in_modes_adds_up() {
        let mut t = CycleTimeline::new();
        t.enter(Timestamp(0), SystemMode::Active, None);
        t.enter(Timestamp(100), SystemMode::Sleep, None);
        t.enter(Timestamp(400), SystemMode::Awake, None);
        let totals = t.time_in_modes(Timestamp(500));
        assert_eq!(totals[&SystemMode::Active], 100);
        assert_eq!(totals[&SystemMode::Sleep], 300);
        assert_eq!(totals[&SystemMode::Awake], 100);
        assert_eq!(totals.values().sum::<i64>(), 500);
    }
}
