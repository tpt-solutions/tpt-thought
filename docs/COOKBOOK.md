# Cookbook

Short recipes for common tasks. Each one links to a runnable example where there is one. Every
snippet uses the public API of the `tpt-thought` facade.

## Build a loop with defaults

```rust
use tpt_thought::prelude::*;

let lp = CognitiveLoop::builder().build()?;
```

With no providers, no engine, no estimator and no executor, the loop can still run. Any decision
that needs a provider comes back unresolved, and any action is refused.

## Add providers

Use `ProviderRegistry::register` with a `ReferenceProvider` for fixed answers, or implement
`IntelligenceProvider` yourself (next recipe). Pass the registry to the builder with
`.registry(registry)`.

## Write a custom provider

Implement `IntelligenceProvider`: `id`, `profile` and `decide`. The provider answers or abstains.
It never needs to know about the loop. See `crates/tpt-thought/examples/custom_provider.rs`.

## Add an executor

An executor implements `ActionSink`. It receives an `Action` and returns the observed outcome. The
loop compares that outcome with the prediction and learns from the difference.

```rust
use tpt_thought::prelude::*;
use tpt_thought::core::StructuredValue;

struct Gripper;

impl ActionSink for Gripper {
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError> {
        // Talk to the hardware or to an executor such as Anima's INP here.
        Ok(action.expected.clone())
    }
}

let lp = CognitiveLoop::builder().sink(Gripper).build()?;
```

## Forward telemetry

Telemetry sinks receive every event. The loop keeps its own recorder for `observatory()`, and
the sink you add is in addition to it.

```rust
use tpt_thought::observatory::{TelemetryEvent, TelemetrySink};

struct Printer;

impl TelemetrySink for Printer {
    fn emit(&mut self, event: &TelemetryEvent) {
        println!("{:?} at {:?}", event.kind, event.at);
    }
}

let lp = CognitiveLoop::builder().telemetry(Printer).build()?;
```

A sink must not fail the operation that emitted the event. Telemetry never changes cognitive state.

## Set the escalation policy

After a rejected fast answer, the loop asks its policy what to do. The decision is an
`Escalate(level)`, an `Accept` or a `Terminate`:

- `Escalate(level)` runs the slow stages that level allows. Intellect always runs. Eve needs L4
  or higher, and the Augur estimate needs L3 or higher.
- `Accept` ends the cycle with the answer unresolved. No further cognition runs.
- `Terminate` also ends the cycle unresolved, with the reason in the report.

The default policy escalates to L5 every time, which is the full slow path. To change that, build
a `PolicySet`:

```rust
use tpt_thought::policy::{Decision, EscalationLevel, PolicyContext, PolicySet};

let mut policy = PolicySet::new();
policy.push(|ctx: &PolicyContext| {
    if ctx.confidence >= 0.8 {
        Decision::Accept
    } else {
        Decision::Escalate(EscalationLevel::ProbabilisticInference)
    }
});

let lp = CognitiveLoop::builder().policy(policy).build()?;
```

The policy sees the level, the best confidence among the rejected answers and the budget. Risk,
branch count and value are zero or absent at this point, because they are not known until a plan
is made. A policy must not treat them as measured.

## Record and replay a run

`ReplayRecord::capture` runs inputs through a loop built from a configuration, and `replay`
re-runs them and checks each step. Both take a function that builds the loop, so the setup is the
same both times. The `thought` command does this from files. See `docs/CLI.md`.

## Sleep over consolidated memory

`CognitiveLoop::sleep` runs a bounded, interruptible consolidation cycle over the loop's
consolidator. Record episodes, beliefs or calibration samples with `consolidator_mut()` first.
Sleep only starts from an idle loop. An interrupt at or above the budget's priority ends the cycle,
and the steps not yet run are reported. See `crates/tpt-thought/examples/reflection_and_sleep.rs`.

## Reflect on uncertain thoughts

A `ReflectionEngine` runs a bounded session over a trigger's targets. The `ReflectionReasoner` you
supply produces structured artifacts, such as `KnowledgeGap`, never a model's private reasoning.
See `crates/tpt-thought/examples/reflection_and_sleep.rs`.

## Compare hypotheses

A `BranchSet` holds competing explanations. Branch with `explore`, reject with `reject`, and pick
one with `collapse` and a `CollapseStrategy`. Collapse only chooses what to act on. It does not
make the explanation a belief. See `crates/tpt-thought/examples/hypotheses.rs`.

## Explain a decision

Each cycle report names the thought it worked on. `CognitiveLoop::provenance()` holds the records
behind it, and `explain(&thought)` returns them in order. `thought why` does this for a recorded
run.

## Update posteriors with evidence

With the `augur` feature, `AugurSession` holds a model. Each `observe` adds one `observe`
statement, and `infer` re-runs the model and returns each posterior with its uncertainty. This
needs an Augur revision with evidence sessions, which is not yet pinned. See
`crates/tpt-thought/examples/augur_evidence.rs` and `docs/INTEGRATION_AUGUR.md`.

## Use your own memory store

Pass any `MemoryStore` to `.memory(store)`. The loop asks it for memories during contextualisation,
and it holds references only, never copies of the memories themselves.
