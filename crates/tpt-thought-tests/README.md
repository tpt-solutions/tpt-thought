# tpt-thought-tests

The test suites and benchmark harness for TPT Thought. This crate has no library API of its own.
It exists so that the cross-crate suites run under `cargo test` and `cargo bench`.

**Status:** pre-release, v0.1.1. Not published (`publish = false`). It is part of the repository
only.

## What it is for

Most TPT Thought crates test their own units. The suites here test behaviour that spans several
crates at once: the acceptance criteria, the golden scenario, adversarial inputs, resource limits,
lifecycle properties, decoder robustness and pipeline performance.

## Running the tests

```sh
cargo test -p tpt-thought-tests
```

This runs the mock-backed suites. They need no sibling repositories and no network access beyond
the crates.io and git sources the workspace already uses.

To run the acceptance and golden scenarios against the real Intellect and Eve adapters, enable the
`real-adapters` feature:

```sh
cargo test -p tpt-thought-tests --features real-adapters
```

This feature pulls in `tpt-thought-integrations` with its `intellect` and `eve` features. Those
pull in sibling repositories pinned to git revisions, so the first build fetches them. See
`docs/INTEGRATION_INTELLECT.md` and `docs/INTEGRATION_EVE.md`.

The benchmark harness runs with:

```sh
cargo bench -p tpt-thought-tests
```

## Suites

| File | Covers |
|---|---|
| `tests/state_properties.rs` | Deterministic tests and property tests over the lifecycle transition table (`tpt-thought-state`). |
| `tests/acceptance.rs` | One test per acceptance criterion, A to M (spec section 122), run against mock adapters. |
| `tests/golden.rs` | The robot and unfamiliar-object demonstration (spec section 123), with mock Anima, Intellect, Eve and Augur. |
| `tests/adversarial.rs` | Contradictions, false observations, memory contamination, provider failure and disagreement, and recursive thought. |
| `tests/resources.rs` | Bounded cognition under budgets, limits and circuit breakers. |
| `tests/fuzz_decoders.rs` | Decoder robustness over arbitrary bytes, driven by proptest. It uses proptest rather than cargo-fuzz so it runs on stable Rust. |
| `tests/benchmark_smoke.rs` | Checks the deterministic counts of the benchmark scenarios under `cargo test`. It never checks timings. |
| `tests/real_adapters.rs` | Criteria D and F against the real adapters. Compiled only with `real-adapters`. |
| `tests/golden_real.rs` | The golden scenario against the real Intellect and Eve adapters. Compiled only with `real-adapters`. Two tests are `#[ignore]`d, each with the reason in its attribute. |
| `benches/pipeline.rs` | The benchmark harness (spec section 96). Timings are informational. Counted metrics are deterministic. |

Shared helpers live in `tests/common/`. `common/scenarios.rs` holds the scenarios that the
benchmark harness and the smoke test both use.

## What this crate deliberately does not do

- It does not define test-only types that production code depends on. Anything a suite needs from
  the library is public API of the owning crate.
- It does not gate on timings. Wall-clock results vary by machine, so the benchmark's pass or fail
  checks use counted metrics only.

## Related crates

The suites depend on these crates as dev-dependencies, so they are checked together here:
`tpt-thought-core`, `-state`, `-provenance`, `-attention`, `-graph`, `-hypothesis`, `-prediction`,
`-providers`, `-deliberation`, `-workspace`, `-safety`, `-loop`, `-intention` and `-observatory`.
With `real-adapters`, `tpt-thought-integrations` is added.

## Licence

Licensed under either of MIT or Apache-2.0, at your option.

## Minimum supported Rust version

1.85.
