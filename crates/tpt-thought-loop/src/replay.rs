// SPDX-License-Identifier: MIT OR Apache-2.0
//! Replay (spec sections 32, 33).
//!
//! A [`ReplayRecord`] holds everything needed to re-run a loop: its configuration (including the
//! identifier seed), the inputs in order, the identities of the providers and models that were
//! involved, the trajectory that resulted and the provenance each step wrote. Replaying first
//! checks the recorded provenance chain against its integrity hashes, then rebuilds the loop,
//! feeds it the same inputs and checks that every step and every provenance record matches.

use crate::action::ActionSink;
use crate::config::LoopConfig;
use crate::cycle::{CognitiveLoop, CycleReport, Input, LoopError, StepProvenance, TrajectoryStep};
use crate::slow::Estimator;
use serde::{Deserialize, Serialize};
use std::fmt;
use tpt_thought_core::ProvenanceId;
use tpt_thought_deliberation::DeliberationEngine;
use tpt_thought_provenance::{IntegrityError, ProvenanceStore};

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
///
/// Format: records captured before provenance was recorded have no `provenance` key. They
/// deserialise with `provenance` set to `None`, and replay checks only the trajectory, as it did
/// before. A record that carries the key is checked against its integrity hashes and against the
/// re-run. Removing the key turns those checks off, so keep the key in records that must be
/// checked.
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
    /// The provenance records the run wrote, in write order, each tagged with its step. `None`
    /// for records captured before provenance was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Vec<StepProvenance>>,
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
            provenance: Some(lp.provenance_log().to_vec()),
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
    /// The recorded provenance chain fails its integrity check, so a record was altered or the
    /// chain is broken. Nothing was re-run.
    Integrity {
        /// The trajectory step that wrote the faulty record.
        step: usize,
        /// What the store check found.
        error: IntegrityError,
    },
    /// A provenance record written by the re-run differs from the recorded one in one field.
    Provenance {
        /// The trajectory step that wrote the record.
        step: usize,
        /// The record that differed.
        record: ProvenanceId,
        /// The field that differed, named as in `ProvenanceBody`.
        field: &'static str,
        /// The recorded value, formatted for display.
        expected: String,
        /// The value the replay produced, formatted for display.
        actual: String,
    },
    /// The re-run wrote a different number of provenance records.
    ProvenanceLength {
        /// Provenance records in the record.
        expected: usize,
        /// Provenance records produced.
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
            Self::Integrity { step, error } => {
                write!(
                    f,
                    "provenance integrity check failed at step {step}: {error}"
                )
            }
            Self::Provenance {
                step,
                record,
                field,
                expected,
                actual,
            } => write!(
                f,
                "provenance record {record} from step {step} differs in {field}: \
                 record has {expected}, replay produced {actual}"
            ),
            Self::ProvenanceLength { expected, actual } => {
                write!(
                    f,
                    "replay wrote {actual} provenance records, record has {expected}"
                )
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// Re-run a record and check that the trajectory matches step for step, and that the provenance
/// matches record for record. The recorded provenance chain is checked against its integrity
/// hashes before the re-run, so an altered record is rejected without running. `build` must
/// construct the same providers, engine, estimator and executor that the original run used.
pub fn replay<E, A, S>(
    record: &ReplayRecord,
    build: impl Fn(&LoopConfig) -> Result<CognitiveLoop<E, A, S>, LoopError>,
) -> Result<Vec<CycleReport>, ReplayError>
where
    E: DeliberationEngine,
    A: Estimator,
    S: ActionSink,
{
    if let Some(recorded) = &record.provenance {
        verify_chain(recorded)?;
    }
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
    if let Some(recorded) = &record.provenance {
        compare_provenance(recorded, lp.provenance_log())?;
    }
    Ok(reports)
}

/// Rebuild the recorded chain as a store, keeping the stored hashes, then verify every hash and
/// parent link. A tampered body or hash fails here.
fn verify_chain(recorded: &[StepProvenance]) -> Result<(), ReplayError> {
    let integrity = |error: IntegrityError| ReplayError::Integrity {
        step: step_of(recorded, &error),
        error,
    };
    let store = ProvenanceStore::restore(recorded.iter().map(|entry| entry.record.clone()))
        .map_err(integrity)?;
    store.verify().map_err(integrity)
}

/// The trajectory step that wrote the record an integrity error names. Falls back to step zero
/// only if the named record is not in the chain, which the checks above rule out.
fn step_of(recorded: &[StepProvenance], error: &IntegrityError) -> usize {
    let record = match error {
        IntegrityError::MissingParent { record, .. }
        | IntegrityError::HashMismatch { record }
        | IntegrityError::Duplicate { record } => record,
    };
    recorded
        .iter()
        .find(|entry| entry.record.body.id == *record)
        .map_or(0, |entry| entry.step)
}

/// Compare the re-run's provenance with the record, one field at a time, in write order.
fn compare_provenance(
    expected: &[StepProvenance],
    actual: &[StepProvenance],
) -> Result<(), ReplayError> {
    for (e, a) in expected.iter().zip(actual) {
        let (step, record) = (e.step, e.record.body.id);
        let (eb, ab) = (&e.record.body, &a.record.body);
        same(step, record, "step", &e.step, &a.step)?;
        same(step, record, "id", &eb.id, &ab.id)?;
        same(step, record, "source", &eb.source, &ab.source)?;
        same(step, record, "thought", &eb.thought, &ab.thought)?;
        same(step, record, "parents", &eb.parents, &ab.parents)?;
        same(step, record, "evidence", &eb.evidence, &ab.evidence)?;
        same(step, record, "memory", &eb.memory, &ab.memory)?;
        same(step, record, "provider", &eb.provider, &ab.provider)?;
        same(step, record, "model", &eb.model, &ab.model)?;
        same(step, record, "version", &eb.version, &ab.version)?;
        same(
            step,
            record,
            "reasoning_path",
            &eb.reasoning_path,
            &ab.reasoning_path,
        )?;
        same(
            step,
            record,
            "verification",
            &eb.verification,
            &ab.verification,
        )?;
        same(step, record, "timestamp", &eb.timestamp, &ab.timestamp)?;
        same(step, record, "hash", &e.record.hash, &a.record.hash)?;
    }
    if expected.len() != actual.len() {
        return Err(ReplayError::ProvenanceLength {
            expected: expected.len(),
            actual: actual.len(),
        });
    }
    Ok(())
}

/// Ok if the recorded and replayed values agree, otherwise a [`ReplayError::Provenance`] naming
/// the field.
fn same<T: PartialEq + fmt::Debug>(
    step: usize,
    record: ProvenanceId,
    field: &'static str,
    expected: &T,
    actual: &T,
) -> Result<(), ReplayError> {
    if expected == actual {
        return Ok(());
    }
    Err(ReplayError::Provenance {
        step,
        record,
        field,
        expected: format!("{expected:?}"),
        actual: format!("{actual:?}"),
    })
}
