# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `AttentionFactors` with ten scoring inputs, and the `AttentionFormula` trait.
- `WeightedSum`, the default formula, with configurable per-factor weights.
- `apply_attention`, which records a score, salience and novelty on a thought.
- `CognitiveDemand` and `demand`, describing the priority, deadline, cost and required
  capabilities a thought asks of the scheduler.
- `WorkingMemory` with a strict capacity, half-life decay, reinforcement, removal and expiry.
- `WorkingMemoryConfig`, `WorkingMemoryItem`, `InsertOutcome` and `ForgettingState`.
