// SPDX-License-Identifier: MIT OR Apache-2.0
//! End-to-end sleep cycles: bounded, interruptible, provenance-preserving consolidation.

use tpt_thought_consolidation::*;
use tpt_thought_core::{
    ContentReference, IdGenerator, ManualClock, ProvenanceId, SeededEntropy, ThoughtId, Timestamp,
};

fn ids(seed: u64) -> IdGenerator {
    IdGenerator::new(SeededEntropy::new(seed))
}

fn episode(ids: &IdGenerator, topic: &str, at: i64, error: f64, question: Option<&str>) -> Episode {
    Episode {
        id: ids.episode(),
        topic: topic.into(),
        at: Timestamp(at),
        prediction_error: error,
        salience: 0.5,
        open_question: question.map(Into::into),
        provenance: vec![ids.provenance()],
    }
}

fn budget(max_steps: usize) -> SleepBudget {
    SleepBudget {
        max_steps,
        max_duration_nanos: i64::MAX,
        wake_priority: 0.9,
    }
}

fn no_interrupts(_: usize) -> Option<Interrupt> {
    None
}

#[test]
fn full_cycle_completes_and_wakes() {
    let clock = ManualClock::new(Timestamp(0));
    let mut c = Consolidator::new(ConsolidationConfig::default());
    let plan = ConsolidationPlan::standard();
    let report = SleepCycle::begin(SystemPhase::Idle, budget(plan.len()))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    assert_eq!(
        report.outcome,
        SleepOutcome::Completed {
            steps_run: plan.len()
        }
    );
    assert_eq!(report.phase, SystemPhase::Awake);
}

#[test]
fn sleep_is_bounded_by_steps_and_resumable() {
    let clock = ManualClock::new(Timestamp(0));
    let mut c = Consolidator::new(ConsolidationConfig::default());
    let plan = ConsolidationPlan::standard();
    let report = SleepCycle::begin(SystemPhase::Idle, budget(2))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    let SleepOutcome::Exhausted {
        steps_run,
        remaining,
    } = report.outcome
    else {
        panic!("expected exhaustion");
    };
    assert_eq!(steps_run, 2);
    assert_eq!(remaining.len(), plan.len() - 2);

    // Resuming the remainder finishes the plan.
    let rest = ConsolidationPlan::from_steps(remaining);
    let report = SleepCycle::begin(SystemPhase::Idle, budget(usize::MAX))
        .unwrap()
        .run(&mut c, &rest, &clock, &mut no_interrupts);
    assert!(matches!(report.outcome, SleepOutcome::Completed { .. }));
}

#[test]
fn sleep_is_bounded_by_wall_time() {
    let clock = ManualClock::new(Timestamp(0));
    let mut c = Consolidator::new(ConsolidationConfig::default());
    let plan = ConsolidationPlan::standard();
    let mut tight = budget(usize::MAX);
    tight.max_duration_nanos = 10;
    // Each step takes 4 nanoseconds of clock time.
    let mut tick = |_: usize| {
        clock.advance(4);
        None
    };
    let report = SleepCycle::begin(SystemPhase::Idle, tight)
        .unwrap()
        .run(&mut c, &plan, &clock, &mut tick);
    assert!(matches!(report.outcome, SleepOutcome::Exhausted { .. }));
    assert_eq!(report.phase, SystemPhase::Awake);
}

#[test]
fn high_priority_interrupt_wakes_without_half_applied_state() {
    let clock = ManualClock::new(Timestamp(0));
    let g = ids(100);
    let mut c = Consolidator::new(ConsolidationConfig::default());
    c.observe_episodes(vec![
        episode(&g, "pump", 1, 0.9, None),
        episode(&g, "valve", 2, 0.2, None),
    ]);
    let plan = ConsolidationPlan::standard();
    let mut interrupt_after_replay = |steps: usize| {
        (steps == 1).then(|| Interrupt {
            priority: 0.95,
            reason: "obstacle detected".into(),
        })
    };
    let report = SleepCycle::begin(SystemPhase::Idle, budget(usize::MAX))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut interrupt_after_replay);
    let SleepOutcome::Interrupted {
        steps_run,
        remaining,
        by,
    } = report.outcome
    else {
        panic!("expected interruption");
    };
    assert_eq!(steps_run, 1);
    assert_eq!(by.reason, "obstacle detected");
    assert_eq!(remaining.len(), plan.len() - 1);
    assert_eq!(report.phase, SystemPhase::Awake);
    // The replay step finished completely before the interrupt was honoured.
    assert_eq!(c.replayed().len(), 2);
    assert_eq!(c.pending_replay(), 0);
}

#[test]
fn interrupt_before_any_step_wakes_from_sleep() {
    let clock = ManualClock::new(Timestamp(0));
    let mut c = Consolidator::new(ConsolidationConfig::default());
    let plan = ConsolidationPlan::standard();
    let mut urgent = |_: usize| {
        Some(Interrupt {
            priority: 1.0,
            reason: "fire".into(),
        })
    };
    let report = SleepCycle::begin(SystemPhase::Idle, budget(usize::MAX))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut urgent);
    assert!(matches!(
        report.outcome,
        SleepOutcome::Interrupted { steps_run: 0, .. }
    ));
    assert!(c.replayed().is_empty());
}

#[test]
fn low_priority_interrupts_are_ignored_and_recorded() {
    let clock = ManualClock::new(Timestamp(0));
    let mut c = Consolidator::new(ConsolidationConfig::default());
    let plan = ConsolidationPlan::standard();
    let mut chatter = |_: usize| {
        Some(Interrupt {
            priority: 0.1,
            reason: "status ping".into(),
        })
    };
    let report = SleepCycle::begin(SystemPhase::Idle, budget(usize::MAX))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut chatter);
    assert!(matches!(report.outcome, SleepOutcome::Completed { .. }));
    assert_eq!(report.ignored_interrupts.len(), plan.len() + 1);
}

#[test]
fn replay_runs_most_important_episodes_first() {
    let clock = ManualClock::new(Timestamp(0));
    let g = ids(101);
    let routine = episode(&g, "routine", 1, 0.05, None);
    let surprising = episode(&g, "surprise", 2, 0.95, None);
    let (routine_id, surprising_id) = (routine.id, surprising.id);
    let mut c = Consolidator::new(ConsolidationConfig::default());
    c.observe_episodes(vec![routine, surprising]);
    let plan = ConsolidationPlan::from_steps(vec![ConsolidationStep::Replay]);
    SleepCycle::begin(SystemPhase::Idle, budget(10))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    assert_eq!(c.replayed(), &[surprising_id, routine_id]);
}

#[test]
fn repeated_success_forms_a_skill_with_provenance() {
    let clock = ManualClock::new(Timestamp(0));
    let g = ids(102);
    let mut c = Consolidator::new(ConsolidationConfig::default());
    let mut all_prov = Vec::new();
    for _ in 0..3 {
        let p = g.provenance();
        all_prov.push(p);
        c.record_attempt(Attempt {
            strategy: "cache-first".into(),
            succeeded: true,
            error_signature: None,
            provenance: vec![p],
        });
    }
    let plan = ConsolidationPlan::from_steps(vec![
        ConsolidationStep::DetectPatterns,
        ConsolidationStep::Abstract,
    ]);
    SleepCycle::begin(SystemPhase::Idle, budget(10))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    let state = c.next_state();
    assert_eq!(state.skills.len(), 1);
    assert_eq!(state.skills[0].provenance, all_prov);
    let procedural = state
        .knowledge
        .iter()
        .find(|k| k.kind == AbstractionKind::Procedural)
        .expect("works -> procedural knowledge");
    assert_eq!(procedural.provenance, all_prov);
}

#[test]
fn belief_promotion_preserves_evidence_provenance() {
    let clock = ManualClock::new(Timestamp(0));
    let g = ids(103);
    let mut c = Consolidator::new(ConsolidationConfig::default());
    let evidence: Vec<ProvenanceId> = (0..3).map(|_| g.provenance()).collect();
    for p in &evidence {
        c.record_belief_evidence("cold valves stick", true, std::slice::from_ref(p));
    }
    let plan = ConsolidationPlan::from_steps(vec![ConsolidationStep::ReevaluateBeliefs]);
    SleepCycle::begin(SystemPhase::Idle, budget(10))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    assert_eq!(
        c.belief("cold valves stick").unwrap().status,
        BeliefStatus::Believed
    );
    assert_eq!(c.belief("cold valves stick").unwrap().provenance, evidence);
    assert_eq!(
        c.belief_transitions(),
        &[("cold valves stick".to_owned(), BeliefStatus::Believed)]
    );
}

#[test]
fn merged_memories_keep_all_provenance_and_release_refs() {
    let clock = ManualClock::new(Timestamp(0));
    let g = ids(104);
    let mut c = Consolidator::new(ConsolidationConfig::default());
    let reference = |n: &str| ContentReference {
        store: "anima".into(),
        id: n.into(),
    };
    let (p1, p2) = (g.provenance(), g.provenance());
    c.track_memory(MemoryItem {
        key: "pump".into(),
        memory: reference("a"),
        relevance: 0.4,
        strength: 0.9,
        durability: Durability::Weak,
        provenance: vec![p1],
    });
    c.track_memory(MemoryItem {
        key: "pump".into(),
        memory: reference("b"),
        relevance: 0.6,
        strength: 0.8,
        durability: Durability::Weak,
        provenance: vec![p2],
    });
    let plan = ConsolidationPlan::from_steps(vec![ConsolidationStep::MergeMemories]);
    SleepCycle::begin(SystemPhase::Idle, budget(10))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    assert_eq!(c.memories().items().len(), 1);
    assert_eq!(c.memories().items()[0].provenance, vec![p1, p2]);
    assert_eq!(c.memories().merges()[0].merged, vec![reference("a")]);
}

#[test]
fn persistent_gaps_become_experiments_and_carry_over() {
    let clock = ManualClock::new(Timestamp(0));
    let g = ids(105);
    let mut c = Consolidator::new(ConsolidationConfig::default());
    c.observe_episodes(vec![
        episode(&g, "pump", 1, 0.5, Some("why does pump 3 surge?")),
        episode(&g, "pump", 2, 0.5, Some("why does pump 3 surge?")),
    ]);
    let plan = ConsolidationPlan::from_steps(vec![
        ConsolidationStep::Replay,
        ConsolidationStep::RecordGaps,
    ]);
    SleepCycle::begin(SystemPhase::Idle, budget(10))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    let state = c.next_state();
    assert_eq!(state.open_gaps, vec!["why does pump 3 surge?".to_owned()]);
    assert_eq!(state.experiments.len(), 1);
    assert!(state.priorities[0].1 > 0.0);
}

#[test]
fn causal_association_requires_provenance_and_is_kept_when_supported() {
    let clock = ManualClock::new(Timestamp(0));
    let g = ids(106);
    let mut c = Consolidator::new(ConsolidationConfig::default());
    let timing_only = Link {
        from: "rain",
        to: "flood",
        causal: true,
        provenance: &[],
    };
    assert!(c.support_association(timing_only, Timestamp(0)).is_err());
    let p = [g.provenance()];
    c.support_association(
        Link {
            from: "heat",
            to: "fault",
            causal: true,
            provenance: &p,
        },
        Timestamp(0),
    )
    .unwrap();
    let plan = ConsolidationPlan::from_steps(vec![
        ConsolidationStep::ReweightAssociations,
        ConsolidationStep::Abstract,
    ]);
    SleepCycle::begin(SystemPhase::Idle, budget(10))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    assert!(c.associations().get("heat", "fault").is_some());
    assert!(c
        .knowledge()
        .iter()
        .any(|k| k.kind == AbstractionKind::CausalModel && k.provenance == p));
}

#[test]
fn history_is_compressed_but_recent_entries_survive() {
    let clock = ManualClock::new(Timestamp(0));
    let g = ids(107);
    let mut c = Consolidator::new(ConsolidationConfig {
        keep_recent_history: 1,
        ..Default::default()
    });
    let thoughts: Vec<ThoughtId> = (0..3).map(|_| g.thought()).collect();
    for (i, t) in thoughts.iter().enumerate() {
        c.record_thought(HistoryEntry {
            thought: *t,
            at: Timestamp(i as i64),
            summary: format!("step {i}"),
            provenance: vec![g.provenance()],
        });
    }
    let plan = ConsolidationPlan::from_steps(vec![ConsolidationStep::CompressHistory]);
    SleepCycle::begin(SystemPhase::Idle, budget(10))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    assert_eq!(c.history().entries().len(), 1);
    assert_eq!(c.history().summaries()[0].covered, thoughts[..2]);
}

#[test]
fn calibration_step_fits_recorded_samples() {
    let clock = ManualClock::new(Timestamp(0));
    let mut c = Consolidator::new(ConsolidationConfig::default());
    for i in 0..10 {
        c.record_calibration(CalibrationSample {
            predicted: 0.95,
            correct: i < 5,
        });
    }
    let plan = ConsolidationPlan::from_steps(vec![ConsolidationStep::Recalibrate]);
    SleepCycle::begin(SystemPhase::Idle, budget(10))
        .unwrap()
        .run(&mut c, &plan, &clock, &mut no_interrupts);
    assert!((c.calibrated(0.95) - 0.5).abs() < 1e-9);
}

#[test]
fn scheduler_grants_sleep_only_when_resources_permit() {
    let scheduler = ThresholdScheduler::default();
    let request = SleepRequest {
        pending_steps: 11,
        backlog_episodes: 4,
    };
    let tight = ResourceSnapshot {
        compute_free: 0.05,
        memory_pressure: 0.9,
        energy_available: 0.5,
    };
    assert!(matches!(
        scheduler.grant(&request, &tight),
        SleepGrant::Deferred { .. }
    ));
    let roomy = ResourceSnapshot {
        compute_free: 0.8,
        memory_pressure: 0.2,
        energy_available: 0.8,
    };
    assert_eq!(
        scheduler.grant(&request, &roomy),
        SleepGrant::Granted(SleepBudget {
            max_steps: 11,
            max_duration_nanos: scheduler.max_duration_nanos,
            wake_priority: scheduler.wake_priority,
        })
    );
}

#[test]
fn cannot_sleep_while_active() {
    assert!(SleepCycle::begin(SystemPhase::Active, budget(1)).is_err());
}
