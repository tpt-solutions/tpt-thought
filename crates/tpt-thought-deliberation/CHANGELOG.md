# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The crate uses semantic versioning.
Repository-wide changes are recorded in the root `CHANGELOG.md`.

## [Unreleased]

### Added

- `DeliberationEngine` trait, with one defaulted method per capability. Each default reports
  `DeliberationError::Unsupported`, so engines implement only their subset (`Capability`).
- Request and result types for each capability: reasoning, hypothesis generation, contradiction
  analysis, causal analysis, abstraction validation, experiment design and active learning
  (`Reasoning`, `HypothesisRequest`, `ContradictionAnalysis`, `CausalAnalysis`,
  `AbstractionVerdict`, `ExperimentPlan`, `QueryCandidate` and related types).
- Metacognition with `NextAction` (`Metacognition`, `NextAction`).
- Bounded deep-investigation loop with `run_investigation`, `InvestigationPolicy`,
  `InvestigationRun` and `Termination`.
- Read-only engine input: `ThoughtSnapshot` and `DeliberationContext`.
- `mock::ScriptedEngine` for tests.
