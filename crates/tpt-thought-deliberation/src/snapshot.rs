// SPDX-License-Identifier: MIT OR Apache-2.0
//! The read-only view of a thought that an engine receives (spec section 46).

use serde::{Deserialize, Serialize};
use tpt_thought_core::{
    Belief, CognitiveBudget, Confidence, EvidenceRef, Hypothesis, Thought, ThoughtContent,
    ThoughtId, ThoughtState, Uncertainty,
};

/// A copy of the parts of a [`Thought`] that deliberation needs. Engines never mutate thoughts
/// directly; they return results that the loop applies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThoughtSnapshot {
    /// Identity of the thought.
    pub id: ThoughtId,
    /// Lifecycle state when the snapshot was taken.
    pub state: ThoughtState,
    /// Payload.
    pub content: ThoughtContent,
    /// Confidence at snapshot time.
    pub confidence: Confidence,
    /// Uncertainty at snapshot time.
    pub uncertainty: Uncertainty,
    /// Candidate explanations.
    pub hypotheses: Vec<Hypothesis>,
    /// Beliefs held.
    pub beliefs: Vec<Belief>,
    /// Supporting evidence.
    pub evidence: Vec<EvidenceRef>,
}

impl ThoughtSnapshot {
    /// Copy the deliberation-relevant parts of `thought`.
    pub fn from_thought(thought: &Thought) -> Self {
        Self {
            id: thought.id,
            state: thought.state,
            content: thought.content.clone(),
            confidence: thought.confidence,
            uncertainty: thought.uncertainty,
            hypotheses: thought.hypotheses.clone(),
            beliefs: thought.beliefs.clone(),
            evidence: thought.evidence.clone(),
        }
    }
}

/// Everything an engine needs to answer one deliberation request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliberationContext {
    /// The thought being deliberated on.
    pub thought: ThoughtSnapshot,
    /// Limits the engine must respect. [`crate::run_investigation`] enforces `max_iterations`;
    /// other dimensions are for the engine to honour.
    pub budget: CognitiveBudget,
}
