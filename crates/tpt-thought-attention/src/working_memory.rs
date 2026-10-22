// SPDX-License-Identifier: MIT OR Apache-2.0
//! Working memory (spec section 13): the strictly bounded current working set. Anima owns
//! persistent memory and decay of stored memories; Thought owns cognitive relevance (section 28).

use serde::{Deserialize, Serialize};
use tpt_thought_core::{ProvenanceId, ThoughtContent, ThoughtId, Timestamp};

/// How durable an item is (spec section 28). Forgetting is a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgettingState {
    /// Short-lived; dropped first.
    Temporary,
    /// Weakly retained.
    Weak,
    /// Strongly retained.
    Strong,
    /// Consolidated into durable knowledge.
    Consolidated,
    /// Relevance has decayed away.
    Expired,
    /// Deliberately removed.
    Pruned,
}

/// An entry in working memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkingMemoryItem {
    /// The thought this item belongs to.
    pub thought: ThoughtId,
    /// What is held.
    pub content: ThoughtContent,
    /// Attention weight; decays over time.
    pub weight: f64,
    /// Explicit priority; not decayed. Higher survives eviction longer.
    pub priority: f64,
    /// When it entered working memory.
    pub inserted_at: Timestamp,
    /// When its weight was last refreshed or decayed.
    pub touched_at: Timestamp,
    /// Provenance of the item.
    pub provenance: Vec<ProvenanceId>,
    /// Retention state.
    pub state: ForgettingState,
}

impl WorkingMemoryItem {
    /// A new temporary item.
    pub fn new(
        thought: ThoughtId,
        content: ThoughtContent,
        weight: f64,
        priority: f64,
        now: Timestamp,
    ) -> Self {
        Self {
            thought,
            content,
            weight,
            priority,
            inserted_at: now,
            touched_at: now,
            provenance: Vec::new(),
            state: ForgettingState::Temporary,
        }
    }

    /// The value compared when deciding what to evict.
    fn retention(&self) -> f64 {
        self.weight + self.priority
    }
}

/// Working memory parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorkingMemoryConfig {
    /// Maximum number of items. Strict.
    pub capacity: usize,
    /// Time for an item's weight to halve, in nanoseconds.
    pub half_life_nanos: u64,
    /// Weight below which an item expires.
    pub expiry_threshold: f64,
}

impl Default for WorkingMemoryConfig {
    fn default() -> Self {
        Self {
            capacity: 7,
            half_life_nanos: 30_000_000_000,
            expiry_threshold: 0.01,
        }
    }
}

/// Result of [`WorkingMemory::insert`].
#[derive(Debug, Clone, PartialEq)]
pub enum InsertOutcome {
    /// Added with room to spare.
    Inserted,
    /// Added; the returned item was evicted to make room.
    Evicted(Box<WorkingMemoryItem>),
    /// Not added: memory is full and every held item outranks the new one.
    Rejected,
}

/// A strictly bounded working set.
#[derive(Debug, Clone)]
pub struct WorkingMemory {
    config: WorkingMemoryConfig,
    items: Vec<WorkingMemoryItem>,
}

impl WorkingMemory {
    /// An empty working memory.
    pub fn new(config: WorkingMemoryConfig) -> Self {
        Self {
            config,
            items: Vec::new(),
        }
    }

    /// Items currently held, in insertion order.
    pub fn items(&self) -> &[WorkingMemoryItem] {
        &self.items
    }

    /// Number of items held. Never exceeds the configured capacity.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Add `item`. When full, the lowest-retention item is evicted if (and only if) the new item
    /// outranks it; ties favour the incumbent. Ranking is weight plus priority.
    pub fn insert(&mut self, item: WorkingMemoryItem) -> InsertOutcome {
        if self.config.capacity == 0 {
            return InsertOutcome::Rejected;
        }
        if self.items.len() < self.config.capacity {
            self.items.push(item);
            return InsertOutcome::Inserted;
        }
        let (idx, lowest) = self
            .items
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.retention()
                    .total_cmp(&b.retention())
                    .then(a.inserted_at.cmp(&b.inserted_at))
            })
            .expect("capacity > 0 and memory full implies non-empty");
        if item.retention() > lowest.retention() {
            let mut evicted = self.items.remove(idx);
            evicted.state = ForgettingState::Pruned;
            self.items.push(item);
            InsertOutcome::Evicted(Box::new(evicted))
        } else {
            InsertOutcome::Rejected
        }
    }

    /// Refresh an item's weight (attending to it again).
    pub fn reinforce(&mut self, thought: &ThoughtId, weight: f64, now: Timestamp) -> bool {
        match self.items.iter_mut().find(|i| &i.thought == thought) {
            Some(i) => {
                i.weight = weight;
                i.touched_at = now;
                true
            }
            None => false,
        }
    }

    /// Remove an item deliberately.
    pub fn remove(&mut self, thought: &ThoughtId) -> Option<WorkingMemoryItem> {
        let idx = self.items.iter().position(|i| &i.thought == thought)?;
        let mut item = self.items.remove(idx);
        item.state = ForgettingState::Pruned;
        Some(item)
    }

    /// Decay every item's weight to `now` using the half-life, then remove and return any that
    /// fell below the expiry threshold. Time moving backwards is ignored.
    pub fn decay(&mut self, now: Timestamp) -> Vec<WorkingMemoryItem> {
        let half_life = self.config.half_life_nanos.max(1) as f64;
        for item in &mut self.items {
            let dt = now.as_nanos().saturating_sub(item.touched_at.as_nanos());
            if dt > 0 {
                item.weight *= 0.5f64.powf(dt as f64 / half_life);
                item.touched_at = now;
            }
        }
        let threshold = self.config.expiry_threshold;
        let mut expired = Vec::new();
        let mut kept = Vec::with_capacity(self.items.len());
        for mut item in self.items.drain(..) {
            if item.weight < threshold && item.state != ForgettingState::Consolidated {
                item.state = ForgettingState::Expired;
                expired.push(item);
            } else {
                kept.push(item);
            }
        }
        self.items = kept;
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy, StructuredValue};

    fn item(ids: &IdGenerator, weight: f64, priority: f64, at: i64) -> WorkingMemoryItem {
        WorkingMemoryItem::new(
            ids.thought(),
            ThoughtContent::Structured(StructuredValue::Null),
            weight,
            priority,
            Timestamp(at),
        )
    }

    fn cfg(capacity: usize) -> WorkingMemoryConfig {
        WorkingMemoryConfig {
            capacity,
            half_life_nanos: 1_000,
            expiry_threshold: 0.1,
        }
    }

    #[test]
    fn evicts_lowest_when_outranked() {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let mut wm = WorkingMemory::new(cfg(2));
        let low = item(&ids, 0.1, 0.0, 0);
        let low_id = low.thought;
        wm.insert(low);
        wm.insert(item(&ids, 0.9, 0.0, 1));
        match wm.insert(item(&ids, 0.5, 0.0, 2)) {
            InsertOutcome::Evicted(e) => {
                assert_eq!(e.thought, low_id);
                assert_eq!(e.state, ForgettingState::Pruned);
            }
            other => panic!("expected eviction, got {other:?}"),
        }
        assert_eq!(wm.len(), 2);
    }

    #[test]
    fn rejects_when_everything_outranks_new_item() {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let mut wm = WorkingMemory::new(cfg(1));
        wm.insert(item(&ids, 0.9, 0.0, 0));
        assert_eq!(wm.insert(item(&ids, 0.1, 0.0, 1)), InsertOutcome::Rejected);
        // Ties favour the incumbent.
        assert_eq!(wm.insert(item(&ids, 0.9, 0.0, 2)), InsertOutcome::Rejected);
    }

    #[test]
    fn priority_protects_items() {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let mut wm = WorkingMemory::new(cfg(1));
        wm.insert(item(&ids, 0.1, 5.0, 0));
        assert_eq!(wm.insert(item(&ids, 1.0, 0.0, 1)), InsertOutcome::Rejected);
    }

    #[test]
    fn decay_halves_weight_and_expires() {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let mut wm = WorkingMemory::new(cfg(4));
        wm.insert(item(&ids, 1.0, 0.0, 0));
        assert!(wm.decay(Timestamp(1_000)).is_empty());
        assert!((wm.items()[0].weight - 0.5).abs() < 1e-9);
        let expired = wm.decay(Timestamp(10_000));
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].state, ForgettingState::Expired);
        assert!(wm.is_empty());
    }

    #[test]
    fn consolidated_items_do_not_expire() {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let mut wm = WorkingMemory::new(cfg(4));
        let mut i = item(&ids, 1.0, 0.0, 0);
        i.state = ForgettingState::Consolidated;
        wm.insert(i);
        assert!(wm.decay(Timestamp(1_000_000)).is_empty());
        assert_eq!(wm.len(), 1);
    }

    #[test]
    fn reinforce_resets_decay_clock() {
        let ids = IdGenerator::new(SeededEntropy::new(1));
        let mut wm = WorkingMemory::new(cfg(4));
        let i = item(&ids, 0.2, 0.0, 0);
        let id = i.thought;
        wm.insert(i);
        assert!(wm.reinforce(&id, 1.0, Timestamp(500)));
        wm.decay(Timestamp(1_500));
        assert!((wm.items()[0].weight - 0.5).abs() < 1e-9);
        assert!(wm.remove(&id).is_some());
        assert!(!wm.reinforce(&id, 1.0, Timestamp(0)));
    }

    proptest! {
        /// Capacity is strict no matter what is inserted.
        #[test]
        fn never_exceeds_capacity(cap in 0usize..8, weights in proptest::collection::vec(0.0f64..10.0, 0..64)) {
            let ids = IdGenerator::new(SeededEntropy::new(2));
            let mut wm = WorkingMemory::new(cfg(cap));
            for (n, w) in weights.into_iter().enumerate() {
                wm.insert(item(&ids, w, 0.0, n as i64));
                prop_assert!(wm.len() <= cap);
            }
        }
    }
}
