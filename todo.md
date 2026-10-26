# TPT Thought — Project Checklist

Organisation: TPT Solutions · Language: Rust · Licence: MIT OR Apache-2.0
Spec: `spec.txt` (section numbers below, e.g. §7, refer to it).

Ground rules (apply to every phase)
- [ ] Create a crate only when a real API/dependency boundary exists (§100)
- [x] No Apache-only dependencies (MIT, MIT/Apache dual, BSD, ISC, Zlib, Unlicense, CC0, Unicode are OK)
- [x] Libraries are runtime-agnostic (native `async fn` in traits, no tokio in libraries)
- [x] `tpt-thought-core` has no TPT runtime dependencies and no backend dependencies (§102)
- [ ] Never branch on a specific backend (`if jev`); everything goes through provider traits (§6, §57)
- [x] Sibling-repo dependencies (github.com/TPT-Solutions/<repo>) only inside `tpt-thought-integrations`
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
- [x] Decide git auth for sibling-repo git deps in CI
- [x] `README.md` (what Thought is, and what it is not, §1)
- [x] `CONTRIBUTING.md` (contributions licensed MIT OR Apache-2.0)
- [x] `docs/` skeleton: ARCHITECTURE, COGNITIVE_MODEL, INTEGRATION_ANIMA, INTEGRATION_EVE, INTEGRATION_INTELLECT, PROVIDERS, BENCHMARKS
- [x] `examples/` and `benches/` directories (the top-level placeholders were removed in Phase 13; real examples and benches live in their crates)
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
- [x] Thought lifecycle API: create, attend, contextualise, associate, complete (§121)
- [x] `Thought::from_observation`
- [x] Source classification of inputs (trusted/untrusted, §41)
- [x] Create `tpt-thought-attention`
- [x] `AttentionState` with multi-factor model: salience, novelty, urgency, goal relevance, risk, uncertainty, information gain, value relevance, recency, resource cost (§11)
- [x] Configurable attention formula (pluggable scoring)
- [x] Cognitive demand output (priority, deadline, estimated cost, capabilities, risk, info gain) for Anima CK (§35)
- [x] Working memory (§13): strict capacity, attention weighting, decay, eviction, priority, provenance
- [x] Forgetting states: temporary, weak, strong, consolidated, expired, pruned (§28)
- [x] Create `tpt-thought-graph`
- [x] Typed edges: supports, contradicts, causes, derived-from, generalises, specialises, triggers, analogous-to (§10, §77)
- [x] Association (↔) and causation (→) as distinct edge types (§76); preceded-by vs caused (§75)
- [x] Causal provenance stored on causal edges
- [x] Graph queries: ancestors, descendants, contradictions, evidence paths
- [x] Graph references Anima memory by ID only (no duplicated storage, §10)
- [x] Thought recursion limits: max_depth, max_children, max_total_compute, max_duration (§85)
- [x] Tests: lifecycle, attention scoring, working-memory eviction, graph invariants

## Phase 2 — Hypotheses
- [x] Create `tpt-thought-hypothesis`
- [x] Hypothesis lifecycle: GENERATED, UNTESTED, SUPPORTED/WEAKENED, TESTING, UPDATED, ACCEPTED/REJECTED/SUSPENDED (§16)
- [x] Branching with per-branch probability, confidence, evidence, cost, risk, expected information gain (§15)
- [x] Branch operations: explore, prune, merge, suspend, promote, reject
- [x] Thought collapse by probability, utility, evidence, formal verification, ensemble agreement, human choice (§64)
- [x] Belief promotion pipeline: thought, hypothesis, supported hypothesis, belief candidate, belief (§43)
- [x] Configurable promotion criteria (repeated observation, proof, experiment, high-confidence intelligence, model agreement, human confirmation)
- [x] Belief demotion pipeline (§44)
- [x] Contradiction representation and handling: attention up, investigate, resolve by new evidence / context separation / conditional validity (§29)
- [x] Rule: a thought is not a belief merely because it was considered
- [x] Information-gain primitive (§37) and investigate-if gain exceeds cost context
- [x] Tests: branch explosion bounds, promotion/demotion round trips, contradiction cases

## Phase 3 — Prediction
- [x] Create `tpt-thought-prediction`
- [x] `Prediction` primitive (predicted state, expected time, confidence, assumptions, source thought, §17)
- [x] Match predictions against later observations
- [x] Prediction error type and error-as-event (§18)
- [x] Configurable error response ladder: small (continue), moderate (update belief), large (raise attention, investigate), catastrophic (suspend model, re-evaluate assumptions)
- [x] Prediction-error fan-out hooks (memory reinforcement, active learning, belief updates, confidence, attention, planning)
- [x] Tests: prediction, observation, error, response thresholds

## Phase 4 — Intellect Integration
- [x] Define provider-neutral traits in Thought (e.g. `IntelligenceProvider`) so core never depends on Intellect
- [x] Create `tpt-thought-policy`
- [x] Policies: escalation, branching, resource use, risk, human escalation, termination
- [x] Escalation levels L0 to L6 (reflex to human), configurable (§20)
- [x] Expected value of additional cognition drives escalation (§36)
- [x] Cognitive Budget enforcement (time, compute, energy, money, memory, branches, model calls, experiments, §63)
- [x] Create `tpt-thought-integrations`
- [x] Review the `tpt-intellect` public API (github.com/TPT-Solutions/tpt-intellect)
- [x] `IntellectAdapter`: map Thought questions to `IntelligenceRequest`, `Decision<T>`
- [x] Request constraints: required confidence, max cost, max latency, risk tolerance, acceptable providers, required capabilities (§55)
- [x] Handle abstention and provenance from Intellect decisions
- [x] Consume calibration metadata (raw vs calibrated vs empirical, §67)
- [x] Capability / provider discovery queries (§58, §91)
- [x] Deterministic reference backend for tests (no external model)
- [x] Fast-path implementation (accept at high confidence, no unnecessary escalation, §60)
- [x] Fast and deliberative cognition as pluggable modes (not hard-coded System 1/2, §21)
- [x] Tests: fast path, abstention, escalation trigger, provider failure

## Phase 5 — Eve Integration
- [x] Define `DeliberationEngine` trait (reason, investigate, reflect, §46)
- [x] `ThoughtSnapshot`, `DeliberationContext`, `Investigation`, result types
- [x] Review `tpt-eve` public API (github.com/TPT-Solutions/tpt-eve)
- [x] `EveAdapter` implementing `DeliberationEngine` without depending on Eve internal crates where avoidable
- [x] Metacognition consumption: certainty, gaps, assumptions, missing evidence, should-continue/act/ask-for-help (§19)
- [x] Hypothesis generation via Eve
- [ ] Contradiction analysis via Eve symbolic/causal reasoning
- [x] Causal reasoning (interventions, counterfactuals, effect estimation)
- [x] Abstraction and analogy validation (§77)
- [x] Active learning and experiment design
- [x] Slow path implemented: Intellect, then Eve, then Augur, then Intellect re-evaluation (§61)
- [x] Deep investigation loop with termination (confidence met / budget exhausted / risk policy / human needed, §62)
- [x] Mock `DeliberationEngine` for tests
- [x] Docs: INTEGRATION_EVE.md, INTEGRATION_INTELLECT.md

## Phase 6 — Anima Integration
- [x] Review Anima crates (sse, inp, zkpa, ck)
- [ ] `AnimaAdapter`: memory retrieval via SSE (`retrieve_context`)
- [ ] Persist consolidated results to SSE without duplicating storage
- [x] Episodic memory records: Episode, EpisodeEvent, EpisodeOutcome, PredictionRecord, ActionRecord (§14, §48.2)
- [ ] Propose or contribute SSE extension for episodes (Anima repo change)
- [x] Global workspace: publish, attend, subscribe, broadcast, request, withdraw (§12)
- [x] Decide: `tpt-anima-workspace` crate vs inside CK/SSE (§48.1)
- [ ] INP integration: intention negotiation and execution (§23, §24)
- [ ] ZKPA integration: identity, permissions, attestation, ownership states (§40)
- [ ] CK integration: send cognitive requirements, receive scheduling decisions (§35)
- [ ] Homeostatic state consumption: compute budget, memory/latency pressure, energy, urgency, risk, uncertainty (§48.3, §81)
- [x] Cognitive circuit breakers: repetition loops, thought/hypothesis explosion, recursive escalation, confidence oscillation, contradiction loops, resource exhaustion (§48.4)
- [x] `ThoughtBubble` sandbox as the execution boundary for experimental cognition: temporary memory/hypotheses/tools/branches/models, no contamination of durable state (§42)
- [x] Docs: INTEGRATION_ANIMA.md

## Phase 7 — Closed Loop
- [x] Create `tpt-thought-loop`
- [x] Cognitive cycle: observe, contextualise, associate, hypothesise, evaluate, decide, intend, act, observe, learn, consolidate (§101)
- [x] Create `tpt-thought-intention` (goal, desired outcome, constraints, risk tolerance, priority, capabilities, deadline, provenance, §23)
- [ ] Action selection via Intellect then Anima/INP (Thought never executes actions directly, §24)
- [x] Action, expected outcome, actual outcome, prediction error, learning event (§25)
- [x] Learning loop explicit in architecture (§26)
- [x] Planning representation (goal, state, candidate plans, predicted outcomes, constraints, cost, risk, §84)
- [x] Cognitive modes: Reactive, Deliberative, Investigative, Creative, Reflective, Planning, Learning, Consolidation, Emergency (§82)
- [x] Modulatory state: urgency, risk, novelty, reward, threat, confidence, uncertainty, arousal (§80)
- [x] Curiosity driver and `CuriosityEvent` (§79)
- [x] Human-in-the-loop as a provider with identity/timestamp/context/confidence (§38, §92)
- [x] Replay: record initial state, inputs, backend versions, config, seeds, model IDs, memory refs (§32)
- [x] Replay engine reproduces a thought trajectory; deterministic vs probabilistic transitions flagged (§33)
- [ ] Integrate replay with Intellect provenance and verification
- [x] Cognitive loop tests: observe, think, act, learn
- [x] Replay tests: recorded episode, reproduced trajectory
- [x] Example: minimal closed loop in `examples/`

## Phase 8 — Reflection (spec addendum)
- [x] Define Reflection vs online thought vs sleep modes
- [x] `ReflectionEngine`, `ReflectionTrigger`, `ReflectionSession`, `CognitiveReview`
- [x] Triggers: errors, uncertainty, contradictions, goal changes, outcome mismatch, periodic
- [x] Structured artifacts, not chain-of-thought: Decision, Evidence, Outcome, Prediction, Prediction error, Confidence, Contradiction, Causal explanation, Knowledge gap, Strategy adjustment, Memory update
- [x] Never require storing or exposing a model's private chain-of-thought
- [x] Review, Metacognition, ErrorAnalysis, StrategyRevision components
- [x] `KnowledgeGapAnalysis`, `StrategyUpdate`, `BeliefRevision`
- [x] Eve supplies reasoning during reflection; Thought decides when, why and what to reflect on
- [x] Circuit breakers prevent runaway reflection
- [x] Decide crate boundary (`tpt-thought-reflection` or module)
- [x] Tests: reflection triggers, review artifacts, bounded reflection

## Phase 9 — Sleep & Consolidation
- [x] System lifecycle: ACTIVE, IDLE, REFLECTION, SLEEP, CONSOLIDATION, AWAKE
- [x] Interruptible sleep (high-priority event wakes the system)
- [x] `SleepCycle`, `ConsolidationPlan`, `ThoughtReplay`, `MemoryReorganisation`
- [ ] Anima CK schedules sleep when resources permit (trait defined; Anima implementation blocked)
- [x] Episode replay, prioritised by importance
- [x] Pattern detection and repeated-mistake identification
- [x] Merge redundant memories
- [x] Strengthen causal relationships; weaken stale/unsupported associations
- [x] Abstraction: episodes to semantic/procedural knowledge, causal model, heuristic, policy, skill (§87)
- [x] Belief re-evaluation with accumulated evidence
- [x] Persistent knowledge-gap identification and future experiment generation
- [x] Confidence recalibration
- [x] Thought-history compression into durable summaries (§86)
- [x] Recurring strategy detection (works/fails)
- [x] Retrieval/salience priority updates
- [x] Produce next-session cognitive state
- [x] Pruning and forgetting rules (Anima owns decay, Thought owns relevance, §28)
- [x] Promotion preserves provenance
- [x] Skill formation: repeated success becomes a reusable skill routed through Intellect as a fast provider (§88)
- [x] Cognitive reflexes: repeated deliberation becomes a learned policy (§89)
- [x] Observatory records the sleep cycle; circuit breakers guard reprocessing
- [x] Tests: replay, consolidation outcomes, interrupt safety, bounded sleep

## Phase 10 — Multi-Provider Cognition
- [x] Provider registry and capability model: accuracy, latency, cost, confidence range, resources, modalities, locality, privacy, verification level (§58)
- [x] Provider selection inputs surfaced to Intellect (§59)
- [x] Provider competition and ensemble reasoning (§65, §118)
- [x] Disagreement handling: ask why, assumptions, evidence, calibration, missing variable; disagreement is a cognitive event (§66)
- [x] Provider calibration consumption and outcome tracking (§67, §68, §119)
- [x] Human provider (`HumanProvider` returning `Decision<T>`, source = human, §92)
- [x] Remote provider via `tpt-intellect-remote` only (no HTTP in Thought, §93)
- [ ] Local provider support via Intellect (tpt-infer, tpt-spark, Ollama) without knowing internals (§94)
- [x] `AugurAdapter`: states, posteriors, evidence updates, residual uncertainty (§49). Discrete states come from the posterior draws (`augur::states_of`); evidence updates and residuals are in `AugurSession`.
- [ ] `AxiomAdapter`: uncertainty types, validated values, formal constraints, verification markers (§50)
- [ ] `PulseAdapter`: cognitive event stream (§53, §116)
- [ ] Multi-agent thought sharing: broadcast, independent evaluation, ensemble merge via INP (§39)
- [x] Backend evolution test: add a new backend without modifying cognitive model (§90)
- [x] Verify acceptance criterion M: same architecture with multiple providers
- [x] Docs: PROVIDERS.md

## Phase 11 — Observability & Cognitive Observatory
- [x] Structured telemetry events (`thought.created`, `thought.attended`, `thought.escalated`, … `thought.aborted`, §98)
- [x] Metrics: ThoughtLatency, ThoughtDepth, BranchCount, InferenceCount, EscalationRate, AbstentionRate, PredictionAccuracy/Error, BeliefRevisionRate, MemoryRetrievalRate, DecisionConfidence, CalibrationError, CognitiveCost, InformationGain, ActionSuccessRate (§97)
- [x] Telemetry sink abstraction (tracing-compatible, no required backend)
- [x] Observatory data model: active thoughts, thought graph, hypotheses, providers, cost, state (§99)
- [ ] Observatory UI (with Anima Observatory): graph view, attention, confidence, prediction errors, provider selection
- [ ] Sleep/reflection cycle visualisation

## Phase 12 — Security, Testing, Benchmarks, Docs & Release
Security (§41)
- [x] Threat model: memory injection, prompt injection, thought manipulation, false provenance, fabricated observations, replay attacks, identity substitution, contaminated knowledge, malicious loops
- [x] Trusted provenance and source classification
- [x] Integrity metadata verification
- [x] Capability restrictions and thought sandboxing
- [x] Loop detection and resource limits
- [x] Dependency audit (cargo-deny, cargo-audit)

Testing (§95)
- [x] Deterministic state tests
- [x] Property tests (no invalid transitions)
- [x] Cognitive loop tests
- [x] Replay tests
- [x] Adversarial tests: contradictions, false observations, memory contamination, provider failure/disagreement, recursive thought
- [x] Resource tests: bounded cognition
- [x] Create `tpt-thought-tests` integration-test crate
- [x] Fuzz serialization decoders

Benchmarks (§96)
- [x] Time-to-decision, decision quality, calibration, unnecessary/missed escalation
- [x] Memory utilisation, prediction accuracy and error reduction
- [x] Branch explosion, compute cost, energy cost
- [x] `benches/` harness; BENCHMARKS.md

Documentation
- [ ] ARCHITECTURE.md, COGNITIVE_MODEL.md final
- [x] Rustdoc for all public APIs; doc-tests
- [x] Explainability: provenance as canonical explanation (LLM translation is presentation only, §71)

Golden demonstration (§123)
- [x] Robot / unfamiliar object scenario with mock Anima, Intellect, Eve, Augur
- [ ] Same scenario against real sibling adapters
- [x] Full episode replay in the demo

Acceptance criteria (§122)
- [x] A. Thought receives an observation
- [x] B. Retrieves relevant memory
- [x] C. Creates multiple hypotheses
- [x] D. Requests structured intelligence from Intellect
- [x] E. Escalates uncertain decisions
- [ ] F. Invokes Eve deliberation
- [x] G. Creates a prediction
- [ ] H. Executes an intention
- [x] I. Compares predicted and actual outcomes
- [x] J. Updates its cognitive state
- [x] K. Preserves provenance
- [x] L. Entire episode can be replayed
- [x] M. Same architecture works with multiple intelligence providers

Release
- [x] Pre-release dependency/licence audit
- [x] Version, CHANGELOG, tag v0.1.0
- [x] Publish decision (crates.io vs git-only)

---

## Phase 13 — Hygiene, Augur API, Usability & Tooling (post-review)
Fix what is broken or stale
- [x] CI: `if: ${{ secrets.TPT_GIT_TOKEN != '' }}` is invalid; pass the secret via `env:` and test that
- [x] Replace the 7 placeholder crate descriptions ("see todo.md") in consolidation, hypothesis, policy, prediction, providers, safety, workspace
- [x] Fix stale docs: DEPENDENCY_POLICY (siblings are in the build), BENCHMARKS (integrations not on hold), THREAT_MODEL (fuzzing exists), COGNITIVE_MODEL (loop wiring claims)
- [x] ARCHITECTURE.md: add a one-cycle sequence diagram
- [x] Remove empty `examples/.gitkeep` and `benches/.gitkeep` once real content exists
- [x] `OsEntropy::fill` (`core/src/id.rs`): fallible path instead of `.expect`, keep infallible wrapper
- [x] `reflection/src/engine.rs`: stop swallowing `session.complete()` error
- [x] Check `RecordingSink` for unbounded growth; add a cap
- [ ] Cut `v0.1.1` (the `v0.1.0` tag predates later commits) and update CHANGELOG

Augur API (in `tpt-augur`, then the adapter here)
- [x] `tpt-augur-runtime`: `Session` with `observe` (evidence update) and `infer`, deterministic re-runs (tpt-augur `c29ecd17`)
- [x] `tpt-augur-runtime`: `Residual` type (sd, 95% interval width, converged, relative uncertainty) and `residual_of` (tpt-augur `c29ecd17`)
- [x] Augur tests (conjugate update after 1 then 3 observations, sd shrinks, determinism), docs/API.md, CHANGELOG, TODO
- [x] `AugurSession` in `tpt-thought-integrations`; map `Residual` to Thought `Uncertainty`
- [x] Bump the three Augur `rev` pins; update INTEGRATION_AUGUR.md and drop the "Not provided" notes

Make the loop easy to use
- [x] `impl Default for LoopConfig`
- [x] Public `NullEngine` and `NullEstimator`; remove the four copy-pasted versions
- [x] `LoopBuilder` with sane defaults
- [x] Injectable telemetry sink; configurable `BreakerConfig` and calibration min-samples
- [x] Wire `policy` escalation (L0-L6) into path selection and expose `sleep` on the loop; document anything left to compose
- [x] Facade: `prelude`, feature forwarding to integrations, re-export `integrations`

Tooling, docs, examples
- [x] `tpt-thought-cli` crate: `run`, `replay`, `why`, `validate`, `schema`, `demo`
- [x] Examples: quickstart, hypotheses, reflection_and_sleep, custom_provider, augur_evidence
- [x] Docs: QUICKSTART.md, CLI.md, COOKBOOK.md; README quickstart block
- [x] Bench regeneration script that updates BENCHMARKS.md

Repo hygiene
- [x] CONTRIBUTING.md: issues only, no pull requests
- [x] Issue templates (bug, feature, integration-gap) and config.yml
- [x] `scripts/check.ps1` and `check.sh` (fmt, clippy, test, deny)

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

---

---

## Blocked and partial (open items)
- **Axiom adapter (partial)**: built behind the `axiom` feature, pinned to `tpt-axiom` `42c495f7`. Covers validated probabilities, confidence and fuzzy variance. Formal constraints and ZK verification markers are not mapped.
- **Remote and local providers**: remote (loopback and caller-supplied transports) and local (`tpt-infer` classifier) are built. The `tpt-infer` pin relies on `Cargo.lock`: cargo rejects a workspace patch pointing at the same source, and the sibling manifest names no revision. Ollama and `tpt-spark` are not in the sibling survey and are not built.
- **Eve contradiction analysis**: `tpt-eve-symbolic` is tokio-free, but its `dirs` dependency pulls in `option-ext` (MPL-2.0), which fails the licence policy. Blocked until Eve drops or replaces that dependency.
- **Eve query ranking**: no tokio-free API found.
- **Golden scenario against real siblings**: written, but two properties are ignored. The real Intellect answers at full confidence on the first pass, so the Eve, Augur and re-evaluation stages never run. The Eve adapter has no investigation step to report.
- **Pulse**: no event stream exists in `tpt-pulse`, only synchronous session-typed channels.
- **Anima**: no `retrieve_context`, episode persistence, global workspace or ZKPA identity type. The broadcast adapter covers provider discovery only.
- **Observatory UI**: belongs to Anima Observatory, out of scope for Thought.
- **Release tag `v0.1.0`**: exists locally (commit `220fc9b`); the later commits are not covered by it.
- **Consolidator input (Phase 13)**: the loop owns a consolidator and exposes `sleep()`, but nothing feeds it episodes, beliefs or calibration samples automatically. Callers record them with `consolidator_mut()`.
