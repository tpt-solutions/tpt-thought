// SPDX-License-Identifier: MIT OR Apache-2.0
//! Competing explanations: branch, reject the weakest, then collapse to one choice.
//!
//! Considering a hypothesis never makes it a belief. Collapse only picks the branch to act on.
//!
//! Run with `cargo run -p tpt-thought --example hypotheses`.

use tpt_thought::core::{IdGenerator, Probability, SeededEntropy};
use tpt_thought::hypothesis::{collapse::collapse, collapse::CollapseStrategy, Branch, BranchSet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ids = IdGenerator::new(SeededEntropy::new(42));
    let mut branches = BranchSet::new(3);

    let explanations = [
        ("the gripper slipped", 0.5),
        ("the cup is heavier than expected", 0.3),
        ("the sensor is miscalibrated", 0.2),
    ];
    for (statement, prior) in explanations {
        let prior = Probability::new(prior)?;
        branches.explore(Branch::new(ids.hypothesis(), statement, prior))?;
    }
    println!("explored {} branches", branches.active_count());

    // Evidence has moved the weights. Reject the one that the evidence contradicts.
    let weakest = branches
        .branches()
        .iter()
        .min_by(|a, b| a.probability().get().total_cmp(&b.probability().get()))
        .map(|b| b.id())
        .expect("at least one branch");
    branches.reject(&weakest)?;
    println!(
        "rejected the weakest explanation; {} remain",
        branches.active_count()
    );

    let chosen = collapse(&branches, CollapseStrategy::Probability)?;
    let statement = branches
        .get(&chosen)
        .map(|b| b.hypothesis.statement.clone())
        .unwrap_or_default();
    println!("collapsed by probability to: {statement}");
    Ok(())
}
