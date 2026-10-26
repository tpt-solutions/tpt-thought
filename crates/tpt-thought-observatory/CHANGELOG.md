# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `EventKind` and `TelemetryEvent`: telemetry events with stable names, a timestamp and optional
  thought, state and value fields.
- `Metric`, `Summary` and `Metrics`: cognitive metrics with deterministic aggregation, rates and
  occurrence recording.
- `TelemetrySink` trait, with `NoopSink` and a bounded `RecordingSink` that counts dropped events.
- `Observatory<S>`: wraps a sink, counts events by kind, and owns the metrics and cycle timeline.
- `ObservatorySnapshot` and its view types (`ActiveThought`, `GraphSnapshot`, `HypothesisView`,
  `ProviderView`, `CostView`), assembled from caller-supplied state.
- `CycleTimeline`, `TimelineEntry` and `SystemMode` for recording time spent in each system mode.
