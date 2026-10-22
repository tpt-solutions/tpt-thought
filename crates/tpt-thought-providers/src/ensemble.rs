// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ensemble reasoning and disagreement (spec sections 65, 66, 118).
//!
//! Disagreement is not an error. It is a cognitive event: it is returned as a [`Disagreement`]
//! carrying the positions taken and the probes that should investigate it.

use crate::decision::{Answer, Decision, DecisionSource};
use tpt_thought_core::StructuredValue;

/// A question to put to a disagreement before it is resolved (spec section 66).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Probe {
    /// Ask each side why it holds its answer.
    AskWhy,
    /// Surface the assumptions each side relies on.
    Assumptions,
    /// Compare the evidence each side cites.
    Evidence,
    /// Check each side's calibration on comparable claims.
    Calibration,
    /// Look for a variable one side has not accounted for.
    MissingVariable,
}

impl Probe {
    /// Every probe, in the order they should be applied.
    pub const ALL: [Probe; 5] = [
        Probe::AskWhy,
        Probe::Assumptions,
        Probe::Evidence,
        Probe::Calibration,
        Probe::MissingVariable,
    ];
}

/// One answer and the sources that gave it.
#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    /// The answer.
    pub value: StructuredValue,
    /// Who gave it.
    pub sources: Vec<DecisionSource>,
}

/// Two or more providers gave different answers to the same request.
#[derive(Debug, Clone, PartialEq)]
pub struct Disagreement {
    /// The distinct answers, largest group first.
    pub positions: Vec<Position>,
    /// How to investigate the disagreement.
    pub probes: Vec<Probe>,
}

/// The outcome of combining decisions.
#[derive(Debug, Clone, PartialEq)]
pub enum Consensus {
    /// No provider answered.
    Insufficient,
    /// Every answering provider gave the same answer.
    Agreed {
        /// The shared answer.
        value: StructuredValue,
    },
    /// Answering providers gave different answers.
    Disagreed(Disagreement),
}

/// Combine decisions. Abstentions are ignored: they neither agree nor disagree.
pub fn combine(decisions: &[Decision<StructuredValue>]) -> Consensus {
    let mut positions: Vec<Position> = Vec::new();
    for decision in decisions {
        let Answer::Answered(value) = &decision.answer else {
            continue;
        };
        match positions.iter_mut().find(|p| &p.value == value) {
            Some(position) => position.sources.push(decision.source.clone()),
            None => positions.push(Position {
                value: value.clone(),
                sources: vec![decision.source.clone()],
            }),
        }
    }
    match positions.len() {
        0 => Consensus::Insufficient,
        1 => Consensus::Agreed {
            value: positions.swap_remove(0).value,
        },
        _ => {
            positions.sort_by_key(|p| std::cmp::Reverse(p.sources.len()));
            Consensus::Disagreed(Disagreement {
                positions,
                probes: Probe::ALL.to_vec(),
            })
        }
    }
}
