// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapters from `tpt-anima` to Thought (spec sections 10, 12, 35, 48.1, 48.2; todo.md Phase 6).
//!
//! Mappings are provided, each over one Anima layer:
//!
//! - [`AnimaMemory`] implements the workspace [`MemoryStore`] port over the SSE store. Memories
//!   are held by Anima; Thought keeps only [`ContentReference`]s to them (§10). Every memory
//!   carries an embedding, so [`AnimaMemory::recall_similar`] ranks by meaning through SSE's
//!   approximate nearest-neighbour index.
//! - [`AnimaBudget`] gives the compute-budget signal from CK's `BudgetManager` (§35), and CK's
//!   compute, energy and thermal state as a [`ResourceSnapshot`] (§48.3). Thought states a cost;
//!   CK decides whether it is admitted. Thought never schedules compute itself.
//! - [`AnimaBubbles`] gives each Thought bubble an execution boundary in CK's `ThoughtBubbleManager`:
//!   opened when the bubble starts, quarantined when a circuit breaker trips, torn down when it
//!   ends (§42, §48.4).
//! - [`AnimaBroadcast`] runs an INP intention round: providers are found through
//!   `CapabilityRegistry`, bids are collected by `IntentBroadcaster`, and INP picks the winner
//!   (§23, §39). [`AnimaIntention`] then tracks the winner's execution through INP's
//!   `NegotiationSession`: the provider runs the work, and Thought records what it reports.
//!   The global workspace's publish, subscribe and withdraw are not mapped.
//! - [`AnimaEpisodes`] stores Thought episodes in SSE's episode store (§14, §48.2).
//!
//! Not mapped, because `tpt-anima` does not provide them: a global workspace, and ZKPA
//! attestation, proofs and the agency ledger (sessions and delegation are in the `zkpa` module).
//! CK reports no memory figures, so memory pressure is supplied by the caller.

use std::time::Duration;

use tpt_anima_ck::thought_bubble::ThoughtBubbleManager;
use tpt_anima_ck::types::{IsolationLevel, ThermalConstraints};
use tpt_anima_ck::{BudgetManager, ComputeBudget, ComputeUnit, ThermalStatus};
use tpt_anima_inp::{
    AgentId as InpAgentId, BroadcastConfig, Capability, CapabilityBid, CapabilityRegistry,
    EconomicTerms, ExecutionResult, ExecutionStatus, Intent, IntentBroadcaster, NegotiationSession,
    NegotiationState, PerformanceProfile, QoSConstraints,
};
use tpt_anima_sse::{
    AgentId as SseAgentId, EpisodeStore, EpisodeStoreError, InMemoryStorage, MemoryId, MemoryNode,
    SensoryModality, VectorEmbedding,
};
use tpt_thought_consolidation::{
    ResourceSnapshot, SleepGrant, SleepRequest, SleepScheduler, ThresholdScheduler,
};
use tpt_thought_core::{ContentReference, MemoryRef};
use tpt_thought_workspace::{Episode, MemoryStore};
use uuid::Uuid;

/// The store name Thought uses in [`ContentReference`]s that point into Anima.
pub const STORE_NAME: &str = "tpt-anima-sse";

/// CK's thermal ceiling in Celsius, used for every budget this adapter creates.
const MAX_TEMP_CELSIUS: f32 = 85.0;

/// CK's power ceiling in watts, used for every budget this adapter creates.
const MAX_POWER_WATTS: f32 = 150.0;

/// Memories held in Anima's SSE, retrievable by tag through the workspace [`MemoryStore`] port
/// and by meaning through [`AnimaMemory::recall_similar`].
pub struct AnimaMemory {
    storage: InMemoryStorage,
    creator: SseAgentId,
    written: u64,
    /// Length of every embedding in the store, fixed by the first memory written. The SSE index
    /// accepts only one vector length, so other lengths are refused before they reach it.
    dim: usize,
}

impl AnimaMemory {
    /// An empty store. `creator` is recorded as the author of every memory written through it.
    pub fn new(creator: SseAgentId) -> Self {
        Self {
            storage: InMemoryStorage::new(),
            creator,
            written: 0,
            dim: 0,
        }
    }

    /// Write a text memory under `tags`, embedded as `embedding`, and return a reference to it.
    /// Returns `None` and writes nothing if the embedding is degenerate (empty, non-finite or
    /// all zeros) or its length differs from the memories already held.
    pub fn remember(
        &mut self,
        text: &str,
        tags: &[&str],
        embedding: &[f32],
    ) -> Option<ContentReference> {
        if !valid_embedding(embedding) {
            return None;
        }
        if self.dim == 0 {
            self.dim = embedding.len();
        } else if embedding.len() != self.dim {
            return None;
        }
        self.written += 1;
        let mut id: MemoryId = [0; 32];
        id[24..].copy_from_slice(&self.written.to_be_bytes());
        let mut node = MemoryNode::new(
            id,
            VectorEmbedding::new(embedding.to_vec()),
            text.as_bytes().to_vec(),
            "text/plain".into(),
            SensoryModality::Textual,
            self.creator,
        );
        node.metadata.tags = tags.iter().map(|t| (*t).to_string()).collect();
        self.storage.write_node(node);
        Some(reference(&id))
    }

    /// Up to `limit` memories whose embeddings are closest to `query` by cosine similarity,
    /// closest first. Empty if `query` is degenerate or has a different length from the store's
    /// embeddings.
    pub fn recall_similar(&self, query: &[f32], limit: usize) -> Vec<ContentReference> {
        if query.len() != self.dim || !valid_embedding(query) {
            return Vec::new();
        }
        self.storage
            .semantic_search(&VectorEmbedding::new(query.to_vec()), limit)
            .into_iter()
            .map(|node| reference(&node.id))
            .collect()
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

/// An embedding the SSE index can rank: non-empty, finite, and not the zero vector (which has
/// no direction to compare).
fn valid_embedding(values: &[f32]) -> bool {
    !values.is_empty() && values.iter().all(|v| v.is_finite()) && values.iter().any(|v| *v != 0.0)
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
    flops_budget: u64,
}

impl AnimaBudget {
    /// A budget of `flops` total across all compute units, under default thermal limits.
    pub fn new(flops: u64) -> Self {
        Self {
            manager: BudgetManager::new(ComputeBudget {
                flops_budget: flops,
                thermal_constraints: ThermalConstraints {
                    max_temp_celsius: MAX_TEMP_CELSIUS,
                    max_power_watts: MAX_POWER_WATTS,
                },
            }),
            held: Vec::new(),
            flops_budget: flops,
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

    /// Record a temperature reading in Celsius and return CK's resulting thermal status.
    pub fn update_temperature(&mut self, temp_celsius: f32) -> ThermalStatus {
        self.manager.update_temperature(temp_celsius)
    }

    /// Record a power draw in watts from hardware telemetry.
    pub fn update_power_draw(&mut self, watts: f32) {
        self.manager.update_power_draw(watts);
    }

    /// Flops CK has not yet allocated.
    pub fn remaining_flops(&self) -> u64 {
        self.manager.remaining_budget()
    }

    /// CK's compute and energy state as the resource snapshot the sleep scheduler reads (§48.3).
    /// `compute_free` is the unallocated share of the flops budget and `energy_available` the
    /// share of power headroom. CK reports no memory figures, so the caller supplies
    /// `memory_pressure`, clamped to `0.0..=1.0`.
    pub fn resource_snapshot(&self, memory_pressure: f64) -> ResourceSnapshot {
        let compute_free = match self.flops_budget {
            0 => 0.0,
            total => self.remaining_flops() as f64 / total as f64,
        };
        ResourceSnapshot {
            compute_free,
            memory_pressure: memory_pressure.clamp(0.0, 1.0),
            energy_available: f64::from(self.manager.power_headroom_watts() / MAX_POWER_WATTS),
        }
    }
}

/// Sleep is granted only while the thermal throttle factor is strictly above this value. CK
/// maps Cool and Warm to 1.0, Hot to 0.75, Throttled to 0.4 and Critical to 0.0, so only Cool and
/// Warm units sleep.
pub const SLEEP_MIN_THROTTLE_FACTOR: f32 = 0.75;

impl SleepScheduler for AnimaBudget {
    /// Grant sleep when CK's thermal state allows it and flops remain, then apply the threshold
    /// rules to the caller's resource snapshot. The trait takes no interrupts, so a high-priority
    /// interrupt must be handled by the caller before this is asked.
    fn grant(&self, request: &SleepRequest, resources: &ResourceSnapshot) -> SleepGrant {
        let defaults = ThresholdScheduler::default();
        if self.thermal_throttle_factor() > SLEEP_MIN_THROTTLE_FACTOR && self.remaining_flops() > 0
        {
            defaults.grant(request, resources)
        } else {
            SleepGrant::Deferred {
                retry_after_nanos: defaults.retry_after_nanos,
            }
        }
    }
}

/// A provider's bid for an intention, as the provider states it (§23). The adapter turns each
/// offer into an INP bid on the provider's registered capability.
#[derive(Debug, Clone, Copy)]
pub struct Offer {
    /// The provider making the offer. Must be registered for the action being negotiated.
    pub provider: InpAgentId,
    /// Estimated cost in INP's cost units.
    pub estimated_cost: f64,
    /// Estimated time to complete the action.
    pub estimated_latency: Duration,
    /// Estimated accuracy, from 0.0 to 1.0.
    pub estimated_accuracy: f32,
    /// The provider's confidence in its own estimates, from 0.0 to 1.0.
    pub confidence: f32,
}

/// Why an intention could not be opened or recorded.
#[derive(Debug, Clone, PartialEq)]
pub enum IntentionError {
    /// No registered provider for the action made an accepted bid.
    NoBids,
    /// The intention is not in a state that allows the step, such as recording an outcome before
    /// execution starts or a second time. Carries the state it was in.
    InvalidState(String),
}

impl std::fmt::Display for IntentionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBids => write!(f, "no registered provider made an accepted bid"),
            Self::InvalidState(state) => write!(f, "intention is {state}, so this step is refused"),
        }
    }
}

impl std::error::Error for IntentionError {}

/// What a provider reports after executing an intention (§23).
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// How the execution ended.
    pub status: ExecutionStatus,
    /// The cost the provider actually incurred.
    pub actual_cost: f64,
    /// The time the provider actually took.
    pub actual_latency: Duration,
    /// The accuracy the provider actually achieved, from 0.0 to 1.0.
    pub actual_accuracy: f32,
    /// Proofs the provider attached to the result.
    pub proofs: Vec<String>,
}

/// An intention that INP has assigned to a winning provider (§23, §24). Thought does not run the
/// work; the provider does. Thought tracks the lifecycle and records the outcome.
pub struct AnimaIntention {
    session: NegotiationSession,
}

impl AnimaIntention {
    /// The current state of the intention.
    pub fn state(&self) -> &NegotiationState {
        &self.session.state
    }

    /// The provider assigned to execute the intention, while it is executing.
    pub fn provider(&self) -> Option<InpAgentId> {
        match self.session.state {
            NegotiationState::Executing { provider } => Some(provider),
            _ => None,
        }
    }

    /// The recorded result, once the provider has reported an outcome.
    pub fn result(&self) -> Option<&ExecutionResult> {
        self.session.result.as_ref()
    }

    /// Record the provider's outcome. Only an executing intention accepts one, and it accepts one
    /// outcome. A completed or partially completed outcome completes the intention; a failure,
    /// timeout or cancellation ends it in the matching state.
    pub fn record(&mut self, outcome: Outcome) -> Result<(), IntentionError> {
        let NegotiationState::Executing { provider } = self.session.state else {
            return Err(IntentionError::InvalidState(format!(
                "{:?}",
                self.session.state
            )));
        };
        let result = ExecutionResult {
            intent_id: self.session.intent.id,
            provider_id: provider,
            actual_cost: outcome.actual_cost,
            actual_latency: outcome.actual_latency,
            actual_accuracy: outcome.actual_accuracy,
            proofs: outcome.proofs,
            status: outcome.status.clone(),
        };
        match outcome.status {
            ExecutionStatus::Completed | ExecutionStatus::PartiallyCompleted { .. } => {
                self.session.complete(result);
            }
            ExecutionStatus::Failed { reason } => {
                self.session.result = Some(result);
                self.session.fail(reason);
            }
            ExecutionStatus::TimedOut => {
                self.session.result = Some(result);
                self.session.fail("timed out".to_string());
            }
            ExecutionStatus::Cancelled => {
                self.session.result = Some(result);
                self.session.cancel();
            }
        }
        Ok(())
    }
}

/// Runs intention rounds through INP's broadcaster (§12, §23, §39).
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
        let intent = self.intent_for(action);
        self.broadcaster.begin_broadcast(&intent, &self.registry)
    }

    /// Run one intention round for `action`. Only registered providers for `action` are heard:
    /// an offer from anyone else, or one the round refuses, is dropped. Returns the provider INP
    /// selects, or `None` if no bid was accepted. The round is closed either way.
    pub fn negotiate(&mut self, action: &str, offers: &[Offer]) -> Option<InpAgentId> {
        let intent = self.intent_for(action);
        let providers = self.broadcaster.begin_broadcast(&intent, &self.registry);
        for offer in offers.iter().filter(|o| providers.contains(&o.provider)) {
            let Some(capability) = self
                .registry
                .lookup_by_action(action)
                .into_iter()
                .find(|c| c.provider_agent_id == offer.provider)
            else {
                continue;
            };
            let bid = CapabilityBid::new(
                intent.id,
                offer.provider,
                capability.id,
                offer.estimated_cost,
                offer.estimated_latency,
                offer.estimated_accuracy,
                offer.confidence,
            );
            // A refused bid (round full or timed out) is dropped; the round settles on the rest.
            let _ = self.broadcaster.submit_bid(bid);
        }
        let result = self.broadcaster.finalize_broadcast(&intent.id)?;
        result
            .select_winner(&QoSConstraints::default())
            .map(|bid| bid.provider_id)
    }

    /// Open an intention for `action` and assign it to the best registered bid. Offers from
    /// unregistered providers are ignored. Fails with [`IntentionError::NoBids`] if no registered
    /// provider bid. The returned intention is executing on the winner.
    pub fn open_intention(
        &mut self,
        action: &str,
        offers: &[Offer],
    ) -> Result<AnimaIntention, IntentionError> {
        let mut session = NegotiationSession::new(self.intent_for(action));
        session.start_broadcasting();
        session.start_collecting();
        for offer in offers {
            let Some(capability) = self
                .registry
                .lookup_by_action(action)
                .into_iter()
                .find(|c| c.provider_agent_id == offer.provider && c.availability.available)
            else {
                continue;
            };
            session.add_bid(CapabilityBid::new(
                session.intent.id,
                offer.provider,
                capability.id,
                offer.estimated_cost,
                offer.estimated_latency,
                offer.estimated_accuracy,
                offer.confidence,
            ));
        }
        let qos = session.intent.qos.clone();
        let winner = session
            .bids
            .iter()
            .min_by(|a, b| {
                a.score(&qos)
                    .partial_cmp(&b.score(&qos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|bid| bid.provider_id)
            .ok_or(IntentionError::NoBids)?;
        session.start_executing(winner);
        Ok(AnimaIntention { session })
    }

    fn intent_for(&self, action: &str) -> Intent {
        Intent::new(
            action.to_string(),
            serde_json::Value::Null,
            QoSConstraints::default(),
            EconomicTerms::FixedPrice { credits: 0.0 },
            self.origin,
        )
    }
}

/// The kind tag under which Thought episodes are stored. The version changes if the encoding does.
pub const EPISODE_KIND: &str = "tpt-thought.episode.v1";

/// Why an episode could not be recorded or loaded.
#[derive(Debug)]
pub enum AnimaEpisodeError {
    /// The episode store refused the append.
    Store(EpisodeStoreError),
    /// The episode could not be encoded, or its start time is before the Unix epoch.
    Encode(String),
    /// A stored record could not be decoded, or is not a Thought episode.
    Decode {
        /// The identifier of the record that failed.
        episode_id: String,
        /// Why it failed.
        reason: String,
    },
}

impl std::fmt::Display for AnimaEpisodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(error) => write!(f, "{error}"),
            Self::Encode(reason) => write!(f, "episode could not be encoded: {reason}"),
            Self::Decode { episode_id, reason } => {
                write!(f, "episode '{episode_id}' could not be decoded: {reason}")
            }
        }
    }
}

impl std::error::Error for AnimaEpisodeError {}

/// Thought episodes kept in Anima's episode store (§14, §48.2). Each episode is stored as JSON,
/// so Anima holds no Thought types. Episodes replay in the order they were recorded.
#[derive(Default)]
pub struct AnimaEpisodes {
    store: EpisodeStore,
}

impl AnimaEpisodes {
    /// An empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an episode, keyed by its identifier, and return its sequence number. A recorded
    /// episode is never rewritten, so recording the same identifier twice is refused.
    pub fn record(&mut self, episode: &Episode) -> Result<u64, AnimaEpisodeError> {
        let started_at_nanos = u64::try_from(episode.started_at.as_nanos())
            .map_err(|_| AnimaEpisodeError::Encode("start time is before the Unix epoch".into()))?;
        let payload = serde_json::to_vec(episode)
            .map_err(|error| AnimaEpisodeError::Encode(error.to_string()))?;
        self.store
            .append(&episode.id, EPISODE_KIND, started_at_nanos, payload)
            .map_err(AnimaEpisodeError::Store)
    }

    /// The episode recorded under `episode_id`, if any.
    pub fn load(&self, episode_id: &str) -> Result<Option<Episode>, AnimaEpisodeError> {
        let Some(record) = self.store.get(episode_id) else {
            return Ok(None);
        };
        let decode_error = |reason: String| AnimaEpisodeError::Decode {
            episode_id: episode_id.to_string(),
            reason,
        };
        if record.kind != EPISODE_KIND {
            return Err(decode_error(format!("kind is '{}'", record.kind)));
        }
        serde_json::from_slice(&record.payload)
            .map(Some)
            .map_err(|error| decode_error(error.to_string()))
    }

    /// Identifiers of every recorded episode, in recording order, for replay.
    pub fn ids(&self) -> Vec<String> {
        self.store.iter().map(|r| r.episode_id.clone()).collect()
    }
}

/// Execution boundaries for Thought bubbles, held by CK's `ThoughtBubbleManager` (§42, §48.4).
/// A bubble is identified by the [`Uuid`] returned from [`AnimaBubbles::open`].
pub struct AnimaBubbles {
    manager: ThoughtBubbleManager,
}

impl Default for AnimaBubbles {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimaBubbles {
    /// An empty manager with no open bubbles.
    pub fn new() -> Self {
        Self {
            manager: ThoughtBubbleManager::new(),
        }
    }

    /// Open a bubble for `sandbox` at `isolation`, and return its handle.
    pub fn open(&mut self, sandbox: &str, isolation: IsolationLevel) -> Uuid {
        self.manager
            .create_bubble(sandbox.to_string(), isolation, Vec::new())
            .id
    }

    /// Mark a bubble compromised. Returns `true` if it existed and was not already quarantined.
    pub fn quarantine(&mut self, bubble: Uuid) -> bool {
        self.manager.quarantine(bubble)
    }

    /// Whether a bubble is quarantined. Unknown bubbles are not.
    pub fn is_quarantined(&self, bubble: Uuid) -> bool {
        self.manager.is_quarantined(bubble)
    }

    /// Remove a bubble. Returns `true` if it existed.
    pub fn teardown(&mut self, bubble: Uuid) -> bool {
        self.manager.teardown(bubble)
    }

    /// Bubbles open and not quarantined.
    pub fn active(&self) -> usize {
        self.manager.active_bubbles()
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
        let kettle = memory
            .remember("kettle boils water", &["kettle", "hot"], &[1.0, 0.0])
            .unwrap();
        let _tea = memory
            .remember("tea needs hot water", &["tea"], &[0.0, 1.0])
            .unwrap();
        let both = memory
            .remember(
                "hot kettle is noisy",
                &["kettle", "hot", "noise"],
                &[1.0, 0.5],
            )
            .unwrap();
        let hot_only = memory.remember("hot bath", &["hot"], &[0.5, 1.0]).unwrap();
        let found = memory.retrieve(&["kettle".into(), "hot".into()], 10);
        assert_eq!(found.len(), 3, "tea has no matching tag");
        assert!(found[..2].contains(&kettle) && found[..2].contains(&both));
        assert_eq!(found[2], hot_only, "a single matching tag ranks last");
        assert_eq!(memory.text_of(&found[2]).as_deref(), Some("hot bath"));
    }

    #[test]
    fn retrieval_respects_the_limit_and_empty_cue() {
        let mut memory = AnimaMemory::new(creator());
        memory.remember("a", &["x"], &[1.0, 0.0]).unwrap();
        memory.remember("b", &["x"], &[0.0, 1.0]).unwrap();
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
        let own = memory.remember("kept", &["k"], &[1.0]).unwrap();
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

    #[test]
    fn similar_memories_are_recalled_closest_first() {
        let mut memory = AnimaMemory::new(creator());
        let east = memory.remember("east", &[], &[1.0, 0.0]).unwrap();
        let north = memory.remember("north", &[], &[0.0, 1.0]).unwrap();
        let _north_east = memory.remember("north-east", &[], &[0.7, 0.7]).unwrap();
        assert_eq!(memory.recall_similar(&[0.9, 0.1], 1), vec![east]);
        assert_eq!(memory.recall_similar(&[0.1, 0.9], 1), vec![north]);
        assert_eq!(memory.recall_similar(&[1.0, 1.0], 10).len(), 3);
    }

    #[test]
    fn writes_with_degenerate_or_mismatched_embeddings_are_refused() {
        let mut memory = AnimaMemory::new(creator());
        assert!(memory.remember("ok", &[], &[1.0, 0.0]).is_some());
        assert!(memory.remember("longer", &[], &[1.0, 0.0, 0.0]).is_none());
        assert!(memory.remember("empty", &[], &[]).is_none());
        assert!(memory.remember("zero", &[], &[0.0, 0.0]).is_none());
        assert!(memory.remember("nan", &[], &[f32::NAN, 1.0]).is_none());
        assert_eq!(
            memory.storage.node_count(),
            1,
            "refused writes store nothing"
        );
    }

    #[test]
    fn recall_refuses_queries_of_the_wrong_shape() {
        let mut memory = AnimaMemory::new(creator());
        memory.remember("a", &[], &[1.0, 0.0]).unwrap();
        assert!(memory.recall_similar(&[1.0, 0.0, 0.0], 5).is_empty());
        assert!(memory.recall_similar(&[0.0, 0.0], 5).is_empty());
        assert!(AnimaMemory::new(creator())
            .recall_similar(&[1.0], 5)
            .is_empty());
    }

    #[test]
    fn bubbles_are_quarantined_once_then_torn_down() {
        let mut bubbles = AnimaBubbles::new();
        let bubble = bubbles.open("plan", IsolationLevel::Thread);
        let other = bubbles.open("act", IsolationLevel::Process);
        assert_eq!(bubbles.active(), 2);
        assert!(bubbles.quarantine(bubble));
        assert!(!bubbles.quarantine(bubble), "already quarantined");
        assert!(bubbles.is_quarantined(bubble));
        assert_eq!(bubbles.active(), 1, "a quarantined bubble is not active");
        assert!(bubbles.teardown(bubble));
        assert!(!bubbles.teardown(bubble), "already gone");
        assert!(!bubbles.is_quarantined(bubble));
        assert!(!bubbles.is_quarantined(other));
        assert_eq!(bubbles.active(), 1);
    }

    fn outcome(status: ExecutionStatus) -> Outcome {
        Outcome {
            status,
            actual_cost: 1.0,
            actual_latency: Duration::from_millis(90),
            actual_accuracy: 0.95,
            proofs: Vec::new(),
        }
    }

    fn two_provider_broadcast() -> (AnimaBroadcast, InpAgentId, InpAgentId) {
        let mut broadcast = AnimaBroadcast::new(InpAgentId::from_u128(1));
        let slow = InpAgentId::from_u128(2);
        let accurate = InpAgentId::from_u128(3);
        broadcast.offer("fetch", slow);
        broadcast.offer("fetch", accurate);
        (broadcast, slow, accurate)
    }

    fn offer_from(provider: InpAgentId, estimated_accuracy: f32) -> Offer {
        Offer {
            provider,
            estimated_cost: 1.0,
            estimated_latency: Duration::from_millis(100),
            estimated_accuracy,
            confidence: 0.9,
        }
    }

    #[test]
    fn the_best_bid_executes_and_a_completed_outcome_is_recorded() {
        let (mut broadcast, slow, accurate) = two_provider_broadcast();
        let offers = [offer_from(slow, 0.6), offer_from(accurate, 0.95)];
        let mut intention = broadcast.open_intention("fetch", &offers).unwrap();
        assert_eq!(intention.provider(), Some(accurate));
        assert!(intention.result().is_none());
        intention
            .record(outcome(ExecutionStatus::Completed))
            .unwrap();
        assert!(matches!(intention.state(), NegotiationState::Completed));
        let result = intention.result().unwrap();
        assert_eq!(result.provider_id, accurate);
        assert_eq!(result.status, ExecutionStatus::Completed);
    }

    #[test]
    fn failed_timed_out_and_cancelled_outcomes_end_the_intention_in_their_own_states() {
        let (mut broadcast, slow, accurate) = two_provider_broadcast();
        let offers = [offer_from(slow, 0.6), offer_from(accurate, 0.95)];
        let mut failed = broadcast.open_intention("fetch", &offers).unwrap();
        let reason = ExecutionStatus::Failed {
            reason: "disk full".into(),
        };
        failed.record(outcome(reason)).unwrap();
        assert!(matches!(
            failed.state(),
            NegotiationState::Failed { reason } if reason == "disk full"
        ));
        assert!(failed.result().is_some(), "the failing outcome is kept");

        let mut timed_out = broadcast.open_intention("fetch", &offers).unwrap();
        timed_out
            .record(outcome(ExecutionStatus::TimedOut))
            .unwrap();
        assert!(matches!(timed_out.state(), NegotiationState::Failed { .. }));

        let mut cancelled = broadcast.open_intention("fetch", &offers).unwrap();
        cancelled
            .record(outcome(ExecutionStatus::Cancelled))
            .unwrap();
        assert!(matches!(cancelled.state(), NegotiationState::Cancelled));
    }

    #[test]
    fn an_intention_takes_one_outcome_and_none_before_execution() {
        let (mut broadcast, slow, accurate) = two_provider_broadcast();
        let offers = [offer_from(slow, 0.6), offer_from(accurate, 0.95)];
        let mut intention = broadcast.open_intention("fetch", &offers).unwrap();
        intention
            .record(outcome(ExecutionStatus::Completed))
            .unwrap();
        assert!(matches!(
            intention.record(outcome(ExecutionStatus::Completed)),
            Err(IntentionError::InvalidState(_))
        ));
        assert!(matches!(
            AnimaBroadcast::new(InpAgentId::from_u128(9))
                .open_intention("fetch", &[])
                .err(),
            Some(IntentionError::NoBids)
        ));
    }

    #[test]
    fn intentions_without_a_registered_bid_are_refused() {
        let (mut broadcast, _, _) = two_provider_broadcast();
        let stranger = offer_from(InpAgentId::from_u128(7), 1.0);
        assert_eq!(
            broadcast.open_intention("fetch", &[stranger]).err(),
            Some(IntentionError::NoBids)
        );
        assert_eq!(
            broadcast.open_intention("launch", &[]).err(),
            Some(IntentionError::NoBids)
        );
    }

    #[test]
    fn negotiation_selects_the_best_registered_bid() {
        let origin = InpAgentId::from_u128(1);
        let slow = InpAgentId::from_u128(2);
        let accurate = InpAgentId::from_u128(3);
        let mut broadcast = AnimaBroadcast::new(origin);
        broadcast.offer("fetch", slow);
        broadcast.offer("fetch", accurate);
        let offer = |provider, estimated_accuracy| Offer {
            provider,
            estimated_cost: 1.0,
            estimated_latency: Duration::from_millis(100),
            estimated_accuracy,
            confidence: 0.9,
        };
        let offers = [offer(slow, 0.6), offer(accurate, 0.95)];
        assert_eq!(broadcast.negotiate("fetch", &offers), Some(accurate));
        assert_eq!(broadcast.broadcaster.active_round_count(), 0);
    }

    #[test]
    fn negotiation_ignores_unregistered_providers_and_closes_empty_rounds() {
        let origin = InpAgentId::from_u128(1);
        let registered = InpAgentId::from_u128(2);
        let stranger = InpAgentId::from_u128(3);
        let mut broadcast = AnimaBroadcast::new(origin);
        broadcast.offer("fetch", registered);
        let stranger_offer = Offer {
            provider: stranger,
            estimated_cost: 0.0,
            estimated_latency: Duration::from_millis(1),
            estimated_accuracy: 1.0,
            confidence: 1.0,
        };
        assert_eq!(broadcast.negotiate("fetch", &[stranger_offer]), None);
        assert_eq!(broadcast.negotiate("fetch", &[]), None);
        assert_eq!(broadcast.negotiate("launch", &[]), None);
        assert_eq!(broadcast.broadcaster.active_round_count(), 0);
    }

    #[test]
    fn resource_snapshot_reports_compute_energy_and_caller_memory() {
        let mut budget = AnimaBudget::new(1_000);
        assert!(budget.admit(ComputeUnit::CPU, 250.0));
        budget.update_power_draw(MAX_POWER_WATTS / 2.0);
        let snapshot = budget.resource_snapshot(0.2);
        assert!((snapshot.compute_free - 0.75).abs() < 1e-9);
        assert!((snapshot.energy_available - 0.5).abs() < 1e-6);
        assert!((snapshot.memory_pressure - 0.2).abs() < 1e-9);
        assert_eq!(budget.resource_snapshot(7.0).memory_pressure, 1.0);
        assert_eq!(AnimaBudget::new(0).resource_snapshot(0.0).compute_free, 0.0);
    }

    #[test]
    fn episodes_round_trip_and_replay_in_recording_order() {
        let mut history = AnimaEpisodes::new();
        let later = Episode::new("later", tpt_thought_core::Timestamp(20));
        let earlier = Episode::new("earlier", tpt_thought_core::Timestamp(10));
        history.record(&later).unwrap();
        history.record(&earlier).unwrap();
        assert_eq!(history.ids(), ["later", "earlier"]);
        assert_eq!(history.load("earlier").unwrap(), Some(earlier));
        assert_eq!(history.load("missing").unwrap(), None);
    }

    #[test]
    fn duplicate_and_pre_epoch_episodes_are_refused() {
        let mut history = AnimaEpisodes::new();
        let episode = Episode::new("e", tpt_thought_core::Timestamp(1));
        history.record(&episode).unwrap();
        assert!(matches!(
            history.record(&episode),
            Err(AnimaEpisodeError::Store(EpisodeStoreError::DuplicateId(_)))
        ));
        let old = Episode::new("old", tpt_thought_core::Timestamp(-1));
        assert!(matches!(
            history.record(&old),
            Err(AnimaEpisodeError::Encode(_))
        ));
        assert_eq!(history.ids(), ["e"], "refused episodes store nothing");
    }

    fn request() -> SleepRequest {
        SleepRequest {
            pending_steps: 10,
            backlog_episodes: 3,
        }
    }

    fn free_resources() -> ResourceSnapshot {
        ResourceSnapshot {
            compute_free: 0.9,
            memory_pressure: 0.1,
            energy_available: 0.9,
        }
    }

    fn budget_at(celsius: f32) -> AnimaBudget {
        let mut budget = AnimaBudget::new(1_000);
        assert_eq!(
            budget.update_temperature(celsius),
            ThermalStatus::from_temperature(celsius, 85.0)
        );
        budget
    }

    #[test]
    fn cool_idle_budget_grants_sleep() {
        let budget = budget_at(30.0);
        assert!(matches!(
            budget.grant(&request(), &free_resources()),
            SleepGrant::Granted(_)
        ));
    }

    #[test]
    fn warm_budget_still_grants_sleep() {
        let budget = budget_at(50.0);
        assert!(matches!(
            budget.grant(&request(), &free_resources()),
            SleepGrant::Granted(_)
        ));
    }

    #[test]
    fn hot_throttled_and_critical_budgets_defer_sleep() {
        for celsius in [70.0, 80.0, 90.0] {
            let budget = budget_at(celsius);
            assert!(
                matches!(
                    budget.grant(&request(), &free_resources()),
                    SleepGrant::Deferred { .. }
                ),
                "sleep granted at {celsius} C"
            );
        }
    }

    #[test]
    fn exhausted_flops_defer_sleep_even_when_cool() {
        let mut budget = budget_at(30.0);
        assert!(budget.admit(ComputeUnit::CPU, 1_000.0));
        assert_eq!(budget.remaining_flops(), 0);
        assert!(matches!(
            budget.grant(&request(), &free_resources()),
            SleepGrant::Deferred { .. }
        ));
    }

    #[test]
    fn caller_resource_thresholds_still_apply_to_a_cool_budget() {
        let budget = budget_at(30.0);
        let starved = ResourceSnapshot {
            compute_free: 0.1,
            ..free_resources()
        };
        assert!(matches!(
            budget.grant(&request(), &starved),
            SleepGrant::Deferred { .. }
        ));
    }
}
