// SPDX-License-Identifier: MIT OR Apache-2.0
//! Persisting durable consolidation output (todo Phase 6; spec sections 10, 14, 28, 87, 88).
//!
//! Thought keeps no copy of what it persists. A [`ConsolidationSink`] writes each durable item to
//! the store that owns it (Anima's SSE in the integrations crate) and returns a reference, and
//! the result of [`persist`] is references only.
//!
//! What is persisted, and why:
//!
//! - **Knowledge** (semantic, procedural, causal model, heuristic, policy and skill): durable
//!   statements that later sessions recall. Each goes to the sink as a memory.
//! - **History summaries** (from `CompressHistory`): these replace the entries they cover, so
//!   they are the only durable record of that history. Each goes to the sink as a memory.
//! - **Episodes** are not written here. Episodes are recorded before consolidation, in the
//!   episode store, and [`crate::Knowledge::sources`] refer to them by identifier. Consolidation
//!   creates no new episodes, so re-storing them would duplicate the record.
//!
//! What is deliberately not persisted:
//!
//! - **Confidence calibration** is a fit over calibration samples, so it is recomputed by each
//!   `Recalibrate` step from the samples the caller records. Storing the fitted bins would make a
//!   second, stale copy of the samples' effect.
//! - **Next-session state** ([`crate::NextSessionState`]) is a view of the consolidator at one
//!   moment. Its knowledge and skills are already persisted above, and its priorities, gaps and
//!   experiments are re-derived on every pass. Persisting the whole view would copy all of them.
//! - **Salience, gaps, experiments and belief statuses** are Thought-owned working state that
//!   each pass recomputes from replayed episodes and recorded evidence.
//! - **Memory relevance and pruning** stay Thought-owned. Pruned references are reported by
//!   [`crate::Consolidator::forgotten`] so the caller can release them from the store.

use crate::{AbstractionKind, Consolidator, HistorySummary, Knowledge};
use tpt_thought_core::ContentReference;

/// Where durable consolidation output is written. Implemented over a memory store by an
/// adapter; the consolidation crate never names a concrete store.
///
/// Each method must be idempotent: storing the same item again returns the reference it got the
/// first time and writes nothing new.
pub trait ConsolidationSink {
    /// Why an item could not be stored.
    type Error;

    /// Store one piece of knowledge and return a reference to it.
    fn store_knowledge(&mut self, knowledge: &Knowledge) -> Result<ContentReference, Self::Error>;

    /// Store one history summary and return a reference to it.
    fn store_summary(&mut self, summary: &HistorySummary) -> Result<ContentReference, Self::Error>;
}

/// Knowledge that has been stored, by kind, with the reference the sink returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedKnowledge {
    /// What kind of knowledge it is.
    pub kind: AbstractionKind,
    /// Where the statement is held.
    pub memory: ContentReference,
}

/// The references to everything [`persist`] stored. It holds no statements or summaries, so
/// Thought does not keep a second copy of them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersistedConsolidation {
    /// Knowledge stored, in the order the consolidator holds it.
    pub knowledge: Vec<PersistedKnowledge>,
    /// History summaries stored, oldest first.
    pub summaries: Vec<ContentReference>,
}

/// Store the durable output of `consolidator` through `sink`.
///
/// Stops at the first item the sink refuses and returns its error. Items stored before that
/// point stay stored, and a later call stores the rest without duplicating them, because the
/// sink is idempotent.
pub fn persist<S: ConsolidationSink>(
    consolidator: &Consolidator,
    sink: &mut S,
) -> Result<PersistedConsolidation, S::Error> {
    let mut out = PersistedConsolidation::default();
    for knowledge in consolidator.knowledge() {
        let memory = sink.store_knowledge(knowledge)?;
        out.knowledge.push(PersistedKnowledge {
            kind: knowledge.kind,
            memory,
        });
    }
    for summary in consolidator.history().summaries() {
        out.summaries.push(sink.store_summary(summary)?);
    }
    Ok(out)
}
