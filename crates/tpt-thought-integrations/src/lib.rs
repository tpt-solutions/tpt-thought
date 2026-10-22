// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapters from TPT Thought to the sibling TPT repositories (spec sections 6, 46, 57, 61).
//!
//! This is the only crate in the workspace that depends on sibling repositories. Each sibling is
//! behind its own Cargo feature and none is enabled by default, so the rest of the workspace
//! never pulls them in. Every enabled feature must keep tokio and other async runtimes out of
//! the dependency tree (`cargo tree -i tokio` must print nothing).
//!
//! | Feature | Sibling (pinned revision) | Adapter | Thought port |
//! |---|---|---|---|
//! | `intellect` | `tpt-intellect` `a502c013` | `intellect::IntellectProvider` | `IntelligenceProvider` |
//! | `eve` | `tpt-eve` `c230d0e8` | `eve::EveAdapter` | `DeliberationEngine` |
//! | `anima` | `tpt-anima` `408c2e00` | `anima::AnimaMemory`, `anima::AnimaBudget`, `anima::AnimaBroadcast` | `MemoryStore`, budget signal, intention broadcast |
//! | `augur` | `tpt-augur` `634f67a5` | `augur::AugurAdapter` | posterior records with residual uncertainty |
//! | `axiom` | `tpt-axiom` `42c495f7` | `axiom::probability`, `axiom::confidence`, `axiom::uncertainty` | `Probability`, `Confidence`, `Uncertainty` (fuzzy variance as epistemic) |
//! | `remote` | `tpt-intellect` `a502c013` (`tpt-intellect-remote`, serde) | `remote::loopback`, `remote::over_transport` | `IntelligenceProvider` |
//! | `local` | `remote` + `tpt-infer` (unpinned in its own manifest; `Cargo.lock` pins it) | `local::LocalClassifier` | `IntelligenceProvider` |
//!
//! `remote` wraps the sibling's `RemoteBackend` in the same `IntellectProvider` as local
//! Intellect backends, so the Thought port is unchanged. Only the sibling's loopback
//! `ProtocolTransport` and caller-supplied transports are used. The `SubprocessTransport` is not
//! wrapped, because it needs a child process. HTTP is not included: the sibling has no HTTP
//! client and Thought does not add one.
//!
//! `local` uses the sibling's `InferClassificationBackend`, which answers classification
//! questions over a numeric feature vector. The caller supplies the text-to-features encoder.
//! `tpt-spark` and Ollama are not in the sibling survey, so no adapter exists for them.
//!
//! # Not implemented (blocked)
//!
//! - **Pulse** (Phase 10 `PulseAdapter`): `tpt-pulse` has no event stream. Only synchronous
//!   session-typed channels exist, and the signal and backend crates are scaffolds.
//! - **Anima gaps**: `tpt-anima` has no `retrieve_context`, no episode persistence, no global
//!   workspace and no ZKPA identity type. Those items are not invented here.
//! - **Eve gaps**: `tpt-eve` has no deliberation entry point. `tpt-eve-neural` pulls in tokio, so
//!   it is not used. Contradiction analysis reports `Unsupported`: `tpt-eve-symbolic` depends on
//!   `dirs`, which pulls in `option-ext` (MPL-2.0), outside the permissive licence policy. Query
//!   ranking reports `Unsupported` too. Interventions and counterfactuals map to `tpt-eve-causal`.
//! - **Augur gaps**: there is no evidence-update or residual-uncertainty API. Residual
//!   uncertainty is derived from the posterior summaries only.

#[cfg(feature = "anima")]
pub mod anima;
#[cfg(feature = "augur")]
pub mod augur;
#[cfg(feature = "axiom")]
pub mod axiom;
#[cfg(feature = "eve")]
pub mod eve;
#[cfg(feature = "intellect")]
pub mod intellect;
#[cfg(feature = "local")]
pub mod local;
#[cfg(feature = "remote")]
pub mod remote;
