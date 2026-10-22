// SPDX-License-Identifier: MIT OR Apache-2.0
//! Provider discovery and selection (spec sections 58, 59, 91).
//!
//! The registry filters providers by the request's constraints and ranks the survivors. The
//! ranking inputs are surfaced as [`Candidate`]s so that Intellect (or any selector) can make the
//! final choice with full information.

use crate::calibration::CalibrationTracker;
use crate::provider::IntelligenceProvider;
use crate::types::{Constraints, ProviderId};
use std::cmp::Ordering;

/// Why a provider could not be registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// A provider with this identity is already registered.
    Duplicate(ProviderId),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(id) => write!(f, "provider {} is already registered", id.as_str()),
        }
    }
}

impl std::error::Error for RegistryError {}

/// A provider that passed the constraints, with the inputs used to rank it.
pub struct Candidate<'a> {
    /// The provider.
    pub provider: &'a dyn IntelligenceProvider,
    /// Expected reliability: empirical if tracked, else profiled accuracy, else the midpoint of its
    /// confidence range. `None` if nothing is known, which ranks it last.
    pub reliability: Option<f64>,
}

/// Holds the available providers.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: Vec<Box<dyn IntelligenceProvider>>,
}

impl ProviderRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a provider. Identities must be unique.
    pub fn register<P: IntelligenceProvider + 'static>(
        &mut self,
        provider: P,
    ) -> Result<(), RegistryError> {
        if self.get(provider.id()).is_some() {
            return Err(RegistryError::Duplicate(provider.id().clone()));
        }
        self.providers.push(Box::new(provider));
        Ok(())
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// True if no providers are registered.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Look up a provider by identity.
    pub fn get(&self, id: &ProviderId) -> Option<&dyn IntelligenceProvider> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .map(|p| p.as_ref())
    }

    /// Providers admitted by `constraints`, best first. Ranking is by expected reliability
    /// (descending), then cost, then latency (ascending), then identity, so it is deterministic.
    pub fn candidates(
        &self,
        constraints: &Constraints,
        calibration: &CalibrationTracker,
    ) -> Vec<Candidate<'_>> {
        let mut out: Vec<Candidate<'_>> = self
            .providers
            .iter()
            .filter(|p| constraints.admits_provider(p.id(), p.profile()))
            .map(|p| Candidate {
                provider: p.as_ref(),
                reliability: expected_reliability(p.as_ref(), calibration),
            })
            .collect();
        out.sort_by(|a, b| rank(a, b));
        out
    }
}

fn expected_reliability(
    provider: &dyn IntelligenceProvider,
    calibration: &CalibrationTracker,
) -> Option<f64> {
    let profile = provider.profile();
    calibration
        .empirical_reliability(provider.id())
        .or(profile.accuracy)
        .map(|p| p.get())
        .or_else(|| {
            profile
                .confidence_range
                .map(|(lo, hi)| (lo.get() + hi.get()) / 2.0)
        })
}

fn rank(a: &Candidate<'_>, b: &Candidate<'_>) -> Ordering {
    let by_reliability = match (a.reliability, b.reliability) {
        (Some(x), Some(y)) => y.total_cmp(&x),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    let by_cost = cmp_optional(a.provider.profile().cost, b.provider.profile().cost);
    let by_latency = cmp_optional(
        a.provider.profile().latency_nanos.map(|n| n as f64),
        b.provider.profile().latency_nanos.map(|n| n as f64),
    );
    by_reliability
        .then(by_cost)
        .then(by_latency)
        .then_with(|| a.provider.id().cmp(b.provider.id()))
}

/// Ascending order with unknown values last.
fn cmp_optional(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
