// SPDX-License-Identifier: MIT OR Apache-2.0
//! Structured telemetry events (spec section 98).

use serde::{Deserialize, Serialize};
use tpt_thought_core::{ThoughtId, ThoughtState, Timestamp};

/// The kind of a telemetry event. [`EventKind::name`] gives the stable wire name used by
/// exporters, so variants may be reordered or added but their names must not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EventKind {
    /// `thought.created`
    ThoughtCreated,
    /// `thought.attended`
    ThoughtAttended,
    /// `thought.state.changed`: any lifecycle transition (carries the new state).
    ThoughtStateChanged,
    /// `thought.branch.created`
    ThoughtBranchCreated,
    /// `thought.hypothesis.created`
    ThoughtHypothesisCreated,
    /// `thought.prediction.created`
    ThoughtPredictionCreated,
    /// `thought.escalated`
    ThoughtEscalated,
    /// `thought.decision.received`
    ThoughtDecisionReceived,
    /// `thought.intention.created`
    ThoughtIntentionCreated,
    /// `thought.action.started`
    ThoughtActionStarted,
    /// `thought.action.completed`
    ThoughtActionCompleted,
    /// `thought.prediction.error`
    ThoughtPredictionError,
    /// `thought.belief.updated`
    ThoughtBeliefUpdated,
    /// `thought.consolidated`
    ThoughtConsolidated,
    /// `thought.aborted`
    ThoughtAborted,
    /// `system.mode.changed`: the system moved between active, idle, reflection, sleep and so on.
    SystemModeChanged,
}

impl EventKind {
    /// Every kind, in declaration order.
    pub const ALL: [EventKind; 16] = [
        Self::ThoughtCreated,
        Self::ThoughtAttended,
        Self::ThoughtStateChanged,
        Self::ThoughtBranchCreated,
        Self::ThoughtHypothesisCreated,
        Self::ThoughtPredictionCreated,
        Self::ThoughtEscalated,
        Self::ThoughtDecisionReceived,
        Self::ThoughtIntentionCreated,
        Self::ThoughtActionStarted,
        Self::ThoughtActionCompleted,
        Self::ThoughtPredictionError,
        Self::ThoughtBeliefUpdated,
        Self::ThoughtConsolidated,
        Self::ThoughtAborted,
        Self::SystemModeChanged,
    ];

    /// The stable dotted name, such as `thought.created`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::ThoughtCreated => "thought.created",
            Self::ThoughtAttended => "thought.attended",
            Self::ThoughtStateChanged => "thought.state.changed",
            Self::ThoughtBranchCreated => "thought.branch.created",
            Self::ThoughtHypothesisCreated => "thought.hypothesis.created",
            Self::ThoughtPredictionCreated => "thought.prediction.created",
            Self::ThoughtEscalated => "thought.escalated",
            Self::ThoughtDecisionReceived => "thought.decision.received",
            Self::ThoughtIntentionCreated => "thought.intention.created",
            Self::ThoughtActionStarted => "thought.action.started",
            Self::ThoughtActionCompleted => "thought.action.completed",
            Self::ThoughtPredictionError => "thought.prediction.error",
            Self::ThoughtBeliefUpdated => "thought.belief.updated",
            Self::ThoughtConsolidated => "thought.consolidated",
            Self::ThoughtAborted => "thought.aborted",
            Self::SystemModeChanged => "system.mode.changed",
        }
    }
}

/// One telemetry event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TelemetryEvent {
    /// What happened.
    pub kind: EventKind,
    /// When it happened.
    pub at: Timestamp,
    /// The thought concerned, if any.
    pub thought: Option<ThoughtId>,
    /// The new lifecycle state, for [`EventKind::ThoughtStateChanged`].
    pub state: Option<ThoughtState>,
    /// An optional numeric payload, such as a prediction error or a cost.
    pub value: Option<f64>,
}

impl TelemetryEvent {
    /// An event with no thought, state or value attached.
    pub fn new(kind: EventKind, at: Timestamp) -> Self {
        Self {
            kind,
            at,
            thought: None,
            state: None,
            value: None,
        }
    }

    /// Attach the thought concerned.
    pub fn for_thought(mut self, thought: ThoughtId) -> Self {
        self.thought = Some(thought);
        self
    }

    /// Attach a lifecycle state.
    pub fn with_state(mut self, state: ThoughtState) -> Self {
        self.state = Some(state);
        self
    }

    /// Attach a numeric value.
    pub fn with_value(mut self, value: f64) -> Self {
        self.value = Some(value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names_are_stable() {
        let names: Vec<&str> = EventKind::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(
            names,
            [
                "thought.created",
                "thought.attended",
                "thought.state.changed",
                "thought.branch.created",
                "thought.hypothesis.created",
                "thought.prediction.created",
                "thought.escalated",
                "thought.decision.received",
                "thought.intention.created",
                "thought.action.started",
                "thought.action.completed",
                "thought.prediction.error",
                "thought.belief.updated",
                "thought.consolidated",
                "thought.aborted",
                "system.mode.changed",
            ]
        );
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<&str> = EventKind::ALL.iter().map(|k| k.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), EventKind::ALL.len());
    }
}
