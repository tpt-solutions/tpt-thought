// SPDX-License-Identifier: MIT OR Apache-2.0
//! Benchmark scenarios (spec section 96). Counted metrics are deterministic for a given cycle
//! count. Timings are wall-clock and informational only.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tpt_thought_core::{MemoryRef, StructuredValue};
use tpt_thought_hypothesis::{Branch, BranchSet};
use tpt_thought_loop::{CognitiveLoop, CycleOutcome, ReplayRecord};
use tpt_thought_providers::{ProviderRegistry, ReferenceProvider};
use tpt_thought_workspace::{MemoryIndex, MemoryStore};

use super::*;

/// Results of one benchmark run.
#[derive(Debug, Clone, PartialEq)]
pub struct Metrics {
    /// Cycles run in the decision-quality scenario.
    pub cycles: usize,
    /// Cycles that completed with the correct action.
    pub correct_actions: usize,
    /// Cycles whose prediction matched the outcome exactly.
    pub prediction_matches: usize,
    /// Provider calls per cycle, the compute-cost proxy.
    pub provider_calls_per_cycle: f64,
    /// Cycles escalated although the answer was reliable (cases that should not escalate).
    pub unnecessary_escalations: usize,
    /// Cases with an unreliable answer that were accepted without escalating.
    pub missed_escalations: usize,
    /// Fraction of the relevant memories returned for their cue.
    pub memory_recall: f64,
    /// Branches admitted under the bound, out of the branches offered.
    pub branches_admitted: usize,
    /// Branches offered in the explosion scenario.
    pub branches_offered: usize,
    /// Active branches after the explosion scenario. Must not exceed the bound.
    pub active_branches: usize,
    /// Mean wall time per cycle, in nanoseconds. Informational only.
    pub mean_cycle_nanos: f64,
}

/// Run every scenario with `cycles` cycles in the decision-quality loop.
pub fn run_all(cycles: usize) -> Metrics {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ProviderRegistry::new();
    registry
        .register(Counting {
            inner: ReferenceProvider::new("intellect", profile()).with_answer(
                QUESTION,
                StructuredValue::Text("grasp".into()),
                prob(0.95),
            ),
            calls: calls.clone(),
            questions: Arc::new(std::sync::Mutex::new(Vec::new())),
        })
        .expect("register");

    let mut lp = CognitiveLoop::new(
        neutral_config(91),
        registry,
        NullEngine,
        NullEstimator,
        Echo::default(),
    )
    .expect("loop");
    let start = Instant::now();
    let mut correct_actions = 0;
    let mut prediction_matches = 0;
    for i in 0..cycles {
        let report = lp.run(&grasp_input(i as i64)).expect("cycle");
        if let CycleOutcome::Completed { action, error, .. } = report.outcome {
            if action == "grasp" {
                correct_actions += 1;
            }
            if error == 0.0 {
                prediction_matches += 1;
            }
        }
    }
    let elapsed = start.elapsed().as_nanos() as f64;
    let provider_calls_per_cycle = calls.load(Ordering::SeqCst) as f64 / cycles.max(1) as f64;

    let (unnecessary_escalations, missed_escalations) = escalation_cases(cycles);
    let memory_recall = memory_recall();
    let (branches_admitted, branches_offered, active_branches) = branch_explosion(200, 8);

    Metrics {
        cycles,
        correct_actions,
        prediction_matches,
        provider_calls_per_cycle,
        unnecessary_escalations,
        missed_escalations,
        memory_recall,
        branches_admitted,
        branches_offered,
        active_branches,
        mean_cycle_nanos: elapsed / cycles.max(1) as f64,
    }
}

/// Reactive cases with a reliable answer (should not escalate) and an unreliable one (should).
/// Returns (unnecessary escalations, missed escalations).
fn escalation_cases(cycles: usize) -> (usize, usize) {
    let mut unnecessary = 0;
    let mut missed = 0;
    for i in 0..cycles {
        let reliable = run_reactive(0.95, i as i64);
        if !reliable.slow_stages.is_empty() {
            unnecessary += 1;
        }
        let unreliable = run_reactive(0.3, i as i64);
        if unreliable.slow_stages.is_empty() {
            missed += 1;
        }
    }
    (unnecessary, missed)
}

fn run_reactive(confidence: f64, at: i64) -> tpt_thought_loop::CycleReport {
    let mut config = reactive_config(92);
    config.accept_at = 0.8;
    let mut lp = CognitiveLoop::new(
        config,
        registry_answering("intellect", confidence),
        NullEngine,
        NullEstimator,
        Echo::default(),
    )
    .expect("loop");
    lp.run(&grasp_input(at)).expect("cycle")
}

/// Ten memories, each tagged with its own topic. Each cue asks for one topic, so recall is
/// the share of cues whose single relevant memory is returned.
fn memory_recall() -> f64 {
    let mut index = MemoryIndex::new();
    for i in 0..10 {
        index.insert(
            MemoryRef {
                store: "anima-sse".into(),
                id: format!("topic-{i}"),
            },
            [format!("topic-{i}")],
        );
    }
    let hits = (0..10)
        .filter(|i| {
            index
                .retrieve(&[format!("topic-{i}")], 1)
                .first()
                .is_some_and(|m| m.id == format!("topic-{i}"))
        })
        .count();
    hits as f64 / 10.0
}

/// Offer `offered` branches under a bound of `max_active`. Returns (admitted, offered, active).
fn branch_explosion(offered: usize, max_active: usize) -> (usize, usize, usize) {
    let ids = ids(93);
    let mut set = BranchSet::new(max_active);
    let mut admitted = 0;
    for i in 0..offered {
        let branch = Branch::new(ids.hypothesis(), format!("candidate {i}"), prob(0.5));
        if set.explore(branch).is_ok() {
            admitted += 1;
        }
    }
    (admitted, offered, set.active_count())
}

/// Replay a recorded run and return how many steps it contains, for the replay throughput line.
pub fn replay_steps(cycles: usize) -> usize {
    let inputs: Vec<_> = (0..cycles).map(|i| grasp_input(i as i64)).collect();
    let record = ReplayRecord::capture(neutral_config(94), Default::default(), inputs, |c| {
        CognitiveLoop::new(
            c.clone(),
            registry_answering("intellect", 0.95),
            NullEngine,
            NullEstimator,
            Echo::default(),
        )
    })
    .expect("capture");
    record.trajectory.len()
}
