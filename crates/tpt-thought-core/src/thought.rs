// SPDX-License-Identifier: MIT OR Apache-2.0
//! The [`Thought`] object (spec section 8). Conceptual rather than a final shape: fields are public
//! and the struct is `#[non_exhaustive]` so it can evolve without breaking dependants.

use crate::confidence::Confidence;
use crate::content::*;
use crate::id::{ProvenanceId, ThoughtId};
use crate::ownership::ThoughtOwnership;
use crate::state::ThoughtState;
use crate::time::{Temporal, Timestamp};
use crate::uncertainty::Uncertainty;
use crate::value::ValueAssessment;
use serde::{Deserialize, Serialize};

/// Where a thought came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThoughtOrigin {
    /// Triggered by an observation of the world.
    Observation,
    /// Spawned by another thought.
    Child,
    /// Produced during reflection or sleep.
    Internal,
    /// Supplied by a human.
    Human,
    /// Received from another agent.
    Agent {
        /// Opaque agent identity.
        id: String,
    },
}

/// The circumstances a thought exists in.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ThoughtContext {
    /// Free-form labels (task, location, session, ...).
    pub labels: Vec<String>,
}

/// Multi-factor attention state (spec section 11). Scoring lives in `tpt-thought-attention`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct AttentionState {
    /// Combined score as last computed.
    pub score: f64,
}

/// Lightweight link from a thought to its detailed provenance records, which live in
/// `tpt-thought-provenance`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ThoughtProvenance {
    /// Records explaining how this thought arose, oldest first.
    pub records: Vec<ProvenanceId>,
}

/// A dynamically evolving cognitive state/process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Thought {
    /// Identity.
    pub id: ThoughtId,
    /// Parent thought, if spawned by one.
    pub parent: Option<ThoughtId>,
    /// Origin.
    pub origin: ThoughtOrigin,
    /// Lifecycle state.
    pub state: ThoughtState,
    /// Who may see or act on it.
    pub ownership: ThoughtOwnership,
    /// Context.
    pub context: ThoughtContext,
    /// Temporal facets.
    pub temporal: Temporal,
    /// Attention.
    pub attention: AttentionState,
    /// Salience, `0.0`-`1.0`.
    pub salience: f64,
    /// Novelty, `0.0`-`1.0`.
    pub novelty: f64,
    /// Payload.
    pub content: ThoughtContent,
    /// Observations feeding this thought.
    pub observations: Vec<ObservationRef>,
    /// Memories associated with it.
    pub memories: Vec<MemoryRef>,
    /// Candidate explanations.
    pub hypotheses: Vec<Hypothesis>,
    /// Predictions made.
    pub predictions: Vec<Prediction>,
    /// Beliefs held.
    pub beliefs: Vec<Belief>,
    /// Alternatives considered.
    pub alternatives: Vec<Alternative>,
    /// Supporting evidence.
    pub evidence: Vec<EvidenceRef>,
    /// Confidence.
    pub confidence: Confidence,
    /// Uncertainty.
    pub uncertainty: Uncertainty,
    /// Value assessment, if made.
    pub value: Option<ValueAssessment>,
    /// Intentions formed.
    pub intentions: Vec<Intention>,
    /// Actions taken.
    pub actions: Vec<ActionRef>,
    /// Outcomes observed.
    pub outcomes: Vec<Outcome>,
    /// Provenance.
    pub provenance: ThoughtProvenance,
}

impl Thought {
    /// A new thought in [`ThoughtState::Created`].
    pub fn new(
        id: ThoughtId,
        origin: ThoughtOrigin,
        content: ThoughtContent,
        now: Timestamp,
    ) -> Self {
        Self {
            id,
            parent: None,
            origin,
            state: ThoughtState::Created,
            ownership: ThoughtOwnership::default(),
            context: ThoughtContext::default(),
            temporal: Temporal::created(now),
            attention: AttentionState::default(),
            salience: 0.0,
            novelty: 0.0,
            content,
            observations: Vec::new(),
            memories: Vec::new(),
            hypotheses: Vec::new(),
            predictions: Vec::new(),
            beliefs: Vec::new(),
            alternatives: Vec::new(),
            evidence: Vec::new(),
            confidence: Confidence::UNKNOWN,
            uncertainty: Uncertainty::unknown(),
            value: None,
            intentions: Vec::new(),
            actions: Vec::new(),
            outcomes: Vec::new(),
            provenance: ThoughtProvenance::default(),
        }
    }

    /// A thought created from an observation.
    pub fn from_observation(id: ThoughtId, observation: Observation, now: Timestamp) -> Self {
        let observed_at = observation.observed_at;
        let obs_id = observation.id;
        let mut t = Self::new(
            id,
            ThoughtOrigin::Observation,
            ThoughtContent::Observation(observation),
            now,
        );
        t.temporal.observed_at = Some(observed_at);
        t.observations.push(obs_id);
        t
    }
}
