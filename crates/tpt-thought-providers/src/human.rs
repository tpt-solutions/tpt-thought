// SPDX-License-Identifier: MIT OR Apache-2.0
//! Human-in-the-loop as a provider (spec sections 38, 92). A person answers through a
//! [`HumanResponder`] supplied by the host application, which owns the UI or channel. The
//! decision keeps the person's identity, timestamp, context and stated confidence.

use crate::decision::{Decision, DecisionSource, ProviderError};
use crate::provider::IntelligenceProvider;
use crate::types::{IntelligenceRequest, Locality, Privacy, ProviderId, ProviderProfile};
use tpt_thought_core::{Confidence, Probability, StructuredValue, Timestamp};

/// A person's answer to a request.
#[derive(Debug, Clone, PartialEq)]
pub struct HumanAnswer {
    /// The answer.
    pub value: StructuredValue,
    /// How sure the person says they are.
    pub confidence: Probability,
    /// The person's reasoning or context, kept with the decision.
    pub context: String,
    /// When the person answered.
    pub answered_at: Timestamp,
}

/// Supplies human answers. Returning `None` means the person did not answer, which is recorded as
/// an abstention.
pub trait HumanResponder {
    /// Ask the person about `request`.
    fn respond(&self, request: &IntelligenceRequest) -> Option<HumanAnswer>;
}

impl<F> HumanResponder for F
where
    F: Fn(&IntelligenceRequest) -> Option<HumanAnswer>,
{
    fn respond(&self, request: &IntelligenceRequest) -> Option<HumanAnswer> {
        self(request)
    }
}

/// A person exposed as an [`IntelligenceProvider`].
pub struct HumanProvider<R> {
    id: ProviderId,
    identity: String,
    profile: ProviderProfile,
    responder: R,
}

impl<R: HumanResponder> HumanProvider<R> {
    /// A human provider with the given identity. Its id is `human:<identity>`.
    pub fn new(identity: impl Into<String>, responder: R) -> Self {
        let identity = identity.into();
        Self {
            id: ProviderId::new(format!("human:{identity}")),
            identity,
            profile: ProviderProfile::new(Locality::Human, Privacy::Confidential),
            responder,
        }
    }
}

impl<R: HumanResponder> IntelligenceProvider for HumanProvider<R> {
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
        let source = DecisionSource::Human {
            identity: self.identity.clone(),
        };
        let decision = match self.responder.respond(request) {
            Some(answer) => {
                let mut decision = Decision::answered(
                    source,
                    answer.value,
                    Confidence::from_raw(answer.confidence),
                    answer.answered_at,
                );
                decision.context = Some(answer.context);
                decision
            }
            None => Decision::abstained(source, "no human response", request.at),
        };
        Ok(decision)
    }
}
