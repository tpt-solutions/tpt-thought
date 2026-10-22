// SPDX-License-Identifier: MIT OR Apache-2.0
//! Provider-neutral intelligence layer (spec sections 6, 21, 55-60, 65-68, 90-92, 118-119).
//!
//! Thought asks questions through [`IntelligenceProvider`] and never depends on a concrete
//! backend. Providers are discovered and ranked by a [`ProviderRegistry`], judged by
//! [`CognitionMode`]s (fast or deliberative), and their track records are kept by a
//! [`CalibrationTracker`]. Sibling-repo adapters live in `tpt-thought-integrations`.

pub mod calibration;
pub mod decision;
pub mod ensemble;
pub mod human;
pub mod mode;
pub mod provider;
pub mod reference;
pub mod registry;
pub mod types;

pub use calibration::{CalibrationTracker, BINS, DEFAULT_MIN_SAMPLES};
pub use decision::{Answer, Decision, DecisionSource, ProviderError, ProviderErrorKind};
pub use ensemble::{combine, Consensus, Disagreement, Position, Probe};
pub use human::{HumanAnswer, HumanProvider, HumanResponder};
pub use mode::{CognitionMode, Deliberative, Escalation, FastPath, Step};
pub use provider::IntelligenceProvider;
pub use reference::{FailingProvider, ReferenceProvider};
pub use registry::{Candidate, ProviderRegistry, RegistryError};
pub use types::{
    Capability, Constraints, IntelligenceRequest, Locality, Modality, Privacy, ProviderId,
    ProviderProfile, VerificationLevel,
};
