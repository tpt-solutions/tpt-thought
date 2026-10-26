// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from the `tpt-augur` probabilistic language to Thought posteriors (spec sections 49,
//! 50; todo.md Phase 10 `AugurAdapter`).
//!
//! Augur takes a model written in its own language, then runs inference over it. This adapter
//! runs the whole chain (parse, lower, infer) and returns one [`PosteriorRecord`] per parameter.
//! Each record carries the posterior's residual uncertainty, and, for a discrete parameter, the
//! possible states with their posterior probabilities (spec section 49: "What are the possible
//! states?"). Augur reports neither of these directly. Residuals come from the posterior summaries
//! and states come from the posterior draws.

use std::collections::BTreeMap;
use tpt_augur_frontend::parse;
use tpt_augur_ir::lower::lower;
use tpt_augur_ir::Model;
use tpt_augur_runtime::{
    hmc, mh, pf, residual_of, select_engine, summarize, vi, InferenceResult, ParamSummary,
    Residual, Session, SessionError,
};
use tpt_thought_core::{Probability, Uncertainty};

pub use tpt_augur_runtime::{Engine, InferOptions};

/// The most distinct values a parameter may take and still be reported as discrete states. A
/// parameter with more distinct draws is treated as continuous and has no states.
pub const MAX_STATES: usize = 64;

/// Tolerance for treating a draw as an integer.
const INTEGER_TOLERANCE: f64 = 1e-9;

/// One possible state of a discrete parameter, with its posterior probability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PossibleState {
    /// The value the parameter takes in this state.
    pub value: f64,
    /// Posterior probability of this state, estimated from the draws.
    pub probability: Probability,
}

/// The possible states of a discrete parameter from its posterior draws, most probable first.
///
/// Returns `None` when a draw is not an integer, when there are no draws, or when there are more
/// than [`MAX_STATES`] distinct values. The states are those seen in the draws, so a value that
/// was never drawn is not listed. Probabilities sum to one.
pub fn states_of(draws: &[f64]) -> Option<Vec<PossibleState>> {
    if draws.is_empty() {
        return None;
    }
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for &draw in draws {
        if !draw.is_finite() || (draw - draw.round()).abs() > INTEGER_TOLERANCE {
            return None;
        }
        *counts.entry(draw.round() as i64).or_default() += 1;
        if counts.len() > MAX_STATES {
            return None;
        }
    }
    let total = draws.len() as f64;
    let mut states: Vec<PossibleState> = counts
        .into_iter()
        .map(|(value, count)| PossibleState {
            value: value as f64,
            probability: Probability::new(count as f64 / total)
                .expect("a ratio of counts is a probability"),
        })
        .collect();
    states.sort_by(|a, b| b.probability.get().total_cmp(&a.probability.get()));
    Some(states)
}

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
    /// The possible states, for a discrete parameter. `None` for a continuous one.
    pub states: Option<Vec<PossibleState>>,
}

impl PosteriorRecord {
    /// Width of the 95% interval, a second view of the residual uncertainty.
    pub fn interval_width(&self) -> f64 {
        self.interval95.1 - self.interval95.0
    }

    fn from_summary(summary: &ParamSummary, draws: &[f64]) -> Self {
        Self {
            name: summary.name.clone(),
            mean: summary.mean,
            sd: summary.sd,
            interval95: (summary.q2_5, summary.q97_5),
            rhat: summary.rhat,
            ess: summary.ess,
            states: states_of(draws),
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

impl std::fmt::Display for AugurError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(errors) => write!(f, "model does not parse: {}", errors.join("; ")),
            Self::Lower(errors) => write!(f, "model does not lower: {}", errors.join("; ")),
        }
    }
}

impl std::error::Error for AugurError {}

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
        let model = lower_checked(source)?;
        let (result, draws) = run_with_draws(&model, &self.options);
        Ok(posteriors(&result, &draws))
    }
}

/// Parses and lowers `source`, failing on any error diagnostic.
fn lower_checked(source: &str) -> Result<Model, AugurError> {
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
    Ok(lowered.model)
}

/// Runs inference over `model` and keeps the draws as well as the summaries.
///
/// This follows `tpt_augur_runtime::run` step for step: the engine is chosen the same way, and the
/// summaries are built the same way. The draws are what `run` discards. A test checks that the
/// summaries agree with `run`'s. If `run` changes, this must change with it.
fn run_with_draws(model: &Model, options: &InferOptions) -> (InferenceResult, Vec<Vec<f64>>) {
    let engine = options.engine.unwrap_or_else(|| select_engine(model));
    let trace = match engine {
        Engine::MetropolisHastings => mh::run_all(model, options),
        Engine::Hmc => hmc::run_all(model, options),
        Engine::Variational => vi::run_all(model, options),
        Engine::ParticleFilter => pf::run_all(model, options),
    };
    let result = summarize(&trace, &model.prior_order, engine);
    (result, trace.flatten())
}

/// One posterior per summary, with states taken from the matching draws.
fn posteriors(result: &InferenceResult, draws: &[Vec<f64>]) -> Vec<PosteriorRecord> {
    result
        .summaries
        .iter()
        .enumerate()
        .map(|(index, summary)| {
            let parameter_draws = draws.get(index).map(Vec::as_slice).unwrap_or(&[]);
            PosteriorRecord::from_summary(summary, parameter_draws)
        })
        .collect()
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

/// Maps a residual onto Thought's uncertainty. The posterior spread is reducible uncertainty
/// from lack of knowledge, so it becomes `epistemic`. A posterior whose chains disagree or whose
/// effective sample size is too small is marked `unknown`: its summary cannot be trusted, so the
/// answer is unknown rather than merely uncertain. No confidence or probability is claimed, since
/// Augur reports neither.
pub fn uncertainty_of(residual: &Residual) -> Uncertainty {
    Uncertainty {
        epistemic: Some(residual.sd),
        unknown: !residual.converged,
        ..Uncertainty::default()
    }
}

/// A posterior with its uncertainty in Thought's terms.
#[derive(Debug, Clone, PartialEq)]
pub struct AssessedPosterior {
    /// The posterior summary as Augur reported it, including any possible states.
    pub posterior: PosteriorRecord,
    /// The residual uncertainty of the posterior.
    pub residual: Residual,
    /// The same residual, expressed as Thought's uncertainty.
    pub uncertainty: Uncertainty,
}

/// Evidence accumulated over an Augur model, for Thought to update as observations arrive.
///
/// Each [`AugurSession::observe`] appends evidence to the model and each [`AugurSession::infer`]
/// re-runs it from source, so results are deterministic for a given evidence sequence.
pub struct AugurSession {
    session: Session,
    options: InferOptions,
}

impl AugurSession {
    /// A session for `model_source` with fixed inference `options`.
    pub fn new(model_source: &str, options: InferOptions) -> Result<Self, AugurError> {
        Session::new(model_source, options.clone())
            .map(|session| Self { session, options })
            .map_err(augur_error)
    }

    /// Add one `observe` statement, for example `observe Normal(mu, 1) = 0.5`. A rejected
    /// statement leaves the session unchanged.
    pub fn observe(&mut self, statement: &str) -> Result<(), AugurError> {
        self.session.observe(statement).map_err(augur_error)
    }

    /// Number of accepted observations.
    pub fn observations(&self) -> usize {
        self.session.observations()
    }

    /// Run inference with all evidence so far and assess every posterior.
    pub fn infer(&self) -> Vec<AssessedPosterior> {
        // The session checked its source when each statement was accepted, so this cannot fail.
        let model = lower_checked(self.session.source())
            .expect("an accepted session source lowers to a model");
        let (result, draws) = run_with_draws(&model, &self.options);
        posteriors(&result, &draws)
            .into_iter()
            .zip(result.summaries.iter())
            .map(|(posterior, summary)| {
                let residual = residual_of(summary);
                AssessedPosterior {
                    posterior,
                    uncertainty: uncertainty_of(&residual),
                    residual,
                }
            })
            .collect()
    }
}

fn augur_error(error: SessionError) -> AugurError {
    match error {
        SessionError::Parse(messages) => AugurError::Parse(messages),
        SessionError::Lower(messages) => AugurError::Lower(messages),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_augur_runtime::run;

    const NORMAL_MEAN: &str = "let mu ~ Normal(0, 1)\nobserve Normal(mu, 1) = 0.5\n";
    const DISCRETE: &str = "let k ~ Binomial(10, 0.5)\nobserve Normal(k, 1) = 6\n";

    fn options() -> InferOptions {
        InferOptions {
            num_chains: 2,
            num_warmup: 500,
            num_samples: 2_000,
            seed: 11,
            ..InferOptions::default()
        }
    }

    fn total(states: &[PossibleState]) -> f64 {
        states.iter().map(|s| s.probability.get()).sum()
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
    fn continuous_parameter_has_no_states() {
        let posteriors = AugurAdapter::new(options()).infer(NORMAL_MEAN).unwrap();
        assert!(posteriors[0].states.is_none());
    }

    #[test]
    fn discrete_parameter_reports_its_possible_states() {
        let posteriors = AugurAdapter::new(options()).infer(DISCRETE).unwrap();
        let k = posteriors.iter().find(|p| p.name == "k").expect("k");
        let states = k.states.as_ref().expect("k is discrete");

        assert!((total(states) - 1.0).abs() < 1e-9, "states sum to one");
        assert!(states.iter().all(|s| s.value.fract() == 0.0));
        assert!(
            states
                .windows(2)
                .all(|w| w[0].probability >= w[1].probability),
            "most probable first"
        );
        let top = states[0].value;
        assert!(
            (4.0..=8.0).contains(&top),
            "observation of 6 favours states near 6, got {top}"
        );
    }

    #[test]
    fn states_are_none_for_continuous_or_too_many_values() {
        assert!(states_of(&[]).is_none());
        assert!(states_of(&[0.5, 1.0]).is_none(), "a non-integer draw");
        let many: Vec<f64> = (0..=MAX_STATES as i64).map(|v| v as f64).collect();
        assert!(states_of(&many).is_none(), "more than MAX_STATES values");
    }

    #[test]
    fn states_are_counted_and_sorted() {
        let states = states_of(&[2.0, 2.0, 2.0, 5.0]).expect("discrete");
        assert_eq!(states[0].value, 2.0);
        assert!((states[0].probability.get() - 0.75).abs() < 1e-12);
        assert_eq!(states[1].value, 5.0);
        assert!((total(&states) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn draw_path_summaries_match_augur_run() {
        // The adapter re-implements the engine dispatch of `run` to keep the draws. Both are
        // seeded, so the summaries must agree exactly.
        let model = lower_checked(NORMAL_MEAN).unwrap();
        let opts = options();
        let via_run = run(&model, &opts);
        let (via_draws, _) = run_with_draws(&model, &opts);
        assert_eq!(via_run.summaries.len(), via_draws.summaries.len());
        for (a, b) in via_run.summaries.iter().zip(&via_draws.summaries) {
            assert_eq!(a.mean, b.mean);
            assert_eq!(a.sd, b.sd);
        }
    }

    #[test]
    fn unparseable_source_is_a_parse_error() {
        let result = AugurAdapter::new(options()).infer("let mu ~ Normal(0,");
        assert!(matches!(result, Err(AugurError::Parse(messages)) if !messages.is_empty()));
    }

    #[test]
    fn session_posterior_narrows_with_each_observation() {
        let mut session = AugurSession::new("let mu ~ Normal(0, 1)", options()).unwrap();
        session.observe("observe Normal(mu, 1) = 0.5").unwrap();
        let after_one = session.infer();
        session.observe("observe Normal(mu, 1) = 0.5").unwrap();
        session.observe("observe Normal(mu, 1) = 0.5").unwrap();
        let after_three = session.infer();
        assert_eq!(session.observations(), 3);

        let sd_one = after_one[0].residual.sd;
        let sd_three = after_three[0].residual.sd;
        assert!(sd_three < sd_one, "{sd_three} should be below {sd_one}");
        assert_eq!(after_three[0].uncertainty.epistemic, Some(sd_three));
    }

    #[test]
    fn session_reports_states_for_discrete_parameters() {
        let session = AugurSession::new(DISCRETE, options()).unwrap();
        let assessed = session.infer();
        let states = assessed[0].posterior.states.as_ref().expect("discrete");
        assert!((total(states) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn session_maps_parse_errors_to_augur_errors() {
        assert!(matches!(
            AugurSession::new("let mu ~ Normal(0,", options()),
            Err(AugurError::Parse(_))
        ));
        let mut session = AugurSession::new("let mu ~ Normal(0, 1)", options()).unwrap();
        assert!(session.observe("observe Normal(mu, 1) = ").is_err());
        assert_eq!(session.observations(), 0);
    }

    #[test]
    fn unconverged_posterior_is_unknown_not_zero_uncertainty() {
        let converged = Residual {
            sd: 0.4,
            interval95_width: 1.5,
            relative: Some(0.8),
            converged: true,
        };
        let u = uncertainty_of(&converged);
        assert_eq!(u.epistemic, Some(0.4));
        assert!(!u.unknown);
        assert!(u.confidence.is_none() && u.probability.is_none());

        let split = Residual {
            converged: false,
            ..converged
        };
        assert!(uncertainty_of(&split).unknown);
    }
}
