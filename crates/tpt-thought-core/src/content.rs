// SPDX-License-Identifier: MIT OR Apache-2.0
//! Typed thought content (spec section 9) and the cognitive primitives it carries.
//!
//! These are deliberately small "shell" types. Richer behaviour (hypothesis lifecycle,
//! prediction matching, intention negotiation) lives in the dedicated crates.

use crate::confidence::{Confidence, Probability};
use crate::id::{HypothesisId, IntentionId, ObservationId, OutcomeId, PredictionId, ThoughtId};
use crate::time::Timestamp;
use crate::uncertainty::{EpistemicStatus, Uncertainty};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Natural-language content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextContent {
    /// The text.
    pub text: String,
    /// Optional BCP 47 language tag.
    pub language: Option<String>,
}

/// Schema-free structured data with deterministic (sorted) map ordering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StructuredValue {
    /// Absent value.
    Null,
    /// Boolean.
    Bool(bool),
    /// Signed integer.
    Int(i64),
    /// Floating point.
    Float(f64),
    /// Text.
    Text(String),
    /// Ordered list.
    List(Vec<StructuredValue>),
    /// Sorted map.
    Map(BTreeMap<String, StructuredValue>),
}

/// One modality's payload within [`MultimodalContent`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModalPart {
    /// Modality label (e.g. `vision`, `audio`, `sensor`).
    pub modality: String,
    /// IANA media type, if applicable.
    pub media_type: Option<String>,
    /// Payload, or a reference to it.
    pub data: ModalData,
}

/// Inline bytes or an external reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModalData {
    /// Inline bytes.
    Inline(Vec<u8>),
    /// External reference.
    Reference(ContentReference),
}

/// Content spanning several modalities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultimodalContent {
    /// The parts.
    pub parts: Vec<ModalPart>,
}

/// A reference to content stored elsewhere (e.g. in Anima). Core never duplicates storage.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentReference {
    /// Which store or namespace holds it.
    pub store: String,
    /// Identifier within that store.
    pub id: String,
}

/// Reference to an observation held elsewhere.
pub type ObservationRef = ObservationId;
/// Reference to a memory held in Anima. Opaque to Thought.
pub type MemoryRef = ContentReference;
/// Reference to a piece of evidence.
pub type EvidenceRef = ContentReference;
/// Reference to an action issued through Anima/INP.
pub type ActionRef = ContentReference;

/// How much a source is trusted (spec section 41).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum SourceTrust {
    /// Nothing is known about the source.
    #[default]
    Untrusted,
    /// Source identity is established but content is unverified.
    Authenticated,
    /// Source is trusted.
    Trusted,
}

/// Something perceived from the world.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// Identity.
    pub id: ObservationId,
    /// What was observed.
    pub content: Box<ThoughtContent>,
    /// Where it came from (provider-defined label).
    pub source: String,
    /// Trust classification of the source.
    pub trust: SourceTrust,
    /// When it was observed.
    pub observed_at: Timestamp,
}

/// A candidate explanation. Lifecycle management lives in `tpt-thought-hypothesis`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    /// Identity.
    pub id: HypothesisId,
    /// What is hypothesised.
    pub statement: String,
    /// Estimated probability, if any.
    pub probability: Option<Probability>,
    /// Confidence in the estimate.
    pub confidence: Confidence,
}

/// A held proposition. A thought is not a belief merely because it was considered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Belief {
    /// What is believed.
    pub statement: String,
    /// Confidence.
    pub confidence: Confidence,
    /// How well-founded it is.
    pub status: EpistemicStatus,
}

/// A prediction about a future state (spec section 17).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    /// Identity.
    pub id: PredictionId,
    /// The predicted state.
    pub predicted_state: StructuredValue,
    /// When the state is expected to hold.
    pub expected_at: Option<Timestamp>,
    /// Confidence.
    pub confidence: Confidence,
    /// Assumptions the prediction rests on.
    pub assumptions: Vec<String>,
    /// The thought that produced it.
    pub source_thought: ThoughtId,
}

/// A question posed to an intelligence provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    /// The question.
    pub text: String,
    /// Capability needed to answer (e.g. `rank:actions`).
    pub required_capability: Option<String>,
}

/// A decision reached about a question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    /// The chosen answer.
    pub answer: StructuredValue,
    /// Confidence in it.
    pub confidence: Confidence,
    /// Uncertainty, including abstention.
    pub uncertainty: Uncertainty,
}

/// A goal-directed intention (spec section 23).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Intention {
    /// Identity.
    pub id: IntentionId,
    /// The goal.
    pub goal: String,
    /// The outcome wanted.
    pub desired_outcome: Option<String>,
    /// Constraints on how it may be achieved.
    pub constraints: Vec<String>,
    /// Acceptable risk, `0.0` (none) to `1.0`.
    pub risk_tolerance: Option<Probability>,
    /// Relative priority.
    pub priority: f64,
    /// Capabilities needed to carry it out.
    pub required_capabilities: Vec<String>,
    /// Deadline, if any.
    pub deadline: Option<Timestamp>,
}

/// The result of an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    /// Identity.
    pub id: OutcomeId,
    /// What actually happened.
    pub actual: StructuredValue,
    /// When it was observed.
    pub observed_at: Timestamp,
}

/// An alternative that was considered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alternative {
    /// Description.
    pub description: String,
    /// Why it was or was not chosen.
    pub rationale: Option<String>,
}

/// The payload of a thought.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ThoughtContent {
    /// Natural language.
    Text(TextContent),
    /// Structured data.
    Structured(StructuredValue),
    /// An observation.
    Observation(Observation),
    /// A hypothesis.
    Hypothesis(Hypothesis),
    /// A belief.
    Belief(Belief),
    /// A prediction.
    Prediction(Prediction),
    /// A question.
    Question(Question),
    /// An intention.
    Intention(Intention),
    /// A decision.
    Decision(Decision),
    /// Multiple modalities.
    Multimodal(MultimodalContent),
    /// A reference to content held elsewhere.
    Reference(ContentReference),
}
