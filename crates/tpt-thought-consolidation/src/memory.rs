// SPDX-License-Identifier: MIT OR Apache-2.0
//! Memory reorganisation: merging redundant memories, expiring and pruning (spec section 28).
//!
//! Anima owns decay (`strength`, supplied by Anima). Thought owns relevance (`relevance`).
//! Pruning returns the references it removed so that Anima can delete the stored content.

use crate::{extend_provenance, unit};
use serde::{Deserialize, Serialize};
use tpt_thought_core::{ContentReference, ProvenanceId};

/// How durable a memory is (spec section 28). Forgetting is a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Durability {
    /// Short-lived; dropped first.
    Temporary,
    /// Weakly retained.
    Weak,
    /// Strongly retained.
    Strong,
    /// Consolidated into durable knowledge; never pruned by relevance.
    Consolidated,
    /// Anima's decay has taken it below the floor.
    Expired,
    /// Removed by relevance pruning.
    Pruned,
}

/// A memory the thought layer tracks, referenced by ID only (spec section 10).
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryItem {
    /// Topic key used for merging and reinforcement.
    pub key: String,
    /// Reference into Anima memory.
    pub memory: ContentReference,
    /// Relevance to the thought layer, `0.0..=1.0` (Thought-owned).
    pub relevance: f64,
    /// Strength after Anima's decay, `0.0..=1.0` (Anima-owned).
    pub strength: f64,
    /// Forgetting state.
    pub durability: Durability,
    /// Provenance; preserved across merges.
    pub provenance: Vec<ProvenanceId>,
}

/// A record of memories folded into one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeRecord {
    /// Topic key.
    pub key: String,
    /// The memory kept.
    pub kept: ContentReference,
    /// Memories merged into it; their references can be released by Anima.
    pub merged: Vec<ContentReference>,
}

/// The working set of memories under reorganisation.
#[derive(Debug, Clone, Default)]
pub struct MemoryReorganisation {
    items: Vec<MemoryItem>,
    merges: Vec<MergeRecord>,
}

impl MemoryReorganisation {
    /// Track a memory.
    pub fn insert(&mut self, item: MemoryItem) {
        self.items.push(item);
    }

    /// Tracked memories.
    pub fn items(&self) -> &[MemoryItem] {
        &self.items
    }

    /// Merges performed so far.
    pub fn merges(&self) -> &[MergeRecord] {
        &self.merges
    }

    /// Raise relevance for every memory under `key` by `boost`, promoting durability when
    /// relevance is high enough. Replay uses this to reinforce what was just rehearsed.
    pub fn reinforce(&mut self, key: &str, boost: f64) {
        for item in self.items.iter_mut().filter(|i| i.key == key && live(i)) {
            item.relevance = unit(item.relevance + boost);
            if item.relevance >= 0.8 && item.durability == Durability::Weak {
                item.durability = Durability::Strong;
            } else if item.relevance >= 0.5 && item.durability == Durability::Temporary {
                item.durability = Durability::Weak;
            }
        }
    }

    /// Fold redundant memories (same key) into one. The kept memory has the highest relevance
    /// and strength, and the union of provenance. Returns the number of memories removed.
    pub fn merge_redundant(&mut self) -> usize {
        let mut removed = 0;
        let mut i = 0;
        while i < self.items.len() {
            if !live(&self.items[i]) {
                i += 1;
                continue;
            }
            let key = self.items[i].key.clone();
            let mut merged = Vec::new();
            let mut j = i + 1;
            while j < self.items.len() {
                if self.items[j].key == key && live(&self.items[j]) {
                    let other = self.items.remove(j);
                    let kept = &mut self.items[i];
                    if other.relevance > kept.relevance {
                        // The more relevant memory becomes the one kept.
                        merged.push(std::mem::replace(&mut kept.memory, other.memory));
                        kept.relevance = other.relevance;
                    } else {
                        merged.push(other.memory);
                    }
                    kept.strength = kept.strength.max(other.strength);
                    if rank(other.durability) > rank(kept.durability) {
                        kept.durability = other.durability;
                    }
                    extend_provenance(&mut kept.provenance, &other.provenance);
                    removed += 1;
                } else {
                    j += 1;
                }
            }
            if !merged.is_empty() {
                self.merges.push(MergeRecord {
                    key,
                    kept: self.items[i].memory.clone(),
                    merged,
                });
            }
            i += 1;
        }
        removed
    }

    /// Mark memories whose strength has fallen below `floor` as expired. Consolidated memories
    /// are exempt: they have already become durable knowledge.
    pub fn expire(&mut self, floor: f64) {
        for item in self
            .items
            .iter_mut()
            .filter(|i| live(i) && i.durability != Durability::Consolidated)
        {
            if item.strength < floor {
                item.durability = Durability::Expired;
            }
        }
    }

    /// Prune live, non-durable memories whose relevance is below `min_relevance`. Returns the
    /// removed references so Anima can delete the stored content. Strong and consolidated
    /// memories are never pruned on relevance alone.
    pub fn prune(&mut self, min_relevance: f64) -> Vec<ContentReference> {
        let mut pruned = Vec::new();
        self.items.retain_mut(|item| {
            let prunable = matches!(
                item.durability,
                Durability::Temporary | Durability::Weak | Durability::Expired
            );
            if prunable && item.relevance < min_relevance {
                item.durability = Durability::Pruned;
                pruned.push(item.memory.clone());
                false
            } else {
                true
            }
        });
        pruned
    }
}

/// A memory that has not been expired or pruned.
fn live(item: &MemoryItem) -> bool {
    !matches!(item.durability, Durability::Expired | Durability::Pruned)
}

/// Ordering used when merging: a consolidated memory outranks a strong one, and so on.
fn rank(d: Durability) -> u8 {
    match d {
        Durability::Temporary => 0,
        Durability::Weak => 1,
        Durability::Strong => 2,
        Durability::Consolidated => 3,
        Durability::Expired | Durability::Pruned => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    fn reference(n: &str) -> ContentReference {
        ContentReference {
            store: "anima".into(),
            id: n.into(),
        }
    }

    fn item(key: &str, n: &str, relevance: f64, strength: f64, prov: ProvenanceId) -> MemoryItem {
        MemoryItem {
            key: key.into(),
            memory: reference(n),
            relevance,
            strength,
            durability: Durability::Weak,
            provenance: vec![prov],
        }
    }

    #[test]
    fn merge_keeps_most_relevant_and_all_provenance() {
        let ids = IdGenerator::new(SeededEntropy::new(10));
        let (p1, p2) = (ids.provenance(), ids.provenance());
        let mut m = MemoryReorganisation::default();
        m.insert(item("k", "low", 0.3, 0.9, p1));
        m.insert(item("k", "high", 0.7, 0.2, p2));
        m.insert(item("other", "x", 0.5, 0.5, p1));
        assert_eq!(m.merge_redundant(), 1);
        assert_eq!(m.items().len(), 2);
        let kept = m.items().iter().find(|i| i.key == "k").unwrap();
        assert_eq!(kept.memory, reference("high"));
        assert_eq!(kept.strength, 0.9);
        assert_eq!(kept.provenance, vec![p1, p2]);
        assert_eq!(m.merges()[0].merged, vec![reference("low")]);
    }

    #[test]
    fn prune_respects_durability_and_returns_refs() {
        let ids = IdGenerator::new(SeededEntropy::new(11));
        let p = ids.provenance();
        let mut m = MemoryReorganisation::default();
        m.insert(item("a", "weak", 0.05, 0.9, p));
        let mut strong = item("b", "strong", 0.05, 0.9, p);
        strong.durability = Durability::Strong;
        m.insert(strong);
        let pruned = m.prune(0.1);
        assert_eq!(pruned, vec![reference("weak")]);
        assert_eq!(m.items().len(), 1);
    }

    #[test]
    fn expire_exempts_consolidated() {
        let ids = IdGenerator::new(SeededEntropy::new(12));
        let p = ids.provenance();
        let mut m = MemoryReorganisation::default();
        m.insert(item("a", "decayed", 0.9, 0.01, p));
        let mut kept = item("b", "kept", 0.9, 0.01, p);
        kept.durability = Durability::Consolidated;
        m.insert(kept);
        m.expire(0.05);
        assert_eq!(m.items()[0].durability, Durability::Expired);
        assert_eq!(m.items()[1].durability, Durability::Consolidated);
    }

    #[test]
    fn reinforce_promotes_durability() {
        let ids = IdGenerator::new(SeededEntropy::new(13));
        let mut m = MemoryReorganisation::default();
        m.insert(item("a", "x", 0.45, 0.9, ids.provenance()));
        m.reinforce("a", 0.1);
        assert_eq!(m.items()[0].durability, Durability::Weak);
        m.reinforce("a", 0.5);
        assert_eq!(m.items()[0].durability, Durability::Strong);
    }
}
