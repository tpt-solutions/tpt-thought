// SPDX-License-Identifier: MIT OR Apache-2.0
//! Evidence updates with Augur. Each observation narrows the posterior, and the residual
//! uncertainty Thought records shrinks with it.
//!
//! Needs the `augur` feature, and the Augur revision that provides evidence sessions:
//! `cargo run -p tpt-thought --features augur --example augur_evidence`.

use tpt_thought::integrations::augur::{AugurSession, Engine, InferOptions};

fn options() -> InferOptions {
    InferOptions {
        engine: Some(Engine::Hmc),
        num_chains: 2,
        num_warmup: 500,
        num_samples: 1_500,
        seed: 11,
        ..InferOptions::default()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A grip-strength prior, centred on zero with unit spread.
    let mut session = AugurSession::new("let mu ~ Normal(0, 1)", options())?;
    print_state(&session);

    // Each observation of the grip reading adds one unit of precision to the estimate.
    for reading in [0.5, 0.4, 0.6] {
        session.observe(&format!("observe Normal(mu, 1) = {reading}"))?;
        print_state(&session);
    }

    // A malformed observation is refused and leaves the evidence unchanged.
    let refused = session.observe("observe Normal(mu, 1) = ");
    println!("malformed observation refused: {}", refused.is_err());
    println!(
        "evidence still holds {} observations",
        session.observations()
    );
    Ok(())
}

fn print_state(session: &AugurSession) {
    for posterior in session.infer() {
        println!(
            "after {} observation(s): {} mean {:.3}, sd {:.3}, epistemic {:?}, unknown {}",
            session.observations(),
            posterior.posterior.name,
            posterior.posterior.mean,
            posterior.residual.sd,
            posterior.uncertainty.epistemic,
            posterior.uncertainty.unknown,
        );
    }
}
