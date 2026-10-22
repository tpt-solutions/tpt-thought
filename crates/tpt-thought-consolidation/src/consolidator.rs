// SPDX-License-Identifier: MIT OR Apache-2.0
//! The consolidator: holds everything a sleep cycle reads and writes, and applies one
//! [`ConsolidationStep`] at a time.

use crate::association::{AssociationStore, Link, ReweightPolicy};
use crate::belief::{BeliefConfig, BeliefRecord, BeliefStatus, Calibration, CalibrationSample};
use crate::gaps::{ExperimentProposal, GapTracker};
use crate::history::{HistoryEntry, ThoughtHistory};
use crate::memory::{MemoryItem, MemoryReorganisation};
use crate::patterns::PatternConfig;
use crate::patterns::{abstract_knowledge, detect, Attempt, Deliberation, Detection, Knowledge};
use crate::plan::ConsolidationStep;
use crate::replay::{Episode, ThoughtReplay};
use crate::state::{NextSessionState, SalienceTable};
use crate::unit;
use std::collections::BTreeMap;
use tpt_thought_core::{ContentReference, EpisodeId, ProvenanceId, Timestamp};

/// Every threshold a consolidation pass uses.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsolidationConfig {
    /// Relevance and salience boost per replayed episode, scaled by its importance.
    pub replay_boost: f64,
    /// Memories with relevance below this are pruned unless durable.
    pub min_relevance: f64,
    /// Memories whose Anima-supplied strength falls below this are expired.
    pub strength_floor: f64,
    /// Association reweighting.
    pub reweight: ReweightPolicy,
    /// Recent history entries kept uncompressed.
    pub keep_recent_history: usize,
    /// Fraction of each salience weight kept per pass.
    pub priority_decay: f64,
    /// Occurrences before an open question counts as a persistent gap.
    pub gap_min_occurrences: u32,
    /// Belief promotion and demotion.
    pub belief: BeliefConfig,
    /// Pattern detection.
    pub patterns: PatternConfig,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            replay_boost: 0.1,
            min_relevance: 0.2,
            strength_floor: 0.05,
            reweight: ReweightPolicy::default(),
            keep_recent_history: 10,
            priority_decay: 0.9,
            gap_min_occurrences: 2,
            belief: BeliefConfig::default(),
            patterns: PatternConfig::default(),
        }
    }
}

/// The state a sleep cycle operates on.
#[derive(Debug, Clone)]
pub struct Consolidator {
    config: ConsolidationConfig,
    replay: ThoughtReplay,
    replayed: Vec<EpisodeId>,
    episodes: Vec<Episode>,
    memories: MemoryReorganisation,
    associations: AssociationStore,
    attempts: Vec<Attempt>,
    deliberations: Vec<Deliberation>,
    detection: Detection,
    knowledge: Vec<Knowledge>,
    beliefs: BTreeMap<String, BeliefRecord>,
    belief_transitions: Vec<(String, BeliefStatus)>,
    calibration_samples: Vec<CalibrationSample>,
    calibration: Calibration,
    gaps: GapTracker,
    experiments: Vec<ExperimentProposal>,
    history: ThoughtHistory,
    salience: SalienceTable,
    forgotten: Vec<ContentReference>,
}

impl Consolidator {
    /// An empty consolidator with `config`.
    pub fn new(config: ConsolidationConfig) -> Self {
        Self {
            config,
            replay: ThoughtReplay::default(),
            replayed: Vec::new(),
            episodes: Vec::new(),
            memories: MemoryReorganisation::default(),
            associations: AssociationStore::default(),
            attempts: Vec::new(),
            deliberations: Vec::new(),
            detection: Detection::default(),
            knowledge: Vec::new(),
            beliefs: BTreeMap::new(),
            belief_transitions: Vec::new(),
            calibration_samples: Vec::new(),
            calibration: Calibration::default(),
            gaps: GapTracker::default(),
            experiments: Vec::new(),
            history: ThoughtHistory::default(),
            salience: SalienceTable::default(),
            forgotten: Vec::new(),
        }
    }

    /// Queue episodes for replay.
    pub fn observe_episodes(&mut self, episodes: Vec<Episode>) {
        self.episodes.extend(episodes.iter().cloned());
        self.replay.extend(episodes);
    }

    /// Record a strategy attempt.
    pub fn record_attempt(&mut self, attempt: Attempt) {
        self.attempts.push(attempt);
    }

    /// Record a deliberation.
    pub fn record_deliberation(&mut self, deliberation: Deliberation) {
        self.deliberations.push(deliberation);
    }

    /// Record evidence for or against a belief.
    pub fn record_belief_evidence(
        &mut self,
        claim: &str,
        supports: bool,
        provenance: &[ProvenanceId],
    ) {
        self.beliefs
            .entry(claim.to_owned())
            .or_insert_with(|| BeliefRecord::new(claim))
            .observe(supports, provenance);
    }

    /// Record a calibration sample.
    pub fn record_calibration(&mut self, sample: CalibrationSample) {
        self.calibration_samples.push(sample);
    }

    /// Record a thought in history.
    pub fn record_thought(&mut self, entry: HistoryEntry) {
        self.history.push(entry);
    }

    /// Track a memory for merging, expiry and pruning.
    pub fn track_memory(&mut self, item: MemoryItem) {
        self.memories.insert(item);
    }

    /// Record an association's supporting evidence.
    pub fn support_association(
        &mut self,
        link: Link<'_>,
        now: Timestamp,
    ) -> Result<(), crate::AssociationError> {
        self.associations.support(link, now)
    }

    /// Apply one step. Each step runs to completion, so the state is consistent between any
    /// two steps.
    pub fn apply(&mut self, step: ConsolidationStep, now: Timestamp) {
        match step {
            ConsolidationStep::Replay => self.replay_queue(),
            ConsolidationStep::DetectPatterns => {
                self.detection = detect(&self.attempts, &self.deliberations, &self.config.patterns);
            }
            ConsolidationStep::MergeMemories => {
                self.memories.merge_redundant();
            }
            ConsolidationStep::PruneForgotten => {
                self.memories.expire(self.config.strength_floor);
                let pruned = self.memories.prune(self.config.min_relevance);
                self.forgotten.extend(pruned);
            }
            ConsolidationStep::ReweightAssociations => {
                self.associations.reweight(now, &self.config.reweight);
            }
            ConsolidationStep::Abstract => {
                let causal = self.associations.causal();
                self.knowledge = abstract_knowledge(&self.detection, &self.episodes, &causal);
            }
            ConsolidationStep::ReevaluateBeliefs => {
                for (claim, belief) in &mut self.beliefs {
                    if let Some(status) = belief.reevaluate(&self.config.belief) {
                        self.belief_transitions.push((claim.clone(), status));
                    }
                }
            }
            ConsolidationStep::RecordGaps => {
                self.experiments = self.gaps.propose(self.config.gap_min_occurrences);
            }
            ConsolidationStep::Recalibrate => {
                self.calibration = Calibration::fit(&self.calibration_samples);
            }
            ConsolidationStep::CompressHistory => {
                self.history.compress(self.config.keep_recent_history);
            }
            ConsolidationStep::UpdatePriorities => {
                self.salience.decay(self.config.priority_decay);
            }
        }
    }

    fn replay_queue(&mut self) {
        while let Some(episode) = self.replay.pop() {
            let importance = episode.importance();
            let boost = self.config.replay_boost * importance;
            self.salience.boost(&episode.topic, boost);
            self.memories.reinforce(&episode.topic, boost);
            if let Some(question) = &episode.open_question {
                self.gaps.record(question, &episode.provenance);
            }
            self.replayed.push(episode.id);
        }
    }

    /// The cognitive state to hand to the next session.
    pub fn next_state(&self) -> NextSessionState {
        NextSessionState {
            priorities: self.salience.ranked(),
            open_gaps: self
                .gaps
                .persistent(self.config.gap_min_occurrences)
                .into_iter()
                .map(|g| g.question.clone())
                .collect(),
            experiments: self.experiments.clone(),
            skills: self.detection.skills.clone(),
            knowledge: self.knowledge.clone(),
            beliefs: self
                .beliefs
                .values()
                .map(|b| (b.key.clone(), b.status))
                .collect(),
            calibration: self.calibration.clone(),
        }
    }

    /// Episodes replayed so far, in replay order.
    pub fn replayed(&self) -> &[EpisodeId] {
        &self.replayed
    }

    /// Episodes still waiting for replay.
    pub fn pending_replay(&self) -> usize {
        self.replay.len()
    }

    /// Tracked memories.
    pub fn memories(&self) -> &MemoryReorganisation {
        &self.memories
    }

    /// Memories pruned so far; Anima can release their stored content.
    pub fn forgotten(&self) -> &[ContentReference] {
        &self.forgotten
    }

    /// Associations.
    pub fn associations(&self) -> &AssociationStore {
        &self.associations
    }

    /// The latest pattern detection.
    pub fn detection(&self) -> &Detection {
        &self.detection
    }

    /// The latest abstracted knowledge.
    pub fn knowledge(&self) -> &[Knowledge] {
        &self.knowledge
    }

    /// A belief by claim.
    pub fn belief(&self, claim: &str) -> Option<&BeliefRecord> {
        self.beliefs.get(claim)
    }

    /// Belief status changes, in the order they happened.
    pub fn belief_transitions(&self) -> &[(String, BeliefStatus)] {
        &self.belief_transitions
    }

    /// The latest calibration.
    pub fn calibration(&self) -> &Calibration {
        &self.calibration
    }

    /// The latest experiment proposals.
    pub fn experiments(&self) -> &[ExperimentProposal] {
        &self.experiments
    }

    /// Thought history.
    pub fn history(&self) -> &ThoughtHistory {
        &self.history
    }

    /// Salience weights.
    pub fn salience(&self) -> &SalienceTable {
        &self.salience
    }

    /// Normalise a raw confidence with the latest calibration.
    pub fn calibrated(&self, raw: f64) -> f64 {
        unit(self.calibration.adjust(raw))
    }
}
