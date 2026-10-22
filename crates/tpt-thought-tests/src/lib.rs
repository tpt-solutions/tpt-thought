// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test suites for TPT Thought (spec sections 95, 96, 122 and 123).
//!
//! The suites live in `tests/` and the benchmark harness in `benches/`. This crate has no
//! library API of its own; it exists so the cross-crate suites run under `cargo test`.
//!
//! Coverage:
//!
//! - `tests/state_properties.rs`: deterministic state tests and property tests over the
//!   lifecycle transition table.
//! - `tests/acceptance.rs`: one test per acceptance criterion A to M (spec section 122).
//! - `tests/golden.rs`: the robot and unfamiliar-object demonstration (spec section 123), with
//!   mock Anima, Intellect, Eve and Augur.
//! - `tests/adversarial.rs`: contradictions, false observations, memory contamination, provider
//!   failure and disagreement, recursive thought.
//! - `tests/resources.rs`: bounded cognition under budgets, limits and breakers.
//! - `tests/fuzz_decoders.rs`: decoder fuzzing over arbitrary bytes, driven by proptest. This
//!   uses proptest rather than cargo-fuzz so the suite runs on stable Rust with no extra tooling.
//! - `benches/pipeline.rs`: the benchmark harness (spec section 96). Timings are informational;
//!   the counted metrics are deterministic.
//!
//! Blocked, and deliberately unchecked in `todo.md`: "same scenario against real sibling
//! adapters" (spec section 123). The golden scenario uses mocks only, because the integrations
//! crate that wraps Intellect, Eve, Anima and Augur is on hold pending approval for building
//! sibling-repository code.
