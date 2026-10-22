// SPDX-License-Identifier: MIT OR Apache-2.0
//! Safety for cognition: circuit breakers, sandboxing, loop detection, resource limits and
//! integrity checks (spec sections 41 and 48.4). See `docs/THREAT_MODEL.md` for the threats each
//! module answers.
//!
//! Nothing here executes actions or talks to a backend. Callers ask these types whether a step
//! may proceed, and they refuse by returning an error that names the reason.

mod breaker;
mod integrity;
mod sandbox;

pub use breaker::{BreakerConfig, BreakerKind, CircuitBreakers, Trip};
pub use integrity::{
    admit_actor, verify_provenance, IdentityMismatch, IntegrityFault, ObservationGate, ReplayGuard,
    TrustRejected,
};
pub use sandbox::{
    find_parent_cycle, CapabilityDenied, LimitViolation, ResourceLimits, Sandbox, Usage,
};
pub use tpt_thought_core::{SourcePolicy, SourceTrust};
