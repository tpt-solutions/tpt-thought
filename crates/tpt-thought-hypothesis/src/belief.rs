// SPDX-License-Identifier: MIT OR Apache-2.0
//! The belief pipeline (spec sections 43 and 44): a claim moves one stage at a time through
//! thought, hypothesis, supported hypothesis, belief candidate and belief, and demotes one stage
//! at a time. A claim is never a belief merely because it was considered: promotion is sequential
//! and every step is gated by evidence, so there is no path from `Thought` to `Belief` that skips
//! the intermediate stages.

use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_core::Probability;

/// The stages a claim passes through, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStage {
    /// Considered, nothing more.
    Thought,
    /// Proposed as an explanation.
    Hypothesis,
    /// Has at least one piece of confirming evidence.
    SupportedHypothesis,
    /// Meets some configured criteria; awaiting the rest.
    BeliefCandidate,
    /// Held as a belief.
    Belief,
}

/// A configurable condition that can justify promotion (spec section 43).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Criterion {
    /// The observation has repeatedly confirmed the claim.
    RepeatedObservation {
        /// Minimum number of confirming observations.
        min: u32,
    },
    /// A proof checker has proven the claim.
    Proof,
    /// Experiments have passed.
    Experiment {
        /// Minimum number of passed experiments.
        min: u32,
    },
    /// Intelligence provider reports high confidence.
    HighConfidenceIntelligence {
        /// Minimum confidence.
        min: Probability,
    },
    /// Independent models agree.
    ModelAgreement {
        /// Minimum number of agreeing models.
        min_models: u32,
    },
    /// A human confirmed the claim.
    HumanConfirmation,
}

/// What is known about a claim, gathered by the caller from the evidence and providers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EvidenceSummary {
    /// Observations confirming the claim.
    pub confirming_observations: u32,
    /// Whether a proof has been accepted.
    pub proven: bool,
    /// Experiments passed.
    pub passed_experiments: u32,
    /// Highest intelligence-provider confidence, if any.
    pub intelligence_confidence: Option<Probability>,
    /// Independent models that agree.
    pub agreeing_models: u32,
    /// Whether a human has confirmed.
    pub human_confirmed: bool,
}

impl Criterion {
    /// True if `summary` satisfies this criterion.
    pub fn is_met(&self, summary: &EvidenceSummary) -> bool {
        match *self {
            Self::RepeatedObservation { min } => summary.confirming_observations >= min,
            Self::Proof => summary.proven,
            Self::Experiment { min } => summary.passed_experiments >= min,
            Self::HighConfidenceIntelligence { min } => summary
                .intelligence_confidence
                .is_some_and(|c| c.get() >= min.get()),
            Self::ModelAgreement { min_models } => summary.agreeing_models >= min_models,
            Self::HumanConfirmation => summary.human_confirmed,
        }
    }
}

/// Which criteria must hold before a belief candidate becomes a belief.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionPolicy {
    criteria: Vec<Criterion>,
    required: usize,
}

/// A policy that cannot be satisfied or cannot be met at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    /// `required` must be at least one, or everything would be believed.
    NothingRequired,
    /// `required` exceeds the number of criteria, so the policy could never be met.
    Unsatisfiable {
        /// Number of criteria supplied.
        criteria: usize,
        /// Number required.
        required: usize,
    },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingRequired => write!(f, "at least one criterion must be required"),
            Self::Unsatisfiable { criteria, required } => write!(
                f,
                "{required} criteria required but only {criteria} supplied"
            ),
        }
    }
}

impl std::error::Error for PolicyError {}

impl PromotionPolicy {
    /// Build a policy requiring `required` of `criteria` to hold.
    pub fn new(criteria: Vec<Criterion>, required: usize) -> Result<Self, PolicyError> {
        if required == 0 {
            return Err(PolicyError::NothingRequired);
        }
        if required > criteria.len() {
            return Err(PolicyError::Unsatisfiable {
                criteria: criteria.len(),
                required,
            });
        }
        Ok(Self { criteria, required })
    }

    /// The configured criteria.
    pub fn criteria(&self) -> &[Criterion] {
        &self.criteria
    }

    /// How many criteria the summary satisfies.
    pub fn satisfied(&self, summary: &EvidenceSummary) -> usize {
        self.criteria.iter().filter(|c| c.is_met(summary)).count()
    }

    /// True once enough criteria hold.
    pub fn is_met(&self, summary: &EvidenceSummary) -> bool {
        self.satisfied(summary) >= self.required
    }
}

/// One recorded move between stages, kept so provenance survives promotion and demotion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageChange {
    /// Stage before the move.
    pub from: ClaimStage,
    /// Stage after the move.
    pub to: ClaimStage,
    /// Free-form reason supplied by the caller.
    pub note: String,
}

/// Why a promotion was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionError {
    /// The claim is already a belief.
    AlreadyBelief,
    /// The evidence does not yet justify the next stage.
    BelowThreshold {
        /// The stage that was requested.
        to: ClaimStage,
    },
}

impl fmt::Display for PromotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyBelief => write!(f, "claim is already a belief"),
            Self::BelowThreshold { to } => write!(f, "evidence does not yet justify {to:?}"),
        }
    }
}

impl std::error::Error for PromotionError {}

/// A claim moving through the pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    statement: String,
    stage: ClaimStage,
    history: Vec<StageChange>,
}

impl Claim {
    /// A claim that has only been considered.
    pub fn new(statement: impl Into<String>) -> Self {
        Self {
            statement: statement.into(),
            stage: ClaimStage::Thought,
            history: Vec::new(),
        }
    }

    /// The claim's statement.
    pub fn statement(&self) -> &str {
        &self.statement
    }

    /// The current stage.
    pub fn stage(&self) -> ClaimStage {
        self.stage
    }

    /// Every stage change so far, oldest first.
    pub fn history(&self) -> &[StageChange] {
        &self.history
    }

    /// True if the claim is currently held as a belief.
    pub fn is_belief(&self) -> bool {
        self.stage == ClaimStage::Belief
    }

    /// Advance exactly one stage if the evidence justifies it.
    ///
    /// Gates: thought to hypothesis needs nothing; hypothesis to supported needs a confirming
    /// observation; supported to candidate needs at least one criterion; candidate to belief needs
    /// the full policy.
    pub fn promote(
        &mut self,
        summary: &EvidenceSummary,
        policy: &PromotionPolicy,
        note: impl Into<String>,
    ) -> Result<ClaimStage, PromotionError> {
        let next = match self.stage {
            ClaimStage::Thought => ClaimStage::Hypothesis,
            ClaimStage::Hypothesis => ClaimStage::SupportedHypothesis,
            ClaimStage::SupportedHypothesis => ClaimStage::BeliefCandidate,
            ClaimStage::BeliefCandidate => ClaimStage::Belief,
            ClaimStage::Belief => return Err(PromotionError::AlreadyBelief),
        };
        let justified = match next {
            ClaimStage::SupportedHypothesis => summary.confirming_observations >= 1,
            ClaimStage::BeliefCandidate => policy.satisfied(summary) >= 1,
            ClaimStage::Belief => policy.is_met(summary),
            _ => true,
        };
        if !justified {
            return Err(PromotionError::BelowThreshold { to: next });
        }
        self.move_to(next, note);
        Ok(next)
    }

    /// Step back one stage. Returns `None` if the claim is already a thought.
    pub fn demote(&mut self, note: impl Into<String>) -> Option<ClaimStage> {
        let prev = match self.stage {
            ClaimStage::Thought => return None,
            ClaimStage::Hypothesis => ClaimStage::Thought,
            ClaimStage::SupportedHypothesis => ClaimStage::Hypothesis,
            ClaimStage::BeliefCandidate => ClaimStage::SupportedHypothesis,
            ClaimStage::Belief => ClaimStage::BeliefCandidate,
        };
        self.move_to(prev, note);
        Some(prev)
    }

    fn move_to(&mut self, to: ClaimStage, note: impl Into<String>) {
        self.history.push(StageChange {
            from: self.stage,
            to,
            note: note.into(),
        });
        self.stage = to;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> PromotionPolicy {
        PromotionPolicy::new(
            vec![
                Criterion::RepeatedObservation { min: 3 },
                Criterion::HumanConfirmation,
                Criterion::Proof,
            ],
            2,
        )
        .unwrap()
    }

    fn strong() -> EvidenceSummary {
        EvidenceSummary {
            confirming_observations: 3,
            proven: false,
            human_confirmed: true,
            ..Default::default()
        }
    }

    #[test]
    fn policy_must_be_satisfiable() {
        assert_eq!(
            PromotionPolicy::new(vec![Criterion::Proof], 0),
            Err(PolicyError::NothingRequired)
        );
        assert_eq!(
            PromotionPolicy::new(vec![Criterion::Proof], 2),
            Err(PolicyError::Unsatisfiable {
                criteria: 1,
                required: 2
            })
        );
    }

    #[test]
    fn criteria_check_their_thresholds() {
        let s = EvidenceSummary {
            intelligence_confidence: Some(Probability::new(0.9).unwrap()),
            agreeing_models: 2,
            ..Default::default()
        };
        assert!(Criterion::HighConfidenceIntelligence {
            min: Probability::new(0.9).unwrap()
        }
        .is_met(&s));
        assert!(!Criterion::HighConfidenceIntelligence {
            min: Probability::new(0.95).unwrap()
        }
        .is_met(&s));
        assert!(Criterion::ModelAgreement { min_models: 2 }.is_met(&s));
        assert!(!Criterion::Experiment { min: 1 }.is_met(&s));
    }

    #[test]
    fn considering_a_claim_does_not_make_it_a_belief() {
        let mut c = Claim::new("the crate is warm");
        // Even with overwhelming evidence, one call moves only one stage.
        assert_eq!(
            c.promote(&strong(), &policy(), "a"),
            Ok(ClaimStage::Hypothesis)
        );
        assert_ne!(c.stage(), ClaimStage::Belief);
        assert!(!c.is_belief());
    }

    #[test]
    fn full_promotion_then_demotion_round_trips() {
        let p = policy();
        let mut c = Claim::new("x");
        c.promote(&strong(), &p, "proposed").unwrap();
        c.promote(&strong(), &p, "confirmed").unwrap();
        c.promote(&strong(), &p, "candidate").unwrap();
        assert_eq!(c.promote(&strong(), &p, "belief"), Ok(ClaimStage::Belief));
        assert!(c.is_belief());
        assert_eq!(
            c.promote(&strong(), &p, "again"),
            Err(PromotionError::AlreadyBelief)
        );
        // Demote back down, one stage at a time.
        let mut seen = Vec::new();
        while let Some(stage) = c.demote("contradicted") {
            seen.push(stage);
        }
        assert_eq!(
            seen,
            vec![
                ClaimStage::BeliefCandidate,
                ClaimStage::SupportedHypothesis,
                ClaimStage::Hypothesis,
                ClaimStage::Thought,
            ]
        );
        // Every move is recorded: 4 promotions and 4 demotions.
        assert_eq!(c.history().len(), 8);
        assert_eq!(c.history()[0].from, ClaimStage::Thought);
        assert_eq!(c.history()[0].note, "proposed");
    }

    #[test]
    fn weak_evidence_blocks_each_gate() {
        let p = policy();
        let mut c = Claim::new("y");
        c.promote(&EvidenceSummary::default(), &p, "a").unwrap();
        assert_eq!(
            c.promote(&EvidenceSummary::default(), &p, "b"),
            Err(PromotionError::BelowThreshold {
                to: ClaimStage::SupportedHypothesis
            })
        );
        let one_observation = EvidenceSummary {
            confirming_observations: 1,
            human_confirmed: true,
            ..Default::default()
        };
        c.promote(&one_observation, &p, "c").unwrap();
        // Supported, but only one criterion holds, and two are required for belief.
        assert_eq!(
            c.promote(&one_observation, &p, "d"),
            Ok(ClaimStage::BeliefCandidate)
        );
        assert_eq!(
            c.promote(&one_observation, &p, "e"),
            Err(PromotionError::BelowThreshold {
                to: ClaimStage::Belief
            })
        );
    }

    #[test]
    fn thought_has_nowhere_further_down() {
        let mut c = Claim::new("z");
        assert_eq!(c.demote("nothing"), None);
        assert!(c.history().is_empty());
    }
}
