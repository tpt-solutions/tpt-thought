// SPDX-License-Identifier: MIT OR Apache-2.0
//! The closed cognitive loop (spec sections 22, 24 to 26, 61, 62, 79 to 84, 101).
//!
//! One cycle runs observe, contextualise, associate, hypothesise, evaluate, decide, intend, act,
//! observe the result, learn and consolidate. Each phase is recorded in a trajectory, and a run
//! can be captured as a [`ReplayRecord`] and re-run to check that it reproduces.
//!
//! Thought never executes actions. Intentions are handed to an [`ActionSink`], which is where
//! Anima's INP or any other executor sits. Intelligence comes from registered providers, deep
//! investigation from a [`tpt_thought_deliberation::DeliberationEngine`], and estimates from an
//! [`Estimator`]. Augur's adapter lives in `tpt-thought-integrations`, which this crate does not
//! depend on.
//!
//! The slow path is ordered: Intellect, then Eve (investigation), then Augur (estimate), then
//! Intellect again with the estimate in context (spec section 61).

mod action;
mod builder;
mod config;
mod curiosity;
mod cycle;
mod mode;
mod modulation;
mod null;
mod plan;
mod replay;
mod slow;

pub use action::{Action, ActionError, ActionSink};
pub use builder::LoopBuilder;
pub use config::LoopConfig;
pub use curiosity::{CuriosityDriver, CuriosityEvent, InvalidThreshold};
pub use cycle::default_policy;
pub use cycle::{
    CognitiveLoop, CycleOutcome, CyclePhase, CycleReport, Input, LoopError, TrajectoryStep,
};
pub use mode::CognitiveMode;
pub use modulation::{Modulation, ModulationError, ModulationFields};
pub use null::{NullActionSink, NullEngine, NullEstimator};
pub use plan::{CandidatePlan, Plan, PlanError};
pub use replay::{replay, ReplayError, ReplayMetadata, ReplayRecord};
pub use slow::{Estimate, Estimator, SlowStage};
