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
- Integration documents for Augur and Axiom in `docs/`.

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

- Eve contradiction analysis: `tpt-eve-symbolic` pulls in `option-ext` (MPL-2.0) through `dirs`.
- Eve query ranking, and any Eve deliberation entry point (`reason`, `investigate`).
- Pulse adapter: `tpt-pulse` has no event stream.
- Anima: `retrieve_context`, episode persistence, a global workspace and a ZKPA identity type.
- Augur evidence updates and a residual-uncertainty API.
- Axiom formal constraints and ZK verification markers.
- Ollama and `tpt-spark` adapters.
