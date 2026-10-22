// SPDX-License-Identifier: MIT OR Apache-2.0
//! Benchmark harness for the cognitive pipeline (spec section 96).
//!
//! Run with `cargo bench -p tpt-thought-tests`. The counted metrics are deterministic. Timings are
//! wall-clock and informational: they vary by machine and must not be used as pass/fail gates.

#[path = "../tests/common/mod.rs"]
mod common;

use common::scenarios;

const CYCLES: usize = 500;

fn main() {
    let m = scenarios::run_all(CYCLES);
    let replay_steps = scenarios::replay_steps(CYCLES);

    println!("TPT Thought pipeline benchmark ({CYCLES} cycles, deterministic seed)");
    println!();
    println!("Counted (deterministic):");
    println!(
        "  decision quality          {}/{} correct actions",
        m.correct_actions, m.cycles
    );
    println!(
        "  prediction accuracy       {}/{} exact matches",
        m.prediction_matches, m.cycles
    );
    println!(
        "  compute cost              {:.2} provider calls per cycle",
        m.provider_calls_per_cycle
    );
    println!(
        "  unnecessary escalations   {} of {} reliable cases",
        m.unnecessary_escalations, CYCLES
    );
    println!(
        "  missed escalations        {} of {} unreliable cases",
        m.missed_escalations, CYCLES
    );
    println!(
        "  memory utilisation        {:.0}% of relevant memories recalled",
        m.memory_recall * 100.0
    );
    println!(
        "  branch explosion          {} admitted of {} offered, {} active (bound 8)",
        m.branches_admitted, m.branches_offered, m.active_branches
    );
    println!("  replay trajectory steps   {replay_steps}");
    println!();
    println!("Timed (informational only):");
    println!(
        "  time to decision          {:.1} us per cycle",
        m.mean_cycle_nanos / 1_000.0
    );
}
