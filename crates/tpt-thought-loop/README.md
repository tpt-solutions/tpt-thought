# tpt-thought-loop

The closed cognitive loop for TPT Thought: one cycle observes, contextualises, associates,
hypothesises, evaluates, decides, intends, acts, observes the result, learns and consolidates. A
run can be recorded and replayed to check that it reproduces.

**Status:** pre-release, v0.1.0. Distributed from git only; not published to crates.io. See the
repository's `docs/DEPENDENCY_POLICY.md`.

## What it does

- **The cycle.** `CognitiveLoop::run` takes an `Input` and runs one cycle through the phases
  `CyclePhase`: `Observe`, `Contextualise`, `Associate`, `Hypothesise`, `Evaluate`, `Decide`,
  `Intend`, `Act`, `ObserveResult`, `Learn` and `Consolidate`. It returns a `CycleReport` whose
  `CycleOutcome` is either `Completed` (with the action, prediction error and response level) or
  `Unresolved` (with a reason).
- **A trajectory.** Each phase is recorded as a `TrajectoryStep`: index, phase, thought state,
  cognitive mode, transition kind and a detail string. `CognitiveLoop::trajectory` returns them.
- **Cognitive modes.** `CognitiveMode` has nine modes: `Reactive`, `Deliberative`, `Investigative`,
  `Creative`, `Reflective`, `Planning`, `Learning`, `Consolidation` and `Emergency`.
  `Modulation::select_mode` picks one from the current modulation.
- **Modulation.** `Modulation` is a validated set of signals in `[0, 1]`: urgency, risk, novelty,
  reward, threat, confidence, uncertainty and arousal (`ModulationFields`). `Modulation::neutral`
  is the default. Out-of-range values are refused with `ModulationError`.
- **Slow path.** When the fast path cannot settle an answer, the loop runs the slow path in a fixed
  order: Intellect, then Eve (investigation), then Augur (estimate), then Intellect again with the
  estimate in context (spec section 61). The stages are reported as `SlowStage`. An `Estimator`
  produces an `Estimate` with a mean and residual uncertainty.
- **Curiosity.** `CuriosityDriver::check` takes novelty and information gain for a question, each
  clamped to `[0, 1]`. Their product is the drive. When the drive reaches the threshold it returns a
  `CuriosityEvent`. Thresholds outside `[0, 1]` are refused with `InvalidThreshold`.
- **Planning.** `Plan` holds a goal, the current state, constraints, a maximum risk and a set of
  `CandidatePlan`s with cost and risk. `Plan::best` and `Plan::named` select among them.
  Invalid plans are refused with `PlanError`.
- **Replay.** `ReplayRecord::capture` takes a `LoopConfig`, `ReplayMetadata`, a list of `Input`s and
  a build closure. It runs the inputs and stores the configuration, inputs and trajectory. `replay`
  takes a record and the same kind of closure, re-runs the inputs and checks the new trajectory
  matches, step for step. `ReplayMetadata` records the providers, model IDs, backend versions and
  memory references involved.
- **Construction.** `CognitiveLoop::new` takes a `LoopConfig`, a `ProviderRegistry`, a deliberation
  engine, an estimator and an action sink. `LoopBuilder` assembles the same parts and also accepts a
  memory store, a policy set and a telemetry sink. `NullEngine`, `NullEstimator` and
  `NullActionSink` are defaults for parts you do not need.
- **Sleep.** `CognitiveLoop::sleep` runs consolidation over a `ConsolidationPlan` within a
  `SleepBudget`, checking an interrupt callback between steps.

## What it deliberately does not do

- **It does not execute actions.** Intentions go to an `ActionSink`. In production that is Anima's
  INP, or another executor. Thought decides *when* and *why* to act and does not act itself.
- **It does not depend on a backend.** Intelligence comes from providers registered in a
  `ProviderRegistry`. Deep investigation comes from a `DeliberationEngine`, and estimates come from
  an `Estimator`. No concrete backend is named here.
- **It does not depend on Augur.** Augur's adapter lives in `tpt-thought-integrations`.
- **It does not run an async runtime.** Calls are synchronous. Asynchronous work is the caller's
  concern.

## Usage

The repository's example is the smallest complete run. It observes, thinks, acts, learns and
consolidates, then replays the recorded run:

```sh
cargo run -p tpt-thought-loop --example closed_loop
```

The core of it, with the registry, sink and input filled in:

```rust
use tpt_thought_core::{Probability, StructuredValue, Timestamp};
use tpt_thought_loop::{
    Action, ActionError, ActionSink, CandidatePlan, CognitiveLoop, CycleOutcome, Input,
    LoopConfig, Modulation, NullEngine, NullEstimator,
};
use tpt_thought_providers::{Locality, Privacy, ProviderProfile, ProviderRegistry, ReferenceProvider};

/// Performs the action and reports what happened.
struct Arm;

impl ActionSink for Arm {
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError> {
        Ok(action.expected.clone())
    }
}

let mut registry = ProviderRegistry::new();
let profile = ProviderProfile::new(Locality::Local, Privacy::OnDevice);
registry.register(ReferenceProvider::new("intellect", profile).with_answer(
    "can the gripper grasp the cup?",
    StructuredValue::Text("grasp".into()),
    Probability::new(0.95)?,
))?;

let config = LoopConfig::new(1, Modulation::neutral());
let mut lp = CognitiveLoop::new(config, registry, NullEngine, NullEstimator, Arm)?;

let input = Input {
    at: Timestamp(1_000),
    source: "camera".into(),
    observation: StructuredValue::Text("cup within reach".into()),
    question: "can the gripper grasp the cup?".into(),
    goal: "pick up the cup".into(),
    constraints: vec![],
    memories: vec![],
    candidates: vec![CandidatePlan {
        name: "grasp".into(),
        predicted_outcome: StructuredValue::Text("cup held".into()),
        cost: 1.0,
        risk: 0.1,
    }],
};

let report = lp.run(&input)?;
if let CycleOutcome::Completed { action, error, .. } = &report.outcome {
    println!("completed `{action}` with prediction error {error:.2}");
}
```

`ActionSink` is the only boundary to the outside world. The example
(`examples/closed_loop.rs`) shows the complete program, including the trajectory printout and the
record-and-replay step.

## API surface

Everything is re-exported from the crate root. The modules in the table below are private.

| Module | Contents |
|---|---|
| `cycle` | `CognitiveLoop`, `Input`, `CycleReport`, `CycleOutcome`, `CyclePhase`, `TrajectoryStep`, `LoopError`, `default_policy` |
| `config` | `LoopConfig` |
| `builder` | `LoopBuilder` |
| `mode` | `CognitiveMode` |
| `modulation` | `Modulation`, `ModulationFields`, `ModulationError` |
| `slow` | `SlowStage`, `Estimate`, `Estimator` |
| `curiosity` | `CuriosityDriver`, `CuriosityEvent`, `InvalidThreshold` |
| `plan` | `Plan`, `CandidatePlan`, `PlanError` |
| `action` | `Action`, `ActionSink`, `ActionError` |
| `replay` | `ReplayRecord`, `ReplayMetadata`, `ReplayError`, `replay` |
| `null` | `NullEngine`, `NullEstimator`, `NullActionSink` |

`CognitiveLoop` also exposes `run`, `trajectory`, `provenance`, `predictions`, `intentions`,
`observatory`, `config`, `sink`, `with_memory`, `with_policy`, `with_telemetry`, `sleep` and the
consolidator accessors.

## Installation

```toml
[dependencies]
tpt-thought-loop = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## Testing

```sh
cargo test -p tpt-thought-loop
```

Integration tests in `tests/loop_tests.rs` exercise the cycle directly. The wider acceptance and
golden scenarios are in `tpt-thought-tests`.

## Changes

Recorded in [CHANGELOG.md](CHANGELOG.md).

## Design notes

- **Determinism.** `LoopConfig` carries a seed. Given the same seed, configuration and inputs, a run
  produces the same trajectory. Replay depends on this. Steps whose outcome came from a provider or
  an executor are marked probabilistic in `TransitionKind`, because a non-deterministic backend may
  answer differently next time. Replay of such steps is only as reproducible as the backend.
- **Bounded.** The configuration sets a budget (`CognitiveBudget`), a circuit breaker
  configuration, an investigation policy and an escalation ladder. Every repeated process has an
  explicit limit.
- **Provenance.** Each cycle records provenance and prediction state. `CognitiveLoop::provenance`
  and `CognitiveLoop::predictions` expose them for "why did the agent think this?" queries.
- **Telemetry.** `CognitiveLoop::observatory` gives access to a `RecordingSink`-backed
  `Observatory`. Attach a different sink with `with_telemetry`.
- **Sleep.** `CognitiveLoop::sleep` runs consolidation through a `Consolidator`, which is itself
  bounded and interruptible.

## Related crates

- `tpt-thought-core`: shared types such as `Timestamp`, `StructuredValue` and `Probability`.
- `tpt-thought-state`, `tpt-thought-hypothesis`, `tpt-thought-prediction`, `tpt-thought-providers`,
  `tpt-thought-deliberation`, `tpt-thought-consolidation`, `tpt-thought-policy`,
  `tpt-thought-safety`, `tpt-thought-workspace`, `tpt-thought-provenance`,
  `tpt-thought-observatory` and `tpt-thought-intention`: the subsystems the cycle composes.
- `tpt-thought-integrations`: connects the loop to Intellect, Eve, Anima and Augur.
- `tpt-thought` (the facade) re-exports this crate as `tpt_thought::cognitive_loop`.

## Licence

Licensed under either of MIT or Apache-2.0, at your option.

## Minimum supported Rust version

1.85.
