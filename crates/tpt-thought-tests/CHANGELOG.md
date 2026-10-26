# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- Acceptance tests, one per criterion A to M (spec section 122), against mock adapters.
- Golden scenario with mock Anima, Intellect, Eve and Augur (spec section 123).
- Adversarial tests for contradictions, false observations, memory contamination, provider failure
  and disagreement, and recursive thought.
- Resource tests for bounded cognition under budgets, limits and circuit breakers.
- Lifecycle property tests over the transition table.
- Decoder fuzzing over arbitrary bytes, driven by proptest.
- Benchmark harness (`benches/pipeline.rs`) with a `cargo test` smoke check of its deterministic
  counts.
- `real-adapters` feature: acceptance criteria D and F, and the golden scenario, against the real
  Intellect and Eve adapters.
