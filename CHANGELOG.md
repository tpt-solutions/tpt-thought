# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

## [0.1.1] - 2026-10-27

### Added

- Eve `investigate`: one bounded step over Eve's metacognition, so the real-adapter golden scenario
  runs the Eve stage.
- Replay verifies provenance: the hash chain is checked before a re-run, and each step's provenance
  is compared field by field. Records without provenance still replay on their trajectory.
- CK admission: `ResourceGate` and `CognitiveLoop::run_gated`. A denied cycle returns
  `CycleOutcome::Deferred` and runs nothing. `AnimaBudget` implements the gate.
- Resource strain raises arousal only, through `Modulation::under_pressure`.
- Consolidation persistence to SSE: `AnimaConsolidation` and `CognitiveLoop::persist_consolidation`,
  content-addressed and idempotent. Thought keeps references only.
- `AnimaActionSink`: actions are executed through an INP intention round, and the loop never sees INP.
- Multi-agent thought sharing: `providers::sharing` (pure merge rule) and `share` (INP transport).
- ZKPA attestation evidence, structural check into provenance, and Groth16 authorization proofs.
- The real-adapter golden scenario runs the full slow path (Intellect, Eve, Augur, re-evaluation).
  Acceptance criteria F (Eve deliberation) and H (intention executed) now have tests.

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

- `tpt-thought-integrations`: adapters to sibling repositories, each behind its own feature and
  pinned to a revision: `intellect` (choice providers over the reference backend), `remote`
  (loopback and caller-supplied transports), `local` (`tpt-infer` classifier), `eve`
  (`DeliberationEngine`), `anima` (memory, budget and provider discovery), `augur` (posteriors with
  residual uncertainty) and `axiom` (probabilities, confidence and fuzzy uncertainty).
- Real-adapter acceptance tests in `tpt-thought-tests` (feature `real-adapters`), and a golden
  scenario against the real Intellect and Eve adapters. Two of its properties are ignored; see
  `docs/INTEGRATION_EVE.md`.
- Integration documents for Augur and Axiom, and `docs/DEPENDENCY_POLICY.md` now covers the
  git-source policy for sibling repositories.
- `tpt-thought-cli`: the `thought` command, with `demo`, `run`, `replay`, `validate`, `why` and
  `schema`. Records and replays runs as JSON and explains a cycle's provenance. See `docs/CLI.md`.
- Examples in `tpt-thought`: `quickstart`, `hypotheses`, `reflection_and_sleep`,
  `custom_provider`, and `augur_evidence` (behind the `augur` feature).
- Documentation: `docs/QUICKSTART.md`, `docs/CLI.md` and `docs/COOKBOOK.md`, and a one-cycle
  sequence diagram in `docs/ARCHITECTURE.md`.
- `tpt-thought-loop`: `LoopBuilder` (`CognitiveLoop::builder()`), public `NullEngine`,
  `NullEstimator` and `NullActionSink`, `with_telemetry`, and `sleep()` with `consolidator()`.
- Escalation policy in the loop: a `PolicySet` decides, after a rejected answer, whether to stop,
  accept or escalate to a level. The level selects the slow stages. The default escalates to L5.
- `tpt-thought` facade: a `prelude` module, feature forwarding to `tpt-thought-integrations`, and
  the `integrations` re-export.
- `scripts/check.sh`, `scripts/check.ps1`, `scripts/bench.sh` and `scripts/bench.ps1`.
- Issue templates (bug, feature request, integration gap). `CONTRIBUTING.md` accepts issues only.
- `tpt-thought-integrations` (`augur` feature): `AugurSession` for evidence updates and
  `uncertainty_of`, which maps a residual to Thought's `Uncertainty`. Pinned to `tpt-augur`
  `c29ecd17`.

### Changed

- `tpt-eve` causal queries: interventions and counterfactuals map onto `tpt-eve-causal`.
- Eve adapter: hypothesis regeneration reuses ids, abstraction reports `Conditional` for partial
  support, and an experiment with a cost cap is refused because Eve cannot check cost.
- Anima budget adapter: refuses non-finite and negative costs, and releases only admitted flops.
- Anima memory adapter: resolves references only from its own store, in canonical hex form.
- Intellect adapter: refuses an empty candidate list as a configuration error.
- `tpt-thought-loop`: wires memory retrieval, hypothesis branching, prediction tracking and
  provenance into the cycle. The acceptance tests for criteria B, C, G and K check those effects
  through the loop.
- `deny.toml`: allows the sibling repository URLs, including the lowercase form used for
  `tpt-axiom`, and the `tpt-infer` source.
- `tpt-axiom` template: flattened so cargo can scan the repository for git dependencies.

### Not yet included

- Eve `reason`: `tpt-eve` has no conclusion-producing entry point. Eve query ranking has no
  tokio-free API.
- Anima: a global workspace, `retrieve_context` (covered by `recall_similar`), and ZKPA ownership
  states and the agency ledger, which the pinned sibling does not provide.
- Axiom: `Proven` status, which needs ZK proof verification.
- Ollama and `tpt-spark` local providers. Neither is in the sibling survey.
- Observatory UI, which belongs to Anima's Observatory.
- Multi-agent sharing: the replies travel over INP through the caller's `PeerLink`. The
  consolidator still holds its in-process working copy until the next abstraction step.
- Distribution to crates.io. Releases are git-only; see `docs/DEPENDENCY_POLICY.md`.
