# tpt-thought-prediction

Predictions, the measured error when an observation resolves one, and the response ladder that
maps an error's size to the actions the cognitive system takes, plus a fan-out hook for the
subsystems that learn from it.

> **Status:** pre-release (v0.1.0). This crate is distributed from git only and is not published
> to crates.io. See `docs/DEPENDENCY_POLICY.md` in the repository.

## What it does

- **Tracks** open predictions (`core::Prediction`) until an observation resolves them, or until
  their expected time passes with a grace period.
- **Compares** a prediction with an observation (`compare`), producing a `PredictionError` with a
  content error, a timing error and a magnitude in `0.0..=1.0`, where `0.0` is a perfect match.
- **Classifies** the magnitude into an `ErrorLevel` (`Small`, `Moderate`, `Large`, `Catastrophic`)
  and plans the responses (`ResponsePlan`) with a configurable `ResponseLadder`.
- **Fans out** the error to learning subsystems through the `ErrorFanOut` trait, via `dispatch`.

Errors are events, not exceptions. A resolved prediction is reported whether or not it was wrong,
and an expired prediction is reported as `PredictionEvent::Expired`.

## What it deliberately does not do

- It does not decide which observation answers which prediction. That is the caller's call, since
  relevance belongs to attention and memory.
- It does not own memory, learning, belief, attention or planning. `ErrorFanOut` is a trait, so
  this crate depends on none of them, and every hook defaults to doing nothing.
- It does not compare observations that carry no structured payload. Those return
  `NotComparable`.

## Main types

| Item | Role |
|---|---|
| `PredictionTracker` | Open predictions. `new(MatchConfig)`, `track`, `resolve`, `expire`, `open_count`. |
| `PredictionEvent` | `Resolved(PredictionError)` or `Expired { .. }`. |
| `PredictionError` | Prediction and observation IDs, content and timing error, timing offset, and `magnitude()`. |
| `MatchConfig` | Tunables for matching. `timing_scale_nanos` sets the offset that counts as a full timing error (default 1 second). |
| `compare` and `value_error` | Compare a `Prediction` with an `Observation`, or two `StructuredValue`s. |
| `ResponseLadder` | Thresholds for moderate, large and catastrophic errors. `new`, `classify`, `plan`. |
| `ErrorLevel` | `Small`, `Moderate`, `Large`, `Catastrophic`. |
| `ResponseAction` | `Continue`, `UpdateBelief`, `RaiseAttention`, `Investigate`, `SuspendModel`, `ReevaluateAssumptions`. |
| `ResponsePlan` | The level reached and the actions it calls for. |
| `ErrorFanOut` and `dispatch` | Hooks for memory, active learning, belief, confidence, attention and planning. |

## Module layout

- `error.rs`: matching, `MatchConfig`, `PredictionError`, `compare` and `value_error`.
- `tracker.rs`: open predictions, `PredictionEvent`, and the track, resolve and expire operations.
- `ladder.rs`: `ErrorLevel`, `ResponseAction`, `ResponseLadder` and `ResponsePlan`.
- `fanout.rs`: the `ErrorFanOut` trait and `dispatch`.

## Usage

```rust
use tpt_thought_prediction::{MatchConfig, PredictionTracker, ResponseLadder};

let ladder = ResponseLadder::new(0.2, 0.5, 0.8)?;
let mut tracker = PredictionTracker::new(MatchConfig::default());

let id = prediction.id;
tracker.track(prediction, predicted_at)?;
let event = tracker.resolve(id, &observation)?;

if let tpt_thought_prediction::PredictionEvent::Resolved(error) = event {
    let plan = ladder.plan(&error);
    // Pass `plan` and `error` to `dispatch` with your own `ErrorFanOut` implementation.
}
```

`MatchConfig::default()` sets a timing scale of one second.

## Design notes

- The ladder thresholds must satisfy `0 <= moderate <= large <= catastrophic <= 1`, and
  `ResponseLadder::new` returns `LadderError` otherwise.
- `ResponseLadder::classify` is `Small` below `moderate`, rising through each threshold, and
  `Catastrophic` at or above `catastrophic`.
- `dispatch` always lets memory and confidence learn from an error. The plan then decides whether
  beliefs are revised, attention is raised and active learning queued, or planning is revised.
- `PredictionTracker::resolve` refuses an observation made before the prediction's `predicted_at`
  with `ResolveError::Premature`.
- `PredictionTracker::expire` only closes predictions that have an expected time, and only once
  that time plus the grace period is past.

## Related crates

- `tpt-thought-core`: `Prediction`, `Observation` and `StructuredValue`.
- `tpt-thought-hypothesis`: hypotheses that predictions are derived from.
- `tpt-thought-loop`: wires prediction tracking into the cognitive cycle.
- `tpt-thought-providers`: may supply the predictions themselves.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0, at your option.
