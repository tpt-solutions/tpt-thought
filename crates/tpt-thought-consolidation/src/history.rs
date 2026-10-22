// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thought-history compression into durable summaries (spec section 86).

use crate::extend_provenance;
use tpt_thought_core::{ProvenanceId, ThoughtId, Timestamp};

/// A record of one thought, kept until it is compressed.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    /// The thought.
    pub thought: ThoughtId,
    /// When it occurred.
    pub at: Timestamp,
    /// A short summary of what it concluded.
    pub summary: String,
    /// Provenance of the thought.
    pub provenance: Vec<ProvenanceId>,
}

/// A durable summary of several compressed entries.
#[derive(Debug, Clone, PartialEq)]
pub struct HistorySummary {
    /// The thoughts it covers, oldest first.
    pub covered: Vec<ThoughtId>,
    /// Start of the covered period.
    pub from: Timestamp,
    /// End of the covered period.
    pub to: Timestamp,
    /// The combined summary.
    pub summary: String,
    /// Union of the covered provenance.
    pub provenance: Vec<ProvenanceId>,
}

/// Recent history plus the summaries of older history.
#[derive(Debug, Clone, Default)]
pub struct ThoughtHistory {
    entries: Vec<HistoryEntry>,
    summaries: Vec<HistorySummary>,
}

impl ThoughtHistory {
    /// Record a thought. Entries are kept in time order.
    pub fn push(&mut self, entry: HistoryEntry) {
        let at = self.entries.partition_point(|e| e.at <= entry.at);
        self.entries.insert(at, entry);
    }

    /// Uncompressed entries, oldest first.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Durable summaries, oldest first.
    pub fn summaries(&self) -> &[HistorySummary] {
        &self.summaries
    }

    /// Compress all but the `keep_recent` newest entries into one summary. Returns how many
    /// entries were compressed.
    pub fn compress(&mut self, keep_recent: usize) -> usize {
        let older = self.entries.len().saturating_sub(keep_recent);
        if older == 0 {
            return 0;
        }
        let compressed: Vec<HistoryEntry> = self.entries.drain(..older).collect();
        let mut provenance = Vec::new();
        for e in &compressed {
            extend_provenance(&mut provenance, &e.provenance);
        }
        let summary = HistorySummary {
            covered: compressed.iter().map(|e| e.thought).collect(),
            from: compressed[0].at,
            to: compressed[older - 1].at,
            summary: compressed
                .iter()
                .map(|e| e.summary.as_str())
                .collect::<Vec<_>>()
                .join("; "),
            provenance,
        };
        self.summaries.push(summary);
        older
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    #[test]
    fn compression_keeps_recent_and_covers_old_entries() {
        let ids = IdGenerator::new(SeededEntropy::new(60));
        let mut h = ThoughtHistory::default();
        let mut thoughts = Vec::new();
        for i in 0..5 {
            let t = ids.thought();
            thoughts.push(t);
            h.push(HistoryEntry {
                thought: t,
                at: Timestamp(i),
                summary: format!("s{i}"),
                provenance: vec![ids.provenance()],
            });
        }
        assert_eq!(h.compress(2), 3);
        assert_eq!(h.entries().len(), 2);
        let s = &h.summaries()[0];
        assert_eq!(s.covered, thoughts[..3]);
        assert_eq!(s.summary, "s0; s1; s2");
        assert_eq!((s.from, s.to), (Timestamp(0), Timestamp(2)));
        assert_eq!(s.provenance.len(), 3);
        assert_eq!(h.compress(2), 0);
    }
}
