# Changelog

All notable changes to `tpt-thought-reflection` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `TriggerSignal` and `ReflectionTrigger`: error, uncertainty, contradiction, goal change, outcome
  mismatch and periodic triggers, with a fixed priority order.
- `ReflectionPolicy` and `ReflectionEngine::decide`, which turn signals into a trigger using
  configured thresholds and schedule.
- `ReflectionSession` with `ReflectionLimits` (steps, artifacts and duration), `SessionStatus`,
  `Halt` and `SessionError`. Limits are circuit breakers for reflection.
- `ReflectionReasoner` trait and `ReasonerError`, the seam where Eve supplies reasoning.
- `ReflectionArtifact`: structured records for decisions, evidence, outcomes, predictions,
  prediction errors, confidence, contradictions, causal explanations, knowledge gaps, strategy
  updates, memory updates and belief revisions. No chain-of-thought is stored.
- `Label`, a validated single-line label of at most 160 bytes.
- `ReflectionEngine::run`, which returns a `CognitiveReview` with `Review` counts, metacognition,
  knowledge gap, error and strategy analyses.
- `CognitiveMode` and `ModeRules`, stating what online thought, reflection and sleep may do.
