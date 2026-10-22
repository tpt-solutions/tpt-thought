// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cognitive modes (spec sections 21, 60). Fast and deliberative cognition are pluggable
//! strategies over providers, not hard-wired System 1 and System 2 stages. Anyone can add a mode
//! by implementing [`CognitionMode`].

use crate::calibration::CalibrationTracker;
use crate::decision::{Decision, ProviderError};
use crate::ensemble::{combine, Consensus, Disagreement};
use crate::registry::ProviderRegistry;
use crate::types::IntelligenceRequest;
use tpt_thought_core::{Probability, StructuredValue};

/// The result of one mode step.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// The mode is satisfied with this decision. No further escalation is needed.
    Accept(Decision<StructuredValue>),
    /// The mode cannot settle the question and wants more cognition.
    Escalate(Escalation),
}

/// Why a mode escalated, and what it gathered on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct Escalation {
    /// Human-readable reason.
    pub reason: String,
    /// Decisions gathered, including unaccepted ones.
    pub decisions: Vec<Decision<StructuredValue>>,
    /// Providers that failed.
    pub failures: Vec<ProviderError>,
    /// Set when providers disagreed.
    pub disagreement: Option<Disagreement>,
}

/// A strategy for answering a request from the registered providers.
pub trait CognitionMode {
    /// A stable name for telemetry and policy.
    fn name(&self) -> &str;

    /// Run one step. Calibration annotations are applied to decisions before they are judged.
    fn respond(
        &self,
        request: &IntelligenceRequest,
        registry: &ProviderRegistry,
        calibration: &CalibrationTracker,
    ) -> Step;
}

/// Asks providers in ranking order and stops at the first acceptable answer that clears the
/// fast-path threshold. Nothing else is asked, so no unnecessary escalation happens (spec section
/// 60).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FastPath {
    /// Reliability at which an answer is accepted without further asking.
    pub accept_at: Probability,
}

impl Default for FastPath {
    fn default() -> Self {
        Self {
            accept_at: Probability::new(0.9).unwrap_or(Probability::ONE),
        }
    }
}

impl CognitionMode for FastPath {
    fn name(&self) -> &str {
        "fast"
    }

    fn respond(
        &self,
        request: &IntelligenceRequest,
        registry: &ProviderRegistry,
        calibration: &CalibrationTracker,
    ) -> Step {
        let mut decisions = Vec::new();
        let mut failures = Vec::new();
        for candidate in registry.candidates(&request.constraints, calibration) {
            match candidate.provider.decide(request) {
                Ok(mut decision) => {
                    calibration.annotate(&mut decision);
                    let reliable = decision.reliability().is_some_and(|p| p >= self.accept_at);
                    if reliable && request.constraints.accepts(&decision) {
                        return Step::Accept(decision);
                    }
                    decisions.push(decision);
                }
                Err(error) => failures.push(error),
            }
        }
        Step::Escalate(Escalation {
            reason: "no fast answer reached the acceptance threshold".into(),
            decisions,
            failures,
            disagreement: None,
        })
    }
}

/// Asks providers, up to an optional limit, and combines their answers. Agreement is accepted;
/// disagreement is returned as a cognitive event for further investigation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Deliberative {
    /// Maximum number of providers to ask. `None` asks every admitted provider.
    pub max_providers: Option<usize>,
}

impl CognitionMode for Deliberative {
    fn name(&self) -> &str {
        "deliberative"
    }

    fn respond(
        &self,
        request: &IntelligenceRequest,
        registry: &ProviderRegistry,
        calibration: &CalibrationTracker,
    ) -> Step {
        let limit = self.max_providers.unwrap_or(usize::MAX);
        let mut decisions = Vec::new();
        let mut failures = Vec::new();
        for candidate in registry
            .candidates(&request.constraints, calibration)
            .into_iter()
            .take(limit)
        {
            match candidate.provider.decide(request) {
                Ok(mut decision) => {
                    calibration.annotate(&mut decision);
                    decisions.push(decision);
                }
                Err(error) => failures.push(error),
            }
        }

        match combine(&decisions) {
            Consensus::Agreed { value } => {
                let best = decisions
                    .iter()
                    .filter(|d| d.value() == Some(&value))
                    .max_by(|a, b| {
                        let ra = a.reliability().map_or(f64::NEG_INFINITY, Probability::get);
                        let rb = b.reliability().map_or(f64::NEG_INFINITY, Probability::get);
                        ra.total_cmp(&rb)
                    })
                    .cloned();
                match best {
                    Some(decision) if request.constraints.accepts(&decision) => {
                        Step::Accept(decision)
                    }
                    _ => Step::Escalate(Escalation {
                        reason: "agreed answer does not meet the constraints".into(),
                        decisions,
                        failures,
                        disagreement: None,
                    }),
                }
            }
            Consensus::Disagreed(disagreement) => Step::Escalate(Escalation {
                reason: "providers disagree".into(),
                decisions,
                failures,
                disagreement: Some(disagreement),
            }),
            Consensus::Insufficient => Step::Escalate(Escalation {
                reason: "no provider answered".into(),
                decisions,
                failures,
                disagreement: None,
            }),
        }
    }
}
