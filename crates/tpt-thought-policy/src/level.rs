// SPDX-License-Identifier: MIT OR Apache-2.0
//! Escalation levels (spec section 20). These are architectural levels, not claims about
//! biological cognition.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Architectural escalation levels, from reflex (`L0`) to human (`L6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationLevel {
    /// `L0`: reflex.
    Reflex,
    /// `L1`: structured intelligence.
    StructuredIntelligence,
    /// `L2`: specialist intelligence.
    SpecialistIntelligence,
    /// `L3`: probabilistic inference.
    ProbabilisticInference,
    /// `L4`: deliberate reasoning.
    DeliberateReasoning,
    /// `L5`: multi-hypothesis investigation.
    MultiHypothesisInvestigation,
    /// `L6`: human escalation.
    Human,
}

impl EscalationLevel {
    /// Every level, lowest first.
    pub const ALL: [EscalationLevel; 7] = [
        Self::Reflex,
        Self::StructuredIntelligence,
        Self::SpecialistIntelligence,
        Self::ProbabilisticInference,
        Self::DeliberateReasoning,
        Self::MultiHypothesisInvestigation,
        Self::Human,
    ];

    /// The numeric index `0..=6` matching the `L<n>` naming.
    pub const fn index(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for EscalationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L{}", self.index())
    }
}

/// The ordered set of levels a policy may escalate through. Configurable, so a deployment can
/// skip levels it does not have (for example, no `L3` probabilistic provider).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<EscalationLevel>", into = "Vec<EscalationLevel>")]
pub struct EscalationLadder {
    levels: Vec<EscalationLevel>,
}

/// The configured ladder is empty or not strictly ascending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LadderError;

impl fmt::Display for LadderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("escalation ladder must be non-empty and strictly ascending")
    }
}

impl std::error::Error for LadderError {}

impl EscalationLadder {
    /// A ladder over `levels`, which must be non-empty and strictly ascending.
    pub fn new(levels: Vec<EscalationLevel>) -> Result<Self, LadderError> {
        if levels.is_empty() || levels.windows(2).any(|w| w[0] >= w[1]) {
            return Err(LadderError);
        }
        Ok(Self { levels })
    }

    /// The levels in order.
    pub fn levels(&self) -> &[EscalationLevel] {
        &self.levels
    }

    /// The lowest configured level above `current`, or `None` if `current` is already the top.
    pub fn next_above(&self, current: EscalationLevel) -> Option<EscalationLevel> {
        self.levels.iter().copied().find(|&l| l > current)
    }
}

impl Default for EscalationLadder {
    fn default() -> Self {
        Self {
            levels: EscalationLevel::ALL.to_vec(),
        }
    }
}

impl TryFrom<Vec<EscalationLevel>> for EscalationLadder {
    type Error = LadderError;

    fn try_from(levels: Vec<EscalationLevel>) -> Result<Self, Self::Error> {
        Self::new(levels)
    }
}

impl From<EscalationLadder> for Vec<EscalationLevel> {
    fn from(ladder: EscalationLadder) -> Self {
        ladder.levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_ordered_and_named() {
        assert!(EscalationLevel::Reflex < EscalationLevel::Human);
        assert_eq!(EscalationLevel::Human.index(), 6);
        assert_eq!(EscalationLevel::DeliberateReasoning.to_string(), "L4");
    }

    #[test]
    fn default_ladder_steps_through_every_level() {
        let ladder = EscalationLadder::default();
        let mut at = EscalationLevel::Reflex;
        let mut seen = vec![at];
        while let Some(next) = ladder.next_above(at) {
            seen.push(next);
            at = next;
        }
        assert_eq!(seen, EscalationLevel::ALL);
    }

    #[test]
    fn sparse_ladder_skips_unconfigured_levels() {
        let ladder = EscalationLadder::new(vec![
            EscalationLevel::Reflex,
            EscalationLevel::ProbabilisticInference,
            EscalationLevel::Human,
        ])
        .unwrap();
        assert_eq!(
            ladder.next_above(EscalationLevel::Reflex),
            Some(EscalationLevel::ProbabilisticInference)
        );
        assert_eq!(ladder.next_above(EscalationLevel::Human), None);
    }

    #[test]
    fn invalid_ladders_are_rejected() {
        assert_eq!(EscalationLadder::new(vec![]), Err(LadderError));
        assert_eq!(
            EscalationLadder::new(vec![EscalationLevel::Human, EscalationLevel::Reflex]),
            Err(LadderError)
        );
        assert_eq!(
            EscalationLadder::new(vec![EscalationLevel::Reflex, EscalationLevel::Reflex]),
            Err(LadderError)
        );
    }
}
