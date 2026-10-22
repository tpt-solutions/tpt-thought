// SPDX-License-Identifier: MIT OR Apache-2.0
//! Episodic memory records (spec sections 14 and 48.2). Thought builds these while it works;
//! Anima persists them in SSE. Records reference other entities by identifier.

use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_core::{ActionRef, ContentReference, ThoughtContent, ThoughtId, Timestamp};

/// One thing that happened during an episode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeEvent {
    /// When it happened.
    pub at: Timestamp,
    /// The thought it concerns, if any.
    pub thought: Option<ThoughtId>,
    /// What happened.
    pub content: ThoughtContent,
}

/// A prediction made during an episode, referenced by identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionRecord {
    /// The prediction, stored in its owning module.
    pub prediction: ContentReference,
    /// When it was made.
    pub predicted_at: Timestamp,
    /// When the predicted state was expected, if a time was predicted.
    pub expected_at: Option<Timestamp>,
    /// Confidence in the prediction, `0.0`-`1.0`.
    pub confidence: f64,
}

/// An action issued during an episode, with its expected and actual outcomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionRecord {
    /// The action, issued through Anima/INP.
    pub action: ActionRef,
    /// When it was issued.
    pub issued_at: Timestamp,
    /// The outcome that was expected, if one was predicted.
    pub expected: Option<ThoughtContent>,
    /// The outcome that was actually observed, once known.
    pub actual: Option<ThoughtContent>,
}

/// How an episode ended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeOutcome {
    /// When it ended.
    pub at: Timestamp,
    /// Summary of the result.
    pub summary: ThoughtContent,
    /// Prediction error for the episode, if it could be measured.
    pub prediction_error: Option<f64>,
}

/// Errors from building an episode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpisodeError {
    /// An event was recorded earlier than the last one, so the record would not replay in order.
    OutOfOrder {
        /// Time of the last recorded event.
        last: Timestamp,
        /// Time of the rejected event.
        attempted: Timestamp,
    },
    /// The episode already has an outcome and cannot change.
    AlreadyClosed,
}

impl fmt::Display for EpisodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfOrder { last, attempted } => write!(
                f,
                "event at {} is earlier than the last event at {}",
                attempted.0, last.0
            ),
            Self::AlreadyClosed => write!(f, "episode is already closed"),
        }
    }
}

impl std::error::Error for EpisodeError {}

/// A bounded run of cognition and action, replayable from its records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    /// Identity, assigned by the owner of the episode store.
    pub id: String,
    /// When the episode started.
    pub started_at: Timestamp,
    /// Events, in the order they happened.
    pub events: Vec<EpisodeEvent>,
    /// Predictions made during the episode.
    pub predictions: Vec<PredictionRecord>,
    /// Actions issued during the episode.
    pub actions: Vec<ActionRecord>,
    /// The outcome, once the episode is closed.
    pub outcome: Option<EpisodeOutcome>,
}

impl Episode {
    /// A new, open episode.
    pub fn new(id: impl Into<String>, started_at: Timestamp) -> Self {
        Self {
            id: id.into(),
            started_at,
            events: Vec::new(),
            predictions: Vec::new(),
            actions: Vec::new(),
            outcome: None,
        }
    }

    /// Record an event. Events must not go backwards in time.
    pub fn record_event(&mut self, event: EpisodeEvent) -> Result<(), EpisodeError> {
        self.ensure_open()?;
        if let Some(last) = self.events.last() {
            if event.at < last.at {
                return Err(EpisodeError::OutOfOrder {
                    last: last.at,
                    attempted: event.at,
                });
            }
        }
        self.events.push(event);
        Ok(())
    }

    /// Record a prediction.
    pub fn record_prediction(&mut self, prediction: PredictionRecord) -> Result<(), EpisodeError> {
        self.ensure_open()?;
        self.predictions.push(prediction);
        Ok(())
    }

    /// Record an issued action.
    pub fn record_action(&mut self, action: ActionRecord) -> Result<(), EpisodeError> {
        self.ensure_open()?;
        self.actions.push(action);
        Ok(())
    }

    /// Close the episode with an outcome. Closing is permitted once.
    pub fn close(&mut self, outcome: EpisodeOutcome) -> Result<(), EpisodeError> {
        self.ensure_open()?;
        self.outcome = Some(outcome);
        Ok(())
    }

    /// True once an outcome has been recorded.
    pub fn is_closed(&self) -> bool {
        self.outcome.is_some()
    }

    fn ensure_open(&self) -> Result<(), EpisodeError> {
        if self.is_closed() {
            Err(EpisodeError::AlreadyClosed)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::TextContent;

    fn text(s: &str) -> ThoughtContent {
        ThoughtContent::Text(TextContent {
            text: s.into(),
            language: None,
        })
    }

    fn event(at: i64) -> EpisodeEvent {
        EpisodeEvent {
            at: Timestamp(at),
            thought: None,
            content: text("e"),
        }
    }

    fn outcome(at: i64) -> EpisodeOutcome {
        EpisodeOutcome {
            at: Timestamp(at),
            summary: text("done"),
            prediction_error: Some(0.1),
        }
    }

    #[test]
    fn events_must_not_go_backwards() {
        let mut ep = Episode::new("ep-1", Timestamp(0));
        ep.record_event(event(5)).unwrap();
        ep.record_event(event(5)).unwrap();
        assert_eq!(
            ep.record_event(event(4)),
            Err(EpisodeError::OutOfOrder {
                last: Timestamp(5),
                attempted: Timestamp(4),
            })
        );
        assert_eq!(ep.events.len(), 2, "rejected event is not stored");
    }

    #[test]
    fn closed_episodes_are_immutable() {
        let mut ep = Episode::new("ep-2", Timestamp(0));
        ep.close(outcome(10)).unwrap();
        assert!(ep.is_closed());
        assert_eq!(ep.record_event(event(11)), Err(EpisodeError::AlreadyClosed));
        assert_eq!(ep.close(outcome(12)), Err(EpisodeError::AlreadyClosed));
        assert_eq!(ep.outcome.unwrap().at, Timestamp(10));
    }

    #[test]
    fn records_predictions_and_actions() {
        let mut ep = Episode::new("ep-3", Timestamp(0));
        ep.record_prediction(PredictionRecord {
            prediction: ContentReference {
                store: "thought".into(),
                id: "p1".into(),
            },
            predicted_at: Timestamp(1),
            expected_at: Some(Timestamp(3)),
            confidence: 0.8,
        })
        .unwrap();
        ep.record_action(ActionRecord {
            action: ContentReference {
                store: "inp".into(),
                id: "a1".into(),
            },
            issued_at: Timestamp(2),
            expected: Some(text("door open")),
            actual: None,
        })
        .unwrap();
        assert_eq!(ep.predictions.len(), 1);
        assert_eq!(ep.actions[0].actual, None);
    }

    #[test]
    fn episode_round_trips_through_json() {
        let mut ep = Episode::new("ep-4", Timestamp(0));
        ep.record_event(event(1)).unwrap();
        ep.close(outcome(2)).unwrap();
        let json = serde_json::to_string(&ep).unwrap();
        let back: Episode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ep);
    }
}
