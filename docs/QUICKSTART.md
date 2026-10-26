# Quickstart

Thought needs Rust 1.85 or later. From the repository root:

```
cargo run -p tpt-thought --example quickstart
```

This runs one grasp decision through the closed loop. It prints the action, the prediction error,
and whether a replay of the same run matches step for step.

## A minimal program

```rust
use tpt_thought::cognitive_loop::{CandidatePlan, CycleOutcome, Input, LoopConfig, Modulation};
use tpt_thought::core::{Probability, StructuredValue, Timestamp};
use tpt_thought::prelude::*;
use tpt_thought::providers::{Locality, Privacy, ProviderProfile, ReferenceProvider};

struct Arm;

impl ActionSink for Arm {
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError> {
        Ok(action.expected.clone())
    }
}

fn main() -> Result<(), LoopError> {
    let question = "can the gripper grasp the cup?";
    let profile = ProviderProfile::new(Locality::Local, Privacy::OnDevice);
    let provider = ReferenceProvider::new("intellect", profile).with_answer(
        question,
        StructuredValue::Text("grasp".into()),
        Probability::new(0.95).expect("valid probability"),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(provider).expect("register provider");

    let mut lp = CognitiveLoop::builder()
        .config(LoopConfig::new(1, Modulation::neutral()))
        .registry(registry)
        .sink(Arm)
        .build()?;

    let report = lp.run(&Input {
        at: Timestamp(1_000),
        source: "camera".into(),
        observation: StructuredValue::Text("cup within reach".into()),
        question: question.into(),
        goal: "pick up the cup".into(),
        constraints: vec![],
        memories: vec![],
        candidates: vec![CandidatePlan {
            name: "grasp".into(),
            predicted_outcome: StructuredValue::Text("cup held".into()),
            cost: 1.0,
            risk: 0.1,
        }],
    })?;

    match report.outcome {
        CycleOutcome::Completed { action, .. } => println!("completed `{action}`"),
        CycleOutcome::Unresolved { reason } => println!("unresolved: {reason}"),
    }
    Ok(())
}
```

The builder fills in the deliberation engine, estimator and memory store with null defaults, so the
sketch does not set them.

## How the pieces fit

- **Providers** answer questions. A rejected fast answer escalates, and a policy decides how far.
- **The executor** (`ActionSink`) performs actions. Thought never acts on its own.
- **The loop** records every step in a trajectory, with provenance for every decision.
- **Replay** re-runs a recorded set of inputs and checks the trajectory. See `docs/CLI.md`.

## Next

- `docs/CLI.md`: record, replay and explain runs from the command line.
- `docs/COOKBOOK.md`: custom providers, telemetry, policies, sleep, reflection and hypotheses.
- `docs/ARCHITECTURE.md`: the crate layers and the one-cycle sequence.
- `crates/tpt-thought/examples/`: `quickstart`, `hypotheses`, `reflection_and_sleep`,
  `custom_provider`, and `augur_evidence` (needs the `augur` feature).
