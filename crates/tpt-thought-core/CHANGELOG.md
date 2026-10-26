# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- IDs for thoughts, observations, hypotheses, predictions, intentions, outcomes, episodes and
  provenance records, generated through `IdGenerator` over an injectable `EntropySource`.
- `SeededEntropy` for deterministic tests and replay, and `OsEntropy` behind the default
  `os-entropy` feature.
- `Thought`, its origin, context, attention state and provenance links.
- Content types: text, structured and multimodal content, content references, observations,
  hypotheses, beliefs, predictions, questions, decisions, intentions, outcomes and alternatives.
- `ThoughtState` with the seventeen lifecycle states.
- `Probability` and `Confidence`, with validated construction.
- `Uncertainty` and `EpistemicStatus`, with an explicit unknown state.
- `Timestamp`, the `Clock` trait, `SystemClock`, `ManualClock` and `Temporal` records.
- `CognitiveBudget`, with `CognitiveBudget::UNLIMITED`.
- `SourcePolicy` and `SourceTrust`, which classify inputs as trusted or untrusted by default.
- `ThoughtOwnership`, `RandomnessMetadata` and `TransitionKind`.
- `ValueAssessment`.
- Versioned wire format: `Envelope`, `WIRE_VERSION`, and JSON and CBOR encoding and decoding.
