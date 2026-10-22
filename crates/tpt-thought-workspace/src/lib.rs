// SPDX-License-Identifier: MIT OR Apache-2.0
//! Global workspace, memory references, episodic records and the thought bubble (spec sections
//! 10, 12, 14, 42, 48.1 and 48.2).
//!
//! # Crate boundary decision
//!
//! The shared global workspace is an Anima concern (spec section 48.1): many agents and modules
//! publish to one workspace, and that state must outlive any single thought. It is therefore
//! implemented in the Anima repository, not here, and no separate `tpt-anima-workspace` crate is
//! created on the Thought side. Anima supplies its implementation of [`GlobalWorkspace`].
//!
//! This crate holds the Thought side only:
//!
//! - [`GlobalWorkspace`] and [`InProcessWorkspace`]: the trait Thought codes against, and a
//!   reference in-process implementation for tests and standalone operation.
//! - [`MemoryStore`] and [`MemoryIndex`]. Durable memory stays in Anima (SSE). Thought keeps only
//!   memory identifiers and retrieval cues, never memory content.
//! - Episodic record types ([`Episode`] and friends), which Thought produces and Anima persists.
//! - [`ThoughtBubble`], the sandbox for experimental cognition. It is the only route by which
//!   temporary cognition reaches durable state, and it does so through an explicit, policy-gated
//!   commit.
//!
//! Thought depends on no Anima crate. Anima-backed implementations belong in
//! `tpt-thought-integrations`.

mod bubble;
mod episode;
mod memory;
mod workspace;

pub use bubble::{
    BubbleError, BubbleKey, BubbleKind, CommitPolicy, CommitReport, DurableStore, ThoughtBubble,
};
pub use episode::{
    ActionRecord, Episode, EpisodeError, EpisodeEvent, EpisodeOutcome, PredictionRecord,
};
pub use memory::{MemoryIndex, MemoryStore};
pub use workspace::{GlobalWorkspace, InProcessWorkspace, Item, ItemId, ItemKind, WorkspaceError};
