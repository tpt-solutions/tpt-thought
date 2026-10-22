// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tracking open predictions until an observation resolves them (spec section 17).
//!
//! Which observation answers which prediction is the caller's decision (relevance is a matter
//! for attention and memory). The tracker checks the match, measures the error and reports it
//! as an event.

use crate::error::{compare, MatchConfig, NotComparable, PredictionError};
use std::collections::BTreeMap;
use std::fmt;
use tpt_thought_core::{Observation, Prediction, PredictionId, ThoughtId, Timestamp};

/// Something that happened to a prediction. Errors are events, not exceptions: a resolved
/// prediction is reported whether or not it was wrong.
#[derive(Debug, Clone, PartialEq)]
pub enum PredictionEvent {
    /// The prediction was tested by an observation.
    Resolved(PredictionError),
    /// The expected time passed (plus grace) with no observation.
    Expired {
        /// The prediction that expired.
        prediction: PredictionId,
        /// The thought that made it.
        source_thought: ThoughtId,
        /// When it was declared expired.
        expired_at: Timestamp,
    },
}

/// Why a prediction could not be tracked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackError {
    /// A prediction with this identifier is already open.
    Duplicate(PredictionId),
}

impl fmt::Display for TrackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(id) => write!(f, "prediction {id:?} is already open"),
        }
    }
}

impl std::error::Error for TrackError {}

/// Why an observation could not resolve a prediction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// No open prediction has this identifier.
    Unknown(PredictionId),
    /// The observation was made before the prediction was made.
    Premature {
        /// The prediction in question.
        prediction: PredictionId,
        /// When the observation was made.
        observed_at: Timestamp,
        /// When the prediction was made.
        predicted_at: Timestamp,
    },
    /// The observation has no structured payload to compare.
    NotComparable(NotComparable),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "no open prediction {id:?}"),
            Self::Premature {
                prediction,
                observed_at,
                predicted_at,
            } => write!(
                f,
                "observation at {observed_at:?} precedes prediction {prediction:?} made at {predicted_at:?}"
            ),
            Self::NotComparable(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ResolveError {}

#[derive(Debug, Clone)]
struct Open {
    prediction: Prediction,
    predicted_at: Timestamp,
}

/// The set of open predictions.
#[derive(Debug, Clone, Default)]
pub struct PredictionTracker {
    config: MatchConfig,
    open: BTreeMap<PredictionId, Open>,
}

impl PredictionTracker {
    /// A tracker using the given match configuration.
    pub fn new(config: MatchConfig) -> Self {
        Self {
            config,
            open: BTreeMap::new(),
        }
    }

    /// Number of predictions awaiting an observation.
    pub fn open_count(&self) -> usize {
        self.open.len()
    }

    /// Start tracking `prediction`, made at `predicted_at`.
    pub fn track(
        &mut self,
        prediction: Prediction,
        predicted_at: Timestamp,
    ) -> Result<(), TrackError> {
        if self.open.contains_key(&prediction.id) {
            return Err(TrackError::Duplicate(prediction.id));
        }
        self.open.insert(
            prediction.id,
            Open {
                prediction,
                predicted_at,
            },
        );
        Ok(())
    }

    /// Test open prediction `id` against `observation`. On success the prediction is closed and
    /// the event returned.
    pub fn resolve(
        &mut self,
        id: PredictionId,
        observation: &Observation,
    ) -> Result<PredictionEvent, ResolveError> {
        let open = self.open.get(&id).ok_or(ResolveError::Unknown(id))?;
        if observation.observed_at < open.predicted_at {
            return Err(ResolveError::Premature {
                prediction: id,
                observed_at: observation.observed_at,
                predicted_at: open.predicted_at,
            });
        }
        let error = compare(&open.prediction, observation, &self.config)
            .map_err(ResolveError::NotComparable)?;
        self.open.remove(&id);
        Ok(PredictionEvent::Resolved(error))
    }

    /// Close every open prediction whose expected time, plus `grace_nanos`, is before `now`.
    /// Predictions without an expected time never expire.
    pub fn expire(&mut self, now: Timestamp, grace_nanos: i64) -> Vec<PredictionEvent> {
        let due: Vec<PredictionId> = self
            .open
            .iter()
            .filter(|(_, o)| {
                o.prediction
                    .expected_at
                    .is_some_and(|e| e.0.saturating_add(grace_nanos) < now.0)
            })
            .map(|(id, _)| *id)
            .collect();
        due.into_iter()
            .filter_map(|id| self.open.remove(&id).map(|o| (id, o)))
            .map(|(id, o)| PredictionEvent::Expired {
                prediction: id,
                source_thought: o.prediction.source_thought,
                expired_at: now,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{
        Confidence, IdGenerator, Observation, Probability, SeededEntropy, SourceTrust,
        StructuredValue, TextContent, ThoughtContent,
    };

    struct Fixture {
        ids: IdGenerator,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                ids: IdGenerator::new(SeededEntropy::new(11)),
            }
        }

        fn prediction(&self, value: i64, expected_at: Option<i64>) -> Prediction {
            Prediction {
                id: self.ids.prediction(),
                predicted_state: StructuredValue::Int(value),
                expected_at: expected_at.map(Timestamp),
                confidence: Confidence::from_raw(Probability::new(0.9).unwrap()),
                assumptions: vec![],
                source_thought: self.ids.thought(),
            }
        }

        fn observation(&self, value: i64, at: i64) -> Observation {
            Observation {
                id: self.ids.observation(),
                content: Box::new(ThoughtContent::Structured(StructuredValue::Int(value))),
                source: "sensor".into(),
                trust: SourceTrust::Trusted,
                observed_at: Timestamp(at),
            }
        }
    }

    #[test]
    fn matching_observation_resolves_with_zero_error() {
        let f = Fixture::new();
        let mut t = PredictionTracker::new(MatchConfig::default());
        let p = f.prediction(42, Some(100));
        let id = p.id;
        t.track(p, Timestamp(0)).unwrap();
        let event = t.resolve(id, &f.observation(42, 100)).unwrap();
        match event {
            PredictionEvent::Resolved(e) => {
                assert!(!e.is_error());
                assert_eq!(e.prediction, id);
            }
            other => panic!("unexpected event {other:?}"),
        }
        assert_eq!(t.open_count(), 0);
    }

    #[test]
    fn wrong_observation_reports_error_as_event() {
        let f = Fixture::new();
        let mut t = PredictionTracker::new(MatchConfig::default());
        let p = f.prediction(10, None);
        let id = p.id;
        t.track(p, Timestamp(0)).unwrap();
        match t.resolve(id, &f.observation(20, 5)).unwrap() {
            PredictionEvent::Resolved(e) => {
                assert!((e.content_error - 0.5).abs() < 1e-12);
                assert_eq!(e.timing_offset_nanos, None);
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn duplicate_tracking_is_rejected() {
        let f = Fixture::new();
        let mut t = PredictionTracker::new(MatchConfig::default());
        let p = f.prediction(1, None);
        let dup = p.clone();
        t.track(p, Timestamp(0)).unwrap();
        assert_eq!(
            t.track(dup.clone(), Timestamp(0)),
            Err(TrackError::Duplicate(dup.id))
        );
    }

    #[test]
    fn unknown_and_premature_resolutions_fail_without_side_effects() {
        let f = Fixture::new();
        let mut t = PredictionTracker::new(MatchConfig::default());
        let unknown = f.prediction(1, None).id;
        assert_eq!(
            t.resolve(unknown, &f.observation(1, 0)),
            Err(ResolveError::Unknown(unknown))
        );

        let p = f.prediction(1, None);
        let id = p.id;
        t.track(p, Timestamp(50)).unwrap();
        assert!(matches!(
            t.resolve(id, &f.observation(1, 10)),
            Err(ResolveError::Premature { .. })
        ));
        assert_eq!(
            t.open_count(),
            1,
            "premature observation must not close the prediction"
        );
    }

    #[test]
    fn non_structured_observation_is_not_comparable() {
        let f = Fixture::new();
        let mut t = PredictionTracker::new(MatchConfig::default());
        let p = f.prediction(1, None);
        let id = p.id;
        t.track(p, Timestamp(0)).unwrap();
        let obs = Observation {
            content: Box::new(ThoughtContent::Text(TextContent {
                text: "hi".into(),
                language: None,
            })),
            ..f.observation(0, 0)
        };
        assert!(matches!(
            t.resolve(id, &obs),
            Err(ResolveError::NotComparable(_))
        ));
        assert_eq!(t.open_count(), 1);
    }

    #[test]
    fn expiry_respects_expected_time_and_grace() {
        let f = Fixture::new();
        let mut t = PredictionTracker::new(MatchConfig::default());
        let timed = f.prediction(1, Some(100));
        let timed_id = timed.id;
        let untimed = f.prediction(2, None);
        t.track(timed, Timestamp(0)).unwrap();
        t.track(untimed, Timestamp(0)).unwrap();

        assert!(
            t.expire(Timestamp(150), 100).is_empty(),
            "still within grace"
        );
        let events = t.expire(Timestamp(201), 100);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            PredictionEvent::Expired { prediction, .. } if *prediction == timed_id
        ));
        assert_eq!(t.open_count(), 1, "untimed predictions never expire");
    }
}
