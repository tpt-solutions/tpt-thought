// SPDX-License-Identifier: MIT OR Apache-2.0
//! Provider-neutral deliberation (spec sections 19, 46, 61, 62 and 77).
//!
//! [`DeliberationEngine`] is the trait a reasoning engine implements. Eve is the intended
//! engine, but this crate has no dependency on Eve or any other sibling repository: the
//! `EveAdapter` belongs to the later integrations crate.
//!
//! Every capability has a default implementation that returns
//! [`DeliberationError::Unsupported`], so an engine implements only the subset it offers.
//!
//! The slow path orchestration (Intellect, then Eve, then Augur, then Intellect re-evaluation,
//! spec section 61) is deliberately **not** implemented here. It belongs to `tpt-thought-loop`,
//! which composes engines with providers. This crate supplies the single-engine pieces and the
//! bounded deep-investigation loop (spec section 62, [`run_investigation`]).

mod engine;
mod investigation;
mod metacognition;
pub mod mock;
mod snapshot;

pub use engine::{
    AbstractionRequest, AbstractionVerdict, ActiveLearningRequest, Capability, CausalAnalysis,
    CausalQuery, CausalRequest, Claim, Conflict, ContradictionAnalysis, ContradictionRequest,
    DeliberationEngine, DeliberationError, ExperimentPlan, ExperimentRequest, HypothesisRequest,
    QueryCandidate, Reasoning, Resolution, Validity,
};
pub use investigation::{
    run_investigation, Investigation, InvestigationOutcome, InvestigationPolicy, InvestigationRun,
    Termination,
};
pub use metacognition::{Metacognition, NextAction};
pub use snapshot::{DeliberationContext, ThoughtSnapshot};
