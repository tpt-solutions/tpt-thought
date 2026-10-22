// SPDX-License-Identifier: MIT OR Apache-2.0
//! Quick run of the benchmark scenarios under `cargo test`. It checks the deterministic counts,
//! and never the timings.

mod common;

use common::scenarios;

#[test]
fn benchmark_scenarios_hold_their_deterministic_invariants() {
    let m = scenarios::run_all(25);
    assert_eq!(
        m.correct_actions, m.cycles,
        "every reliable cycle takes the right action"
    );
    assert_eq!(
        m.prediction_matches, m.cycles,
        "the echo executor matches every prediction"
    );
    assert_eq!(
        m.provider_calls_per_cycle, 1.0,
        "one provider call per cycle"
    );
    assert_eq!(
        m.unnecessary_escalations, 0,
        "reliable answers do not escalate"
    );
    assert_eq!(
        m.missed_escalations, 0,
        "unreliable answers always escalate"
    );
    assert_eq!(m.memory_recall, 1.0, "each cue recalls its memory");
    assert_eq!(
        m.branches_admitted, 8,
        "the bound admits exactly eight branches"
    );
    assert!(m.active_branches <= 8, "the branch bound is never exceeded");
    assert!(
        scenarios::replay_steps(3) > 0,
        "a recorded run has trajectory steps"
    );
}
