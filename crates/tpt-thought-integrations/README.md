# tpt-thought-integrations

Adapters from TPT Thought to the sibling TPT repositories: Intellect, Eve, Anima, Augur and Axiom.

**Status:** pre-release, v0.1.0. Distributed from git only; not published to crates.io. Each
sibling is pinned to a git revision, and the pins are listed below.

This is the only crate in the workspace that depends on sibling repositories. Every sibling sits
behind its own Cargo feature, and **none is enabled by default**. Building the rest of the
workspace never fetches a sibling.

## Features

| Feature | Sibling (pinned revision) | Adapter | Thought port |
|---|---|---|---|
| `intellect` | `tpt-intellect` `a502c0139a7e5cc12b7adf8fb9bdbbae31f69ffc` | `intellect::IntellectProvider`, `intellect::reference_provider` | `IntelligenceProvider` |
| `remote` | `tpt-intellect` `a502c013` (`tpt-intellect-remote`, `serde` feature) | `remote::loopback`, `remote::over_transport` | `IntelligenceProvider` |
| `local` | `remote`, plus `tpt-infer` (not pinned in its own manifest; `Cargo.lock` pins it) | `local::LocalClassifier` | `IntelligenceProvider` |
| `eve` | `tpt-eve` `db6e8df72344dc820bcf5fac5b9aaff8e438ef48` | `eve::EveAdapter` | `DeliberationEngine` |
| `anima` | `tpt-anima` `5e9da51b4e13b79204bcfeeabaf915cb224d7bdf` | `anima::AnimaMemory`, `anima::AnimaBudget`, `anima::AnimaBroadcast` | `MemoryStore` (tag and similarity recall), a budget signal, a resource snapshot, intention negotiation and `SleepScheduler` |
| `zkpa` | `tpt-anima` `5e9da51b4e13b79204bcfeeabaf915cb224d7bdf` (`tpt-anima-zkpa`, `default-features = false`) | `zkpa::AnimaIdentity` | Task-scoped sessions and permission delegation (§40) |
| `augur` | `tpt-augur` `c29ecd17` | `augur::AugurAdapter` | Posterior records with residual uncertainty |
| `axiom` | `tpt-axiom` `42c495f7751db5acc3c571bb890da41a153f75ba` | `axiom::probability`, `axiom::confidence`, `axiom::uncertainty` | `Probability`, `Confidence`, `Uncertainty` |

Feature relationships: `remote` enables `intellect`, and `local` enables `remote`. The `anima`
feature also enables the `tpt-thought-consolidation` dependency, since `AnimaBudget` implements its
`SleepScheduler`.

## What each adapter does

- **Intellect.** `IntellectProvider` wraps any Intellect backend over string candidates, so the
  Thought provider port is unchanged. `reference_provider` builds one on Intellect's reference
  backend. An empty candidate list is refused as a configuration error.
- **Remote.** `remote::loopback` uses the sibling's in-process `ProtocolTransport`, which encodes
  every request and response on the wire format. `remote::over_transport` accepts any transport the
  caller supplies. Thought performs no HTTP and spawns no processes. The sibling's
  `SubprocessTransport` is not wrapped, and no HTTP transport is included, because the sibling has
  no HTTP client.
- **Local.** `local::LocalClassifier` answers through Intellect's `infer` bridge to `tpt-infer`. The
  bridge takes numeric features, so the caller supplies an `Encoder` from question text to
  features. It answers classification questions only, with labels fixed when the graph is built.
- **Eve.** `eve::EveAdapter` implements `DeliberationEngine`. Facts, observations and causal
  relations are supplied through `assert_fact`, `observe`, `relate`, `fit` and `add_example` before
  deliberation. Causal queries map onto `tpt-eve-causal`. Contradiction analysis runs over
  `tpt-eve-symbolic`'s consistency checker. Reasoning, investigation and query ranking report
  `Unsupported`.
- **Anima.** `AnimaMemory` implements the workspace `MemoryStore` over Anima's SSE store. It holds
  IDs, tags and the caller's embeddings. Tag ranking is deterministic, and `recall_similar` ranks by
  cosine similarity through SSE's nearest-neighbour index. `AnimaBudget` admits and releases compute
  against a flops budget, reports a resource snapshot for sleep, and implements `SleepScheduler`:
  sleep is granted only while the thermal throttle factor is strictly above
  `SLEEP_MIN_THROTTLE_FACTOR` (0.75) and flops remain. `AnimaBroadcast::negotiate` runs an INP
  intention round: registered providers bid and INP selects the winner.
- **Augur.** `AugurAdapter::infer` parses, lowers and runs an Augur model. It returns one
  `PosteriorRecord` per parameter, with an interval width. Errors are reported as `AugurError`.
- **Axiom.** Conversions from Axiom's probability, confidence and fuzzy-uncertainty types into
  Thought's `Probability`, `Confidence` and `Uncertainty`. Fuzzy variance is treated as epistemic
  uncertainty.

## Not implemented

These are recorded as blocked rather than invented:

- **Anima gaps.** `tpt-anima` has no global workspace. `retrieve_context` is not provided, but
  `recall_similar` covers the same role. ZKPA attestation, proofs and the agency ledger are not
  mapped.
- **Eve gaps.** `tpt-eve` has no deliberation entry point. `tpt-eve-neural` pulls in tokio, so it is
  not used.
- **Augur gaps.** There is no evidence-update or residual-uncertainty API. Residual uncertainty is
  derived from the posterior summaries only.
- **Ollama and `tpt-spark`.** Neither appears in the sibling survey, so no adapter exists.

## Constraints every feature must keep

Libraries in this workspace are runtime-agnostic. Every enabled feature must keep tokio and other
async runtimes out of the dependency tree. Check with:

```sh
cargo tree -i tokio
```

The command must print nothing.

## Installation

```toml
[dependencies]
tpt-thought-integrations = { git = "https://github.com/TPT-Solutions/tpt-thought", features = ["intellect", "eve"] }
```

Enable only the features you need. Each one fetches its sibling from git at the pinned revision.

## Documentation

Each adapter has a document in the repository's `docs/` directory:

- `docs/INTEGRATION_INTELLECT.md`, covering the `intellect`, `remote` and `local` features.
- `docs/INTEGRATION_EVE.md`
- `docs/INTEGRATION_ANIMA.md`
- `docs/INTEGRATION_AUGUR.md`
- `docs/INTEGRATION_AXIOM.md`

`docs/DEPENDENCY_POLICY.md` covers the git-source policy for sibling repositories.

## Related crates

- `tpt-thought-core`, `tpt-thought-providers`, `tpt-thought-deliberation` and
  `tpt-thought-workspace`: the Thought ports each adapter implements.
- `tpt-thought-consolidation`: the `SleepScheduler` port, used by the `anima` feature.
- `tpt-thought-loop`: consumes the adapters through those ports. This crate does not depend on the
  loop.

## Licence

Licensed under either of MIT or Apache-2.0, at your option. Sibling repositories carry their own
licences, which the dependency policy checks.

## Minimum supported Rust version

1.85.
