# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- Adapters to sibling repositories, each behind its own feature and pinned to a revision:
  `intellect` (choice providers over the reference backend), `remote` (loopback and
  caller-supplied transports), `local` (`tpt-infer` classifier), `eve` (`DeliberationEngine`),
  `anima` (memory, budget and provider discovery), `augur` (posteriors with residual uncertainty)
  and `axiom` (probabilities, confidence and fuzzy uncertainty).
- `anima` feature: `AnimaBudget` implements `SleepScheduler`, granting sleep only while the thermal
  throttle factor is above 0.75 and flops remain.
- `anima` feature: `AnimaMemory::recall_similar` ranks memories by cosine similarity through
  SSE's approximate nearest-neighbour index.
- `anima` feature: `AnimaBroadcast::negotiate` runs an INP intention round, in which registered
  providers bid and INP selects the winner. Offers from unregistered providers are ignored.
- `anima` feature: `AnimaBroadcast::open_intention` runs the bid round through INP's
  `NegotiationSession`, and `AnimaIntention` records the provider's outcome. Completed and partial
  outcomes complete the intention. Failed, timed-out and cancelled outcomes end it in their own
  states. An intention accepts one outcome.
- `anima` feature: `AnimaBudget::resource_snapshot` reports CK's compute and energy state for the
  sleep scheduler, with memory pressure supplied by the caller.
- `anima` feature: `AnimaBubbles` opens, quarantines and tears down CK thought bubbles as execution
  boundaries.
- `anima` feature: `AnimaEpisodes` records Thought episodes in SSE's episode store as JSON under
  the kind `tpt-thought.episode.v1`, and loads them back in recording order.
- `eve` feature: `EveAdapter::analyse_contradictions` runs `tpt-eve-symbolic`'s consistency checker
  over claims in `subject predicate object` form. Only `is` and `was` are exclusive. Malformed
  claims are refused.
- `zkpa` feature: `zkpa::AnimaIdentity` opens and closes task-scoped sessions, and delegates
  permissions that a parent holds. `tpt-anima-zkpa` is used with `default-features = false`.
- Sibling pins: `tpt-eve` moved to `db6e8df` (no MPL-2.0 dependency). `tpt-anima` moved to
  `5e9da51`, which adds the episode store and the ZKPA `async` feature.
- `deny.toml` allows the `tpt-solutions/tpt-pulse` git source, which the `pulse` feature uses.
- Integration documents for Augur and Axiom in `docs/`.
- `axiom::verify_comparison`: checks with `tpt-axiom-verify` that a circuit constraint encodes a
  claimed comparison, and returns `EpistemicStatus::Verified` when it does.
- `pulse` feature: `pulse::PulseSink`, a `TelemetrySink` that publishes each observatory event to a
  `tpt-pulse-core` `EventStream`. It is pinned to `tpt-pulse` revision `f36c1f0`.

### Changed

- `tpt-eve` causal queries: interventions and counterfactuals map onto `tpt-eve-causal`.
- Eve adapter: hypothesis regeneration reuses IDs, abstraction reports `Conditional` for partial
  support, and an experiment with a cost cap is refused because Eve cannot check cost.
- Anima budget adapter: refuses non-finite and negative costs, and releases only admitted flops.
- Anima memory adapter: resolves references only from its own store, in canonical hex form.
- Intellect adapter: refuses an empty candidate list as a configuration error.
- `tpt-axiom` is pinned to the commit that moved its cargo-generate template out of the crate
  tree, so cargo can scan the repository for git dependencies.

### Not yet included

- Eve query ranking, and any Eve deliberation entry point (`reason`, `investigate`).
- Anima: `retrieve_context` (covered by `recall_similar`) and a global workspace. ZKPA attestation,
  proofs and the agency ledger.
- Augur evidence updates and a residual-uncertainty API.
- Axiom formal constraints and ZK verification markers.
- Ollama and `tpt-spark` adapters.
