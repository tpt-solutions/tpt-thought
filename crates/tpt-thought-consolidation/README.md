# tpt-thought-consolidation

Bounded, interruptible sleep and memory consolidation for TPT Thought.

Sleep is the offline phase in which a thought system replays what it has done, tidies its memory,
reweights its associations, re-examines its beliefs and recalibrates its confidence. This crate
runs that work as a plan of discrete steps. Each step is applied atomically, so an interrupt at
any step boundary leaves the state consistent, and the steps not yet run are returned so the cycle
can resume later.

Anima CK decides when sleep is affordable. This crate never schedules CPU. Anima owns memory decay.
Thought owns relevance, which is what this crate computes.

> **Status:** pre-release, `v0.1.1`. Distributed from git only, not published on crates.io.

## What it does

- **Plan.** `ConsolidationPlan::standard()` gives the full pass, in order. Each `ConsolidationStep`
  is one unit of work:
  `Replay`, `DetectPatterns`, `MergeMemories`, `PruneForgotten`, `ReweightAssociations`, `Abstract`,
  `ReevaluateBeliefs`, `RecordGaps`, `Recalibrate`, `CompressHistory` and `UpdatePriorities`.
  `ConsolidationPlan::from_steps` builds a custom plan.
- **Sleep cycle.** `SleepCycle::begin` moves the system from a `SystemPhase` into sleep, checking
  the transition, and fails with `IllegalPhaseTransition` if it is not allowed. `SleepCycle::run`
  applies steps under a `SleepBudget` of maximum steps, maximum wall time and a wake priority. Before
  each step it asks an interrupt callback. An `Interrupt` at or above the wake priority stops the
  cycle at that boundary. Lower-priority interrupts are recorded as `ignored_interrupts`. The
  `SleepReport` says whether the cycle `Completed`, was `Exhausted` or was `Interrupted`.
- **Scheduling seam.** The `SleepScheduler` trait takes a `SleepRequest` and a `ResourceSnapshot`
  and returns a `SleepGrant`: `Granted(budget)` or `Deferred { retry_after_nanos }`.
  `ThresholdScheduler` is a default implementation based on minimum free compute and energy and a
  maximum memory pressure. Anima CK is expected to supply the production scheduler.
- **Consolidator.** `Consolidator` holds everything a cycle reads and writes and applies one step at
  a time with `apply`. Inputs are fed in first: `observe_episodes`, `record_attempt`,
  `record_deliberation`, `record_belief_evidence`, `record_calibration`, `record_thought`,
  `track_memory` and `support_association`. Results are read back through accessors such as
  `memories`, `associations`, `knowledge`, `belief`, `calibration`, `experiments`, `history` and
  `salience`. `calibrated(raw)` maps a raw confidence through the fitted calibration.
- **Episode replay.** `ThoughtReplay` replays episodes in importance order. Each replayed episode
  boosts salience for its topic and reinforces the matching memory by `replay_boost` scaled by its
  importance. An open question on the episode is recorded as a knowledge gap.
- **Patterns and skills.** `detect` finds strategies, repeated mistakes and learned policies from
  attempts and deliberations. `abstract_knowledge` turns a detection, the episodes and the causal
  links into `Knowledge` entries of four `AbstractionKind`s: `Semantic`, `Procedural`,
  `CausalModel` and `Heuristic`. `Skill`, `SkillRoute` and `StrategyReport` describe the
  learned behaviour.
- **Beliefs.** `BeliefRecord` accumulates supporting and opposing observations with provenance.
  `reevaluate` moves a record between `Candidate`, `Believed` and `Demoted` under `BeliefConfig`,
  and `belief_transitions` records each change.
- **Calibration.** `Calibration::fit` builds a reliability curve over `CALIBRATION_BINS` (10) from
  `CalibrationSample`s. `adjust` maps a raw confidence through it, and `expected_error` reports the
  remaining miscalibration.
- **Associations.** `AssociationStore` tracks support for links between items and reweights them
  under a `ReweightPolicy`. Causal links are marked separately (`causal()`), and a causal link
  without provenance is refused with `AssociationError::CausalWithoutProvenance`.
- **Gaps and experiments.** `GapTracker` records open questions with their provenance. Questions
  that recur at least `min_occurrences` times become `KnowledgeGap`s, and `propose` turns them into
  `ExperimentProposal`s.
- **History.** `ThoughtHistory` keeps `HistoryEntry`s and compresses older ones into
  `HistorySummary`s, keeping the most recent entries in full.

## What it deliberately does not do

- It does not schedule itself or decide when sleep is affordable. That belongs to the
  `SleepScheduler`.
- It does not decay memory. Anima supplies strength values, and this crate expires memories below
  `strength_floor`.
- It does not run during online thought. Its intended entry point is `SleepCycle::run`, which the
  phase machine only reaches from `Idle` through `PhaseEvent::Sleep`.

## Usage

```rust
use tpt_thought_consolidation::{
    Consolidator, ConsolidationConfig, ConsolidationPlan, SleepBudget, SleepCycle, SystemPhase,
};

let mut consolidator = Consolidator::new(ConsolidationConfig::default());
consolidator.observe_episodes(episodes);

let plan = ConsolidationPlan::standard();
let budget = SleepBudget {
    max_steps: 20,
    max_duration_nanos: 2_000_000_000,
    wake_priority: 0.8,
};

let cycle = SleepCycle::begin(SystemPhase::Idle, budget)?;
let report = cycle.run(&mut consolidator, &plan, &clock, &mut |_steps_run| None);

let calibrated = consolidator.calibrated(0.9);
```

Pass a callback that returns `Some(Interrupt { priority, reason })` to model a wake-up. Use
`ThresholdScheduler` in tests, or a scheduler backed by Anima CK in production.

## Module layout

| Module | Contents |
|---|---|
| `consolidator` | `Consolidator`, `ConsolidationConfig` |
| `plan` | `ConsolidationPlan`, `ConsolidationStep` |
| `sleep` | `SleepCycle`, `SleepBudget`, `SleepScheduler`, `ThresholdScheduler`, `SleepGrant`, `SleepReport`, `Interrupt` |
| `phase` | `SystemPhase`, `PhaseEvent`, `IllegalPhaseTransition` |
| `replay` | `ThoughtReplay`, `Episode` |
| `patterns` | `detect`, `abstract_knowledge`, `Knowledge`, `Skill`, `LearnedPolicy`, `RepeatedMistake` |
| `belief` | `BeliefRecord`, `BeliefConfig`, `BeliefStatus`, `Calibration`, `CalibrationSample` |
| `association` | `AssociationStore`, `Association`, `Link`, `ReweightPolicy` |
| `memory` | `MemoryItem`, `MemoryReorganisation`, `MergeRecord`, `Durability` |
| `gaps` | `GapTracker`, `KnowledgeGap`, `ExperimentProposal` |
| `history` | `ThoughtHistory`, `HistoryEntry`, `HistorySummary` |
| `state` | `SalienceTable`, `NextSessionState` |

## Design notes

- Every step is atomic. The consolidator is never left half-updated when a sleep is interrupted.
- Scheduling is a seam, not an implementation. The crate asks for a grant and respects the budget
  it is given.
- Calibration and belief thresholds are explicit configuration, so behaviour can be inspected and
  tested.
- `tests/cycle.rs` runs end-to-end sleep cycles, including interrupts and pending replay.

## Related crates

- `tpt-thought-core` supplies `Clock`, `Timestamp`, `EpisodeId`, `ProvenanceId` and
  `ContentReference`, which the consolidator uses.
- `tpt-thought-workspace` produces the episodic records that replay consumes.
- `tpt-thought-reflection` handles reflection, the online counterpart to sleep.
- `tpt-thought-provenance` supplies the provenance identifiers that beliefs and gaps carry.

## Installation

```toml
[dependencies]
tpt-thought-consolidation = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
