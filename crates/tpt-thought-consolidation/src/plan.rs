// SPDX-License-Identifier: MIT OR Apache-2.0
//! The consolidation plan: an ordered list of steps that a sleep cycle works through.

use serde::{Deserialize, Serialize};

/// One unit of consolidation work. Each step is applied atomically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationStep {
    /// Replay episodes in importance order.
    Replay,
    /// Detect strategies, repeated mistakes, learned policies and skills.
    DetectPatterns,
    /// Merge redundant memories.
    MergeMemories,
    /// Expire decayed memories and prune irrelevant ones.
    PruneForgotten,
    /// Strengthen causal links and weaken stale associations.
    ReweightAssociations,
    /// Abstract episodes and patterns into knowledge.
    Abstract,
    /// Re-evaluate beliefs against accumulated evidence.
    ReevaluateBeliefs,
    /// Turn persistent knowledge gaps into experiment proposals.
    RecordGaps,
    /// Refit confidence calibration.
    Recalibrate,
    /// Compress old thought history into durable summaries.
    CompressHistory,
    /// Decay retrieval and salience priorities.
    UpdatePriorities,
}

/// An ordered list of consolidation steps.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConsolidationPlan {
    steps: Vec<ConsolidationStep>,
}

impl ConsolidationPlan {
    /// The full consolidation pass, in dependency order: replay feeds pattern detection, which
    /// feeds abstraction; gaps and calibration are refreshed last.
    pub fn standard() -> Self {
        use ConsolidationStep as S;
        Self::from_steps(vec![
            S::Replay,
            S::DetectPatterns,
            S::MergeMemories,
            S::PruneForgotten,
            S::ReweightAssociations,
            S::Abstract,
            S::ReevaluateBeliefs,
            S::RecordGaps,
            S::Recalibrate,
            S::CompressHistory,
            S::UpdatePriorities,
        ])
    }

    /// A plan with exactly these steps, in this order (used to resume an interrupted cycle).
    pub fn from_steps(steps: Vec<ConsolidationStep>) -> Self {
        Self { steps }
    }

    /// The steps in order.
    pub fn steps(&self) -> &[ConsolidationStep] {
        &self.steps
    }

    /// Number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// True if there are no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_plan_replays_before_abstracting() {
        let plan = ConsolidationPlan::standard();
        let pos = |s| plan.steps().iter().position(|x| *x == s).unwrap();
        assert!(pos(ConsolidationStep::Replay) < pos(ConsolidationStep::DetectPatterns));
        assert!(pos(ConsolidationStep::DetectPatterns) < pos(ConsolidationStep::Abstract));
        assert_eq!(plan.len(), 11);
    }
}
