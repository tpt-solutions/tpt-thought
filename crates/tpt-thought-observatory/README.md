# tpt-thought-observatory

Telemetry events, cognitive metrics, telemetry sinks and observatory data snapshots for
TPT Thought.

**Status:** pre-release, v0.1.0. Distributed from git only; not published to crates.io. See the
repository's `docs/DEPENDENCY_POLICY.md`.

## What it does

The observatory is how a running cognitive system reports what it is doing. It provides:

- **Telemetry events** with stable names: `EventKind` (sixteen kinds, from `ThoughtCreated` to
  `ThoughtAborted`, plus `SystemModeChanged`) and `TelemetryEvent`, which carries a timestamp and
  optionally a thought, a lifecycle state and a numeric value.
- **Cognitive metrics** with deterministic aggregation: `Metric` (fifteen metrics such as
  `ThoughtLatency`, `EscalationRate`, `CalibrationError` and `ActionSuccessRate`), `Summary`
  (count, sum, min, max and mean) and `Metrics`, which records values and occurrences and reports
  rates.
- **Telemetry sinks** with no required backend: the `TelemetrySink` trait, `NoopSink`, and
  `RecordingSink`, a bounded in-memory sink that counts what it drops.
- **Observatory** (`Observatory<S>`), which wraps a sink, counts events by kind, owns the metrics
  and the cycle timeline, and assembles snapshots.
- **Snapshot data** (`ObservatorySnapshot`, with `ActiveThought`, `GraphSnapshot`, `HypothesisView`,
  `ProviderView` and `CostView`): serialisable views for an external UI to read.
- **Cycle timeline** (`CycleTimeline`, `TimelineEntry`, `SystemMode`): a record of which system mode
  (`Active`, `Idle`, `Reflection`, `Sleep`, `Consolidation`, `Awake`) the system was in, and for how
  long.

## What it deliberately does not do

- It does not render anything. The Observatory UI is an Anima concern, and this crate supplies the
  data that UI would read.
- It does not require a backend. Export to a real telemetry system is the sink implementor's job.
- It does not reach into thoughts. `Observatory::snapshot` reflects exactly the state the caller
  passes in.

## Usage

```rust
use tpt_thought_core::Timestamp;
use tpt_thought_observatory::{EventKind, Metric, Observatory, RecordingSink, TelemetryEvent};

let mut observatory = Observatory::new(RecordingSink::new());
observatory.emit(&TelemetryEvent::new(EventKind::ThoughtCreated, Timestamp(1)));
observatory.metrics_mut().record(Metric::ThoughtLatency, 12.5);

assert_eq!(observatory.count(EventKind::ThoughtCreated), 1);
assert_eq!(observatory.sink().events().len(), 1);
```

To send events elsewhere, implement `TelemetrySink` for your own type and pass it to
`Observatory::new`.

## Module layout

| Module | Contents |
|---|---|
| `event` | `EventKind`, `TelemetryEvent` |
| `metric` | `Metric`, `Summary`, `Metrics` |
| `sink` | `TelemetrySink`, `NoopSink`, `RecordingSink`, `DEFAULT_RECORDING_CAPACITY` |
| `observatory` | `Observatory<S>` |
| `snapshot` | `ObservatorySnapshot` and its view types |
| `timeline` | `CycleTimeline`, `TimelineEntry`, `SystemMode` |

## Installation

```toml
[dependencies]
tpt-thought-observatory = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## Design notes

- Event and metric names are stable identifiers, so recorded data stays comparable across runs.
- Aggregation is deterministic. The same sequence of recorded values always yields the same
  summaries.
- `RecordingSink` is bounded. Once full, it counts dropped events rather than growing without limit.
- Snapshots and timeline entries derive `Serialize` and `Deserialize`.

## Related crates

- `tpt-thought-core` supplies `Timestamp`, `ThoughtId`, `ThoughtState` and `Probability`.
- `tpt-thought-loop` emits telemetry through an `Observatory` while it runs a cycle.

## Licence

Licensed under either of MIT or Apache-2.0, at your option.

## Minimum supported Rust version

1.85.
