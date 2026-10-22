// SPDX-License-Identifier: MIT OR Apache-2.0
use serde::{Deserialize, Serialize};
use tpt_thought_core::{
    Confidence, ContentReference, Probability, ProvenanceId, ThoughtId, Timestamp,
};

/// Longest permitted [`Label`], in bytes.
pub const MAX_LABEL_LEN: usize = 160;

/// A short, single-line human label, such as a gap name or strategy reason.
///
/// Labels are deliberately bounded so that reflection cannot accumulate free-form reasoning
/// text. Anything longer belongs in a referenced store, addressed by [`ContentReference`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Label(String);

/// A label was empty, too long, or spanned several lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelError;

impl std::fmt::Display for LabelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "label must be 1..={MAX_LABEL_LEN} bytes on one line")
    }
}

impl std::error::Error for LabelError {}

impl Label {
    /// Validate and wrap a label.
    pub fn new(text: impl Into<String>) -> Result<Self, LabelError> {
        let text = text.into();
        let ok =
            !text.is_empty() && text.len() <= MAX_LABEL_LEN && !text.chars().any(char::is_control);
        if ok {
            Ok(Self(text))
        } else {
            Err(LabelError)
        }
    }

    /// The label text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A decision that was reviewed, and the records behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// Thought holding the decision.
    pub thought: ThoughtId,
    /// The decision itself, by reference.
    pub decision: ContentReference,
    /// Provenance records that the decision rests on.
    pub basis: Vec<ProvenanceId>,
}

/// A piece of evidence bearing on a thought.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    /// Thought the evidence bears on.
    pub thought: ThoughtId,
    /// Where the evidence is stored.
    pub source: ContentReference,
    /// Whether it supports (`true`) or undermines (`false`) the thought.
    pub supports: bool,
    /// Strength of the evidence.
    pub strength: Probability,
}

/// What an action actually produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutcomeRecord {
    /// Thought that issued the action.
    pub thought: ThoughtId,
    /// The action (an Anima/INP action reference).
    pub action: ContentReference,
    /// The observed result.
    pub observed: ContentReference,
    /// When it was observed.
    pub observed_at: Timestamp,
}

/// A prediction that was made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionRecord {
    /// Thought that made the prediction.
    pub thought: ThoughtId,
    /// The prediction, by reference.
    pub prediction: ContentReference,
    /// When the prediction is expected to resolve, if known.
    pub expected_at: Option<Timestamp>,
}

/// The gap between a prediction and what was observed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionErrorRecord {
    /// Thought that made the prediction.
    pub thought: ThoughtId,
    /// The prediction that failed, by reference.
    pub prediction: ContentReference,
    /// Magnitude of the error.
    pub magnitude: Probability,
}

/// A confidence claim and, once known, whether it held.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceRecord {
    /// Thought the confidence belongs to.
    pub thought: ThoughtId,
    /// The confidence, with its raw, calibrated and empirical forms kept apart.
    pub confidence: Confidence,
    /// Whether the claim was later borne out, if known.
    pub held: Option<bool>,
}

/// Two thoughts that disagree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContradictionRecord {
    /// One side.
    pub a: ThoughtId,
    /// The other side.
    pub b: ThoughtId,
    /// Whether it has been resolved by evidence, context separation or conditional validity.
    pub resolved: bool,
}

/// A causal explanation linking an effect to a cause through recorded provenance.
///
/// Causal claims come from Eve, Augur, experiments or external evidence, never from timing
/// alone (spec section 75), so `provenance` should be non-empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausalExplanation {
    /// Thought the explanation concerns.
    pub thought: ThoughtId,
    /// The effect being explained, by reference.
    pub effect: ContentReference,
    /// The proposed cause, by reference.
    pub cause: ContentReference,
    /// Records supporting the causal claim.
    pub provenance: Vec<ProvenanceId>,
}

/// Something the system does not know that it needs to know.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeGap {
    /// Thought in which the gap appeared.
    pub thought: ThoughtId,
    /// Short name of the gap.
    pub name: Label,
    /// References to the question or missing evidence.
    pub missing: Vec<ContentReference>,
}

/// How a strategy should change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyChange {
    /// Start using the strategy.
    Adopt,
    /// Adjust parameters or conditions of the strategy.
    Adjust,
    /// Stop using the strategy.
    Abandon,
}

/// A proposed change to how the system approaches a class of problem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyUpdate {
    /// Thought that prompted the change.
    pub thought: ThoughtId,
    /// The strategy, by reference.
    pub strategy: ContentReference,
    /// The change.
    pub change: StrategyChange,
    /// Short reason.
    pub reason: Label,
}

/// How a memory should change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAction {
    /// Strengthen a memory that proved useful.
    Reinforce,
    /// Weaken a memory that proved stale or unsupported.
    Weaken,
    /// Store a new memory.
    Create,
}

/// A proposed change to a memory held in Anima, referenced by ID only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryUpdate {
    /// Thought that prompted the change.
    pub thought: ThoughtId,
    /// The memory, by reference.
    pub memory: ContentReference,
    /// The change.
    pub action: MemoryAction,
}

/// Direction of a belief revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Revision {
    /// Confidence rose on new evidence.
    Strengthened,
    /// Confidence fell on new evidence.
    Weakened,
    /// The belief no longer holds.
    Retracted,
}

/// A proposed change to a belief. Promotion and demotion pipelines live elsewhere; reflection
/// only proposes the revision and cites its evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeliefRevision {
    /// Thought that prompted the revision.
    pub thought: ThoughtId,
    /// The belief, by reference.
    pub belief: ContentReference,
    /// Direction of the revision.
    pub revision: Revision,
    /// Evidence cited for the revision.
    pub evidence: Vec<ContentReference>,
}

/// A structured reflection artifact. Structured artifacts, not chain-of-thought: there is no
/// free-form reasoning field anywhere in this enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReflectionArtifact {
    /// A decision reviewed.
    Decision(DecisionRecord),
    /// Evidence bearing on a thought.
    Evidence(EvidenceRecord),
    /// An observed outcome.
    Outcome(OutcomeRecord),
    /// A prediction made.
    Prediction(PredictionRecord),
    /// A prediction error.
    PredictionError(PredictionErrorRecord),
    /// A confidence claim.
    Confidence(ConfidenceRecord),
    /// A contradiction.
    Contradiction(ContradictionRecord),
    /// A causal explanation.
    CausalExplanation(CausalExplanation),
    /// A knowledge gap.
    KnowledgeGap(KnowledgeGap),
    /// A strategy adjustment.
    StrategyUpdate(StrategyUpdate),
    /// A memory update.
    MemoryUpdate(MemoryUpdate),
    /// A belief revision.
    BeliefRevision(BeliefRevision),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_short_single_line() {
        assert!(Label::new("gap: thermal baseline").is_ok());
        assert_eq!(Label::new(""), Err(LabelError));
        assert_eq!(Label::new("two\nlines"), Err(LabelError));
        assert_eq!(Label::new("x".repeat(MAX_LABEL_LEN + 1)), Err(LabelError));
    }
}
