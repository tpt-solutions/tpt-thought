// SPDX-License-Identifier: MIT OR Apache-2.0
//! The shortest useful program: one observation, one decision, one action, then a replay check.
//!
//! Run with `cargo run -p tpt-thought --example quickstart`.

use tpt_thought::cognitive_loop::{
    CandidatePlan, CycleOutcome, Input, LoopConfig, Modulation, ReplayMetadata, ReplayRecord,
};
use tpt_thought::core::{Probability, StructuredValue, Timestamp};
use tpt_thought::prelude::*;
use tpt_thought::providers::{Locality, Privacy, ProviderProfile, ReferenceProvider};

const QUESTION: &str = "can the gripper grasp the cup?";

/// Stands in for an executor such as Anima's INP. Thought never acts on its own.
struct SimulatedArm;

impl ActionSink for SimulatedArm {
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError> {
        println!("  executor performs `{}`", action.name);
        Ok(action.expected.clone())
    }
}

/// One provider that knows the answer to the question, with 95% reliability.
fn registry() -> ProviderRegistry {
    let profile = ProviderProfile::new(Locality::Local, Privacy::OnDevice);
    let provider = ReferenceProvider::new("intellect", profile).with_answer(
        QUESTION,
        StructuredValue::Text("grasp".into()),
        Probability::new(0.95).expect("valid probability"),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(provider).expect("register provider");
    registry
}

fn build(
    config: &LoopConfig,
) -> Result<CognitiveLoop<NullEngine, NullEstimator, SimulatedArm>, LoopError> {
    CognitiveLoop::builder()
        .config(config.clone())
        .registry(registry())
        .sink(SimulatedArm)
        .build()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = LoopConfig::new(1, Modulation::neutral());
    let input = Input {
        at: Timestamp(1_000),
        source: "camera".into(),
        observation: StructuredValue::Text("cup within reach".into()),
        question: QUESTION.into(),
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

    let mut lp = build(&config)?;
    let report = lp.run(&input)?;
    match &report.outcome {
        CycleOutcome::Completed {
            action,
            error,
            level,
        } => {
            println!("completed `{action}`, prediction error {error:.2} ({level:?})");
        }
        CycleOutcome::Unresolved { reason } => println!("unresolved: {reason}"),
    }

    // A recorded run can be re-run, and the re-run must match step for step.
    let record = ReplayRecord::capture(
        config,
        ReplayMetadata {
            providers: vec!["intellect".into()],
            ..ReplayMetadata::default()
        },
        vec![input],
        build,
    )?;
    tpt_thought::cognitive_loop::replay(&record, build)?;
    println!("replay reproduced the run step for step");
    Ok(())
}
