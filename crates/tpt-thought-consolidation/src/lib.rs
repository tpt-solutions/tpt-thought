// SPDX-License-Identifier: MIT OR Apache-2.0
//! Sleep and consolidation (todo Phase 9; spec sections 28, 86, 87, 88, 89).
//!
//! Sleep is bounded and interruptible. Each consolidation step is applied atomically, so an
//! interrupt always leaves the state consistent, and the steps not yet run are returned so the
//! cycle can be resumed later. Anima CK decides when sleep is granted (see [`SleepScheduler`]).
//! Anima owns memory decay; Thought owns relevance, which is what this crate computes.

mod association;
mod belief;
mod consolidator;
mod gaps;
mod history;
mod memory;
mod patterns;
mod persist;
mod phase;
mod plan;
mod replay;
mod sleep;
mod state;

use tpt_thought_core::ProvenanceId;

pub use association::{Association, AssociationError, AssociationStore, Link, ReweightPolicy};
pub use belief::{
    BeliefConfig, BeliefRecord, BeliefStatus, Calibration, CalibrationSample, CALIBRATION_BINS,
};
pub use consolidator::{ConsolidationConfig, Consolidator};
pub use gaps::{ExperimentProposal, GapTracker, KnowledgeGap};
pub use history::{HistoryEntry, HistorySummary, ThoughtHistory};
pub use memory::{Durability, MemoryItem, MemoryReorganisation, MergeRecord};
pub use patterns::{
    abstract_knowledge, detect, AbstractionKind, Attempt, Deliberation, Detection, Knowledge,
    LearnedPolicy, PatternConfig, RepeatedMistake, Skill, SkillRoute, StrategyReport, Verdict,
};
pub use persist::{persist, ConsolidationSink, PersistedConsolidation, PersistedKnowledge};
pub use phase::{IllegalPhaseTransition, PhaseEvent, SystemPhase};
pub use plan::{ConsolidationPlan, ConsolidationStep};
pub use replay::{Episode, ThoughtReplay};
pub use sleep::{
    Interrupt, ResourceSnapshot, SleepBudget, SleepCycle, SleepGrant, SleepOutcome, SleepReport,
    SleepRequest, SleepScheduler, ThresholdScheduler,
};
pub use state::{NextSessionState, SalienceTable};

/// Clamp to `0.0..=1.0`, mapping non-finite values to `0.0`.
pub(crate) fn unit(x: f64) -> f64 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Append each id in `from` that is not already in `into`, keeping first-seen order.
pub(crate) fn extend_provenance(into: &mut Vec<ProvenanceId>, from: &[ProvenanceId]) {
    for id in from {
        if !into.contains(id) {
            into.push(*id);
        }
    }
}
