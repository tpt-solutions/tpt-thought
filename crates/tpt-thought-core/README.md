# tpt-thought-core

The vocabulary of TPT Thought: thoughts, identifiers, confidence, uncertainty, time, content and
the versioned wire format. Every other `tpt-thought-*` crate builds on the types defined here.

> **Status:** pre-release (v0.1.1). Distributed from git only, not on crates.io. See
> `docs/DEPENDENCY_POLICY.md` for the release policy.

## Installation

```toml
[dependencies]
tpt-thought-core = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## What this crate does

`tpt-thought-core` defines the nouns of cognition. It answers "what is a thought, and what can it
contain?" It does not decide how a thought evolves. Orchestration lives in `tpt-thought-loop`,
and lifecycle transitions live in `tpt-thought-state`.

Its contents, by module:

| Module | Main types | Purpose |
|---|---|---|
| `id` | `ThoughtId`, `ObservationId`, `HypothesisId`, `PredictionId`, `IntentionId`, `OutcomeId`, `EpisodeId`, `ProvenanceId`, `IdGenerator`, `SeededEntropy`, `OsEntropy` | Opaque 16-byte identifiers and a generator over an entropy source |
| `thought` | `Thought`, `ThoughtOrigin`, `ThoughtContext`, `AttentionState`, `ThoughtProvenance` | The `Thought` object, the unit of cognition (`#[non_exhaustive]`) |
| `content` | `ThoughtContent`, `TextContent`, `StructuredValue`, `MultimodalContent`, `ContentReference`, `Observation`, `Hypothesis`, `Belief`, `Prediction`, `Question`, `Decision`, `Intention`, `Outcome`, `Alternative` | What a thought holds |
| `state` | `ThoughtState` | The seventeen lifecycle states, with `ThoughtState::ALL` and `is_terminal` |
| `confidence` | `Probability`, `Confidence`, `InvalidProbability` | Validated probabilities and confidence records |
| `uncertainty` | `Uncertainty`, `EpistemicStatus` | Epistemic uncertainty, including an explicit "unknown" |
| `time` | `Timestamp`, `Clock`, `SystemClock`, `ManualClock`, `Temporal`, `TemporalEvent` | Injectable time, so tests and replays are deterministic |
| `budget` | `CognitiveBudget` | Limits on cognition, with `CognitiveBudget::UNLIMITED` |
| `trust` | `SourcePolicy`, `SourceTrust` | Trust classification of inputs, untrusted by default |
| `ownership` | `ThoughtOwnership` | Who owns a thought |
| `random` | `RandomnessMetadata`, `TransitionKind` | Record of where non-determinism entered a transition |
| `value` | `ValueAssessment` | Value assessment of a thought |
| `wire` | `Envelope`, `WireError`, `WIRE_VERSION`, `to_json`, `from_json`, `to_cbor`, `from_cbor` | Versioned serialisation to JSON and CBOR |

## Usage

Identifiers, thoughts and state come from the crate root. A seeded generator gives reproducible
identifiers for tests and replays:

```rust
use tpt_thought_core::{IdGenerator, SeededEntropy, TextContent, Thought, ThoughtContent,
    ThoughtOrigin, Timestamp};

let ids = IdGenerator::new(SeededEntropy::new(1));
let thought = Thought::new(
    ids.thought(),
    ThoughtOrigin::Internal,
    ThoughtContent::Text(TextContent { text: "hello".into(), language: None }),
    Timestamp(0),
);
assert_eq!(thought.origin, ThoughtOrigin::Internal);
```

Validated probabilities reject out-of-range values:

```rust
use tpt_thought_core::Probability;

assert!(Probability::new(0.5).is_ok());
assert!(Probability::new(1.5).is_err());
```

Production code uses OS entropy instead:

```rust
use tpt_thought_core::IdGenerator;

let ids = IdGenerator::os(); // requires the default `os-entropy` feature
```

Time is supplied by a `Clock`, never read implicitly, so a replay with the same clock is exact:

```rust
use tpt_thought_core::{Clock, ManualClock, Timestamp};

let clock = ManualClock::new(Timestamp(0));
clock.advance(1_000);
assert_eq!(clock.now(), Timestamp(1_000));
```

## Wire format

`wire` encodes any `Serialize` / `DeserializeOwned` value inside an `Envelope` that records the
schema version (`WIRE_VERSION`, currently `1`). Decoding a payload written under another version
returns `WireError::UnsupportedVersion` rather than guessing. JSON is for people and logs; CBOR is
compact and is what the provenance hashes are computed over.

## Features

| Feature | Default | Effect |
|---|---|---|
| `os-entropy` | yes | Enables `OsEntropy`, `IdGenerator::os()`, and the `getrandom` dependency. Disable it for targets without OS entropy and supply an `EntropySource` yourself. |

## Design notes

- **No TPT or backend dependencies.** The crate depends only on `serde`, `serde_json`,
  `minicbor-serde` and the optional `getrandom`. This is enforced by policy and must stay true.
- **No hidden clocks or randomness.** Time comes from a `Clock`, and identifiers come from an
  `EntropySource`. Both are injectable.
- **`SeededEntropy` is not cryptographic.** It exists for deterministic tests and replay. Use
  `OsEntropy` or `IdGenerator::os()` for identifiers that must be unpredictable.
- **Trust is explicit.** `SourcePolicy` classifies sources, and anything not listed is untrusted.
- **Types are non-exhaustive where they will grow.** Match on `ThoughtState` and the content
  enums with a wildcard arm.

## Related crates

- `tpt-thought-state`: lifecycle transitions over `ThoughtState`.
- `tpt-thought-provenance`: hash-linked records that refer to these identifiers.
- `tpt-thought-attention`, `tpt-thought-graph`, `tpt-thought-hypothesis`, `tpt-thought-prediction`,
  `tpt-thought-intention`: each adds behaviour to the types defined here.
- `tpt-thought`: the facade, which re-exports this crate as `tpt_thought::core`.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
