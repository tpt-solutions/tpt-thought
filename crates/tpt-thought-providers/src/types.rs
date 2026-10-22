// SPDX-License-Identifier: MIT OR Apache-2.0
//! Provider identity, capability profiles and request constraints (spec sections 55, 58, 59, 91).

use crate::decision::Decision;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tpt_thought_core::{Probability, Timestamp};

/// Identifies a provider within a registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProviderId(String);

impl ProviderId {
    /// Wrap an identifier string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The identifier as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A named ability a provider offers, such as `reason:causal` or `estimate:cost`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Capability(String);

impl Capability {
    /// Wrap a capability name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Input or output channel a provider can work with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    /// Natural-language text.
    Text,
    /// Schema-free structured data.
    Structured,
    /// Images or video.
    Vision,
    /// Audio.
    Audio,
    /// Other sensor streams.
    Sensor,
}

/// Where a provider runs relative to the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locality {
    /// In the same process or on the same device.
    Local,
    /// Across a network.
    Remote,
    /// A person.
    Human,
}

/// How sensitive the data a provider may receive is allowed to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Privacy {
    /// Data never leaves the device.
    OnDevice,
    /// Data stays within the owner's infrastructure.
    Confidential,
    /// Data may be shared with third parties.
    External,
}

/// How a provider's answers are checked. Ordered from weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationLevel {
    /// No verification.
    None,
    /// Checked by a verifier (for example Axiom contracts).
    Checked,
    /// Formally proven.
    Formal,
}

/// What is known about a provider before asking it anything (spec section 58). Every measured
/// field is optional: an unknown value never excludes a provider on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderProfile {
    /// Abilities offered.
    pub capabilities: BTreeSet<Capability>,
    /// Modalities supported.
    pub modalities: BTreeSet<Modality>,
    /// Long-run accuracy, if measured.
    pub accuracy: Option<Probability>,
    /// Typical latency in nanoseconds.
    pub latency_nanos: Option<u64>,
    /// Typical cost per request, in the deployment's cost units.
    pub cost: Option<f64>,
    /// Range of confidence values the provider tends to emit.
    pub confidence_range: Option<(Probability, Probability)>,
    /// Compute the provider needs per request, in provider-defined units.
    pub resources: Option<f64>,
    /// Where it runs.
    pub locality: Locality,
    /// What data it may receive.
    pub privacy: Privacy,
    /// How its answers are verified.
    pub verification: VerificationLevel,
}

impl ProviderProfile {
    /// A profile with the given locality and privacy and nothing else known.
    pub fn new(locality: Locality, privacy: Privacy) -> Self {
        Self {
            capabilities: BTreeSet::new(),
            modalities: BTreeSet::new(),
            accuracy: None,
            latency_nanos: None,
            cost: None,
            confidence_range: None,
            resources: None,
            locality,
            privacy,
            verification: VerificationLevel::None,
        }
    }

    /// Add a capability.
    pub fn with_capability(mut self, name: impl Into<String>) -> Self {
        self.capabilities.insert(Capability::new(name));
        self
    }

    /// Add a modality.
    pub fn with_modality(mut self, modality: Modality) -> Self {
        self.modalities.insert(modality);
        self
    }

    /// Set measured accuracy.
    pub fn with_accuracy(mut self, accuracy: Probability) -> Self {
        self.accuracy = Some(accuracy);
        self
    }

    /// Set typical latency.
    pub fn with_latency_nanos(mut self, nanos: u64) -> Self {
        self.latency_nanos = Some(nanos);
        self
    }

    /// Set typical cost.
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost = Some(cost);
        self
    }

    /// Set the verification level.
    pub fn with_verification(mut self, level: VerificationLevel) -> Self {
        self.verification = level;
        self
    }
}

/// Limits and requirements a request places on acceptable answers (spec section 55). `None`
/// means the dimension is unconstrained.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Constraints {
    /// Minimum reliability an answer must have to be accepted.
    pub min_confidence: Option<Probability>,
    /// Maximum tolerated risk, where risk is `1 - reliability`.
    pub risk_tolerance: Option<Probability>,
    /// Maximum cost a provider may carry.
    pub max_cost: Option<f64>,
    /// Maximum latency a provider may carry, in nanoseconds.
    pub max_latency_nanos: Option<u64>,
    /// If set, only these providers may be asked.
    pub acceptable_providers: Option<BTreeSet<ProviderId>>,
    /// Every listed capability must be offered by the provider.
    pub required_capabilities: BTreeSet<Capability>,
    /// Minimum verification level.
    pub required_verification: Option<VerificationLevel>,
}

impl Constraints {
    /// True if a provider with identity `id` and `profile` may be asked under these constraints.
    /// Unknown profile values never exclude a provider.
    pub fn admits_provider(&self, id: &ProviderId, profile: &ProviderProfile) -> bool {
        self.acceptable_providers
            .as_ref()
            .is_none_or(|set| set.contains(id))
            && self.required_capabilities.is_subset(&profile.capabilities)
            && self
                .required_verification
                .is_none_or(|level| profile.verification >= level)
            && self
                .max_cost
                .is_none_or(|max| profile.cost.is_none_or(|c| c <= max))
            && self
                .max_latency_nanos
                .is_none_or(|max| profile.latency_nanos.is_none_or(|l| l <= max))
    }

    /// True if `decision` may be acted on: it answered, it is resolved, and its reliability meets
    /// the minimum confidence and the risk tolerance. Without reliability information, an answer
    /// is rejected whenever either threshold is set.
    pub fn accepts<T>(&self, decision: &Decision<T>) -> bool {
        if decision.is_abstention() || decision.uncertainty.is_unresolved() {
            return false;
        }
        let reliability = decision.reliability().map(Probability::get);
        let meets_min = self
            .min_confidence
            .is_none_or(|min| reliability.is_some_and(|r| r >= min.get()));
        let within_risk = self
            .risk_tolerance
            .is_none_or(|tolerance| reliability.is_some_and(|r| 1.0 - r <= tolerance.get()));
        meets_min && within_risk
    }
}

/// A question put to providers, with the constraints its answer must satisfy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntelligenceRequest {
    /// The question, as a stable key. Providers may answer by exact match or by reasoning.
    pub question: String,
    /// Constraints on acceptable answers.
    pub constraints: Constraints,
    /// Free-form context labels passed along to providers.
    pub context: Vec<String>,
    /// When the request was made.
    pub at: Timestamp,
}

impl IntelligenceRequest {
    /// A request with no constraints or context.
    pub fn new(question: impl Into<String>, at: Timestamp) -> Self {
        Self {
            question: question.into(),
            constraints: Constraints::default(),
            context: Vec::new(),
            at,
        }
    }
}
