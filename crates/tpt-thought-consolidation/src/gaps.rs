// SPDX-License-Identifier: MIT OR Apache-2.0
//! Persistent knowledge gaps and the experiments they suggest (todo Phase 9).

use crate::extend_provenance;
use std::collections::BTreeMap;
use tpt_thought_core::ProvenanceId;

/// A question that keeps coming up without being answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeGap {
    /// The open question.
    pub question: String,
    /// How often it was left open.
    pub occurrences: u32,
    /// Provenance of the occurrences.
    pub provenance: Vec<ProvenanceId>,
}

/// A proposed experiment to close a persistent gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentProposal {
    /// The gap the experiment targets.
    pub gap: String,
    /// What the experiment should establish.
    pub design: String,
    /// Provenance of the gap.
    pub provenance: Vec<ProvenanceId>,
}

/// Tracks open questions across episodes.
#[derive(Debug, Clone, Default)]
pub struct GapTracker {
    gaps: BTreeMap<String, KnowledgeGap>,
}

impl GapTracker {
    /// Record that `question` was left open.
    pub fn record(&mut self, question: &str, provenance: &[ProvenanceId]) {
        let gap = self
            .gaps
            .entry(question.to_owned())
            .or_insert_with(|| KnowledgeGap {
                question: question.to_owned(),
                occurrences: 0,
                provenance: Vec::new(),
            });
        gap.occurrences += 1;
        extend_provenance(&mut gap.provenance, provenance);
    }

    /// Questions open at least `min_occurrences` times.
    pub fn persistent(&self, min_occurrences: u32) -> Vec<&KnowledgeGap> {
        self.gaps
            .values()
            .filter(|g| g.occurrences >= min_occurrences)
            .collect()
    }

    /// One experiment proposal per persistent gap.
    pub fn propose(&self, min_occurrences: u32) -> Vec<ExperimentProposal> {
        self.persistent(min_occurrences)
            .into_iter()
            .map(|g| ExperimentProposal {
                gap: g.question.clone(),
                design: format!("run a controlled test that settles: {}", g.question),
                provenance: g.provenance.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    #[test]
    fn only_persistent_gaps_get_experiments() {
        let ids = IdGenerator::new(SeededEntropy::new(50));
        let mut t = GapTracker::default();
        t.record("why does pump 3 surge?", &[ids.provenance()]);
        assert!(t.propose(2).is_empty());
        t.record("why does pump 3 surge?", &[ids.provenance()]);
        t.record("one-off", &[ids.provenance()]);
        let proposals = t.propose(2);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].provenance.len(), 2);
    }
}
