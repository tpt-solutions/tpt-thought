// SPDX-License-Identifier: MIT OR Apache-2.0
//! Branching hypotheses (spec section 15). A [`BranchSet`] holds the live alternatives for one
//! question and bounds how many can be active at once, which is the guard against branch explosion.

use crate::lifecycle::{HypothesisStatus, IllegalHypothesisTransition};
use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_core::{
    Confidence, EpistemicStatus, EvidenceRef, Hypothesis, HypothesisId, Probability,
};

/// One alternative explanation with its bookkeeping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    /// The hypothesis itself (identity, statement, probability and confidence).
    pub hypothesis: Hypothesis,
    /// Lifecycle status.
    pub status: HypothesisStatus,
    /// The branch this one was explored from, if any.
    pub parent: Option<HypothesisId>,
    /// Evidence gathered for or against.
    pub evidence: Vec<EvidenceRef>,
    /// Estimated cost of testing this branch.
    pub cost: f64,
    /// Risk if this branch is wrong.
    pub risk: f64,
    /// Expected information gain from testing it (see [`crate::expected_information_gain`]).
    pub expected_information_gain: f64,
    /// Strongest verification applied so far.
    pub verification: EpistemicStatus,
    /// Models or providers that independently agree with this branch.
    pub supporting_models: Vec<String>,
}

impl Branch {
    /// A newly generated branch with the given prior probability.
    pub fn new(id: HypothesisId, statement: impl Into<String>, prior: Probability) -> Self {
        Self {
            hypothesis: Hypothesis {
                id,
                statement: statement.into(),
                probability: Some(prior),
                confidence: Confidence::UNKNOWN,
            },
            status: HypothesisStatus::Generated,
            parent: None,
            evidence: Vec::new(),
            cost: 0.0,
            risk: 0.0,
            expected_information_gain: 0.0,
            verification: EpistemicStatus::Informal,
            supporting_models: Vec::new(),
        }
    }

    /// The branch identifier.
    pub fn id(&self) -> HypothesisId {
        self.hypothesis.id
    }

    /// The branch's current probability (zero if unset).
    pub fn probability(&self) -> Probability {
        self.hypothesis.probability.unwrap_or(Probability::ZERO)
    }

    /// Set the branch's probability.
    pub fn set_probability(&mut self, p: Probability) {
        self.hypothesis.probability = Some(p);
    }

    /// True if the branch is still in play (not rejected).
    pub fn is_active(&self) -> bool {
        self.status != HypothesisStatus::Rejected
    }
}

/// Why a branch operation failed.
#[derive(Debug, Clone, PartialEq)]
pub enum BranchError {
    /// Exploring another branch would exceed the active-branch bound.
    LimitReached {
        /// The configured bound.
        limit: usize,
    },
    /// A branch with this identifier is already present.
    Duplicate(HypothesisId),
    /// No branch with this identifier.
    Unknown(HypothesisId),
    /// The parent named by a new branch is not present.
    UnknownParent(HypothesisId),
    /// A merge needed two different branches.
    SameBranch(HypothesisId),
    /// A lifecycle transition was refused.
    Transition(IllegalHypothesisTransition),
}

impl fmt::Display for BranchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitReached { limit } => write!(f, "active branch limit of {limit} reached"),
            Self::Duplicate(id) => write!(f, "branch {id} already exists"),
            Self::Unknown(id) => write!(f, "no branch {id}"),
            Self::UnknownParent(id) => write!(f, "parent branch {id} does not exist"),
            Self::SameBranch(id) => write!(f, "cannot merge branch {id} with itself"),
            Self::Transition(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BranchError {}

impl From<IllegalHypothesisTransition> for BranchError {
    fn from(e: IllegalHypothesisTransition) -> Self {
        Self::Transition(e)
    }
}

/// The live alternatives for one question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchSet {
    branches: Vec<Branch>,
    max_active: usize,
    merges: Vec<(HypothesisId, HypothesisId)>,
}

impl BranchSet {
    /// An empty set that allows at most `max_active` non-rejected branches.
    pub fn new(max_active: usize) -> Self {
        Self {
            branches: Vec::new(),
            max_active,
            merges: Vec::new(),
        }
    }

    /// All branches, including rejected ones kept for provenance.
    pub fn branches(&self) -> &[Branch] {
        &self.branches
    }

    /// The configured bound on active branches.
    pub fn max_active(&self) -> usize {
        self.max_active
    }

    /// Number of branches that are still in play.
    pub fn active_count(&self) -> usize {
        self.branches.iter().filter(|b| b.is_active()).count()
    }

    /// Look up a branch.
    pub fn get(&self, id: &HypothesisId) -> Option<&Branch> {
        self.branches.iter().find(|b| b.id() == *id)
    }

    /// Look up a branch mutably.
    pub fn get_mut(&mut self, id: &HypothesisId) -> Option<&mut Branch> {
        self.branches.iter_mut().find(|b| b.id() == *id)
    }

    /// Record of `(absorbed, into)` pairs from [`BranchSet::merge`], oldest first.
    pub fn merges(&self) -> &[(HypothesisId, HypothesisId)] {
        &self.merges
    }

    fn position(&self, id: &HypothesisId) -> Result<usize, BranchError> {
        self.branches
            .iter()
            .position(|b| b.id() == *id)
            .ok_or(BranchError::Unknown(*id))
    }

    /// Add a new branch. Refused when the active bound is reached, so exploration cannot grow
    /// without limit.
    pub fn explore(&mut self, branch: Branch) -> Result<(), BranchError> {
        if self.get(&branch.id()).is_some() {
            return Err(BranchError::Duplicate(branch.id()));
        }
        if let Some(parent) = branch.parent {
            if self.get(&parent).is_none() {
                return Err(BranchError::UnknownParent(parent));
            }
        }
        if branch.is_active() && self.active_count() >= self.max_active {
            return Err(BranchError::LimitReached {
                limit: self.max_active,
            });
        }
        self.branches.push(branch);
        Ok(())
    }

    /// Move a branch through its lifecycle.
    pub fn transition(
        &mut self,
        id: &HypothesisId,
        to: HypothesisStatus,
    ) -> Result<(), BranchError> {
        let branch = self.get_mut(id).ok_or(BranchError::Unknown(*id))?;
        branch.status = branch.status.transition(to)?;
        Ok(())
    }

    /// Park a branch.
    pub fn suspend(&mut self, id: &HypothesisId) -> Result<(), BranchError> {
        self.transition(id, HypothesisStatus::Suspended)
    }

    /// Reject a branch. It stays in the set so the reason remains traceable, but no longer counts
    /// towards the active bound.
    pub fn reject(&mut self, id: &HypothesisId) -> Result<(), BranchError> {
        self.transition(id, HypothesisStatus::Rejected)
    }

    /// Accept a supported branch as the working explanation.
    pub fn promote(&mut self, id: &HypothesisId) -> Result<(), BranchError> {
        self.transition(id, HypothesisStatus::Accepted)
    }

    /// Remove a branch entirely, returning it. Used to discard generation artefacts.
    pub fn prune(&mut self, id: &HypothesisId) -> Result<Branch, BranchError> {
        let at = self.position(id)?;
        Ok(self.branches.remove(at))
    }

    /// Fold `from` into `into`. Evidence and supporting models are pooled, probabilities are added
    /// (capped at one), costs add, and risk and information gain take the maximum. The absorbed
    /// branch is removed and the pair is logged in [`BranchSet::merges`].
    pub fn merge(&mut self, into: &HypothesisId, from: &HypothesisId) -> Result<(), BranchError> {
        if into == from {
            return Err(BranchError::SameBranch(*into));
        }
        if self.get(into).is_none() {
            return Err(BranchError::Unknown(*into));
        }
        let from_at = self.position(from)?;
        let absorbed = self.branches.remove(from_at);
        let target = self.get_mut(into).ok_or(BranchError::Unknown(*into))?;
        let p = (target.probability().get() + absorbed.probability().get()).min(1.0);
        target.set_probability(Probability::new(p).unwrap_or(Probability::ONE));
        for ev in absorbed.evidence {
            if !target.evidence.contains(&ev) {
                target.evidence.push(ev);
            }
        }
        for model in absorbed.supporting_models {
            if !target.supporting_models.contains(&model) {
                target.supporting_models.push(model);
            }
        }
        target.cost += absorbed.cost;
        target.risk = target.risk.max(absorbed.risk);
        target.expected_information_gain = target
            .expected_information_gain
            .max(absorbed.expected_information_gain);
        self.merges.push((*from, *into));
        Ok(())
    }

    /// Record a test result for a branch. Supporting evidence moves it to `Supported`, weakening
    /// evidence to `Weakened`; it first moves to `Testing` if the lifecycle requires that.
    pub fn record_test(
        &mut self,
        id: &HypothesisId,
        evidence: EvidenceRef,
        supports: bool,
    ) -> Result<(), BranchError> {
        let branch = self.get_mut(id).ok_or(BranchError::Unknown(*id))?;
        for step in [HypothesisStatus::Untested, HypothesisStatus::Testing] {
            if branch.status.can_move_to(step) {
                branch.status = step;
            }
        }
        let outcome = if supports {
            HypothesisStatus::Supported
        } else {
            HypothesisStatus::Weakened
        };
        branch.status = branch.status.transition(outcome)?;
        branch.evidence.push(evidence);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{ContentReference, IdGenerator, SeededEntropy};

    fn ids() -> IdGenerator {
        IdGenerator::new(SeededEntropy::new(7))
    }

    fn ev(n: &str) -> EvidenceRef {
        ContentReference {
            store: "test".into(),
            id: n.into(),
        }
    }

    fn branch(ids: &IdGenerator, p: f64) -> Branch {
        Branch::new(ids.hypothesis(), "h", Probability::new(p).unwrap())
    }

    #[test]
    fn explosion_is_bounded() {
        let g = ids();
        let mut set = BranchSet::new(2);
        set.explore(branch(&g, 0.5)).unwrap();
        set.explore(branch(&g, 0.5)).unwrap();
        assert_eq!(
            set.explore(branch(&g, 0.5)),
            Err(BranchError::LimitReached { limit: 2 })
        );
        // Rejecting frees a slot without losing the record.
        let first = set.branches()[0].id();
        set.reject(&first).unwrap();
        set.explore(branch(&g, 0.5)).unwrap();
        assert_eq!(set.branches().len(), 3);
        assert_eq!(set.active_count(), 2);
    }

    #[test]
    fn duplicates_and_unknown_parents_are_refused() {
        let g = ids();
        let mut set = BranchSet::new(5);
        let b = branch(&g, 0.5);
        let id = b.id();
        set.explore(b.clone()).unwrap();
        assert_eq!(set.explore(b), Err(BranchError::Duplicate(id)));
        let mut child = branch(&g, 0.1);
        child.parent = Some(g.hypothesis());
        assert!(matches!(
            set.explore(child),
            Err(BranchError::UnknownParent(_))
        ));
    }

    #[test]
    fn merge_pools_evidence_and_probability() {
        let g = ids();
        let mut set = BranchSet::new(5);
        let (a, b) = (branch(&g, 0.4), branch(&g, 0.8));
        let (ida, idb) = (a.id(), b.id());
        set.explore(a).unwrap();
        set.explore(b).unwrap();
        set.record_test(&ida, ev("x"), true).unwrap();
        set.record_test(&idb, ev("x"), true).unwrap();
        set.record_test(&idb, ev("y"), true).unwrap();
        set.merge(&ida, &idb).unwrap();
        let merged = set.get(&ida).unwrap();
        assert_eq!(merged.probability().get(), 1.0);
        assert_eq!(merged.evidence.len(), 2);
        assert!(set.get(&idb).is_none());
        assert_eq!(set.merges(), &[(idb, ida)]);
        assert_eq!(set.merge(&ida, &ida), Err(BranchError::SameBranch(ida)));
    }

    #[test]
    fn record_test_follows_lifecycle() {
        let g = ids();
        let mut set = BranchSet::new(3);
        let b = branch(&g, 0.5);
        let id = b.id();
        set.explore(b).unwrap();
        set.record_test(&id, ev("obs"), false).unwrap();
        assert_eq!(set.get(&id).unwrap().status, HypothesisStatus::Weakened);
        set.record_test(&id, ev("obs2"), true).unwrap();
        assert_eq!(set.get(&id).unwrap().status, HypothesisStatus::Supported);
        set.promote(&id).unwrap();
        assert_eq!(set.get(&id).unwrap().status, HypothesisStatus::Accepted);
        // Accepted cannot jump straight to Supported.
        assert!(set.transition(&id, HypothesisStatus::Supported).is_err());
    }

    #[test]
    fn prune_removes_and_suspend_parks() {
        let g = ids();
        let mut set = BranchSet::new(3);
        let (a, b) = (branch(&g, 0.5), branch(&g, 0.5));
        let (ida, idb) = (a.id(), b.id());
        set.explore(a).unwrap();
        set.explore(b).unwrap();
        set.transition(&ida, HypothesisStatus::Untested).unwrap();
        set.suspend(&ida).unwrap();
        assert_eq!(set.get(&ida).unwrap().status, HypothesisStatus::Suspended);
        assert!(set.get(&ida).unwrap().is_active());
        let gone = set.prune(&idb).unwrap();
        assert_eq!(gone.id(), idb);
        assert!(set.prune(&idb).is_err());
    }
}
