# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- Facade re-exporting the Thought crates as modules: `core`, `state`, `provenance`, `attention`,
  `graph`, `hypothesis`, `prediction`, `providers`, `deliberation`, `policy`, `workspace`,
  `safety`, `reflection`, `consolidation`, `observatory`, `intention`, `cognitive_loop` and
  `integrations`.
- `prelude` module with the types most programs need.
- Features `intellect`, `eve`, `anima`, `augur`, `axiom`, `remote` and `local`. Each enables the
  matching adapter in `tpt-thought-integrations`. None is enabled by default.
