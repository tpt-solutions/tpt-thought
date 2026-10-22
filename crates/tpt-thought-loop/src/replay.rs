// SPDX-License-Identifier: MIT OR Apache-2.0
//! Replay (spec sections 32, 33).
//!
//! A [`ReplayRecord`] holds everything needed to re-run a loop: its configuration (including the
//! identifier seed), the inputs in order, the identities of the providers and models that were
//! involved, and the trajectory that resulted. Replaying rebuilds the loop, feeds it the same
//! inputs and checks that every step matches.

use crate::action::ActionSink;
use crate::config::LoopConfig;
use crate::cycle::{CognitiveLoop, CycleReport, Input, LoopError, TrajectoryStep};
use crate::slow::Estimator;
use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_deliberation::DeliberationEngine;

/// Identities of the backends and memories a run depended on (spec section 32). These are
/// recorded so a replay can be checked against the same backends; they are not used to choose
/// behaviour during replay.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReplayMetadata {
    /// Provider identifiers, in registration order.
    pub providers: Vec<String>,
    /// Model identifiers the providers reported.
    pub model_ids: Vec<String>,
    /// Backend versions.
    pub backend_versions: Vec<String>,
    /// Memory references the inputs drew on.
    pub memory_refs: Vec<String>,
}

/// A recorded run that can be re-run and checked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayRecord {
    /// The configuration in force, including the identifier seed.
    pub config: LoopConfig,
    /// Backend and memory identities.
    pub metadata: ReplayMetadata,
    /// The inputs, in order.
    pub inputs: Vec<Input>,
    /// The trajectory the run produced.
    pub trajectory: Vec<TrajectoryStep>,
}

impl ReplayRecord {
    /// Run `inputs` through a loop built by `build` and record the result. `build` is called with
    /// the configuration, so it can construct providers and executors deterministically.
    pub fn capture<E, A, S>(
        config: LoopConfig,
        metadata: ReplayMetadata,
        inputs: Vec<Input>,
        build: impl Fn(&LoopConfig) -> Result<CognitiveLoop<E, A, S>, LoopError>,
    ) -> Result<Self, LoopError>
    where
        E: DeliberationEngine,
        A: Estimator,
        S: ActionSink,
    {
        let mut lp = build(&config)?;
        for input in &inputs {
            lp.run(input)?;
        }
        Ok(Self {
            config,
            metadata,
            inputs,
            trajectory: lp.trajectory().to_vec(),
        })
    }
}

/// A replay that could not reproduce its record.
#[derive(Debug)]
pub enum ReplayError {
    /// The loop failed while replaying.
    Run(LoopError),
    /// A step differed from the record.
    Diverged {
        /// Position of the first difference.
        index: usize,
        /// The recorded step.
        expected: Box<TrajectoryStep>,
        /// The step the replay produced.
        actual: Box<TrajectoryStep>,
    },
    /// The replay produced a different number of steps.
    Length {
        /// Steps in the record.
        expected: usize,
        /// Steps produced.
        actual: usize,
    },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Run(e) => write!(f, "replay failed: {e}"),
            Self::Diverged { index, .. } => write!(f, "replay diverged at step {index}"),
            Self::Length { expected, actual } => {
                write!(f, "replay produced {actual} steps, record has {expected}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// Re-run a record and check that the trajectory matches step for step. `build` must construct
/// the same providers, engine, estimator and executor that the original run used.
pub fn replay<E, A, S>(
    record: &ReplayRecord,
    build: impl Fn(&LoopConfig) -> Result<CognitiveLoop<E, A, S>, LoopError>,
) -> Result<Vec<CycleReport>, ReplayError>
where
    E: DeliberationEngine,
    A: Estimator,
    S: ActionSink,
{
    let mut lp = build(&record.config).map_err(ReplayError::Run)?;
    let mut reports = Vec::with_capacity(record.inputs.len());
    for input in &record.inputs {
        reports.push(lp.run(input).map_err(ReplayError::Run)?);
    }
    let actual = lp.trajectory();
    for (index, (a, e)) in actual.iter().zip(&record.trajectory).enumerate() {
        if a != e {
            return Err(ReplayError::Diverged {
                index,
                expected: Box::new(e.clone()),
                actual: Box::new(a.clone()),
            });
        }
    }
    if actual.len() != record.trajectory.len() {
        return Err(ReplayError::Length {
            expected: record.trajectory.len(),
            actual: actual.len(),
        });
    }
    Ok(reports)
}
