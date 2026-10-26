# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `ThoughtEvent` and the transition table `TRANSITIONS`, with legal loops for re-hypothesising,
  re-investigating and reconsidering after an outcome.
- `next`, `can_transition`, `permitted_events` and `kind` for querying the table.
- `apply`, which updates a thought's state and `updated_at` and leaves the thought unchanged on
  error.
- `ThoughtLifecycle`, a trait with one method per lifecycle step.
- `IllegalTransition`, reporting the rejected state and event.
