// SPDX-License-Identifier: MIT OR Apache-2.0
//! Deterministic backends for tests and examples. No external model is involved, so behaviour is
//! fully predictable. [`FailingProvider`] injects provider failures.

use crate::decision::{Decision, DecisionSource, ProviderError, ProviderErrorKind};
use crate::provider::IntelligenceProvider;
use crate::types::{IntelligenceRequest, ProviderId, ProviderProfile};
use std::collections::BTreeMap;
use tpt_thought_core::{Confidence, Probability, StructuredValue};

/// Answers from a fixed table. Questions not in the table are abstained on.
pub struct ReferenceProvider {
    id: ProviderId,
    profile: ProviderProfile,
    answers: BTreeMap<String, (StructuredValue, Probability)>,
}

impl ReferenceProvider {
    /// An empty reference provider.
    pub fn new(id: impl Into<String>, profile: ProviderProfile) -> Self {
        Self {
            id: ProviderId::new(id),
            profile,
            answers: BTreeMap::new(),
        }
    }

    /// Add a fixed answer and the raw confidence it carries.
    pub fn with_answer(
        mut self,
        question: impl Into<String>,
        value: StructuredValue,
        confidence: Probability,
    ) -> Self {
        self.answers.insert(question.into(), (value, confidence));
        self
    }
}

impl IntelligenceProvider for ReferenceProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn profile(&self) -> &ProviderProfile {
        &self.profile
    }

    fn decide(
        &self,
        request: &IntelligenceRequest,
    ) -> Result<Decision<StructuredValue>, ProviderError> {
        let source = DecisionSource::Provider(self.id.clone());
        Ok(match self.answers.get(&request.question) {
            Some((value, confidence)) => Decision::answered(
                source,
                value.clone(),
                Confidence::from_raw(*confidence),
                request.at,
            ),
            None => Decision::abstained(source, "unknown question", request.at),
        })
    }
}

/// Always fails with the configured error. Used to test failover.
pub struct FailingProvider {
    id: ProviderId,
    profile: ProviderProfile,
    kind: ProviderErrorKind,
}

impl FailingProvider {
    /// A provider that always fails with `kind`.
    pub fn new(id: impl Into<String>, profile: ProviderProfile, kind: ProviderErrorKind) -> Self {
        Self {
            id: ProviderId::new(id),
            profile,
            kind,
        }
    }
}

impl IntelligenceProvider for FailingProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn profile(&self) -> &ProviderProfile {
        &self.profile
    }

    fn decide(
        &self,
        _request: &IntelligenceRequest,
    ) -> Result<Decision<StructuredValue>, ProviderError> {
        Err(ProviderError {
            provider: self.id.clone(),
            kind: self.kind.clone(),
        })
    }
}
