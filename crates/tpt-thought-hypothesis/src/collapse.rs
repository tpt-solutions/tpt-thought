// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thought collapse (spec section 64): choosing one branch from a [`BranchSet`] by a named rule.
//! Only branches still in play (not rejected or suspended) are candidates.

use crate::branch::{Branch, BranchSet};
use crate::lifecycle::HypothesisStatus;
use std::fmt;
use tpt_thought_core::{EpistemicStatus, HypothesisId};

/// How to choose the surviving branch.
#[derive(Clone, Copy)]
pub enum CollapseStrategy<'a> {
    /// Highest probability.
    Probability,
    /// Highest score under a caller-supplied utility function.
    Utility(&'a dyn Fn(&Branch) -> f64),
    /// Most pieces of supporting evidence.
    Evidence,
    /// Highest probability among branches verified at least to `Verified`.
    FormalVerification,
    /// Most agreeing models, provided at least `min_models` agree.
    EnsembleAgreement {
        /// Minimum number of agreeing models for a branch to qualify.
        min_models: usize,
    },
    /// A human has chosen this branch explicitly.
    Human(HypothesisId),
}

impl fmt::Debug for CollapseStrategy<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probability => write!(f, "Probability"),
            Self::Utility(_) => write!(f, "Utility(..)"),
            Self::Evidence => write!(f, "Evidence"),
            Self::FormalVerification => write!(f, "FormalVerification"),
            Self::EnsembleAgreement { min_models } => {
                write!(f, "EnsembleAgreement({min_models})")
            }
            Self::Human(id) => write!(f, "Human({id})"),
        }
    }
}

/// Why collapse produced no single winner.
#[derive(Debug, Clone, PartialEq)]
pub enum CollapseError {
    /// No branch is in play.
    NoCandidates,
    /// Candidates exist but none meets the strategy's requirement.
    NoQualifyingBranch,
    /// Two or more candidates tie for first place. Gather more evidence or ask a human.
    Tie(Vec<HypothesisId>),
    /// A human-chosen branch is not present or not in play.
    NotInPlay(HypothesisId),
}

impl fmt::Display for CollapseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCandidates => write!(f, "no branches are in play"),
            Self::NoQualifyingBranch => write!(f, "no branch qualifies under this strategy"),
            Self::Tie(ids) => write!(f, "{} branches tie for first place", ids.len()),
            Self::NotInPlay(id) => write!(f, "branch {id} is not in play"),
        }
    }
}

impl std::error::Error for CollapseError {}

fn in_play(b: &Branch) -> bool {
    !matches!(
        b.status,
        HypothesisStatus::Rejected | HypothesisStatus::Suspended
    )
}

/// Pick the single branch that wins under `strategy`. Ties are reported rather than broken
/// silently, because an arbitrary tie-break would hide an unresolved question.
pub fn collapse(
    set: &BranchSet,
    strategy: CollapseStrategy<'_>,
) -> Result<HypothesisId, CollapseError> {
    if let CollapseStrategy::Human(id) = strategy {
        return match set.get(&id) {
            Some(b) if in_play(b) => Ok(id),
            _ => Err(CollapseError::NotInPlay(id)),
        };
    }
    let candidates: Vec<&Branch> = set.branches().iter().filter(|b| in_play(b)).collect();
    if candidates.is_empty() {
        return Err(CollapseError::NoCandidates);
    }
    let qualifying: Vec<(&Branch, f64)> = match strategy {
        CollapseStrategy::Probability => score(candidates, |b| b.probability().get()),
        CollapseStrategy::Utility(f) => score(candidates, |b| f(b)),
        CollapseStrategy::Evidence => score(candidates, |b| b.evidence.len() as f64),
        CollapseStrategy::FormalVerification => score(
            candidates
                .into_iter()
                .filter(|b| b.verification >= EpistemicStatus::Verified)
                .collect(),
            |b| b.probability().get(),
        ),
        CollapseStrategy::EnsembleAgreement { min_models } => score(
            candidates
                .into_iter()
                .filter(|b| b.supporting_models.len() >= min_models)
                .collect(),
            |b| b.supporting_models.len() as f64,
        ),
        CollapseStrategy::Human(_) => unreachable!("handled above"),
    };
    pick(qualifying)
}

fn score(branches: Vec<&Branch>, f: impl Fn(&Branch) -> f64) -> Vec<(&Branch, f64)> {
    branches.into_iter().map(|b| (b, f(b))).collect()
}

fn pick(scored: Vec<(&Branch, f64)>) -> Result<HypothesisId, CollapseError> {
    if scored.is_empty() {
        return Err(CollapseError::NoQualifyingBranch);
    }
    let best = scored
        .iter()
        .map(|(_, s)| *s)
        .fold(f64::NEG_INFINITY, f64::max);
    let winners: Vec<HypothesisId> = scored
        .iter()
        .filter(|(_, s)| *s == best)
        .map(|(b, _)| b.id())
        .collect();
    match winners.as_slice() {
        [only] => Ok(*only),
        _ => Err(CollapseError::Tie(winners)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::HypothesisStatus;
    use tpt_thought_core::{ContentReference, IdGenerator, Probability, SeededEntropy};

    fn ev(n: u32) -> ContentReference {
        ContentReference {
            store: "t".into(),
            id: n.to_string(),
        }
    }

    fn set_with(ids: &IdGenerator, probs: &[f64]) -> (BranchSet, Vec<HypothesisId>) {
        let mut set = BranchSet::new(10);
        let mut out = Vec::new();
        for p in probs {
            let b = Branch::new(ids.hypothesis(), "h", Probability::new(*p).unwrap());
            out.push(b.id());
            set.explore(b).unwrap();
        }
        (set, out)
    }

    #[test]
    fn probability_picks_highest() {
        let g = IdGenerator::new(SeededEntropy::new(1));
        let (set, ids) = set_with(&g, &[0.2, 0.7, 0.1]);
        assert_eq!(collapse(&set, CollapseStrategy::Probability), Ok(ids[1]));
    }

    #[test]
    fn ties_are_reported_not_broken() {
        let g = IdGenerator::new(SeededEntropy::new(2));
        let (set, ids) = set_with(&g, &[0.5, 0.5]);
        assert_eq!(
            collapse(&set, CollapseStrategy::Probability),
            Err(CollapseError::Tie(ids))
        );
    }

    #[test]
    fn rejected_and_suspended_branches_do_not_compete() {
        let g = IdGenerator::new(SeededEntropy::new(3));
        let (mut set, ids) = set_with(&g, &[0.9, 0.4]);
        set.reject(&ids[0]).unwrap();
        assert_eq!(collapse(&set, CollapseStrategy::Probability), Ok(ids[1]));
        set.transition(&ids[1], HypothesisStatus::Untested).unwrap();
        set.suspend(&ids[1]).unwrap();
        assert_eq!(
            collapse(&set, CollapseStrategy::Probability),
            Err(CollapseError::NoCandidates)
        );
        assert_eq!(
            collapse(&set, CollapseStrategy::Human(ids[1])),
            Err(CollapseError::NotInPlay(ids[1]))
        );
    }

    #[test]
    fn evidence_and_utility_strategies() {
        let g = IdGenerator::new(SeededEntropy::new(4));
        let (mut set, ids) = set_with(&g, &[0.3, 0.6]);
        set.get_mut(&ids[0])
            .unwrap()
            .evidence
            .extend([ev(1), ev(2)]);
        assert_eq!(collapse(&set, CollapseStrategy::Evidence), Ok(ids[0]));
        let cheap_first = |b: &Branch| -b.probability().get();
        assert_eq!(
            collapse(&set, CollapseStrategy::Utility(&cheap_first)),
            Ok(ids[0])
        );
    }

    #[test]
    fn verification_and_ensemble_filter_candidates() {
        let g = IdGenerator::new(SeededEntropy::new(5));
        let (mut set, ids) = set_with(&g, &[0.9, 0.2]);
        assert_eq!(
            collapse(&set, CollapseStrategy::FormalVerification),
            Err(CollapseError::NoQualifyingBranch)
        );
        set.get_mut(&ids[1]).unwrap().verification = EpistemicStatus::Proven;
        assert_eq!(
            collapse(&set, CollapseStrategy::FormalVerification),
            Ok(ids[1])
        );
        set.get_mut(&ids[0]).unwrap().supporting_models = vec!["a".into(), "b".into()];
        assert_eq!(
            collapse(&set, CollapseStrategy::EnsembleAgreement { min_models: 2 }),
            Ok(ids[0])
        );
        assert_eq!(
            collapse(&set, CollapseStrategy::EnsembleAgreement { min_models: 3 }),
            Err(CollapseError::NoQualifyingBranch)
        );
    }

    #[test]
    fn human_choice_is_honoured() {
        let g = IdGenerator::new(SeededEntropy::new(6));
        let (set, ids) = set_with(&g, &[0.9, 0.1]);
        assert_eq!(collapse(&set, CollapseStrategy::Human(ids[1])), Ok(ids[1]));
    }
}
