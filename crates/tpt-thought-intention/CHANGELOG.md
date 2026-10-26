# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `ManagedIntention`: validated construction of an `Intention`, a lifecycle with transition
  history, and provenance records.
- `InvalidIntention`: rejects blank goals, non-finite priorities and blank constraint or capability
  entries.
- `IntentionStatus` with a permitted-transition table (`EDGES`) that includes the retry loop from
  `Failed` back to `Dispatched`, and `IllegalIntentionTransition` for refused transitions.
- `require_capabilities` and `MissingCapabilities`: check granted capabilities against an
  intention's requirements.
- `IntentionSet`: intentions indexed by ID, with priority ordering and deadline queries
  (`due_before`, `overdue`, `next_deadline`).
