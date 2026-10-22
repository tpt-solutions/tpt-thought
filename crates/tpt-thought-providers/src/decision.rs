// SPDX-License-Identifier: MIT OR Apache-2.0
//! Decisions returned by providers (spec sections 38, 55, 67, 92). A decision records who
//! answered, how sure they were, whether they abstained, and what the answer rests on.

use crate::types::ProviderId;
use serde::{Deserialize, Serialize};
use tpt_thought_core::{Confidence, Probability, ProvenanceId, Timestamp, Uncertainty};

/// The answer itself, or a deliberate refusal to answer. Abstention is a result, not an error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Answer<T> {
    /// The provider answered.
    Answered(T),
    /// The provider declined to answer, and why.
    Abstained {
        /// Human-readable reason.
        reason: String,
    },
}

/// Who produced a decision.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionSource {
    /// A machine provider.
    Provider(ProviderId),
    /// A person, identified by an opaque identity string.
    Human {
        /// Who answered.
        identity: String,
    },
}

impl DecisionSource {
    /// The key under which this source's track record is kept. Humans map to `human:<identity>`.
    pub fn provider_id(&self) -> ProviderId {
        match self {
            Self::Provider(id) => id.clone(),
            Self::Human { identity } => ProviderId::new(format!("human:{identity}")),
        }
    }
}

/// A provider's decision on one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision<T> {
    /// The answer or abstention.
    pub answer: Answer<T>,
    /// Confidence, kept as raw, calibrated and empirical values (spec section 67).
    pub confidence: Confidence,
    /// Uncertainty flags. `abstained` is set whenever `answer` is an abstention.
    pub uncertainty: Uncertainty,
    /// Who produced the decision.
    pub source: DecisionSource,
    /// Provenance records that explain the decision.
    pub provenance: Vec<ProvenanceId>,
    /// Context supplied with the decision, for example a human's reasoning.
    pub context: Option<String>,
    /// When the decision was made.
    pub decided_at: Timestamp,
}

impl<T> Decision<T> {
    /// A decision that answers, with the given confidence.
    pub fn answered(
        source: DecisionSource,
        value: T,
        confidence: Confidence,
        decided_at: Timestamp,
    ) -> Self {
        Self {
            answer: Answer::Answered(value),
            confidence,
            uncertainty: Uncertainty::default(),
            source,
            provenance: Vec::new(),
            context: None,
            decided_at,
        }
    }

    /// A decision that abstains. Confidence is unknown and the abstention flag is set.
    pub fn abstained(
        source: DecisionSource,
        reason: impl Into<String>,
        decided_at: Timestamp,
    ) -> Self {
        let reason = reason.into();
        Self {
            answer: Answer::Abstained { reason },
            confidence: Confidence::UNKNOWN,
            uncertainty: Uncertainty {
                abstained: true,
                ..Uncertainty::default()
            },
            source,
            provenance: Vec::new(),
            context: None,
            decided_at,
        }
    }

    /// The answer, if one was given.
    pub fn value(&self) -> Option<&T> {
        match &self.answer {
            Answer::Answered(value) => Some(value),
            Answer::Abstained { .. } => None,
        }
    }

    /// True if the provider abstained.
    pub fn is_abstention(&self) -> bool {
        matches!(self.answer, Answer::Abstained { .. })
    }

    /// The most trustworthy reliability available: empirical, then calibrated, then raw.
    pub fn reliability(&self) -> Option<Probability> {
        self.confidence.best()
    }
}

/// Why a provider could not produce a decision at all. Distinct from abstention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderErrorKind {
    /// The provider is not reachable or not running.
    Unavailable,
    /// The provider did not respond in time.
    Timeout,
    /// The provider refused the request.
    Refused(String),
    /// Anything else.
    Other(String),
}

/// A provider failed to produce a decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderError {
    /// The failing provider.
    pub provider: ProviderId,
    /// What went wrong.
    pub kind: ProviderErrorKind,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "provider {} failed: {:?}",
            self.provider.as_str(),
            self.kind
        )
    }
}

impl std::error::Error for ProviderError {}
