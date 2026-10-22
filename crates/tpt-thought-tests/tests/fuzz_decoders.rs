// SPDX-License-Identifier: MIT OR Apache-2.0
//! Decoder fuzzing (spec section 95), driven by proptest rather than cargo-fuzz so it runs on
//! stable Rust with no extra tooling. Every decoder must return `Ok` or `Err` for arbitrary input,
//! and must never panic. A panic fails the property.

mod common;

use proptest::prelude::*;
use tpt_thought_core::wire;
use tpt_thought_core::{StructuredValue, Thought, ThoughtContent, ThoughtOrigin, Timestamp};
use tpt_thought_loop::ReplayRecord;
use tpt_thought_provenance::ProvenanceRecord;

fn sample_thought() -> Thought {
    let ids = common::ids(61);
    Thought::new(
        ids.thought(),
        ThoughtOrigin::Observation,
        ThoughtContent::Structured(StructuredValue::Text("cup within reach".into())),
        Timestamp(1_000),
    )
}

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..1024)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn core_json_decoder_never_panics(input in bytes()) {
        if let Ok(text) = std::str::from_utf8(&input) {
            let _ = wire::from_json::<Thought>(text);
        }
    }

    #[test]
    fn core_cbor_decoder_never_panics(input in bytes()) {
        let _ = wire::from_cbor::<Thought>(&input);
    }

    #[test]
    fn replay_record_decoder_never_panics(input in bytes()) {
        let _ = serde_json::from_slice::<ReplayRecord>(&input);
    }

    #[test]
    fn provenance_record_decoder_never_panics(input in bytes()) {
        let _ = serde_json::from_slice::<ProvenanceRecord>(&input);
    }

    /// Mutating a valid encoding in place (flip, insert or truncate) must still decode safely.
    #[test]
    fn mutated_valid_json_never_panics(
        edits in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..16),
        cut in any::<prop::sample::Index>(),
    ) {
        let mut encoded = wire::to_json(&sample_thought()).expect("encode").into_bytes();
        for (at, value) in edits {
            let i = at.index(encoded.len());
            encoded[i] = value;
        }
        encoded.truncate(cut.index(encoded.len() + 1));
        if let Ok(text) = std::str::from_utf8(&encoded) {
            let _ = wire::from_json::<Thought>(text);
        }
    }

    /// Mutating a valid CBOR encoding must also decode safely.
    #[test]
    fn mutated_valid_cbor_never_panics(
        edits in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..16),
    ) {
        let mut encoded = wire::to_cbor(&sample_thought()).expect("encode");
        for (at, value) in edits {
            let i = at.index(encoded.len());
            encoded[i] = value;
        }
        let _ = wire::from_cbor::<Thought>(&encoded);
    }
}

#[test]
fn valid_encodings_still_round_trip() {
    let thought = sample_thought();
    let json = wire::to_json(&thought).expect("json");
    assert_eq!(wire::from_json::<Thought>(&json).expect("decode"), thought);
    let cbor = wire::to_cbor(&thought).expect("cbor");
    assert_eq!(wire::from_cbor::<Thought>(&cbor).expect("decode"), thought);
}
