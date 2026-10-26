// SPDX-License-Identifier: MIT OR Apache-2.0
//! The minimal closed loop: observe, think, act, learn, consolidate, then replay the run.
//!
//! Run with `cargo run -p tpt-thought-loop --example closed_loop`.

use tpt_thought_core::{Probability, StructuredValue, Timestamp};
use tpt_thought_loop::{
    replay, Action, ActionError, ActionSink, CandidatePlan, CognitiveLoop, CycleOutcome, Input,
    LoopConfig, LoopError, Modulation, NullEngine, NullEstimator, ReplayMetadata, ReplayRecord,
};
use tpt_thought_providers::{
    Locality, Privacy, ProviderProfile, ProviderRegistry, ReferenceProvider,
};

const QUESTION: &str = "can the gripper grasp the cup?";

/// Stands in for Anima's INP: it performs the action and reports what happened.
struct SimulatedArm;

impl ActionSink for SimulatedArm {
    fn execute(&mut self, action: &Action) -> Result<StructuredValue, ActionError> {
        println!("  executor performs `{}`", action.name);
        Ok(action.expected.clone())
    }
}

fn registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    let profile = ProviderProfile::new(Locality::Local, Privacy::OnDevice);
    registry
        .register(ReferenceProvider::new("intellect", profile).with_answer(
            QUESTION,
            StructuredValue::Text("grasp".into()),
            Probability::new(0.95).expect("valid probability"),
        ))
        .expect("register provider");
    registry
}

fn build(
    config: &LoopConfig,
) -> Result<CognitiveLoop<NullEngine, NullEstimator, SimulatedArm>, LoopError> {
    CognitiveLoop::new(
        config.clone(),
        registry(),
        NullEngine,
        NullEstimator,
        SimulatedArm,
    )
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

    println!("Run:");
    let mut lp = build(&config)?;
    let report = lp.run(&input)?;
    match &report.outcome {
        CycleOutcome::Completed {
            action,
            error,
            level,
        } => {
            println!("  completed `{action}`, prediction error {error:.2} ({level:?})");
        }
        CycleOutcome::Unresolved { reason } => println!("  unresolved: {reason}"),
        CycleOutcome::Deferred { reason } => println!("  deferred: {reason}"),
    }

    println!("Trajectory:");
    for step in lp.trajectory() {
        println!(
            "  {:>2} {:<14} state={:<15?} mode={:<13} {:?} {}",
            step.index,
            format!("{:?}", step.phase),
            step.state,
            step.mode.name(),
            step.transition,
            step.detail
        );
    }

    println!("Replay:");
    let record = ReplayRecord::capture(
        config,
        ReplayMetadata {
            providers: vec!["intellect".into()],
            ..ReplayMetadata::default()
        },
        vec![input],
        build,
    )?;
    replay(&record, build)?;
    println!("  recorded run reproduced step for step");
    Ok(())
}
