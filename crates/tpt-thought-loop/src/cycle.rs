// SPDX-License-Identifier: MIT OR Apache-2.0
//! The cognitive cycle (spec sections 25, 26, 61, 101).
//!
//! A cycle moves one thought through the lifecycle in `tpt-thought-state`: receive, attend,
//! contextualise, associate, hypothesise, evaluate, accept, intend, act, observe, learn,
//! consolidate, complete. When evaluation cannot accept an answer, the thought goes through the
//! slow path (uncertain, investigating, re-evaluating) before it is accepted or left uncertain.

use crate::action::{Action, ActionError, ActionSink};
use crate::config::LoopConfig;
use crate::curiosity::{CuriosityDriver, CuriosityEvent};
use crate::mode::CognitiveMode;
use crate::plan::{CandidatePlan, Plan, PlanError};
use crate::resources::{Admission, CognitiveRequirement, ResourceGate, Unmetered};
use crate::slow::{Estimator, SlowStage};
use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_consolidation::{
    persist, ConsolidationConfig, ConsolidationPlan, ConsolidationSink, Consolidator,
    IllegalPhaseTransition, Interrupt, PersistedConsolidation, SleepBudget, SleepCycle,
    SleepReport, SystemPhase,
};
use tpt_thought_core::{
    Clock, Confidence, Hypothesis, IdGenerator, Intention, IntentionId, MemoryRef, Observation,
    Prediction, PredictionId, Probability, ProvenanceId, SeededEntropy, SourceTrust,
    StructuredValue, Thought, ThoughtContent, ThoughtId, ThoughtState, Timestamp, TransitionKind,
};
use tpt_thought_deliberation::{
    run_investigation, DeliberationContext, DeliberationEngine, DeliberationError, Termination,
    ThoughtSnapshot,
};
use tpt_thought_hypothesis::{Branch, BranchSet};
use tpt_thought_intention::{
    require_capabilities, IntentionSet, IntentionStatus, InvalidIntention, ManagedIntention,
    SetError,
};
use tpt_thought_observatory::{
    EventKind, Observatory, RecordingSink, TelemetryEvent, TelemetrySink,
};
use tpt_thought_policy::{
    Decision as PolicyDecision, EscalationLevel, PolicyContext, PolicySet, ResourceBudget,
    ResourceUsage,
};
use tpt_thought_prediction::{
    ErrorLevel, MatchConfig, NotComparable, PredictionError, PredictionEvent, PredictionTracker,
    ResolveError, ResponseLadder,
};
use tpt_thought_provenance::{ProvenanceBody, ProvenanceRecord, ProvenanceSource, ProvenanceStore};
use tpt_thought_providers::{
    CalibrationTracker, CognitionMode, Decision, Deliberative, Escalation, FastPath,
    IntelligenceRequest, ProviderRegistry, Step,
};
use tpt_thought_safety::{CircuitBreakers, Trip};
use tpt_thought_state::{apply, IllegalTransition, ThoughtEvent, ThoughtLifecycle};
use tpt_thought_workspace::{MemoryIndex, MemoryStore};

/// How many memories contextualisation asks the store for, per cycle.
const MEMORY_LIMIT: usize = 8;

/// The phases of one cycle (spec section 101).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CyclePhase {
    /// Input arrived and became a thought.
    Observe,
    /// Attention was allocated and context gathered.
    Contextualise,
    /// Memories were associated.
    Associate,
    /// A hypothesis was formed from the question.
    Hypothesise,
    /// The hypothesis was evaluated against providers.
    Evaluate,
    /// An answer was accepted, or the slow path ran.
    Decide,
    /// An intention was formed and a plan chosen.
    Intend,
    /// The action was handed to the executor.
    Act,
    /// The actual outcome was observed.
    ObserveResult,
    /// The outcome was compared with the prediction.
    Learn,
    /// The thought was consolidated and completed.
    Consolidate,
}

/// One step of a run. Steps whose outcome came from a provider or an executor are marked
/// probabilistic, because a non-deterministic backend may answer differently next time (spec
/// section 33).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Position in the trajectory, from zero.
    pub index: usize,
    /// The phase.
    pub phase: CyclePhase,
    /// The thought's state after the phase.
    pub state: ThoughtState,
    /// The cognitive mode in force.
    pub mode: CognitiveMode,
    /// Whether the step is deterministic.
    pub transition: TransitionKind,
    /// A short, deterministic description of what happened.
    pub detail: String,
}

/// A provenance record together with the trajectory step that wrote it. Replay checks these
/// against a re-run, so a record can name the step that produced each piece of cognition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepProvenance {
    /// Index of the trajectory step that wrote the record.
    pub step: usize,
    /// The sealed record, including its integrity hash.
    pub record: ProvenanceRecord,
}

/// What the loop is told about the world for one cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    /// When the input arrived.
    pub at: Timestamp,
    /// The source label, such as a sensor name.
    pub source: String,
    /// What was observed.
    pub observation: StructuredValue,
    /// The question to decide about.
    pub question: String,
    /// The goal the question serves.
    pub goal: String,
    /// Constraints the plan must respect.
    pub constraints: Vec<String>,
    /// Memories already retrieved for this thought (by reference only).
    pub memories: Vec<MemoryRef>,
    /// Candidate plans, with predicted outcomes.
    pub candidates: Vec<CandidatePlan>,
}

/// How a cycle ended.
#[derive(Debug, Clone, PartialEq)]
pub enum CycleOutcome {
    /// The action ran and its outcome was learned from.
    Completed {
        /// The plan that was executed.
        action: String,
        /// Prediction error magnitude, in `0.0..=1.0`.
        error: f64,
        /// The error level it falls into.
        level: ErrorLevel,
    },
    /// The cycle stopped without acting.
    Unresolved {
        /// Why.
        reason: String,
    },
    /// The resource gate refused the cycle, so no provider was asked and nothing ran. The thought
    /// is left uncertain and the cycle may be run again once resources allow it.
    Deferred {
        /// The gate's reason for refusing.
        reason: String,
    },
}

/// The result of one cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct CycleReport {
    /// The thought the cycle worked on.
    pub thought: ThoughtId,
    /// The mode selected from the modulatory state.
    pub mode: CognitiveMode,
    /// How the cycle ended.
    pub outcome: CycleOutcome,
    /// Slow-path stages that ran, in order. Empty if the first pass was accepted.
    pub slow_stages: Vec<SlowStage>,
    /// How the investigation ended, if Eve ran one.
    pub investigation: Option<Termination>,
    /// Curiosity raised by the outcome, if the drive reached the threshold.
    pub curiosity: Option<CuriosityEvent>,
    /// Hypotheses recorded on the thought: the question, plus one per candidate plan.
    pub hypotheses: usize,
    /// Memories the decision was made with: the caller's first, then those retrieved.
    pub memories: Vec<MemoryRef>,
    /// The prediction tracked for the chosen action, if an action was taken.
    pub prediction: Option<PredictionId>,
    /// Provenance records attached to the thought.
    pub provenance: Vec<ProvenanceId>,
}

/// Why a cycle or its setup failed.
#[derive(Debug)]
pub enum LoopError {
    /// The configuration was rejected.
    Config(String),
    /// The plan was not valid.
    Plan(PlanError),
    /// A lifecycle transition was illegal. This is a bug in the loop, not in the input.
    Lifecycle(IllegalTransition),
    /// The deliberation engine failed.
    Deliberation(DeliberationError),
    /// The executor failed.
    Action(ActionError),
    /// The outcome could not be compared with the prediction.
    Learning(NotComparable),
    /// An intention was rejected.
    Intention(String),
    /// A circuit breaker tripped. The loop refuses to continue until it is reset.
    Halted(String),
    /// A hypothesis could not be recorded.
    Hypothesis(String),
    /// A prediction could not be tracked or resolved.
    Prediction(String),
    /// A provenance record could not be written.
    Provenance(String),
}

impl fmt::Display for LoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(reason) => write!(f, "invalid loop configuration: {reason}"),
            Self::Plan(e) => write!(f, "invalid plan: {e}"),
            Self::Lifecycle(e) => write!(f, "lifecycle error: {e}"),
            Self::Deliberation(e) => write!(f, "{e}"),
            Self::Action(e) => write!(f, "{e}"),
            Self::Learning(e) => write!(f, "cannot learn from outcome: {e}"),
            Self::Intention(reason) => write!(f, "intention rejected: {reason}"),
            Self::Halted(trip) => write!(f, "loop halted by circuit breaker: {trip}"),
            Self::Hypothesis(reason) => write!(f, "hypothesis rejected: {reason}"),
            Self::Prediction(reason) => write!(f, "prediction failed: {reason}"),
            Self::Provenance(reason) => write!(f, "provenance rejected: {reason}"),
        }
    }
}

impl std::error::Error for LoopError {}

impl From<PlanError> for LoopError {
    fn from(e: PlanError) -> Self {
        Self::Plan(e)
    }
}

impl From<IllegalTransition> for LoopError {
    fn from(e: IllegalTransition) -> Self {
        Self::Lifecycle(e)
    }
}

impl From<DeliberationError> for LoopError {
    fn from(e: DeliberationError) -> Self {
        Self::Deliberation(e)
    }
}

impl From<ActionError> for LoopError {
    fn from(e: ActionError) -> Self {
        Self::Action(e)
    }
}

impl From<NotComparable> for LoopError {
    fn from(e: NotComparable) -> Self {
        Self::Learning(e)
    }
}

impl From<ResolveError> for LoopError {
    fn from(e: ResolveError) -> Self {
        match e {
            ResolveError::NotComparable(e) => Self::Learning(e),
            other => Self::Prediction(format!("{other:?}")),
        }
    }
}

/// The closed cognitive loop. Generic over the deliberation engine, the estimator and the action
/// executor, so nothing here depends on a concrete sibling implementation.
pub struct CognitiveLoop<E, A, S> {
    config: LoopConfig,
    ids: IdGenerator,
    registry: ProviderRegistry,
    calibration: CalibrationTracker,
    engine: E,
    estimator: A,
    sink: S,
    observatory: Observatory<RecordingSink>,
    breakers: CircuitBreakers,
    intentions: IntentionSet,
    ladder: ResponseLadder,
    curiosity: CuriosityDriver,
    fast: FastPath,
    deliberative: Deliberative,
    memory: Box<dyn MemoryStore>,
    predictions: PredictionTracker,
    provenance: ProvenanceStore,
    provenance_log: Vec<StepProvenance>,
    trajectory: Vec<TrajectoryStep>,
    telemetry: Option<Box<dyn TelemetrySink>>,
    consolidator: Consolidator,
    policy: PolicySet,
}

/// The policy every loop starts with: after a rejected fast answer, always run the full slow path.
/// Build the loop with [`crate::LoopBuilder::policy`] to change that.
pub fn default_policy() -> PolicySet {
    let mut policy = PolicySet::new();
    policy.push(|_: &PolicyContext| {
        PolicyDecision::Escalate(EscalationLevel::MultiHypothesisInvestigation)
    });
    policy
}

/// The result of the slow path.
struct SlowOutcome {
    decision: Option<Decision<StructuredValue>>,
    stages: Vec<SlowStage>,
    investigation: Option<Termination>,
}

impl<E: DeliberationEngine, A: Estimator, S: ActionSink> CognitiveLoop<E, A, S> {
    /// Build a loop. Fails if the configuration's thresholds are out of range.
    pub fn new(
        config: LoopConfig,
        registry: ProviderRegistry,
        engine: E,
        estimator: A,
        sink: S,
    ) -> Result<Self, LoopError> {
        let ladder = ResponseLadder::new(config.ladder[0], config.ladder[1], config.ladder[2])
            .map_err(|e| LoopError::Config(format!("prediction ladder: {e:?}")))?;
        let curiosity = CuriosityDriver::new(config.curiosity_threshold)
            .map_err(|e| LoopError::Config(e.to_string()))?;
        let accept_at = Probability::new(config.accept_at)
            .map_err(|e| LoopError::Config(format!("accept_at: {e:?}")))?;
        Ok(Self {
            ids: IdGenerator::new(SeededEntropy::new(config.seed)),
            calibration: CalibrationTracker::new(config.calibration_min_samples),
            observatory: Observatory::new(RecordingSink::new()),
            breakers: CircuitBreakers::new(config.breaker.clone()),
            intentions: IntentionSet::new(),
            fast: FastPath { accept_at },
            deliberative: Deliberative {
                max_providers: config.deliberative_max_providers,
            },
            ladder,
            curiosity,
            memory: Box::new(MemoryIndex::new()),
            predictions: PredictionTracker::new(MatchConfig::default()),
            provenance: ProvenanceStore::new(),
            provenance_log: Vec::new(),
            trajectory: Vec::new(),
            telemetry: None,
            consolidator: Consolidator::new(ConsolidationConfig::default()),
            policy: default_policy(),
            config,
            registry,
            engine,
            estimator,
            sink,
        })
    }

    /// Use `store` for memory retrieval in every cycle. The default store holds nothing.
    pub fn with_memory(mut self, store: impl MemoryStore + 'static) -> Self {
        self.memory = Box::new(store);
        self
    }

    /// The consolidator that sleep acts on. It starts empty; nothing feeds it automatically yet,
    /// so callers record episodes into it with [`CognitiveLoop::consolidator_mut`].
    pub fn consolidator(&self) -> &Consolidator {
        &self.consolidator
    }

    /// The consolidator, mutably, for recording episodes, beliefs and calibration samples.
    pub fn consolidator_mut(&mut self) -> &mut Consolidator {
        &mut self.consolidator
    }

    /// Run a bounded, interruptible sleep cycle over the loop's consolidator (spec sections 28
    /// and 86 to 89). The loop must be idle; the cycle begins from [`SystemPhase::Idle`].
    ///
    /// `interrupts` is asked before each step with the number of steps run so far, and an
    /// interrupt at or above the budget's priority wakes the system. Steps are applied
    /// atomically, so an interrupt never leaves the consolidator half-updated.
    pub fn sleep(
        &mut self,
        budget: SleepBudget,
        plan: &ConsolidationPlan,
        clock: &dyn Clock,
        interrupts: &mut dyn FnMut(usize) -> Option<Interrupt>,
    ) -> Result<SleepReport, IllegalPhaseTransition> {
        let cycle = SleepCycle::begin(SystemPhase::Idle, budget)?;
        Ok(cycle.run(&mut self.consolidator, plan, clock, interrupts))
    }

    /// Write the durable output of the loop's consolidator through `sink` (todo Phase 6). The
    /// sink decides where it is held; the loop only sees the trait, and gets back references.
    /// Calling it again after a sleep cycle stores nothing twice, because the sink is idempotent.
    pub fn persist_consolidation<K: ConsolidationSink>(
        &self,
        sink: &mut K,
    ) -> Result<PersistedConsolidation, K::Error> {
        persist(&self.consolidator, sink)
    }

    /// As [`CognitiveLoop::with_memory`], for a store already behind a box.
    pub(crate) fn with_boxed_memory(mut self, store: Box<dyn MemoryStore>) -> Self {
        self.memory = store;
        self
    }

    /// Use `policy` to decide, after a rejected fast answer, whether to stop, accept the current
    /// answer, or escalate to a level. See [`PolicyDecision`] for what each means here.
    pub fn with_policy(mut self, policy: PolicySet) -> Self {
        self.policy = policy;
        self
    }

    /// Forward every telemetry event to `sink` as well as to the built-in recorder that backs
    /// [`CognitiveLoop::observatory`]. Emitting never changes cognitive state.
    pub fn with_telemetry(mut self, sink: impl TelemetrySink + 'static) -> Self {
        self.telemetry = Some(Box::new(sink));
        self
    }

    /// The configuration in force.
    pub fn config(&self) -> &LoopConfig {
        &self.config
    }

    /// Predictions still awaiting an observation.
    pub fn predictions(&self) -> &PredictionTracker {
        &self.predictions
    }

    /// Provenance records written so far, one per accepted decision and per learning step.
    pub fn provenance(&self) -> &ProvenanceStore {
        &self.provenance
    }

    /// Every step taken so far, across all cycles.
    pub fn trajectory(&self) -> &[TrajectoryStep] {
        &self.trajectory
    }

    /// Provenance records in the order they were written, each tagged with its trajectory step.
    pub fn provenance_log(&self) -> &[StepProvenance] {
        &self.provenance_log
    }

    /// The telemetry recorded so far.
    pub fn observatory(&self) -> &Observatory<RecordingSink> {
        &self.observatory
    }

    /// Intentions formed so far, with their statuses.
    pub fn intentions(&self) -> &IntentionSet {
        &self.intentions
    }

    /// The executor, for inspection.
    pub fn sink(&self) -> &S {
        &self.sink
    }

    /// Run one full cycle for `input`, with no resource budget. Equivalent to
    /// [`CognitiveLoop::run_gated`] with an empty requirement and [`Unmetered`].
    pub fn run(&mut self, input: &Input) -> Result<CycleReport, LoopError> {
        self.run_gated(input, &CognitiveRequirement::default(), &mut Unmetered)
    }

    /// Run one full cycle for `input`, asking `gate` to admit `requirement` before any provider
    /// is asked (spec sections 35 and 48.3).
    ///
    /// The mode is selected from the modulatory state under the gate's current resource snapshot,
    /// so strain shifts it toward cheaper cognition (see `Modulation::under_pressure`). A denied
    /// requirement returns [`CycleOutcome::Deferred`] with the gate's reason, and nothing runs. An
    /// admitted requirement is released when the cycle ends, however it ends.
    pub fn run_gated(
        &mut self,
        input: &Input,
        requirement: &CognitiveRequirement,
        gate: &mut dyn ResourceGate,
    ) -> Result<CycleReport, LoopError> {
        let at = input.at;
        let mode = self
            .config
            .modulation
            .under_pressure(&gate.snapshot())
            .select_mode();

        // Observe: the input becomes a thought and is received.
        let observation = Observation {
            id: self.ids.observation(),
            content: Box::new(ThoughtContent::Structured(input.observation.clone())),
            source: input.source.clone(),
            trust: SourceTrust::Trusted,
            observed_at: at,
        };
        let mut thought = Thought::from_observation(self.ids.thought(), observation, at);
        thought.receive(at)?;
        self.emit(EventKind::ThoughtCreated, &thought, at);
        self.record(
            &thought,
            CyclePhase::Observe,
            mode,
            TransitionKind::Deterministic,
            format!("source={}", input.source),
        );

        // Contextualise: attend, gather context, associate memories.
        thought.attend(at)?;
        self.emit(EventKind::ThoughtAttended, &thought, at);
        thought.contextualise(at)?;
        self.record(
            &thought,
            CyclePhase::Contextualise,
            mode,
            TransitionKind::Deterministic,
            "attended".into(),
        );
        // The store is asked by the question and the goal; the caller's memories come first.
        let cue: Vec<String> = [input.question.as_str(), input.goal.as_str()]
            .iter()
            .flat_map(|text| text.split(|c: char| !c.is_alphanumeric()))
            .filter(|word| !word.is_empty())
            .map(str::to_lowercase)
            .collect();
        let mut memories = input.memories.clone();
        for found in self.memory.retrieve(&cue, MEMORY_LIMIT) {
            if !memories.contains(&found) {
                memories.push(found);
            }
        }
        thought.associate(memories.clone(), at)?;
        self.record(
            &thought,
            CyclePhase::Associate,
            mode,
            TransitionKind::Deterministic,
            format!("memories={}", memories.len()),
        );

        // Hypothesise: the question itself, plus one branch per candidate plan. Branches are
        // held in a bounded set, so the number of hypotheses per cycle is capped by the plans.
        apply(&mut thought, ThoughtEvent::Hypothesise, at)?;
        let mut statements = vec![input.question.clone()];
        statements.extend(
            input
                .candidates
                .iter()
                .map(|c| format!("plan {} achieves: {}", c.name, input.goal)),
        );
        let prior = Probability::new(1.0 / statements.len() as f64)
            .map_err(|e| LoopError::Hypothesis(format!("prior: {e:?}")))?;
        let mut branches = BranchSet::new(statements.len());
        for statement in statements {
            let id = self.ids.hypothesis();
            thought.hypotheses.push(Hypothesis {
                id,
                statement: statement.clone(),
                probability: Some(prior),
                confidence: Confidence::UNKNOWN,
            });
            let branch = Branch::new(id, statement, prior);
            branches
                .explore(branch)
                .map_err(|e| LoopError::Hypothesis(format!("{e:?}")))?;
        }
        self.emit(EventKind::ThoughtHypothesisCreated, &thought, at);
        self.record(
            &thought,
            CyclePhase::Hypothesise,
            mode,
            TransitionKind::Deterministic,
            format!(
                "question={} branches={}",
                input.question,
                branches.active_count()
            ),
        );
        apply(&mut thought, ThoughtEvent::Evaluate, at)?;
        self.record(
            &thought,
            CyclePhase::Evaluate,
            mode,
            TransitionKind::Deterministic,
            "evaluated".into(),
        );

        // Admission: the gate decides whether this cycle may spend the compute it needs. A denied
        // cycle asks no provider and runs no action, and its reason is reported to the caller.
        if let Admission::Denied { reason } = gate.admit(requirement) {
            apply(&mut thought, ThoughtEvent::MarkUncertain, at)?;
            self.record(
                &thought,
                CyclePhase::Decide,
                mode,
                TransitionKind::Probabilistic,
                format!("deferred: {reason}"),
            );
            return Ok(stopped(
                &thought,
                mode,
                &memories,
                CycleOutcome::Deferred { reason },
            ));
        }
        let _lease = Lease {
            gate,
            requirement: requirement.clone(),
        };

        // Decide: ask providers in the mode selected from the modulatory state.
        let mut request = IntelligenceRequest::new(input.question.clone(), at);
        request.context = vec![
            format!("goal:{}", input.goal),
            format!("mode:{}", mode.name()),
        ];
        request.context.extend(
            memories
                .iter()
                .map(|m| format!("memory:{}:{}", m.store, m.id)),
        );
        let level = match mode {
            CognitiveMode::Reactive | CognitiveMode::Emergency => {
                EscalationLevel::StructuredIntelligence
            }
            _ => EscalationLevel::DeliberateReasoning,
        };
        let step = match mode {
            CognitiveMode::Reactive | CognitiveMode::Emergency => {
                self.fast
                    .respond(&request, &self.registry, &self.calibration)
            }
            _ => self
                .deliberative
                .respond(&request, &self.registry, &self.calibration),
        };
        let (decision, slow_stages, investigation) = match step {
            Step::Accept(decision) => (Some(decision), Vec::new(), None),
            Step::Escalate(escalation) => {
                apply(&mut thought, ThoughtEvent::MarkUncertain, at)?;
                self.emit(EventKind::ThoughtEscalated, &thought, at);
                self.record(
                    &thought,
                    CyclePhase::Decide,
                    mode,
                    TransitionKind::Probabilistic,
                    format!("escalated: {}", escalation.reason),
                );
                let ctx = self.policy_context(level, &escalation);
                match self.policy.decide(&ctx) {
                    PolicyDecision::Escalate(target) => {
                        apply(&mut thought, ThoughtEvent::Investigate, at)?;
                        self.record(
                            &thought,
                            CyclePhase::Decide,
                            mode,
                            TransitionKind::Probabilistic,
                            format!("investigating at {target}"),
                        );
                        let slow = self.slow_path(&thought, &request, target)?;
                        apply(&mut thought, ThoughtEvent::ReEvaluate, at)?;
                        self.record(
                            &thought,
                            CyclePhase::Decide,
                            mode,
                            TransitionKind::Probabilistic,
                            format!("re-evaluating after {} slow stages", slow.stages.len()),
                        );
                        self.breakers
                            .observe_escalation(slow.stages.len() as u32)
                            .map_err(halted)?;
                        (slow.decision, slow.stages, slow.investigation)
                    }
                    PolicyDecision::Accept => {
                        let reason =
                            "policy accepted the uncertain answer; no further cognition".to_owned();
                        self.record(
                            &thought,
                            CyclePhase::Decide,
                            mode,
                            TransitionKind::Probabilistic,
                            reason.clone(),
                        );
                        return Ok(unresolved(&thought, mode, &memories, reason));
                    }
                    PolicyDecision::Terminate(termination) => {
                        let reason = format!("policy stopped cognition: {termination:?}");
                        self.record(
                            &thought,
                            CyclePhase::Decide,
                            mode,
                            TransitionKind::Probabilistic,
                            reason.clone(),
                        );
                        return Ok(unresolved(&thought, mode, &memories, reason));
                    }
                }
            }
        };

        let Some(decision) = decision else {
            apply(&mut thought, ThoughtEvent::MarkUncertain, at)?;
            let reason = "no accepted answer from intellect, eve or augur".to_owned();
            self.record(
                &thought,
                CyclePhase::Decide,
                mode,
                TransitionKind::Probabilistic,
                reason.clone(),
            );
            return Ok(CycleReport {
                thought: thought.id,
                mode,
                outcome: CycleOutcome::Unresolved { reason },
                slow_stages,
                investigation,
                curiosity: None,
                hypotheses: thought.hypotheses.len(),
                memories: memories.clone(),
                prediction: None,
                provenance: thought.provenance.records.clone(),
            });
        };
        apply(&mut thought, ThoughtEvent::Accept, at)?;
        self.emit(EventKind::ThoughtDecisionReceived, &thought, at);
        self.record(
            &thought,
            CyclePhase::Decide,
            mode,
            TransitionKind::Probabilistic,
            format!("accepted from {}", decision.source.provider_id().as_str()),
        );
        let decision_record = self.write_provenance(
            &mut thought.provenance.records,
            ProvenanceBody {
                id: self.ids.provenance(),
                source: ProvenanceSource::Decision,
                thought: Some(thought.id),
                parents: Vec::new(),
                evidence: Vec::new(),
                memory: memories.clone(),
                provider: Some(decision.source.provider_id().as_str().to_owned()),
                model: None,
                version: None,
                reasoning_path: vec![format!("mode:{}", mode.name())],
                verification: None,
                timestamp: at,
            },
        )?;

        // Intend: choose a plan and form an intention.
        let plan = Plan::new(
            input.goal.clone(),
            input.observation.clone(),
            input.candidates.clone(),
            input.constraints.clone(),
            self.config.max_risk,
        )?;
        let answered = match decision.value() {
            Some(StructuredValue::Text(name)) => plan.named(name),
            _ => None,
        }
        .filter(|c| c.risk <= plan.max_risk());
        let Some(chosen) = answered.or_else(|| plan.best()).cloned() else {
            let reason = "no candidate plan within the risk limit".to_owned();
            return Ok(CycleReport {
                thought: thought.id,
                mode,
                outcome: CycleOutcome::Unresolved { reason },
                slow_stages,
                investigation,
                curiosity: None,
                hypotheses: thought.hypotheses.len(),
                memories: memories.clone(),
                prediction: None,
                provenance: thought.provenance.records.clone(),
            });
        };
        let intention = Intention {
            id: self.ids.intention(),
            goal: input.goal.clone(),
            desired_outcome: Some(chosen.name.clone()),
            constraints: input.constraints.clone(),
            risk_tolerance: Some(
                Probability::new(self.config.max_risk)
                    .map_err(|e| LoopError::Config(format!("max_risk: {e:?}")))?,
            ),
            priority: 1.0,
            required_capabilities: Vec::new(),
            deadline: None,
        };
        require_capabilities(&intention, std::iter::empty())
            .map_err(|missing| LoopError::Intention(format!("{missing:?}")))?;
        let intention_id: IntentionId = intention.id;
        let managed = ManagedIntention::new(intention.clone(), Vec::new())
            .map_err(|e: InvalidIntention| LoopError::Intention(format!("{e:?}")))?;
        self.intentions
            .insert(managed)
            .map_err(|e: SetError| LoopError::Intention(format!("{e:?}")))?;
        thought.intentions.push(intention);
        apply(&mut thought, ThoughtEvent::Intend, at)?;
        self.emit(EventKind::ThoughtIntentionCreated, &thought, at);
        self.record(
            &thought,
            CyclePhase::Intend,
            mode,
            TransitionKind::Deterministic,
            format!("plan={}", chosen.name),
        );

        // Act: the executor performs the action; Thought only records the expectation.
        apply(&mut thought, ThoughtEvent::Act, at)?;
        let prediction = Prediction {
            id: self.ids.prediction(),
            predicted_state: chosen.predicted_outcome.clone(),
            expected_at: Some(at),
            confidence: Confidence::UNKNOWN,
            assumptions: vec![format!("plan {} achieves the goal", chosen.name)],
            source_thought: thought.id,
        };
        thought.predictions.push(prediction.clone());
        self.predictions
            .track(prediction.clone(), at)
            .map_err(|e| LoopError::Prediction(e.to_string()))?;
        self.emit(EventKind::ThoughtPredictionCreated, &thought, at);
        self.intentions
            .advance(&intention_id, IntentionStatus::Dispatched)
            .map_err(|e| LoopError::Intention(format!("{e:?}")))?;
        self.emit(EventKind::ThoughtActionStarted, &thought, at);
        self.intentions
            .advance(&intention_id, IntentionStatus::Executing)
            .map_err(|e| LoopError::Intention(format!("{e:?}")))?;
        let action = Action {
            intention: intention_id,
            name: chosen.name.clone(),
            expected: chosen.predicted_outcome.clone(),
        };
        let actual = match self.sink.execute(&action) {
            Ok(actual) => actual,
            Err(error) => {
                self.intentions
                    .advance(&intention_id, IntentionStatus::Failed)
                    .map_err(|e| LoopError::Intention(format!("{e:?}")))?;
                return Err(LoopError::Action(error));
            }
        };
        self.emit(EventKind::ThoughtActionCompleted, &thought, at);
        self.record(
            &thought,
            CyclePhase::Act,
            mode,
            TransitionKind::Probabilistic,
            format!("action={}", chosen.name),
        );

        // Observe the result.
        apply(&mut thought, ThoughtEvent::Observe, at)?;
        let outcome = Observation {
            id: self.ids.observation(),
            content: Box::new(ThoughtContent::Structured(actual)),
            source: "action".into(),
            trust: SourceTrust::Trusted,
            observed_at: at,
        };
        thought.observations.push(outcome.id);
        self.record(
            &thought,
            CyclePhase::ObserveResult,
            mode,
            TransitionKind::Deterministic,
            "result observed".into(),
        );

        // Learn: the tracker resolves the prediction against the outcome, and the error is
        // responded to.
        let error: PredictionError = match self.predictions.resolve(prediction.id, &outcome)? {
            PredictionEvent::Resolved(error) => error,
            PredictionEvent::Expired { .. } => {
                return Err(LoopError::Prediction(
                    "prediction expired before resolution".into(),
                ));
            }
        };
        let magnitude = error.magnitude();
        let level = self.ladder.classify(magnitude);
        let plan = self.ladder.plan(&error);
        apply(&mut thought, ThoughtEvent::Learn, at)?;
        self.emit(EventKind::ThoughtPredictionError, &thought, at);
        self.record(
            &thought,
            CyclePhase::Learn,
            CognitiveMode::Learning,
            TransitionKind::Deterministic,
            format!(
                "error={magnitude:.3} level={level:?} actions={:?}",
                plan.actions
            ),
        );
        self.write_provenance(
            &mut thought.provenance.records,
            ProvenanceBody {
                id: self.ids.provenance(),
                source: ProvenanceSource::Inference,
                thought: Some(thought.id),
                parents: vec![decision_record],
                evidence: Vec::new(),
                memory: Vec::new(),
                provider: None,
                model: None,
                version: None,
                reasoning_path: vec![format!("prediction error {magnitude:.3}")],
                verification: None,
                timestamp: at,
            },
        )?;
        let curiosity = self.curiosity.check(
            self.config.modulation.fields().novelty,
            magnitude,
            &input.question,
        );

        // Consolidate and complete.
        apply(&mut thought, ThoughtEvent::Consolidate, at)?;
        self.emit(EventKind::ThoughtConsolidated, &thought, at);
        apply(&mut thought, ThoughtEvent::Complete, at)?;
        self.record(
            &thought,
            CyclePhase::Consolidate,
            CognitiveMode::Consolidation,
            TransitionKind::Deterministic,
            "consolidated and complete".into(),
        );
        self.intentions
            .advance(&intention_id, IntentionStatus::Achieved)
            .map_err(|e| LoopError::Intention(format!("{e:?}")))?;
        self.breakers.check().map_err(halted)?;

        Ok(CycleReport {
            thought: thought.id,
            mode,
            outcome: CycleOutcome::Completed {
                action: chosen.name,
                error: magnitude,
                level,
            },
            slow_stages,
            investigation,
            curiosity,
            hypotheses: thought.hypotheses.len(),
            memories,
            prediction: Some(prediction.id),
            provenance: thought.provenance.records.clone(),
        })
    }

    /// The slow path (spec section 61), in order: Intellect was already asked and escalated;
    /// Eve investigates; Augur estimates; Intellect is asked again with the estimate in context.
    /// Eve is recorded only if the engine supports investigation. Augur is always asked.
    /// The slow stages a level allows. Intellect always runs. Investigation (Eve) needs level L4
    /// or higher, and the estimate (Augur) needs L3 or higher.
    fn slow_path(
        &mut self,
        thought: &Thought,
        request: &IntelligenceRequest,
        level: EscalationLevel,
    ) -> Result<SlowOutcome, LoopError> {
        let mut stages = vec![SlowStage::Intellect];
        let mut investigation = None;
        if level >= EscalationLevel::DeliberateReasoning {
            let ctx = DeliberationContext {
                thought: ThoughtSnapshot::from_thought(thought),
                budget: self.config.budget,
            };
            match run_investigation(
                &mut self.engine,
                &ctx,
                &request.question,
                &self.config.investigation,
            ) {
                Ok(run) => {
                    stages.push(SlowStage::Eve);
                    investigation = Some(run.termination);
                }
                Err(DeliberationError::Unsupported(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }

        let estimate = if level >= EscalationLevel::ProbabilisticInference {
            stages.push(SlowStage::Augur);
            self.estimator.estimate(&request.question)
        } else {
            None
        };

        let mut reevaluation = request.clone();
        if let Some(estimate) = estimate {
            reevaluation.context.push(format!(
                "estimate:{} sd:{}",
                estimate.mean, estimate.residual_uncertainty
            ));
        }
        let decision =
            match self
                .deliberative
                .respond(&reevaluation, &self.registry, &self.calibration)
            {
                Step::Accept(decision) => Some(decision),
                Step::Escalate(_) => None,
            };
        stages.push(SlowStage::IntellectReevaluation);
        Ok(SlowOutcome {
            decision,
            stages,
            investigation,
        })
    }

    /// The policy's view of an escalation. Risk, branches and value are not known at this point
    /// in the cycle, so they are zero or absent, and a policy must not treat them as measured.
    fn policy_context(&self, level: EscalationLevel, escalation: &Escalation) -> PolicyContext {
        let confidence = escalation
            .decisions
            .iter()
            .filter_map(|d| d.confidence.best())
            .map(|p| p.get())
            .fold(0.0, f64::max);
        PolicyContext {
            level,
            confidence,
            risk: 0.0,
            branches: 0,
            failed_escalations: 0,
            value: None,
            usage: ResourceUsage::default(),
            budget: ResourceBudget::from(self.config.budget),
        }
    }

    fn record(
        &mut self,
        thought: &Thought,
        phase: CyclePhase,
        mode: CognitiveMode,
        transition: TransitionKind,
        detail: String,
    ) {
        let index = self.trajectory.len();
        self.trajectory.push(TrajectoryStep {
            index,
            phase,
            state: thought.state,
            mode,
            transition,
            detail,
        });
    }

    fn emit(&mut self, kind: EventKind, thought: &Thought, at: Timestamp) {
        let event = TelemetryEvent::new(kind, at)
            .for_thought(thought.id)
            .with_state(thought.state);
        self.observatory.emit(&event);
        if let Some(sink) = self.telemetry.as_mut() {
            sink.emit(&event);
        }
    }

    /// Store `body` as a provenance record and attach its id to the thought's `records`. Both
    /// callers record their trajectory step immediately before writing, so the latest step is
    /// the one that produced the record.
    fn write_provenance(
        &mut self,
        records: &mut Vec<ProvenanceId>,
        body: ProvenanceBody,
    ) -> Result<ProvenanceId, LoopError> {
        let record = self
            .provenance
            .insert(body)
            .map_err(|e| LoopError::Provenance(format!("{e:?}")))?
            .clone();
        let id = record.body.id;
        self.provenance_log.push(StepProvenance {
            step: self.trajectory.len().saturating_sub(1),
            record,
        });
        records.push(id);
        Ok(id)
    }
}

/// The report for a cycle that ended without an answer before any slow stage ran.
fn unresolved(
    thought: &Thought,
    mode: CognitiveMode,
    memories: &[MemoryRef],
    reason: String,
) -> CycleReport {
    stopped(thought, mode, memories, CycleOutcome::Unresolved { reason })
}

/// The report for a cycle that stopped before any slow stage ran, with the given outcome.
fn stopped(
    thought: &Thought,
    mode: CognitiveMode,
    memories: &[MemoryRef],
    outcome: CycleOutcome,
) -> CycleReport {
    CycleReport {
        thought: thought.id,
        mode,
        outcome,
        slow_stages: Vec::new(),
        investigation: None,
        curiosity: None,
        hypotheses: thought.hypotheses.len(),
        memories: memories.to_vec(),
        prediction: None,
        provenance: thought.provenance.records.clone(),
    }
}

fn halted(trip: Trip) -> LoopError {
    LoopError::Halted(format!("{trip:?}"))
}

/// An admitted requirement, held for the rest of a cycle. Dropping it releases the requirement, so
/// the compute returns to the gate on every exit path, including errors.
struct Lease<'a> {
    gate: &'a mut dyn ResourceGate,
    requirement: CognitiveRequirement,
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.gate.release(&self.requirement);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_consolidation::SleepOutcome;

    #[test]
    fn errors_have_readable_messages() {
        let e = LoopError::Halted("loop".into());
        assert!(e.to_string().contains("halted"));
    }

    fn budget() -> SleepBudget {
        SleepBudget {
            max_steps: 100,
            max_duration_nanos: i64::MAX,
            wake_priority: 0.9,
        }
    }

    #[test]
    fn idle_loop_completes_a_standard_sleep_cycle() {
        let mut lp = CognitiveLoop::builder().build().expect("default loop");
        let clock = tpt_thought_core::ManualClock::new(Timestamp(0));
        let plan = ConsolidationPlan::standard();
        let report = lp
            .sleep(budget(), &plan, &clock, &mut |_| None)
            .expect("sleep from idle");
        assert!(matches!(report.outcome, SleepOutcome::Completed { .. }));
    }

    #[test]
    fn high_priority_interrupt_wakes_the_loop() {
        let mut lp = CognitiveLoop::builder().build().expect("default loop");
        let clock = tpt_thought_core::ManualClock::new(Timestamp(0));
        let plan = ConsolidationPlan::standard();
        let mut wake = |_: usize| {
            Some(Interrupt {
                priority: 1.0,
                reason: "user input".into(),
            })
        };
        let report = lp
            .sleep(budget(), &plan, &clock, &mut wake)
            .expect("sleep from idle");
        assert!(matches!(report.outcome, SleepOutcome::Interrupted { .. }));
    }
}
