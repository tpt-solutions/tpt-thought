// SPDX-License-Identifier: MIT OR Apache-2.0
//! Retrieval and salience priorities, and the cognitive state handed to the next session.

use crate::{unit, BeliefStatus, Calibration, ExperimentProposal, Knowledge, Skill};
use std::collections::BTreeMap;

/// Salience weights per topic, used to prioritise retrieval.
#[derive(Debug, Clone, Default)]
pub struct SalienceTable {
    weights: BTreeMap<String, f64>,
}

impl SalienceTable {
    /// Raise the weight of `key` by `amount`, capped at `1.0`.
    pub fn boost(&mut self, key: &str, amount: f64) {
        let w = self.weights.entry(key.to_owned()).or_insert(0.0);
        *w = unit(*w + amount);
    }

    /// Multiply every weight by `factor`, dropping weights that reach zero.
    pub fn decay(&mut self, factor: f64) {
        for w in self.weights.values_mut() {
            *w = unit(*w * factor);
        }
        self.weights.retain(|_, w| *w > 0.0);
    }

    /// The weight of `key`, `0.0` if unknown.
    pub fn get(&self, key: &str) -> f64 {
        self.weights.get(key).copied().unwrap_or(0.0)
    }

    /// All weights, highest first; ties break by key.
    pub fn ranked(&self) -> Vec<(String, f64)> {
        let mut out: Vec<(String, f64)> =
            self.weights.iter().map(|(k, w)| (k.clone(), *w)).collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out
    }
}

/// The cognitive state a consolidation pass hands to the next session (todo Phase 9).
#[derive(Debug, Clone, PartialEq)]
pub struct NextSessionState {
    /// Retrieval priorities, highest first.
    pub priorities: Vec<(String, f64)>,
    /// Questions that persistently remain open.
    pub open_gaps: Vec<String>,
    /// Experiments proposed for those gaps.
    pub experiments: Vec<ExperimentProposal>,
    /// Skills formed so far.
    pub skills: Vec<Skill>,
    /// Knowledge abstracted so far.
    pub knowledge: Vec<Knowledge>,
    /// Belief statuses by claim.
    pub beliefs: Vec<(String, BeliefStatus)>,
    /// Confidence calibration to apply next session.
    pub calibration: Calibration,
}
