# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The crate uses semantic versioning.
Repository-wide changes are recorded in the root `CHANGELOG.md`.

## [Unreleased]

### Added

- Typed, directed edges between thoughts, hypotheses, observations, predictions, beliefs, actions
  and memories, with association and causation kept distinct (`EdgeKind`, `Edge`, `NodeRef`).
- Causal edges that require `CausalProvenance`, and non-causal edges that forbid it.
- Graph invariant checks on insertion, reported as `GraphError`.
- Queries: ancestors, descendants, contradictions, evidence paths and causal provenance lookup.
- Bounded recursive spawning with `ThoughtTree`, `SpawnLimits` and `SpawnError`, limiting depth,
  children, total compute and duration.
