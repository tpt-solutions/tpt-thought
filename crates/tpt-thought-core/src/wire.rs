// SPDX-License-Identifier: MIT OR Apache-2.0
//! Versioned wire format with JSON and CBOR encodings (spec section 72).
//!
//! Every encoded value is wrapped in an [`Envelope`] carrying a schema version so decoders can
//! reject or migrate incompatible data. CBOR output is deterministic: structs encode in field
//! order, maps in sorted order, and floats/ints in their shortest form.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt;

/// Current wire schema version. Bump on any incompatible change to encoded types.
pub const WIRE_VERSION: u32 = 1;

/// Versioned wrapper around an encoded value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Schema version the payload was encoded with.
    pub version: u32,
    /// The payload.
    pub payload: T,
}

/// Encoding or decoding failure.
#[derive(Debug)]
pub enum WireError {
    /// Payload written with a different schema version.
    UnsupportedVersion {
        /// Version found in the data.
        found: u32,
    },
    /// JSON failure.
    Json(serde_json::Error),
    /// CBOR encoding failure.
    CborEncode(String),
    /// CBOR decoding failure.
    CborDecode(String),
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { found } => {
                write!(
                    f,
                    "unsupported wire version {found} (expected {WIRE_VERSION})"
                )
            }
            Self::Json(e) => write!(f, "json: {e}"),
            Self::CborEncode(e) => write!(f, "cbor encode: {e}"),
            Self::CborDecode(e) => write!(f, "cbor decode: {e}"),
        }
    }
}

impl std::error::Error for WireError {}

fn check<T>(env: Envelope<T>) -> Result<T, WireError> {
    if env.version == WIRE_VERSION {
        Ok(env.payload)
    } else {
        Err(WireError::UnsupportedVersion { found: env.version })
    }
}

/// Encode as JSON.
pub fn to_json<T: Serialize>(value: &T) -> Result<String, WireError> {
    serde_json::to_string(&Envelope {
        version: WIRE_VERSION,
        payload: value,
    })
    .map_err(WireError::Json)
}

/// Decode from JSON.
pub fn from_json<T: DeserializeOwned>(s: &str) -> Result<T, WireError> {
    check(serde_json::from_str::<Envelope<T>>(s).map_err(WireError::Json)?)
}

/// Encode as deterministic CBOR.
pub fn to_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, WireError> {
    minicbor_serde::to_vec(&Envelope {
        version: WIRE_VERSION,
        payload: value,
    })
    .map_err(|e| WireError::CborEncode(e.to_string()))
}

/// Decode from CBOR.
pub fn from_cbor<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, WireError> {
    check(
        minicbor_serde::from_slice::<Envelope<T>>(bytes)
            .map_err(|e| WireError::CborDecode(e.to_string()))?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn sample() -> Thought {
        let ids = IdGenerator::new(SeededEntropy::new(42));
        let obs = Observation {
            id: ids.observation(),
            content: Box::new(ThoughtContent::Text(TextContent {
                text: "There is smoke.".into(),
                language: Some("en".into()),
            })),
            source: "sensor-1".into(),
            trust: SourceTrust::Authenticated,
            observed_at: Timestamp(1_000),
        };
        let mut t = Thought::from_observation(ids.thought(), obs, Timestamp(2_000));
        t.confidence = Confidence::from_raw(Probability::new(0.8).unwrap());
        t.hypotheses.push(Hypothesis {
            id: ids.hypothesis(),
            statement: "There may be a fire.".into(),
            probability: Probability::new(0.8).ok(),
            confidence: Confidence::UNKNOWN,
        });
        t.predictions.push(Prediction {
            id: ids.prediction(),
            predicted_state: StructuredValue::Map(
                [("temperature_c".to_string(), StructuredValue::Float(850.0))].into(),
            ),
            expected_at: Some(Timestamp(3_000)),
            confidence: Confidence::UNKNOWN,
            assumptions: vec!["sensor calibrated".into()],
            source_thought: t.id,
        });
        t
    }

    #[test]
    fn json_round_trip() {
        let t = sample();
        let back: Thought = from_json(&to_json(&t).unwrap()).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn cbor_round_trip() {
        let t = sample();
        let back: Thought = from_cbor(&to_cbor(&t).unwrap()).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn cbor_is_deterministic() {
        assert_eq!(to_cbor(&sample()).unwrap(), to_cbor(&sample()).unwrap());
    }

    #[test]
    fn wrong_version_rejected() {
        let json = r#"{"version":999,"payload":"x"}"#;
        assert!(matches!(
            from_json::<String>(json),
            Err(WireError::UnsupportedVersion { found: 999 })
        ));
    }

    /// Schema-stability guard: if this changes, the wire format changed and `WIRE_VERSION`
    /// must be bumped (or the change reverted).
    #[test]
    fn json_schema_is_stable() {
        let json = to_json(&ThoughtState::Uncertain).unwrap();
        assert_eq!(json, r#"{"version":1,"payload":"uncertain"}"#);
        let id = IdGenerator::new(SeededEntropy::new(0)).thought();
        assert!(to_json(&id)
            .unwrap()
            .starts_with(r#"{"version":1,"payload":"thought:"#));
    }
}
