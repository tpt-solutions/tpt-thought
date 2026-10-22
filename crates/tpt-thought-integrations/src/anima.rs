// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapters from `tpt-anima` to Thought (spec sections 10, 12, 35, 48.1, 48.2; todo.md Phase 6).
//!
//! Three mappings are provided, each over one Anima layer:
//!
//! - [`AnimaMemory`] implements the workspace [`MemoryStore`] port over the SSE in-memory store.
//!   Memories are held by Anima; Thought keeps only [`ContentReference`]s to them (§10).
//! - [`AnimaBudget`] gives the compute-budget signal from CK's `BudgetManager` (§35). Thought
//!   states a cost; CK decides whether it is admitted. Thought never schedules compute itself.
//! - [`AnimaBroadcast`] resolves which providers can serve an intention through INP's
//!   `IntentBroadcaster` and `CapabilityRegistry`. It covers the provider-discovery part of a
//!   broadcast only. The global workspace's publish, subscribe and withdraw are not mapped,
//!   because `tpt-anima` has no global workspace type to map them onto.
//!
//! Not mapped, because `tpt-anima` does not provide them: `retrieve_context`, episode
//! persistence (§14, §48.2), and a ZKPA identity type. `tpt-anima-zkpa` pulls in tokio and is not
//! used.

use tpt_anima_ck::types::ThermalConstraints;
use tpt_anima_ck::{BudgetManager, ComputeBudget, ComputeUnit};
use tpt_anima_inp::{
    AgentId as InpAgentId, BroadcastConfig, Capability, CapabilityRegistry, EconomicTerms, Intent,
    IntentBroadcaster, PerformanceProfile, QoSConstraints,
};
use tpt_anima_sse::{
    AgentId as SseAgentId, InMemoryStorage, MemoryId, MemoryNode, SensoryModality, VectorEmbedding,
};
use tpt_thought_core::{ContentReference, MemoryRef};
use tpt_thought_workspace::MemoryStore;

/// The store name Thought uses in [`ContentReference`]s that point into Anima.
pub const STORE_NAME: &str = "tpt-anima-sse";

/// Memories held in Anima's SSE, retrievable by tag through the workspace [`MemoryStore`] port.
pub struct AnimaMemory {
    storage: InMemoryStorage,
    creator: SseAgentId,
    written: u64,
}

impl AnimaMemory {
    /// An empty store. `creator` is recorded as the author of every memory written through it.
    pub fn new(creator: SseAgentId) -> Self {
        Self {
            storage: InMemoryStorage::new(),
            creator,
            written: 0,
        }
    }

    /// Write a text memory under `tags` and return a reference to it.
    pub fn remember(&mut self, text: &str, tags: &[&str]) -> ContentReference {
        self.written += 1;
        let mut id: MemoryId = [0; 32];
        id[24..].copy_from_slice(&self.written.to_be_bytes());
        let mut node = MemoryNode::new(
            id,
            VectorEmbedding::new(vec![0.0]),
            text.as_bytes().to_vec(),
            "text/plain".into(),
            SensoryModality::Textual,
            self.creator,
        );
        node.metadata.tags = tags.iter().map(|t| (*t).to_string()).collect();
        self.storage.write_node(node);
        reference(&id)
    }

    /// The text of a memory previously written through [`AnimaMemory::remember`]. References
    /// into any other store resolve to nothing.
    pub fn text_of(&self, memory: &MemoryRef) -> Option<String> {
        if memory.store != STORE_NAME {
            return None;
        }
        let id = parse_id(&memory.id)?;
        let node = self.storage.read_node(&id)?;
        String::from_utf8(node.payload.clone()).ok()
    }
}

impl MemoryStore for AnimaMemory {
    /// Memories sharing the most tags with `cue` rank first, then by identifier, so the order
    /// is deterministic. A memory with no matching tag is never returned.
    fn retrieve(&self, cue: &[String], limit: usize) -> Vec<MemoryRef> {
        let mut scored: Vec<(usize, [u8; 32])> = self
            .storage
            .iter()
            .filter_map(|node| {
                let hits = node
                    .metadata
                    .tags
                    .iter()
                    .filter(|tag| cue.iter().any(|c| c == *tag))
                    .count();
                (hits > 0).then_some((hits, node.id))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, id)| reference(&id))
            .collect()
    }
}

fn reference(id: &MemoryId) -> ContentReference {
    ContentReference {
        store: STORE_NAME.into(),
        id: id.iter().map(|b| format!("{b:02x}")).collect(),
    }
}

/// Whole flops for a finite, non-negative cost. Anything else has no valid budget.
fn flops_of(cost: f64) -> Option<u64> {
    (cost.is_finite() && cost >= 0.0).then(|| cost.round() as u64)
}

/// Parse a canonical identifier: 64 lowercase hex digits, exactly as `reference` writes them.
fn parse_id(hex: &str) -> Option<MemoryId> {
    if hex.len() != 64 || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut id = [0u8; 32];
    for (i, byte) in id.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(id)
}

/// The compute-budget signal from CK (§35). A demand is admitted if the flops it needs fit the
/// remaining budget for its compute unit.
pub struct AnimaBudget {
    manager: BudgetManager,
    held: Vec<(ComputeUnit, u64)>,
}

impl AnimaBudget {
    /// A budget of `flops` total across all compute units, under default thermal limits.
    pub fn new(flops: u64) -> Self {
        Self {
            manager: BudgetManager::new(ComputeBudget {
                flops_budget: flops,
                thermal_constraints: ThermalConstraints {
                    max_temp_celsius: 85.0,
                    max_power_watts: 150.0,
                },
            }),
            held: Vec::new(),
        }
    }

    /// Ask CK to admit `estimated_cost` flops on `unit`. Returns `true` and holds the flops if
    /// they fit, and `false` (holding nothing) if they do not. Non-finite or negative costs are
    /// refused.
    pub fn admit(&mut self, unit: ComputeUnit, estimated_cost: f64) -> bool {
        let Some(flops) = flops_of(estimated_cost) else {
            return false;
        };
        if self.manager.allocate_flops(unit, flops).is_err() {
            return false;
        }
        self.add_held(unit, flops);
        true
    }

    /// Return flops previously admitted on `unit`. Returns `false` and releases nothing unless
    /// at least `estimated_cost` flops are currently held on that unit.
    pub fn release(&mut self, unit: ComputeUnit, estimated_cost: f64) -> bool {
        let Some(flops) = flops_of(estimated_cost) else {
            return false;
        };
        let Some(slot) = self.held.iter_mut().find(|(u, _)| *u == unit) else {
            return false;
        };
        if slot.1 < flops {
            return false;
        }
        slot.1 -= flops;
        self.manager.release_flops(unit, flops);
        true
    }

    fn add_held(&mut self, unit: ComputeUnit, flops: u64) {
        match self.held.iter_mut().find(|(u, _)| *u == unit) {
            Some(slot) => slot.1 += flops,
            None => self.held.push((unit, flops)),
        }
    }

    /// How much CK throttles work at the current temperature, from 0.0 (stopped) to 1.0.
    pub fn thermal_throttle_factor(&self) -> f32 {
        self.manager.thermal_throttle_factor()
    }
}

/// Resolves which providers can serve an action, through INP's broadcaster (§12, §39).
pub struct AnimaBroadcast {
    broadcaster: IntentBroadcaster,
    registry: CapabilityRegistry,
    origin: InpAgentId,
}

impl AnimaBroadcast {
    /// An empty registry. `origin` identifies this agent as the requester of intentions.
    pub fn new(origin: InpAgentId) -> Self {
        Self {
            broadcaster: IntentBroadcaster::new(BroadcastConfig::default()),
            registry: CapabilityRegistry::new(),
            origin,
        }
    }

    /// Register that `provider` can perform `action`.
    pub fn offer(&mut self, action: &str, provider: InpAgentId) {
        let capability = Capability::new(
            vec![action.to_string()],
            PerformanceProfile {
                typical_latency: std::time::Duration::from_millis(100),
                max_latency: std::time::Duration::from_secs(5),
                typical_accuracy: 0.9,
                throughput_per_second: 10.0,
            },
            EconomicTerms::FixedPrice { credits: 0.0 },
            provider,
        );
        self.registry.register(capability);
    }

    /// Providers able to serve an intention to perform `action`. Empty if none is registered.
    pub fn providers_for(&mut self, action: &str) -> Vec<InpAgentId> {
        let intent = Intent::new(
            action.to_string(),
            serde_json::Value::Null,
            QoSConstraints::default(),
            EconomicTerms::FixedPrice { credits: 0.0 },
            self.origin,
        );
        self.broadcaster.begin_broadcast(&intent, &self.registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creator() -> SseAgentId {
        [7; 32]
    }

    #[test]
    fn memories_are_retrieved_by_matching_tags_most_matches_first() {
        let mut memory = AnimaMemory::new(creator());
        let kettle = memory.remember("kettle boils water", &["kettle", "hot"]);
        let _tea = memory.remember("tea needs hot water", &["tea"]);
        let both = memory.remember("hot kettle is noisy", &["kettle", "hot", "noise"]);
        let hot_only = memory.remember("hot bath", &["hot"]);
        let found = memory.retrieve(&["kettle".into(), "hot".into()], 10);
        assert_eq!(found.len(), 3, "tea has no matching tag");
        assert!(found[..2].contains(&kettle) && found[..2].contains(&both));
        assert_eq!(found[2], hot_only, "a single matching tag ranks last");
        assert_eq!(memory.text_of(&found[2]).as_deref(), Some("hot bath"));
    }

    #[test]
    fn retrieval_respects_the_limit_and_empty_cue() {
        let mut memory = AnimaMemory::new(creator());
        memory.remember("a", &["x"]);
        memory.remember("b", &["x"]);
        assert_eq!(memory.retrieve(&["x".into()], 1).len(), 1);
        assert!(memory.retrieve(&[], 10).is_empty());
    }

    #[test]
    fn unknown_references_have_no_text() {
        let memory = AnimaMemory::new(creator());
        let bogus = MemoryRef {
            store: STORE_NAME.into(),
            id: "not-a-real-id".into(),
        };
        assert_eq!(memory.text_of(&bogus), None);
    }

    #[test]
    fn budget_admits_within_limit_and_refuses_beyond() {
        let mut budget = AnimaBudget::new(1_000);
        assert!(budget.admit(ComputeUnit::CPU, 600.0));
        assert!(!budget.admit(ComputeUnit::CPU, 600.0), "only 400 remain");
        assert!(budget.admit(ComputeUnit::CPU, 400.0));
    }

    #[test]
    fn releasing_frees_budget_for_later_demands() {
        let mut budget = AnimaBudget::new(100);
        assert!(budget.admit(ComputeUnit::GPU, 100.0));
        assert!(!budget.admit(ComputeUnit::GPU, 1.0));
        budget.release(ComputeUnit::GPU, 100.0);
        assert!(budget.admit(ComputeUnit::GPU, 100.0));
    }

    #[test]
    fn broadcast_finds_only_providers_offering_the_action() {
        let origin = InpAgentId::from_u128(1);
        let fetcher = InpAgentId::from_u128(2);
        let mut broadcast = AnimaBroadcast::new(origin);
        broadcast.offer("fetch", fetcher);
        assert_eq!(broadcast.providers_for("fetch"), vec![fetcher]);
        assert!(broadcast.providers_for("launch").is_empty());
    }

    #[test]
    fn non_finite_and_negative_costs_are_refused() {
        let mut budget = AnimaBudget::new(1_000);
        assert!(!budget.admit(ComputeUnit::CPU, f64::NAN));
        assert!(!budget.admit(ComputeUnit::CPU, f64::INFINITY));
        assert!(!budget.admit(ComputeUnit::CPU, -5.0));
        assert!(budget.admit(ComputeUnit::CPU, 1_000.0), "nothing was held");
    }

    #[test]
    fn release_cannot_free_more_than_was_admitted() {
        let mut budget = AnimaBudget::new(1_000);
        assert!(budget.admit(ComputeUnit::CPU, 600.0));
        assert!(!budget.release(ComputeUnit::CPU, 800.0), "only 600 held");
        assert!(!budget.admit(ComputeUnit::CPU, 500.0), "still 400 remain");
        assert!(budget.release(ComputeUnit::CPU, 600.0));
        assert!(budget.admit(ComputeUnit::CPU, 1_000.0));
    }

    #[test]
    fn references_into_other_stores_resolve_to_nothing() {
        let mut memory = AnimaMemory::new(creator());
        let own = memory.remember("kept", &["k"]);
        assert_eq!(memory.text_of(&own).as_deref(), Some("kept"));
        let foreign = MemoryRef {
            store: "other-store".into(),
            id: own.id.clone(),
        };
        assert_eq!(memory.text_of(&foreign), None);
    }

    #[test]
    fn ids_must_be_canonical_lowercase_hex() {
        assert!(parse_id(&"0".repeat(64)).is_some());
        assert!(parse_id(&format!("+{}", "0".repeat(63))).is_none());
        assert!(parse_id(&"A".repeat(64)).is_none());
    }
}
