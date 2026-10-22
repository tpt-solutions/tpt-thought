# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `tpt-thought-core`: IDs, thoughts, observations, hypotheses, beliefs, predictions, intentions,
  outcomes, confidence, uncertainty, time (injectable clock) and a versioned wire format. Source
  trust policy (`SourcePolicy`) classifies inputs as trusted or untrusted by default.
- `tpt-thought-state`: lifecycle state machine with legal loops and deterministic transitions.
- `tpt-thought-provenance`: hash-linked provenance records and "why" queries.
- `tpt-thought-attention`: weighted and pluggable attention scoring, cognitive demand output and
  working memory with forgetting states.
- `tpt-thought-graph`: typed edges (association and causation kept distinct), graph queries and
  bounded recursive spawning.
- `tpt-thought-hypothesis`: hypothesis branches, collapse strategies, contradictions, information
  gain and an evidence-gated belief promotion and demotion pipeline.
- `tpt-thought-prediction`: prediction matching, prediction errors, the response ladder and
  fan-out hooks.
- `tpt-thought-providers`: provider-neutral intelligence traits, registry, calibration tracking,
  fast and deliberative modes, ensemble disagreement and a human provider.
- `tpt-thought-deliberation`: `DeliberationEngine` trait, metacognition types and a bounded
  investigation loop.
- `tpt-thought-policy`: escalation levels L0 to L6, expected value of cognition, resource budgets
  and termination policies.
- `tpt-thought-workspace`: in-process global workspace, memory references by ID, episodic records
  and the thought bubble sandbox.
- `tpt-thought-safety`: circuit breakers, capability sandbox, resource limits, loop detection and
  provenance integrity checks. Threat model in `docs/THREAT_MODEL.md`.
- `tpt-thought-reflection`: bounded reflection sessions with structured review artifacts.
- `tpt-thought-consolidation`: interruptible sleep cycle, consolidation plan, episode replay and
  skill formation.
- `tpt-thought-observatory`: telemetry events, metrics, telemetry sinks and snapshot data.
- `tpt-thought-intention`: intentions with validated construction, a lifecycle and set queries.
- `tpt-thought-loop`: the closed cognitive cycle, modes, modulation, planning, curiosity, the slow
  path and replay.
- `tpt-thought-tests`: acceptance, adversarial, replay and resource tests, plus benchmarks.
- `tpt-thought` facade re-exporting the crates above.
- CI: build, test, clippy, fmt, documentation, MSRV, cargo-deny and cargo-audit.

### Not yet included

- `tpt-thought-integrations`: adapters to Intellect, Eve, Anima, Augur and Axiom. On hold pending
  a decision about depending on sibling-repository code. Pulse has no event stream to adapt.
- Observatory UI, which belongs to Anima's Observatory.
- The "same scenario against real sibling adapters" golden demonstration.
- Distribution to crates.io. Releases are git-only; see `docs/DEPENDENCY_POLICY.md`.
