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
//! | `eve` | `tpt-eve` `db6e8df7` | `eve::EveAdapter` | `DeliberationEngine` |
//! | `anima` | `tpt-anima` `5e9da51b` | `anima::AnimaMemory`, `anima::AnimaBudget`, `anima::AnimaBroadcast` | `MemoryStore` (tag and similarity recall), budget signal, resource snapshot, intention negotiation, `SleepScheduler` (throttle factor above 0.75 and flops remaining) |
//! | `augur` | `tpt-augur` `c29ecd17` | `augur::AugurAdapter`, `augur::AugurSession` | posterior records with residual uncertainty, possible states for discrete parameters, and evidence updates |
//! | `axiom` | `tpt-axiom` `42c495f7` | `axiom::probability`, `axiom::confidence`, `axiom::uncertainty`, `axiom::verify_comparison` | `Probability`, `Confidence`, `Uncertainty` (fuzzy variance as epistemic), `EpistemicStatus::Verified` for checked constraints |
//! | `remote` | `tpt-intellect` `a502c013` (`tpt-intellect-remote`, serde) | `remote::loopback`, `remote::over_transport` | `IntelligenceProvider` |
//! | `local` | `remote` + `tpt-infer` (unpinned in its own manifest; `Cargo.lock` pins it) | `local::LocalClassifier` | `IntelligenceProvider` |
//! | `pulse` | `tpt-pulse` event stream (`tpt-pulse-core` `f36c1f0`) | `pulse::PulseSink` | `TelemetrySink` (observatory events on a broadcast stream) |
//! | `zkpa` | `tpt-anima` `5e9da51b` (`tpt-anima-zkpa`, `default-features = false`) | `zkpa::AnimaIdentity` | task-scoped sessions and permission delegation (§40) |
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
//! - **Anima gaps**: `tpt-anima` has no global workspace. ZKPA attestation, proofs and the agency
//!   ledger are not mapped. `retrieve_context` is not provided; `anima::AnimaMemory::recall_similar` covers
//!   its role. Those items are not invented here.
//! - **Eve gaps**: `tpt-eve` has no deliberation entry point. `tpt-eve-neural` pulls in tokio, so
//!   it is not used. Query ranking reports `Unsupported`. Interventions and counterfactuals map to
//!   `tpt-eve-causal`, and contradiction analysis maps to `tpt-eve-symbolic`.
//! - **Augur**: the pinned revision provides evidence sessions and residual uncertainty, so
//!   `AugurSession` builds with the `augur` feature.

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
#[cfg(feature = "pulse")]
pub mod pulse;
#[cfg(feature = "remote")]
pub mod remote;
#[cfg(feature = "zkpa")]
pub mod zkpa;
