// SPDX-License-Identifier: MIT OR Apache-2.0
//! Predictions, prediction errors and the error response ladder (spec sections 17 and 18).
//!
//! A `Prediction` (from `tpt-thought-core`) is tracked until an observation resolves it.
//! Matching produces a [`PredictionError`], which is reported as a [`PredictionEvent`] and
//! mapped to a [`ResponsePlan`] by a [`ResponseLadder`]. [`fanout::dispatch`] then delivers the
//! error to the subsystems that learn from it.

mod error;
pub mod fanout;
mod ladder;
mod tracker;

pub use error::{compare, value_error, MatchConfig, NotComparable, PredictionError};
pub use fanout::{dispatch, ErrorFanOut};
pub use ladder::{ErrorLevel, LadderError, ResponseAction, ResponseLadder, ResponsePlan};
pub use tracker::{PredictionEvent, PredictionTracker, ResolveError, TrackError};
