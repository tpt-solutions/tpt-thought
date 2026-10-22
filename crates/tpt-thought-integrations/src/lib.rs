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
//!
//! # Not implemented (blocked)
//!
//! - **Axiom** (Phase 10 `AxiomAdapter`): `tpt-axiom` cannot be resolved as a git dependency.
//!   Its repository contains `crates/tpt-axiom-template/{{project-name}}/Cargo.toml`, whose
//!   package name is invalid, and cargo fails while reading the repository. The dependency is
//!   left out rather than patching the sibling.
//! - **Pulse** (Phase 10 `PulseAdapter`): `tpt-pulse` has no event stream. Only synchronous
//!   session-typed channels exist, and the signal and backend crates are scaffolds.
//! - **Anima gaps**: `tpt-anima` has no `retrieve_context`, no episode persistence, no global
//!   workspace and no ZKPA identity type. Those items are not invented here.
//! - **Eve gaps**: `tpt-eve` has no deliberation entry point. `tpt-eve-neural` pulls in tokio, so
//!   it is not used. Contradiction analysis and query ranking report `Unsupported`.
//! - **Augur gaps**: there is no evidence-update or residual-uncertainty API. Residual
//!   uncertainty is derived from the posterior summaries only.

#[cfg(feature = "anima")]
pub mod anima;
#[cfg(feature = "augur")]
pub mod augur;
#[cfg(feature = "eve")]
pub mod eve;
#[cfg(feature = "intellect")]
pub mod intellect;
