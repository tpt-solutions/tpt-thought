# TPT Thought — Project Checklist

Organisation: TPT Solutions · Language: Rust · Licence: MIT OR Apache-2.0
Spec: `spec.txt` (section numbers below, e.g. §7, refer to it).

Ground rules (apply to every phase)
- [ ] Create a crate only when a real API/dependency boundary exists (§100)
- [ ] No Apache-only dependencies (MIT, MIT/Apache dual, BSD, ISC, Zlib, Unlicense, CC0, Unicode are OK)
- [ ] Libraries are runtime-agnostic (native `async fn` in traits, no tokio in libraries)
- [ ] `tpt-thought-core` has no TPT runtime dependencies and no backend dependencies (§102)
- [ ] Never branch on a specific backend (`if jev`); everything goes through provider traits (§6, §57)
- [ ] Sibling-repo dependencies (github.com/TPT-Solutions/<repo>) only inside `tpt-thought-integrations`
- [ ] Every phase ends with tests green, docs updated and `cargo deny check` passing

---

## Phase A — Repository Setup & Licensing
- [x] `git init`, `.gitignore`, default branch, initial commit
- [x] Cargo workspace `Cargo.toml` (resolver 2, shared package metadata, edition, MSRV)
- [x] Add `LICENSE-MIT` (copyright TPT Solutions)
- [x] Add `LICENSE-APACHE` (Apache-2.0 text)
- [x] Set `license = "MIT OR Apache-2.0"` and `authors/repository` for every crate
- [x] SPDX header (`MIT OR Apache-2.0`) convention for source files
- [x] `cargo-deny` config (`deny.toml`): allow-list of MIT-compatible licences, deny Apache-2.0-only, ban vendor SDK crates (§57)
- [x] `rustfmt.toml`, `clippy` settings (`-D warnings`), `#![forbid(unsafe_code)]` policy
- [x] CI workflow written (not yet run on GitHub): build, test, clippy, fmt, cargo-deny, docs build (Windows + Linux)
- [ ] Decide git auth for sibling-repo git deps in CI
- [x] `README.md` (what Thought is, and what it is not, §1)
- [x] `CONTRIBUTING.md` (contributions licensed MIT OR Apache-2.0)
- [x] `docs/` skeleton: ARCHITECTURE, COGNITIVE_MODEL, INTEGRATION_ANIMA, INTEGRATION_EVE, INTEGRATION_INTELLECT, PROVIDERS, BENCHMARKS
- [x] `examples/` and `benches/` directories
- [x] Facade crate `tpt-thought` re-exporting public API
- [x] Dependency policy doc: what core may and may not depend on

## Phase 0 — Foundation (core types, state machine, IDs, serialization, provenance)
- [x] Create `tpt-thought-core`
- [x] IDs: `ThoughtId`, `EpisodeId`, `HypothesisId`, `PredictionId`, `IntentionId`, `OutcomeId` (globally unique, cryptographically strong, traceable, §73)
- [x] ID generation without a mandatory runtime (injectable entropy source for deterministic tests)
- [x] Temporal types: `created_at`, `updated_at`, `observed_at`, `predicted_at`, `expected_at`, `resolved_at`; events as first-class (§74)
- [x] Injectable clock for deterministic replay
- [x] `Thought` struct (§8), kept loosely coupled (not mandatory final shape)
- [x] `ThoughtOrigin`, `ThoughtContext`, `ThoughtOwnership` (private/shared/delegated/published/archived, §40)
- [x] `ThoughtContent` enum: Text, Structured, Observation, Hypothesis, Belief, Prediction, Question, Intention, Decision, Multimodal, Reference (§9)
- [x] `Observation`, `Belief`, `Question`, `Decision` shell types
- [x] `Confidence` types: raw, calibrated, empirical reliability (§67)
- [x] `Uncertainty` types: confidence, probability, epistemic, aleatoric, unknown, contradiction, abstention (§30)
- [x] Epistemic status markers: informal, probabilistic, verified, proven (§50)
- [x] `ValueAssessment` (§22) with extension point for future `tpt-value`
- [x] `CognitiveBudget` struct (§63)
- [x] Transition kind marker: deterministic vs probabilistic (§33)
- [x] Randomness metadata: seed, source, distribution, sampling method (§34)
- [x] Create `tpt-thought-state`: lifecycle enum (CREATED … COMPLETE, §7) with loops permitted
- [x] Transition table + validation (illegal transitions rejected)
- [x] Deterministic transition function (same input, same transition)
- [x] serde support for all core types
- [x] JSON encoding
- [x] CBOR encoding (deterministic form)
- [x] Round-trip and schema-stability tests, with a versioned wire format
- [x] Create `tpt-thought-provenance`: machine-readable provenance record (source, parent, evidence, memory, provider, model, version, reasoning path, verification, timestamp, §70)
- [x] Provenance chain/tree query API ("why did the agent think this?", §31)
- [x] Integrity metadata (hashes) on provenance records
- [x] Property tests: no invalid state transitions
- [x] Docs: COGNITIVE_MODEL.md first draft

## Phase 1 — Thought Lifecycle
- [ ] Thought lifecycle API: create, attend, contextualise, associate, complete (§121)
- [ ] `Thought::from_observation`
- [ ] Source classification of inputs (trusted/untrusted, §41)
- [ ] Create `tpt-thought-attention`
- [ ] `AttentionState` with multi-factor model: salience, novelty, urgency, goal relevance, risk, uncertainty, information gain, value relevance, recency, resource cost (§11)
- [ ] Configurable attention formula (pluggable scoring)
- [ ] Cognitive demand output (priority, deadline, estimated cost, capabilities, risk, info gain) for Anima CK (§35)
- [ ] Working memory (§13): strict capacity, attention weighting, decay, eviction, priority, provenance
- [ ] Forgetting states: temporary, weak, strong, consolidated, expired, pruned (§28)
- [ ] Create `tpt-thought-graph`
- [ ] Typed edges: supports, contradicts, causes, derived-from, generalises, specialises, triggers, analogous-to (§10, §77)
- [ ] Association (↔) and causation (→) as distinct edge types (§76); preceded-by vs caused (§75)
- [ ] Causal provenance stored on causal edges
- [ ] Graph queries: ancestors, descendants, contradictions, evidence paths
- [ ] Graph references Anima memory by ID only (no duplicated storage, §10)
- [ ] Thought recursion limits: max_depth, max_children, max_total_compute, max_duration (§85)
- [ ] Tests: lifecycle, attention scoring, working-memory eviction, graph invariants

## Phase 2 — Hypotheses
- [ ] Create `tpt-thought-hypothesis`
- [ ] Hypothesis lifecycle: GENERATED, UNTESTED, SUPPORTED/WEAKENED, TESTING, UPDATED, ACCEPTED/REJECTED/SUSPENDED (§16)
- [ ] Branching with per-branch probability, confidence, evidence, cost, risk, expected information gain (§15)
- [ ] Branch operations: explore, prune, merge, suspend, promote, reject
- [ ] Thought collapse by probability, utility, evidence, formal verification, ensemble agreement, human choice (§64)
- [ ] Belief promotion pipeline: thought, hypothesis, supported hypothesis, belief candidate, belief (§43)
- [ ] Configurable promotion criteria (repeated observation, proof, experiment, high-confidence intelligence, model agreement, human confirmation)
- [ ] Belief demotion pipeline (§44)
- [ ] Contradiction representation and handling: attention up, investigate, resolve by new evidence / context separation / conditional validity (§29)
- [ ] Rule: a thought is not a belief merely because it was considered
- [ ] Information-gain primitive (§37) and investigate-if gain exceeds cost context
- [ ] Tests: branch explosion bounds, promotion/demotion round trips, contradiction cases

## Phase 3 — Prediction
- [ ] Create `tpt-thought-prediction`
- [ ] `Prediction` primitive (predicted state, expected time, confidence, assumptions, source thought, §17)
- [ ] Match predictions against later observations
- [ ] Prediction error type and error-as-event (§18)
- [ ] Configurable error response ladder: small (continue), moderate (update belief), large (raise attention, investigate), catastrophic (suspend model, re-evaluate assumptions)
- [ ] Prediction-error fan-out hooks (memory reinforcement, active learning, belief updates, confidence, attention, planning)
- [ ] Tests: prediction, observation, error, response thresholds

## Phase 4 — Intellect Integration
- [ ] Define provider-neutral traits in Thought (e.g. `IntelligenceProvider`) so core never depends on Intellect
- [ ] Create `tpt-thought-policy`
- [ ] Policies: escalation, branching, resource use, risk, human escalation, termination
- [ ] Escalation levels L0 to L6 (reflex to human), configurable (§20)
- [ ] Expected value of additional cognition drives escalation (§36)
- [ ] Cognitive Budget enforcement (time, compute, energy, money, memory, branches, model calls, experiments, §63)
- [ ] Create `tpt-thought-integrations`
- [ ] Review the `tpt-intellect` public API (github.com/TPT-Solutions/tpt-intellect)
- [ ] `IntellectAdapter`: map Thought questions to `IntelligenceRequest`, `Decision<T>`
- [ ] Request constraints: required confidence, max cost, max latency, risk tolerance, acceptable providers, required capabilities (§55)
- [ ] Handle abstention and provenance from Intellect decisions
- [ ] Consume calibration metadata (raw vs calibrated vs empirical, §67)
- [ ] Capability / provider discovery queries (§58, §91)
- [ ] Deterministic reference backend for tests (no external model)
- [ ] Fast-path implementation (accept at high confidence, no unnecessary escalation, §60)
- [ ] Fast and deliberative cognition as pluggable modes (not hard-coded System 1/2, §21)
- [ ] Tests: fast path, abstention, escalation trigger, provider failure

## Phase 5 — Eve Integration
- [ ] Define `DeliberationEngine` trait (reason, investigate, reflect, §46)
- [ ] `ThoughtSnapshot`, `DeliberationContext`, `Investigation`, result types
- [ ] Review `tpt-eve` public API (github.com/TPT-Solutions/tpt-eve)
- [ ] `EveAdapter` implementing `DeliberationEngine` without depending on Eve internal crates where avoidable
- [ ] Metacognition consumption: certainty, gaps, assumptions, missing evidence, should-continue/act/ask-for-help (§19)
- [ ] Hypothesis generation via Eve
- [ ] Contradiction analysis via Eve symbolic/causal reasoning
- [ ] Causal reasoning (interventions, counterfactuals, effect estimation)
- [ ] Abstraction and analogy validation (§77)
- [ ] Active learning and experiment design
- [ ] Slow path implemented: Intellect, then Eve, then Augur, then Intellect re-evaluation (§61)
- [ ] Deep investigation loop with termination (confidence met / budget exhausted / risk policy / human needed, §62)
- [ ] Mock `DeliberationEngine` for tests
- [ ] Docs: INTEGRATION_EVE.md, INTEGRATION_INTELLECT.md

## Phase 6 — Anima Integration
- [ ] Review Anima crates (sse, inp, zkpa, ck)
- [ ] `AnimaAdapter`: memory retrieval via SSE (`retrieve_context`)
- [ ] Persist consolidated results to SSE without duplicating storage
- [ ] Episodic memory records: Episode, EpisodeEvent, EpisodeOutcome, PredictionRecord, ActionRecord (§14, §48.2)
- [ ] Propose or contribute SSE extension for episodes (Anima repo change)
- [ ] Global workspace: publish, attend, subscribe, broadcast, request, withdraw (§12)
- [ ] Decide: `tpt-anima-workspace` crate vs inside CK/SSE (§48.1)
- [ ] INP integration: intention negotiation and execution (§23, §24)
- [ ] ZKPA integration: identity, permissions, attestation, ownership states (§40)
- [ ] CK integration: send cognitive requirements, receive scheduling decisions (§35)
- [ ] Homeostatic state consumption: compute budget, memory/latency pressure, energy, urgency, risk, uncertainty (§48.3, §81)
- [ ] Cognitive circuit breakers: repetition loops, thought/hypothesis explosion, recursive escalation, confidence oscillation, contradiction loops, resource exhaustion (§48.4)
- [ ] `ThoughtBubble` sandbox as the execution boundary for experimental cognition: temporary memory/hypotheses/tools/branches/models, no contamination of durable state (§42)
- [ ] Docs: INTEGRATION_ANIMA.md

## Phase 7 — Closed Loop
- [ ] Create `tpt-thought-loop`
- [ ] Cognitive cycle: observe, contextualise, associate, hypothesise, evaluate, decide, intend, act, observe, learn, consolidate (§101)
- [ ] Create `tpt-thought-intention` (goal, desired outcome, constraints, risk tolerance, priority, capabilities, deadline, provenance, §23)
- [ ] Action selection via Intellect then Anima/INP (Thought never executes actions directly, §24)
- [ ] Action, expected outcome, actual outcome, prediction error, learning event (§25)
- [ ] Learning loop explicit in architecture (§26)
- [ ] Planning representation (goal, state, candidate plans, predicted outcomes, constraints, cost, risk, §84)
- [ ] Cognitive modes: Reactive, Deliberative, Investigative, Creative, Reflective, Planning, Learning, Consolidation, Emergency (§82)
- [ ] Modulatory state: urgency, risk, novelty, reward, threat, confidence, uncertainty, arousal (§80)
- [ ] Curiosity driver and `CuriosityEvent` (§79)
- [ ] Human-in-the-loop as a provider with identity/timestamp/context/confidence (§38, §92)
- [ ] Replay: record initial state, inputs, backend versions, config, seeds, model IDs, memory refs (§32)
- [ ] Replay engine reproduces a thought trajectory; deterministic vs probabilistic transitions flagged (§33)
- [ ] Integrate replay with Intellect provenance and verification
- [ ] Cognitive loop tests: observe, think, act, learn
- [ ] Replay tests: recorded episode, reproduced trajectory
- [ ] Example: minimal closed loop in `examples/`

## Phase 8 — Reflection (spec addendum)
- [ ] Define Reflection vs online thought vs sleep modes
- [ ] `ReflectionEngine`, `ReflectionTrigger`, `ReflectionSession`, `CognitiveReview`
- [ ] Triggers: errors, uncertainty, contradictions, goal changes, outcome mismatch, periodic
- [ ] Structured artifacts, not chain-of-thought: Decision, Evidence, Outcome, Prediction, Prediction error, Confidence, Contradiction, Causal explanation, Knowledge gap, Strategy adjustment, Memory update
- [ ] Never require storing or exposing a model's private chain-of-thought
- [ ] Review, Metacognition, ErrorAnalysis, StrategyRevision components
- [ ] `KnowledgeGapAnalysis`, `StrategyUpdate`, `BeliefRevision`
- [ ] Eve supplies reasoning during reflection; Thought decides when, why and what to reflect on
- [ ] Circuit breakers prevent runaway reflection
- [ ] Decide crate boundary (`tpt-thought-reflection` or module)
- [ ] Tests: reflection triggers, review artifacts, bounded reflection

## Phase 9 — Sleep & Consolidation
- [ ] System lifecycle: ACTIVE, IDLE, REFLECTION, SLEEP, CONSOLIDATION, AWAKE
- [ ] Interruptible sleep (high-priority event wakes the system)
- [ ] `SleepCycle`, `ConsolidationPlan`, `ThoughtReplay`, `MemoryReorganisation`
- [ ] Anima CK schedules sleep when resources permit
- [ ] Episode replay, prioritised by importance
- [ ] Pattern detection and repeated-mistake identification
- [ ] Merge redundant memories
- [ ] Strengthen causal relationships; weaken stale/unsupported associations
- [ ] Abstraction: episodes to semantic/procedural knowledge, causal model, heuristic, policy, skill (§87)
- [ ] Belief re-evaluation with accumulated evidence
- [ ] Persistent knowledge-gap identification and future experiment generation
- [ ] Confidence recalibration
- [ ] Thought-history compression into durable summaries (§86)
- [ ] Recurring strategy detection (works/fails)
- [ ] Retrieval/salience priority updates
- [ ] Produce next-session cognitive state
- [ ] Pruning and forgetting rules (Anima owns decay, Thought owns relevance, §28)
- [ ] Promotion preserves provenance
- [ ] Skill formation: repeated success becomes a reusable skill routed through Intellect as a fast provider (§88)
- [ ] Cognitive reflexes: repeated deliberation becomes a learned policy (§89)
- [ ] Observatory records the sleep cycle; circuit breakers guard reprocessing
- [ ] Tests: replay, consolidation outcomes, interrupt safety, bounded sleep

## Phase 10 — Multi-Provider Cognition
- [ ] Provider registry and capability model: accuracy, latency, cost, confidence range, resources, modalities, locality, privacy, verification level (§58)
- [ ] Provider selection inputs surfaced to Intellect (§59)
- [ ] Provider competition and ensemble reasoning (§65, §118)
- [ ] Disagreement handling: ask why, assumptions, evidence, calibration, missing variable; disagreement is a cognitive event (§66)
- [ ] Provider calibration consumption and outcome tracking (§67, §68, §119)
- [ ] Human provider (`HumanProvider` returning `Decision<T>`, source = human, §92)
- [ ] Remote provider via `tpt-intellect-remote` only (no HTTP in Thought, §93)
- [ ] Local provider support via Intellect (tpt-infer, tpt-spark, Ollama) without knowing internals (§94)
- [ ] `AugurAdapter`: states, posteriors, evidence updates, residual uncertainty (§49)
- [ ] `AxiomAdapter`: uncertainty types, validated values, formal constraints, verification markers (§50)
- [ ] `PulseAdapter`: cognitive event stream (§53, §116)
- [ ] Multi-agent thought sharing: broadcast, independent evaluation, ensemble merge via INP (§39)
- [ ] Backend evolution test: add a new backend without modifying cognitive model (§90)
- [ ] Verify acceptance criterion M: same architecture with multiple providers
- [ ] Docs: PROVIDERS.md

## Phase 11 — Observability & Cognitive Observatory
- [ ] Structured telemetry events (`thought.created`, `thought.attended`, `thought.escalated`, … `thought.aborted`, §98)
- [ ] Metrics: ThoughtLatency, ThoughtDepth, BranchCount, InferenceCount, EscalationRate, AbstentionRate, PredictionAccuracy/Error, BeliefRevisionRate, MemoryRetrievalRate, DecisionConfidence, CalibrationError, CognitiveCost, InformationGain, ActionSuccessRate (§97)
- [ ] Telemetry sink abstraction (tracing-compatible, no required backend)
- [ ] Observatory data model: active thoughts, thought graph, hypotheses, providers, cost, state (§99)
- [ ] Observatory UI (with Anima Observatory): graph view, attention, confidence, prediction errors, provider selection
- [ ] Sleep/reflection cycle visualisation

## Phase 12 — Security, Testing, Benchmarks, Docs & Release
Security (§41)
- [ ] Threat model: memory injection, prompt injection, thought manipulation, false provenance, fabricated observations, replay attacks, identity substitution, contaminated knowledge, malicious loops
- [ ] Trusted provenance and source classification
- [ ] Integrity metadata verification
- [ ] Capability restrictions and thought sandboxing
- [ ] Loop detection and resource limits
- [ ] Dependency audit (cargo-deny, cargo-audit)

Testing (§95)
- [ ] Deterministic state tests
- [ ] Property tests (no invalid transitions)
- [ ] Cognitive loop tests
- [ ] Replay tests
- [ ] Adversarial tests: contradictions, false observations, memory contamination, provider failure/disagreement, recursive thought
- [ ] Resource tests: bounded cognition
- [ ] Create `tpt-thought-tests` integration-test crate
- [ ] Fuzz serialization decoders

Benchmarks (§96)
- [ ] Time-to-decision, decision quality, calibration, unnecessary/missed escalation
- [ ] Memory utilisation, prediction accuracy and error reduction
- [ ] Branch explosion, compute cost, energy cost
- [ ] `benches/` harness; BENCHMARKS.md

Documentation
- [ ] ARCHITECTURE.md, COGNITIVE_MODEL.md final
- [ ] Rustdoc for all public APIs; doc-tests
- [ ] Explainability: provenance as canonical explanation (LLM translation is presentation only, §71)

Golden demonstration (§123)
- [ ] Robot / unfamiliar object scenario with mock Anima, Intellect, Eve, Augur
- [ ] Same scenario against real sibling adapters
- [ ] Full episode replay in the demo

Acceptance criteria (§122)
- [ ] A. Thought receives an observation
- [ ] B. Retrieves relevant memory
- [ ] C. Creates multiple hypotheses
- [ ] D. Requests structured intelligence from Intellect
- [ ] E. Escalates uncertain decisions
- [ ] F. Invokes Eve deliberation
- [ ] G. Creates a prediction
- [ ] H. Executes an intention
- [ ] I. Compares predicted and actual outcomes
- [ ] J. Updates its cognitive state
- [ ] K. Preserves provenance
- [ ] L. Entire episode can be replayed
- [ ] M. Same architecture works with multiple intelligence providers

Release
- [ ] Pre-release dependency/licence audit
- [ ] Version, CHANGELOG, tag v0.1.0
- [ ] Publish decision (crates.io vs git-only)

---

## Deferred / Future (not v1)
- [ ] Dreams / idle (offline) cognition mode (§78)
- [ ] MessagePack and custom binary serialization (§72)
- [ ] `tpt-value` repository (utility, reward, preference, prediction error, §112)
- [ ] `tpt-planning` repository (§113)
- [ ] Creative-cognition mode tuning (§83)
- [ ] Cerebrum research bridge: biological hypothesis, Thought experiment, benchmark (§54, §115) — research only, never a runtime dependency
- [ ] Do NOT create `tpt-consciousness` (§114)

## Out of scope (never add to Thought, §111)
Models · databases · LLM clients · tensor ops · probability maths · formal proof · network protocols · agent identity · vendor-specific logic
