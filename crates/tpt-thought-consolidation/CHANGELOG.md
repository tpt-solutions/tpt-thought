# Changelog

All notable changes to `tpt-thought-consolidation` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `SleepCycle` with `SleepBudget` (steps, wall time and wake priority), `Interrupt` handling at
  step boundaries, and `SleepReport` (`Completed`, `Exhausted` or `Interrupted`).
- `SleepScheduler` trait, `SleepRequest`, `ResourceSnapshot`, `SleepGrant` and a default
  `ThresholdScheduler`. Anima CK is the intended production scheduler.
- `SystemPhase`, `PhaseEvent` and `IllegalPhaseTransition`, with sleep reachable from idle.
- `ConsolidationPlan` and `ConsolidationStep`: eleven ordered steps, with `standard()` and
  `from_steps()`.
- `Consolidator` and `ConsolidationConfig`, applying one step at a time to replay, memory,
  associations, patterns, beliefs, gaps, calibration, history and salience.
- `ThoughtReplay` and `Episode`: replay in importance order, with reinforcement and gap recording.
- Pattern detection and abstraction (`detect`, `abstract_knowledge`) producing `Knowledge`,
  `Skill`, `SkillRoute`, `LearnedPolicy`, `RepeatedMistake` and `StrategyReport`.
- `BeliefRecord`, `BeliefConfig` and `BeliefStatus`: evidence-gated re-evaluation between
  candidate, believed and demoted.
- `Calibration` and `CalibrationSample`: a ten-bin reliability fit with `adjust` and
  `expected_error`.
- `AssociationStore`, `Link` and `ReweightPolicy`: support and reweighting of links, with causal
  links requiring provenance.
- `GapTracker`, `KnowledgeGap` and `ExperimentProposal`: recurring open questions become proposed
  experiments.
- `ThoughtHistory`, `HistoryEntry` and `HistorySummary`: compression of older history.
- `MemoryItem`, `MemoryReorganisation`, `MergeRecord` and `Durability`.
- `SalienceTable` and `NextSessionState`: priorities carried into the next session.
