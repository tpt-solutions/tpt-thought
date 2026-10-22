// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from the `tpt-augur` probabilistic language to Thought posteriors (spec sections 49,
//! 50; todo.md Phase 10 `AugurAdapter`).
//!
//! Augur takes a model written in its own language, then runs inference over it. This adapter
//! runs the whole chain (parse, lower, infer) and returns one [`PosteriorRecord`] per parameter.
//! Each record carries the posterior's residual uncertainty. Augur has no evidence-update API
//! and no residual-uncertainty API of its own, so the residual values are derived here from the
//! posterior summaries.

use tpt_augur_frontend::parse;
use tpt_augur_ir::lower::lower;
use tpt_augur_runtime::{run, InferOptions, ParamSummary};

/// A parameter's posterior, as Thought consumes it.
#[derive(Debug, Clone, PartialEq)]
pub struct PosteriorRecord {
    /// The parameter's name in the Augur model.
    pub name: String,
    /// Posterior mean.
    pub mean: f64,
    /// Posterior standard deviation: the residual uncertainty of the estimate.
    pub sd: f64,
    /// Central 95% credible interval, as (lower, upper).
    pub interval95: (f64, f64),
    /// Split-chain potential scale reduction. Values near 1.0 indicate the chains agree.
    pub rhat: f64,
    /// Effective sample size.
    pub ess: f64,
}

impl PosteriorRecord {
    /// Width of the 95% interval, a second view of the residual uncertainty.
    pub fn interval_width(&self) -> f64 {
        self.interval95.1 - self.interval95.0
    }

    fn from_summary(summary: &ParamSummary) -> Self {
        Self {
            name: summary.name.clone(),
            mean: summary.mean,
            sd: summary.sd,
            interval95: (summary.q2_5, summary.q97_5),
            rhat: summary.rhat,
            ess: summary.ess,
        }
    }
}

/// Why an Augur model could not be turned into posteriors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AugurError {
    /// The source did not parse. Carries the error messages.
    Parse(Vec<String>),
    /// The parsed program could not be lowered to a model. Carries the error messages.
    Lower(Vec<String>),
}

/// Runs Augur models with fixed inference options.
pub struct AugurAdapter {
    options: InferOptions,
}

impl AugurAdapter {
    /// An adapter that runs inference with `options`.
    pub fn new(options: InferOptions) -> Self {
        Self { options }
    }

    /// Parse, lower and infer `source`, returning one posterior per parameter in model order.
    ///
    /// # Errors
    /// [`AugurError::Parse`] if the source has parse errors, and [`AugurError::Lower`] if it
    /// parses but cannot be lowered. Warnings are not errors.
    pub fn infer(&self, source: &str) -> Result<Vec<PosteriorRecord>, AugurError> {
        let parsed = parse(source);
        let parse_errors = errors(&parsed.diagnostics);
        if !parse_errors.is_empty() {
            return Err(AugurError::Parse(parse_errors));
        }
        let lowered = lower(&parsed.program);
        let lower_errors = errors(&lowered.diagnostics);
        if !lower_errors.is_empty() {
            return Err(AugurError::Lower(lower_errors));
        }
        let result = run(&lowered.model, &self.options);
        Ok(result
            .summaries
            .iter()
            .map(PosteriorRecord::from_summary)
            .collect())
    }
}

fn errors<D: DiagnosticLike>(diagnostics: &[D]) -> Vec<String> {
    diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.message().to_string())
        .collect()
}

trait DiagnosticLike {
    fn is_error(&self) -> bool;
    fn message(&self) -> &str;
}

impl DiagnosticLike for tpt_augur_frontend::Diagnostic {
    fn is_error(&self) -> bool {
        self.severity == tpt_augur_frontend::Severity::Error
    }
    fn message(&self) -> &str {
        &self.message
    }
}

impl DiagnosticLike for tpt_augur_ir::lower::Diagnostic {
    fn is_error(&self) -> bool {
        self.severity == tpt_augur_ir::lower::Severity::Error
    }
    fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORMAL_MEAN: &str = "let mu ~ Normal(0, 1)\nobserve Normal(mu, 1) = 0.5\n";

    fn options() -> InferOptions {
        InferOptions {
            num_chains: 2,
            num_warmup: 500,
            num_samples: 2_000,
            seed: 11,
            ..InferOptions::default()
        }
    }

    #[test]
    fn normal_mean_posterior_matches_the_closed_form() {
        // Prior N(0, 1) with one observation of 0.5 under unit noise gives a posterior with
        // mean 0.25 and standard deviation 1 / sqrt(2), about 0.707.
        let posteriors = AugurAdapter::new(options()).infer(NORMAL_MEAN).unwrap();
        let mu = posteriors
            .iter()
            .find(|p| p.name == "mu")
            .expect("a posterior for mu");
        assert!((mu.mean - 0.25).abs() < 0.15, "mean was {}", mu.mean);
        assert!((mu.sd - 0.707).abs() < 0.15, "sd was {}", mu.sd);
        assert!(mu.interval95.0 < mu.mean && mu.mean < mu.interval95.1);
        assert!(mu.interval_width() > 0.0);
    }

    #[test]
    fn unparseable_source_is_a_parse_error() {
        let result = AugurAdapter::new(options()).infer("let mu ~ Normal(0,");
        assert!(matches!(result, Err(AugurError::Parse(messages)) if !messages.is_empty()));
    }
}
