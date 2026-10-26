# tpt-thought-deliberation

A provider-neutral trait for reasoning engines, the request and result shapes of each engine
capability, metacognition, and a bounded deep-investigation loop that drives an engine until its
answer is good enough, risky, or needs a human.

> **Status:** pre-release (v0.1.1). This crate is distributed from git only and is not published
> to crates.io. See `docs/DEPENDENCY_POLICY.md` in the repository.

## What it does

- **Defines the engine contract.** `DeliberationEngine` has one method per capability: `reason`,
  `investigate`, `reflect`, `generate_hypotheses`, `analyse_contradictions`, `analyse_causation`,
  `validate_abstraction`, `design_experiment` and `rank_queries`. Each defaults to
  `DeliberationError::Unsupported(Capability::...)`, so an engine implements only what it offers.
- **Drives investigation.** `run_investigation` repeatedly asks an engine for one step and stops
  on the first of four `Termination` reasons, or when the step cap is reached.
- **Records metacognition.** `Metacognition` holds the engine's certainty, gaps, assumptions,
  missing evidence and a `NextAction` (`Continue`, `Act` or `AskForHelp`).
- **Gives engines a read-only view.** `DeliberationContext` carries a `ThoughtSnapshot` and the
  `CognitiveBudget`. Engines never mutate a thought; they return results that the loop applies.
- **Provides a test engine.** `mock::ScriptedEngine` returns queued responses in order, so tests can
  exercise the loop without a real engine.

## What it deliberately does not do

- It has no dependency on Eve or any other sibling repository. The Eve adapter (`EveAdapter`)
  belongs to `tpt-thought-integrations`.
- It does not implement the slow path. The ordering of Intellect, Eve, Augur and Intellect again
  belongs to `tpt-thought-loop`, which composes engines with providers.
- It does not store the investigation's conclusions. It returns an `InvestigationRun` and leaves
  recording to the caller.

## Main types

| Item | Role |
|---|---|
| `DeliberationEngine` | The engine trait, with one defaulted method per capability. |
| `Capability` | `Reason`, `Investigate`, `Reflect`, `HypothesisGeneration`, `ContradictionAnalysis`, `CausalReasoning`, `AbstractionValidation`, `ExperimentDesign` and `ActiveLearning`. |
| `DeliberationError` | `Unsupported(Capability)` or `Failed(String)`. |
| `Reasoning` | A conclusion: hypotheses, confidence, uncertainty and metacognition. |
| `Investigation` and `InvestigationOutcome` | One step's request and its findings, confidence, uncertainty, risk and whether a human is needed. |
| `InvestigationPolicy` | `target_confidence`, `max_risk` and `max_steps`. |
| `run_investigation` | The bounded loop. Returns an `InvestigationRun` with every outcome and its `Termination`. |
| `Termination` | `ConfidenceMet`, `BudgetExhausted`, `RiskPolicy` or `HumanNeeded`. |
| `Metacognition` and `NextAction` | The engine's view of its own state and what it should do next. |
| `DeliberationContext` and `ThoughtSnapshot` | The read-only input an engine receives. |
| `ContradictionAnalysis`, `CausalAnalysis`, `ExperimentPlan` and others | Result types for the matching capabilities. |
| `mock::ScriptedEngine` | A scripted engine for tests. |

## Module layout

- `engine.rs`: the `DeliberationEngine` trait, `Capability`, `DeliberationError` and the request and
  result types for each capability.
- `investigation.rs`: `run_investigation`, the investigation policy, outcomes and termination.
- `metacognition.rs`: `Metacognition` and `NextAction`.
- `snapshot.rs`: `ThoughtSnapshot` and `DeliberationContext`.
- `mock.rs`: `ScriptedEngine`.

## Usage

```rust
use tpt_thought_core::{
    CognitiveBudget, Confidence, IdGenerator, Probability, SeededEntropy, TextContent, Thought,
    ThoughtContent, ThoughtOrigin, Timestamp, Uncertainty,
};
use tpt_thought_deliberation::{
    mock::ScriptedEngine, run_investigation, DeliberationContext, InvestigationOutcome,
    InvestigationPolicy, Termination, ThoughtSnapshot,
};

fn investigate() -> Result<(), Box<dyn std::error::Error>> {
    let ids = IdGenerator::new(SeededEntropy::new(1));
    let thought = Thought::new(
        ids.thought(),
        ThoughtOrigin::Internal,
        ThoughtContent::Text(TextContent {
            text: "why is the drawer stuck?".into(),
            language: None,
        }),
        Timestamp(0),
    );
    let ctx = DeliberationContext {
        thought: ThoughtSnapshot::from_thought(&thought),
        budget: CognitiveBudget::default(),
    };

    let policy = InvestigationPolicy::new(
        Probability::new(0.9)?, // stop once confidence reaches 0.9
        Probability::new(0.3)?, // stop if an outcome's risk exceeds 0.3
        5,                      // hard cap on steps
    );

    let outcome = InvestigationOutcome {
        findings: vec!["a loose runner is jamming the drawer".into()],
        confidence: Confidence::from_raw(Probability::new(0.95)?),
        uncertainty: Uncertainty::default(),
        risk: Probability::new(0.1)?,
        needs_human: false,
    };

    // A scripted engine returns the outcomes it was given, one per step.
    let mut engine = ScriptedEngine::default();
    engine.investigations.push_back(Ok(outcome));

    let run = run_investigation(&mut engine, &ctx, "why is the drawer stuck?", &policy)?;
    assert_eq!(run.termination, Termination::ConfidenceMet);
    Ok(())
}
```

In production the loop builds the snapshot from the live thought, and a real engine produces the
outcomes. The example builds both by hand so it runs on its own.

## Design notes

- The step cap is the lower of `InvestigationPolicy::max_steps` and the thought's
  `max_iterations` budget. No engine can keep a thought running indefinitely.
- After each step the checks run in a fixed order: risk policy, then human need, then confidence.
  The first match ends the run.
- An engine error aborts the run and is returned unchanged.
- Engines take `&mut self`, because they may keep caches or scripted state between calls.
- Abstractions and analogies are validated through `Capability::AbstractionValidation`. Eve is
  expected to decide whether an analogy is valid, not Thought.

## Related crates

- `tpt-thought-core`: `Confidence`, `Uncertainty`, `Probability`, `Hypothesis` and
  `CognitiveBudget`.
- `tpt-thought-providers`: the provider trait that the slow path composes with engines.
- `tpt-thought-hypothesis`: information gain and contradictions that engines help resolve.
- `tpt-thought-policy`: decides when to escalate to a deeper level.
- `tpt-thought-loop`: runs the slow path and calls `run_investigation`.
- `tpt-thought-integrations`: the Eve adapter that implements `DeliberationEngine`.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0, at your option.
