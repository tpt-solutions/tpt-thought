// SPDX-License-Identifier: MIT OR Apache-2.0
//! The provider-neutral trait (spec sections 6, 57). Core and the cognitive modes depend only on
//! this trait, never on a concrete backend such as Intellect, Eve, Augur or a remote model.

use crate::decision::{Decision, ProviderError};
use crate::types::{IntelligenceRequest, ProviderId, ProviderProfile};
use tpt_thought_core::StructuredValue;

/// Anything that can answer an [`IntelligenceRequest`].
///
/// Implementations may be local models, remote services, rule engines, verifiers or people.
/// Adding a new backend means implementing this trait and registering it; the cognitive modes
/// do not change (spec section 90).
pub trait IntelligenceProvider {
    /// Stable identity within a registry.
    fn id(&self) -> &ProviderId;

    /// What the provider offers and what is known about it.
    fn profile(&self) -> &ProviderProfile;

    /// Answer the request. Abstention is `Ok` with an abstained decision; `Err` means the provider
    /// could not produce a decision at all.
    fn decide(
        &self,
        request: &IntelligenceRequest,
    ) -> Result<Decision<StructuredValue>, ProviderError>;
}
