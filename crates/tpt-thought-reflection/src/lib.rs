// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reflection (spec addendum, Phase 8).
//!
//! Reflection is a bounded, deliberate review of past thoughts. It is distinct from online
//! thought (latency-bound, no durable belief change) and from sleep (offline consolidation,
//! owned by `tpt-thought-consolidation`). Thought decides *when*, *why* and *what* to reflect
//! on; the reasoning itself is supplied through [`ReflectionReasoner`] (Eve, in production).
//!
//! Reflection records structured artifacts, never a model's private chain-of-thought. Every
//! artifact holds identifiers, references, numbers and short [`Label`]s only.

mod artifact;
mod engine;
mod mode;
mod review;
mod session;
mod trigger;

pub use artifact::{
    BeliefRevision, CausalExplanation, ConfidenceRecord, ContradictionRecord, DecisionRecord,
    EvidenceRecord, KnowledgeGap, Label, LabelError, MemoryAction, MemoryUpdate, OutcomeRecord,
    PredictionErrorRecord, PredictionRecord, ReflectionArtifact, Revision, StrategyChange,
    StrategyUpdate,
};
pub use engine::{
    ReasonerError, ReflectionEngine, ReflectionPolicy, ReflectionReasoner, ReflectionStep,
};
pub use mode::{CognitiveMode, ModeRules};
pub use review::{
    CognitiveReview, ErrorAnalysis, KnowledgeGapAnalysis, Metacognition, Recommendation, Review,
    StrategyRevision,
};
pub use session::{Halt, ReflectionLimits, ReflectionSession, SessionError, SessionStatus};
pub use trigger::{ReflectionTrigger, TriggerSignal};
