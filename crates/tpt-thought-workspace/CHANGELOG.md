# Changelog

All notable changes to `tpt-thought-workspace` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `GlobalWorkspace` trait, the contract Thought codes against for publication, broadcast,
  requests, responses, attention, inboxes and withdrawal. Anima's shared workspace implements it.
- `InProcessWorkspace`, a deterministic reference implementation for tests and standalone use.
- `MemoryStore` trait and `MemoryIndex`: memory references by identifier and tag cue. No memory
  content is held.
- Episodic records: `Episode`, `EpisodeEvent`, `PredictionRecord`, `ActionRecord`,
  `EpisodeOutcome` and `EpisodeError`, with closure enforced.
- `ThoughtBubble`, the sandbox for experimental cognition. It works on a snapshot and commits only
  the kinds a `CommitPolicy` allows.
- `BubbleKind` with a fixed rule that tools, branches and models are never committable.
- `DurableStore`, `CommitReport` and `BubbleError`.
