# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `CognitiveLoop` and the cycle: observe, contextualise, associate, hypothesise, evaluate, decide,
  intend, act, observe the result, learn and consolidate, reported through `CycleReport` and
  `CycleOutcome`.
- `TrajectoryStep` and `CyclePhase`: a per-phase trajectory that can be inspected after a run.
- `CognitiveMode` (nine modes) and `Modulation`, a validated set of signals selecting the mode.
- The slow path in the fixed order Intellect, Eve, Augur, Intellect, with `SlowStage`, `Estimate`
  and the `Estimator` trait.
- `CuriosityDriver` and `CuriosityEvent`, with validated thresholds.
- `Plan` and `CandidatePlan`, with validated candidate plans and best-plan selection.
- `ReplayRecord`, `ReplayMetadata` and `replay`: capture a run and re-run it to check that it
  reproduces step for step.
- `LoopConfig` with a seed, budget, circuit breaker configuration, investigation policy and
  escalation ladder.
- `LoopBuilder`, which assembles a loop from its parts.
- `Action`, `ActionSink` and `ActionError`, the boundary through which intentions leave the loop.
- `NullEngine`, `NullEstimator` and `NullActionSink` for parts a caller does not need.
- `closed_loop` example, which runs the cycle and replays it.

### Changed

- The cycle wires memory retrieval, hypothesis branching, prediction tracking and provenance
  into each run. The acceptance tests for criteria B, C, G and K check those effects through the
  loop.
