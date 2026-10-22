// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::artifact::{
    BeliefRevision, CausalExplanation, KnowledgeGap, MemoryUpdate, PredictionErrorRecord,
    ReflectionArtifact, StrategyUpdate,
};
use crate::session::{ReflectionSession, SessionStatus};
use crate::trigger::ReflectionTrigger;
use serde::{Deserialize, Serialize};
use tpt_thought_core::{Probability, ThoughtId};

/// Certainty below which unresolved gaps push the system to ask for help.
const LOW_CERTAINTY: f64 = 0.5;

/// Counts of what a reflection examined.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    /// Decisions reviewed.
    pub decisions: u32,
    /// Outcomes observed.
    pub outcomes: u32,
    /// Predictions examined.
    pub predictions: u32,
    /// Prediction errors found.
    pub prediction_errors: u32,
    /// Contradictions still unresolved.
    pub open_contradictions: u32,
    /// Evidence items examined.
    pub evidence: u32,
}

/// What the system should do next, following spec section 19.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    /// Keep cognition going.
    Continue,
    /// Enough is known to act.
    Act,
    /// Too uncertain to proceed alone.
    AskForHelp,
}

/// Self-assessment derived from the artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Metacognition {
    /// Mean certainty across confidence records, using calibrated where present, else raw.
    pub certainty: Option<Probability>,
    /// The recommended next step.
    pub recommendation: Recommendation,
}

/// Knowledge gaps found during reflection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeGapAnalysis {
    /// The gaps, in the order found.
    pub gaps: Vec<KnowledgeGap>,
}

/// Errors and their causes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorAnalysis {
    /// Prediction errors found.
    pub prediction_errors: Vec<PredictionErrorRecord>,
    /// Proposed causes, each carrying its provenance.
    pub causes: Vec<CausalExplanation>,
}

/// Proposed changes to strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyRevision {
    /// The proposed updates.
    pub updates: Vec<StrategyUpdate>,
}

/// The full result of one reflection session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveReview {
    /// Why reflection ran.
    pub trigger: ReflectionTrigger,
    /// Whether the session completed or a circuit breaker halted it.
    pub status: SessionStatus,
    /// Thoughts reflected on, in order.
    pub reviewed: Vec<ThoughtId>,
    /// A reasoner failure message, if one ended the session.
    pub failure: Option<String>,
    /// Counts of what was examined.
    pub review: Review,
    /// Self-assessment.
    pub metacognition: Metacognition,
    /// Knowledge gaps.
    pub gaps: KnowledgeGapAnalysis,
    /// Errors and causes.
    pub error_analysis: ErrorAnalysis,
    /// Strategy changes.
    pub strategy_revision: StrategyRevision,
    /// Belief revisions proposed.
    pub belief_revisions: Vec<BeliefRevision>,
    /// Memory changes proposed.
    pub memory_updates: Vec<MemoryUpdate>,
    /// Every artifact recorded, in order.
    pub artifacts: Vec<ReflectionArtifact>,
}

impl CognitiveReview {
    /// Build the review from a finished session.
    pub(crate) fn from_session(
        session: ReflectionSession,
        reviewed: Vec<ThoughtId>,
        failure: Option<String>,
    ) -> Self {
        let (trigger, status, artifacts) = session.into_parts();

        let mut counts = Review::default();
        let mut certainties = Vec::new();
        let mut gaps = Vec::new();
        let mut prediction_errors = Vec::new();
        let mut causes = Vec::new();
        let mut strategy = Vec::new();
        let mut beliefs = Vec::new();
        let mut memory = Vec::new();

        for artifact in &artifacts {
            match artifact {
                ReflectionArtifact::Decision(_) => counts.decisions += 1,
                ReflectionArtifact::Evidence(_) => counts.evidence += 1,
                ReflectionArtifact::Outcome(_) => counts.outcomes += 1,
                ReflectionArtifact::Prediction(_) => counts.predictions += 1,
                ReflectionArtifact::PredictionError(e) => {
                    counts.prediction_errors += 1;
                    prediction_errors.push(e.clone());
                }
                ReflectionArtifact::Confidence(c) => {
                    if let Some(p) = c.confidence.calibrated.or(c.confidence.raw) {
                        certainties.push(p.get());
                    }
                }
                ReflectionArtifact::Contradiction(c) => {
                    if !c.resolved {
                        counts.open_contradictions += 1;
                    }
                }
                ReflectionArtifact::CausalExplanation(c) => causes.push(c.clone()),
                ReflectionArtifact::KnowledgeGap(g) => gaps.push(g.clone()),
                ReflectionArtifact::StrategyUpdate(s) => strategy.push(s.clone()),
                ReflectionArtifact::MemoryUpdate(m) => memory.push(m.clone()),
                ReflectionArtifact::BeliefRevision(b) => beliefs.push(b.clone()),
            }
        }

        let certainty = mean(&certainties).and_then(|m| Probability::new(m).ok());
        let recommendation = recommend(certainty, !gaps.is_empty(), counts.open_contradictions > 0);

        Self {
            trigger,
            status,
            reviewed,
            failure,
            review: counts,
            metacognition: Metacognition {
                certainty,
                recommendation,
            },
            gaps: KnowledgeGapAnalysis { gaps },
            error_analysis: ErrorAnalysis {
                prediction_errors,
                causes,
            },
            strategy_revision: StrategyRevision { updates: strategy },
            belief_revisions: beliefs,
            memory_updates: memory,
            artifacts,
        }
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn recommend(
    certainty: Option<Probability>,
    has_gaps: bool,
    has_contradictions: bool,
) -> Recommendation {
    let uncertain = certainty.is_none_or(|c| c.get() < LOW_CERTAINTY);
    if has_gaps && uncertain {
        Recommendation::AskForHelp
    } else if has_gaps || has_contradictions {
        Recommendation::Continue
    } else {
        Recommendation::Act
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_information_asks_for_nothing_but_does_not_act() {
        // No gaps and no contradictions: the system may act.
        assert_eq!(recommend(None, false, false), Recommendation::Act);
        // Gaps with unknown certainty: ask for help.
        assert_eq!(recommend(None, true, false), Recommendation::AskForHelp);
    }

    #[test]
    fn contradictions_without_gaps_continue() {
        let high = Probability::new(0.9).ok();
        assert_eq!(recommend(high, false, true), Recommendation::Continue);
    }

    #[test]
    fn mean_of_nothing_is_none() {
        assert_eq!(mean(&[]), None);
        assert_eq!(mean(&[0.5, 1.0]), Some(0.75));
    }
}
