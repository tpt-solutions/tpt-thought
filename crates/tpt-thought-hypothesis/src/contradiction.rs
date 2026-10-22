// SPDX-License-Identifier: MIT OR Apache-2.0
//! Contradictions between hypotheses (spec section 29). A contradiction raises attention, is
//! investigated only if the expected information gain exceeds its cost, and is resolved by new
//! evidence, by separating the contexts in which each side holds, or by making one side
//! conditionally valid.

use crate::information::should_investigate;
use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_core::{EvidenceRef, HypothesisId};

/// How a contradiction was settled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Resolution {
    /// New evidence decided between the two sides.
    NewEvidence(EvidenceRef),
    /// Both sides can be true, each in its own context.
    ContextSeparation {
        /// Context labels in which the first hypothesis holds.
        a_context: Vec<String>,
        /// Context labels in which the second hypothesis holds.
        b_context: Vec<String>,
    },
    /// One side holds only under a stated condition.
    ConditionalValidity {
        /// The condition under which the claim is valid.
        condition: String,
    },
}

/// Why a contradiction operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContradictionError {
    /// A hypothesis cannot contradict itself.
    SelfContradiction,
    /// The contradiction already has a resolution.
    AlreadyResolved,
}

impl fmt::Display for ContradictionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelfContradiction => write!(f, "a hypothesis cannot contradict itself"),
            Self::AlreadyResolved => write!(f, "contradiction is already resolved"),
        }
    }
}

impl std::error::Error for ContradictionError {}

/// Two hypotheses that cannot both hold, as things stand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contradiction {
    a: HypothesisId,
    b: HypothesisId,
    attention: f64,
    resolution: Option<Resolution>,
}

/// What handling a contradiction decided to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContradictionAction {
    /// Expected gain exceeds cost: investigate now.
    Investigate,
    /// Not worth the cost yet: keep the raised attention and defer.
    Defer,
}

impl Contradiction {
    /// A new, open contradiction between two distinct hypotheses.
    pub fn new(a: HypothesisId, b: HypothesisId) -> Result<Self, ContradictionError> {
        if a == b {
            return Err(ContradictionError::SelfContradiction);
        }
        Ok(Self {
            a,
            b,
            attention: 0.0,
            resolution: None,
        })
    }

    /// The first hypothesis.
    pub fn a(&self) -> HypothesisId {
        self.a
    }

    /// The second hypothesis.
    pub fn b(&self) -> HypothesisId {
        self.b
    }

    /// Current attention weight.
    pub fn attention(&self) -> f64 {
        self.attention
    }

    /// True if `id` is one of the two sides.
    pub fn involves(&self, id: &HypothesisId) -> bool {
        self.a == *id || self.b == *id
    }

    /// True until a resolution is recorded.
    pub fn is_open(&self) -> bool {
        self.resolution.is_none()
    }

    /// The resolution, once settled.
    pub fn resolution(&self) -> Option<&Resolution> {
        self.resolution.as_ref()
    }

    /// Raise attention by `delta`. Negative and non-finite amounts are ignored.
    pub fn raise_attention(&mut self, delta: f64) {
        if delta.is_finite() && delta > 0.0 {
            self.attention += delta;
        }
    }

    /// Record how the contradiction was settled.
    pub fn resolve(&mut self, resolution: Resolution) -> Result<(), ContradictionError> {
        if !self.is_open() {
            return Err(ContradictionError::AlreadyResolved);
        }
        self.resolution = Some(resolution);
        Ok(())
    }
}

/// Handle a contradiction when it is noticed: attention goes up, then the investigate-or-defer
/// decision is made from the expected gain against the cost (spec sections 29 and 37).
pub fn handle(
    contradiction: &mut Contradiction,
    attention_gain: f64,
    expected_gain: f64,
    cost: f64,
) -> ContradictionAction {
    contradiction.raise_attention(attention_gain);
    if should_investigate(expected_gain, cost) {
        ContradictionAction::Investigate
    } else {
        ContradictionAction::Defer
    }
}

/// All contradictions known for a body of hypotheses.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ContradictionLedger {
    items: Vec<Contradiction>,
}

impl ContradictionLedger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Every contradiction, open and resolved.
    pub fn all(&self) -> &[Contradiction] {
        &self.items
    }

    /// Record a contradiction between `a` and `b`. An existing contradiction for the same pair,
    /// in either order, is returned instead of duplicated. Returns its index.
    pub fn record(
        &mut self,
        a: HypothesisId,
        b: HypothesisId,
    ) -> Result<usize, ContradictionError> {
        if let Some(i) = self
            .items
            .iter()
            .position(|c| (c.a == a && c.b == b) || (c.a == b && c.b == a))
        {
            return Ok(i);
        }
        self.items.push(Contradiction::new(a, b)?);
        Ok(self.items.len() - 1)
    }

    /// The contradiction at `index`, mutably.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Contradiction> {
        self.items.get_mut(index)
    }

    /// Open contradictions involving `id`.
    pub fn open_for(&self, id: &HypothesisId) -> Vec<&Contradiction> {
        self.items
            .iter()
            .filter(|c| c.is_open() && c.involves(id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    fn ids() -> (HypothesisId, HypothesisId) {
        let g = IdGenerator::new(SeededEntropy::new(9));
        (g.hypothesis(), g.hypothesis())
    }

    #[test]
    fn self_contradiction_is_refused() {
        let (a, _) = ids();
        assert_eq!(
            Contradiction::new(a, a),
            Err(ContradictionError::SelfContradiction)
        );
    }

    #[test]
    fn handling_raises_attention_and_gates_investigation() {
        let (a, b) = ids();
        let mut c = Contradiction::new(a, b).unwrap();
        // Gain above cost: investigate.
        assert_eq!(
            handle(&mut c, 0.5, 0.9, 0.4),
            ContradictionAction::Investigate
        );
        // Gain below cost: attention still rises, but the decision is to defer.
        assert_eq!(handle(&mut c, 0.5, 0.1, 0.4), ContradictionAction::Defer);
        assert_eq!(c.attention(), 1.0);
        let mut cheap = Contradiction::new(a, b).unwrap();
        assert_eq!(
            handle(&mut cheap, 0.2, 0.1, 0.4),
            ContradictionAction::Defer
        );
        assert_eq!(cheap.attention(), 0.2);
        cheap.raise_attention(f64::NAN);
        cheap.raise_attention(-3.0);
        assert_eq!(cheap.attention(), 0.2);
    }

    #[test]
    fn each_resolution_kind_settles_once() {
        let (a, b) = ids();
        let mut c = Contradiction::new(a, b).unwrap();
        assert!(c.is_open());
        c.resolve(Resolution::ContextSeparation {
            a_context: vec!["indoors".into()],
            b_context: vec!["outdoors".into()],
        })
        .unwrap();
        assert!(!c.is_open());
        assert_eq!(
            c.resolve(Resolution::ConditionalValidity {
                condition: "x".into()
            }),
            Err(ContradictionError::AlreadyResolved)
        );

        let mut d = Contradiction::new(a, b).unwrap();
        d.resolve(Resolution::ConditionalValidity {
            condition: "temperature below 20C".into(),
        })
        .unwrap();
        assert!(matches!(
            d.resolution(),
            Some(Resolution::ConditionalValidity { .. })
        ));
    }

    #[test]
    fn ledger_deduplicates_pairs_and_tracks_open_items() {
        let (a, b) = ids();
        let mut ledger = ContradictionLedger::new();
        let i = ledger.record(a, b).unwrap();
        assert_eq!(ledger.record(b, a).unwrap(), i);
        assert_eq!(ledger.all().len(), 1);
        assert_eq!(ledger.open_for(&a).len(), 1);
        ledger
            .get_mut(i)
            .unwrap()
            .resolve(Resolution::NewEvidence(
                tpt_thought_core::ContentReference {
                    store: "lab".into(),
                    id: "run-1".into(),
                },
            ))
            .unwrap();
        assert!(ledger.open_for(&b).is_empty());
        assert!(ledger.record(a, a).is_err());
    }
}
