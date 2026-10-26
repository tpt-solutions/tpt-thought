// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reflection after an uncertain thought, then a bounded sleep cycle over the loop's memory.
//!
//! Reflection produces structured artifacts, such as a knowledge gap, never a model's private
//! reasoning. Sleep is interruptible: here an interrupt is raised after the first step, so the
//! cycle stops early and reports the steps it did not run.
//!
//! Run with `cargo run -p tpt-thought --example reflection_and_sleep`.

use tpt_thought::consolidation::Interrupt;
use tpt_thought::consolidation::{ConsolidationPlan, SleepBudget, SleepOutcome};
use tpt_thought::core::{
    IdGenerator, ManualClock, SeededEntropy, TextContent, Thought, ThoughtContent, ThoughtOrigin,
    Timestamp,
};
use tpt_thought::prelude::*;
use tpt_thought::reflection::{
    KnowledgeGap, Label, ReasonerError, ReflectionArtifact, ReflectionEngine, ReflectionPolicy,
    ReflectionReasoner, ReflectionStep, ReflectionTrigger,
};

/// A reasoner that reports one knowledge gap for each thought it reflects on.
struct GapFinder;

impl ReflectionReasoner for GapFinder {
    fn reflect(
        &mut self,
        step: &ReflectionStep<'_>,
    ) -> Result<Vec<ReflectionArtifact>, ReasonerError> {
        let name = Label::new("grip force for wet cups")
            .map_err(|_| ReasonerError::new("label rejected"))?;
        Ok(vec![ReflectionArtifact::KnowledgeGap(KnowledgeGap {
            thought: step.thought.id,
            name,
            missing: vec![],
        })])
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ids = IdGenerator::new(SeededEntropy::new(5));
    let thought = Thought::new(
        ids.thought(),
        ThoughtOrigin::Internal,
        ThoughtContent::Text(TextContent {
            text: "the grasp felt unstable".into(),
            language: None,
        }),
        Timestamp(0),
    );
    let clock = ManualClock::new(Timestamp(10));

    let engine = ReflectionEngine::new(ReflectionPolicy::default());
    let trigger = ReflectionTrigger::Uncertainty {
        targets: vec![thought.id],
        uncertainty: 0.7,
    };
    let review = engine.run(
        trigger,
        std::slice::from_ref(&thought),
        &mut GapFinder,
        &clock,
    );
    println!(
        "reflection recorded {} knowledge gap(s)",
        review.gaps.gaps.len()
    );

    // Sleep over a loop whose consolidator has been given the reflection's findings.
    let mut lp = CognitiveLoop::builder().build()?;
    let budget = SleepBudget {
        max_steps: 100,
        max_duration_nanos: i64::MAX,
        wake_priority: 0.9,
    };
    let plan = ConsolidationPlan::standard();
    let mut wake_after_first_step = |steps_run: usize| {
        (steps_run == 1).then(|| Interrupt {
            priority: 1.0,
            reason: "a sensor reading arrived".into(),
        })
    };
    let report = lp.sleep(budget, &plan, &clock, &mut wake_after_first_step)?;
    match report.outcome {
        SleepOutcome::Completed { steps_run } => println!("slept through {steps_run} steps"),
        SleepOutcome::Exhausted {
            steps_run,
            remaining,
        } => {
            println!(
                "budget ran out after {steps_run} steps, {} left",
                remaining.len()
            );
        }
        SleepOutcome::Interrupted {
            steps_run,
            remaining,
            by,
        } => {
            println!(
                "woken after {steps_run} steps ({}), {} steps deferred",
                by.reason,
                remaining.len()
            );
        }
    }
    Ok(())
}
