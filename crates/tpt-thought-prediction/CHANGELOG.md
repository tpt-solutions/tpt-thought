# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The crate uses semantic versioning.
Repository-wide changes are recorded in the root `CHANGELOG.md`.

## [Unreleased]

### Added

- Prediction tracking until an observation resolves the prediction or its expected time passes
  (`PredictionTracker`, `PredictionEvent`, `TrackError`, `ResolveError`).
- Matching of predictions against observations, producing a `PredictionError` with content and
  timing error (`compare`, `value_error`, `MatchConfig`, `NotComparable`).
- The configurable response ladder from `Small` to `Catastrophic`, with `ResponseAction` and
  `ResponsePlan` (`ResponseLadder`, `ErrorLevel`, `LadderError`).
- Fan-out of prediction errors to learning subsystems through the `ErrorFanOut` trait, with
  `dispatch`. Every hook defaults to doing nothing.
