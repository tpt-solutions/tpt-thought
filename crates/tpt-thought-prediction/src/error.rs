// SPDX-License-Identifier: MIT OR Apache-2.0
//! Matching an observation against a prediction, and the resulting prediction error (spec
//! section 18).

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use tpt_thought_core::{
    Observation, ObservationId, Prediction, PredictionId, StructuredValue, ThoughtContent,
    ThoughtId, Timestamp,
};

/// Tunables for comparing predictions with observations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MatchConfig {
    /// A timing offset of this many nanoseconds (or more) counts as a full timing error of `1.0`.
    pub timing_scale_nanos: i64,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            timing_scale_nanos: 1_000_000_000,
        }
    }
}

/// The observation carries no structured payload, so it cannot be compared with a prediction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotComparable {
    /// The observation that could not be compared.
    pub observation: ObservationId,
}

impl fmt::Display for NotComparable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "observation {:?} has no structured payload to compare",
            self.observation
        )
    }
}

impl std::error::Error for NotComparable {}

/// The measured difference between a prediction and what was observed. Magnitudes are in
/// `0.0..=1.0`, where `0.0` is a perfect match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionError {
    /// The prediction that was tested.
    pub prediction: PredictionId,
    /// The observation that tested it.
    pub observation: ObservationId,
    /// The thought that made the prediction.
    pub source_thought: ThoughtId,
    /// Error in the predicted state.
    pub content_error: f64,
    /// Error in the predicted time; `0.0` when no time was predicted.
    pub timing_error: f64,
    /// Observed time minus expected time, in nanoseconds, when a time was predicted.
    pub timing_offset_nanos: Option<i64>,
    /// When the observation was made.
    pub observed_at: Timestamp,
}

impl PredictionError {
    /// The overall magnitude: the larger of the content and timing errors.
    pub fn magnitude(&self) -> f64 {
        self.content_error.max(self.timing_error)
    }

    /// True when the observation diverged from the prediction at all.
    pub fn is_error(&self) -> bool {
        self.magnitude() > 0.0
    }
}

/// Compare `prediction` with `observation`.
pub fn compare(
    prediction: &Prediction,
    observation: &Observation,
    config: &MatchConfig,
) -> Result<PredictionError, NotComparable> {
    let observed = payload(&observation.content).ok_or(NotComparable {
        observation: observation.id,
    })?;
    let content_error = value_error(&prediction.predicted_state, observed);
    let (timing_error, timing_offset_nanos) = match prediction.expected_at {
        Some(expected) => {
            let offset = observation.observed_at.0.saturating_sub(expected.0);
            let scale = config.timing_scale_nanos.max(1);
            let error = (offset.unsigned_abs() as f64 / scale as f64).min(1.0);
            (error, Some(offset))
        }
        None => (0.0, None),
    };
    Ok(PredictionError {
        prediction: prediction.id,
        observation: observation.id,
        source_thought: prediction.source_thought,
        content_error,
        timing_error,
        timing_offset_nanos,
        observed_at: observation.observed_at,
    })
}

fn payload(content: &ThoughtContent) -> Option<&StructuredValue> {
    match content {
        ThoughtContent::Structured(v) => Some(v),
        ThoughtContent::Observation(o) => payload(&o.content),
        _ => None,
    }
}

/// Error between two structured values, in `0.0..=1.0`.
///
/// Numbers differ by their relative size; text and booleans are all-or-nothing; containers
/// average over their members, with a member that is missing on one side counting as `1.0`.
pub fn value_error(predicted: &StructuredValue, observed: &StructuredValue) -> f64 {
    use StructuredValue as V;
    match (predicted, observed) {
        (V::Null, V::Null) => 0.0,
        (V::Bool(a), V::Bool(b)) => f64::from(a != b),
        (V::Text(a), V::Text(b)) => f64::from(a != b),
        (V::Int(_) | V::Float(_), V::Int(_) | V::Float(_)) => {
            let (a, b) = (as_f64(predicted), as_f64(observed));
            if a.is_nan() || b.is_nan() {
                return 1.0;
            }
            if a == b {
                return 0.0;
            }
            ((a - b).abs() / a.abs().max(b.abs()).max(1.0)).min(1.0)
        }
        (V::List(a), V::List(b)) => {
            let len = a.len().max(b.len());
            if len == 0 {
                return 0.0;
            }
            let total: f64 = (0..len)
                .map(|i| match (a.get(i), b.get(i)) {
                    (Some(x), Some(y)) => value_error(x, y),
                    _ => 1.0,
                })
                .sum();
            total / len as f64
        }
        (V::Map(a), V::Map(b)) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            if keys.is_empty() {
                return 0.0;
            }
            let total: f64 = keys
                .iter()
                .map(|k| match (a.get(*k), b.get(*k)) {
                    (Some(x), Some(y)) => value_error(x, y),
                    _ => 1.0,
                })
                .sum();
            total / keys.len() as f64
        }
        _ => 1.0,
    }
}

fn as_f64(v: &StructuredValue) -> f64 {
    match v {
        StructuredValue::Int(i) => *i as f64,
        StructuredValue::Float(f) => *f,
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tpt_thought_core::{
        Confidence, IdGenerator, Probability, SeededEntropy, SourceTrust, TextContent,
        ThoughtContent,
    };

    fn ids() -> IdGenerator {
        IdGenerator::new(SeededEntropy::new(7))
    }

    fn prediction(
        ids: &IdGenerator,
        state: StructuredValue,
        expected_at: Option<i64>,
    ) -> Prediction {
        Prediction {
            id: ids.prediction(),
            predicted_state: state,
            expected_at: expected_at.map(Timestamp),
            confidence: Confidence::from_raw(Probability::new(0.8).unwrap()),
            assumptions: vec!["stable lighting".into()],
            source_thought: ids.thought(),
        }
    }

    fn observation(ids: &IdGenerator, content: ThoughtContent, at: i64) -> Observation {
        Observation {
            id: ids.observation(),
            content: Box::new(content),
            source: "sensor".into(),
            trust: SourceTrust::Trusted,
            observed_at: Timestamp(at),
        }
    }

    #[test]
    fn identical_values_have_no_error() {
        let v = StructuredValue::Float(2.5);
        assert_eq!(value_error(&v, &v.clone()), 0.0);
    }

    #[test]
    fn numbers_scale_by_relative_difference() {
        let e = value_error(&StructuredValue::Float(10.0), &StructuredValue::Int(11));
        assert!((e - 1.0 / 11.0).abs() < 1e-12);
        let far = value_error(&StructuredValue::Int(1), &StructuredValue::Int(1000));
        assert!((far - 0.999).abs() < 1e-12);
    }

    #[test]
    fn mismatched_types_and_text_are_full_errors() {
        assert_eq!(
            value_error(&StructuredValue::Bool(true), &StructuredValue::Bool(false)),
            1.0
        );
        assert_eq!(
            value_error(
                &StructuredValue::Text("a".into()),
                &StructuredValue::Text("a".into())
            ),
            0.0
        );
        assert_eq!(
            value_error(&StructuredValue::Null, &StructuredValue::Int(0)),
            1.0
        );
    }

    #[test]
    fn maps_average_over_keys_and_missing_keys_count_fully() {
        let mut predicted = BTreeMap::new();
        predicted.insert("temp".to_string(), StructuredValue::Float(20.0));
        predicted.insert("door".to_string(), StructuredValue::Bool(true));
        let mut observed = BTreeMap::new();
        observed.insert("temp".to_string(), StructuredValue::Float(20.0));
        // "door" is missing from the observation: 1.0 for that key, 0.0 for temp.
        let e = value_error(
            &StructuredValue::Map(predicted),
            &StructuredValue::Map(observed),
        );
        assert_eq!(e, 0.5);
    }

    #[test]
    fn lists_compare_by_position() {
        let p = StructuredValue::List(vec![StructuredValue::Int(1), StructuredValue::Int(2)]);
        let o = StructuredValue::List(vec![StructuredValue::Int(1)]);
        assert_eq!(value_error(&p, &o), 0.5);
    }

    #[test]
    fn compare_matches_observation_against_prediction() {
        let ids = ids();
        let p = prediction(&ids, StructuredValue::Int(5), Some(100));
        let o = observation(
            &ids,
            ThoughtContent::Structured(StructuredValue::Int(5)),
            100,
        );
        let e = compare(&p, &o, &MatchConfig::default()).unwrap();
        assert!(!e.is_error());
        assert_eq!(e.timing_offset_nanos, Some(0));
        assert_eq!(e.prediction, p.id);
        assert_eq!(e.observation, o.id);
        assert_eq!(e.source_thought, p.source_thought);
    }

    #[test]
    fn timing_error_uses_configured_scale() {
        let ids = ids();
        let p = prediction(&ids, StructuredValue::Null, Some(0));
        let o = observation(&ids, ThoughtContent::Structured(StructuredValue::Null), 500);
        let cfg = MatchConfig {
            timing_scale_nanos: 1000,
        };
        let e = compare(&p, &o, &cfg).unwrap();
        assert_eq!(e.content_error, 0.0);
        assert_eq!(e.timing_error, 0.5);
        assert_eq!(e.timing_offset_nanos, Some(500));
        assert_eq!(e.magnitude(), 0.5);
        assert!(e.is_error());
    }

    #[test]
    fn nested_observation_payload_is_unwrapped() {
        let ids = ids();
        let p = prediction(&ids, StructuredValue::Text("open".into()), None);
        let inner = observation(
            &ids,
            ThoughtContent::Structured(StructuredValue::Text("closed".into())),
            0,
        );
        let o = observation(&ids, ThoughtContent::Observation(inner), 0);
        let e = compare(&p, &o, &MatchConfig::default()).unwrap();
        assert_eq!(e.content_error, 1.0);
        assert_eq!(e.timing_offset_nanos, None);
    }

    #[test]
    fn non_structured_observations_are_not_comparable() {
        let ids = ids();
        let p = prediction(&ids, StructuredValue::Int(1), None);
        let o = observation(
            &ids,
            ThoughtContent::Text(TextContent {
                text: "hello".into(),
                language: None,
            }),
            0,
        );
        let err = compare(&p, &o, &MatchConfig::default()).unwrap_err();
        assert_eq!(err.observation, o.id);
    }
}
