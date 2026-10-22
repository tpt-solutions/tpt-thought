// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapter from `tpt-intellect` backends to the Thought [`IntelligenceProvider`] port
//! (spec sections 55, 58, 60, 67; todo.md Phase 4).

use std::time::Duration;

use tpt_intellect_backend::{BackendError, IntelligenceBackend};
use tpt_intellect_core::{
    Candidate, DecisionKind, DecisionOutcome, IntelligenceRequest as SiblingRequest,
    RequestConstraints, StructuredInput, Value,
};
use tpt_thought_core::{Confidence, Probability, StructuredValue, Timestamp};
use tpt_thought_providers::{
    Decision, DecisionSource, IntelligenceProvider, IntelligenceRequest, ProviderError,
    ProviderErrorKind, ProviderId, ProviderProfile,
};

/// Wraps a sibling backend that answers choice questions over string candidates.
///
/// The Thought question becomes the sibling input text, and the candidates are fixed when the
/// provider is built. The sibling confidence is a raw probability and is recorded as one. The
/// sibling backend's identifier becomes the provider identity.
pub struct IntellectProvider<B> {
    id: ProviderId,
    profile: ProviderProfile,
    backend: B,
    candidates: Vec<String>,
}

impl<B: IntelligenceBackend<String>> IntellectProvider<B> {
    /// Wrap `backend`, answering over `candidates`. `profile` describes what Thought may assume.
    pub fn new(backend: B, candidates: Vec<String>, profile: ProviderProfile) -> Self {
        let id = ProviderId::new(backend.id().0.clone());
        Self {
            id,
            profile,
            backend,
            candidates,
        }
    }

    /// Whether the sibling backend advertises choice decisions (capability discovery, §58).
    pub fn supports_choice(&self) -> bool {
        self.backend.capabilities().supports(DecisionKind::Choice)
    }

    fn sibling_request(&self, request: &IntelligenceRequest) -> SiblingRequest<String> {
        let constraints = RequestConstraints {
            max_latency: request
                .constraints
                .max_latency_nanos
                .map(Duration::from_nanos),
            ..RequestConstraints::default()
        };
        SiblingRequest::new(
            StructuredInput::new(Value::Text(request.question.clone())),
            DecisionKind::Choice,
        )
        .with_candidates(
            self.candidates
                .iter()
                .cloned()
                .map(Candidate::new)
                .collect(),
        )
        .with_constraints(constraints)
    }

    fn failure(&self, kind: ProviderErrorKind) -> ProviderError {
        ProviderError {
            provider: self.id.clone(),
            kind,
        }
    }
}

impl<B: IntelligenceBackend<String>> IntelligenceProvider for IntellectProvider<B> {
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
        if self.candidates.is_empty() {
            return Err(self.failure(ProviderErrorKind::Other(
                "no candidates configured for this provider".into(),
            )));
        }
        let response = self
            .backend
            .decide(&self.sibling_request(request))
            .map_err(|error| {
                self.failure(match error {
                    BackendError::Unavailable { .. } => ProviderErrorKind::Unavailable,
                    BackendError::Timeout => ProviderErrorKind::Timeout,
                    BackendError::UnsupportedRequest => {
                        ProviderErrorKind::Refused("unsupported request".into())
                    }
                    other => ProviderErrorKind::Other(other.to_string()),
                })
            })?;
        let value = response.decision.confidence.value();
        let raw = Probability::new(value).map_err(|_| {
            self.failure(ProviderErrorKind::Other(format!(
                "sibling confidence {value} is outside 0..=1"
            )))
        })?;
        let confidence = Confidence::from_raw(raw);
        let source = DecisionSource::Provider(self.id.clone());
        let at = Timestamp(request.at.0);
        Ok(match response.decision.outcome {
            DecisionOutcome::Choice(choice) => {
                Decision::answered(source, StructuredValue::Text(choice.chosen), confidence, at)
            }
            DecisionOutcome::Abstain(abstention) => {
                Decision::abstained(source, format!("{abstention:?}"), at)
            }
            other => Decision::abstained(
                source,
                format!("outcome not used by this adapter: {other:?}"),
                at,
            ),
        })
    }
}

/// A provider backed by Intellect's reference backend, answering over `candidates`. Lets callers
/// exercise the real sibling without naming it as a dependency.
pub fn reference_provider(
    candidates: Vec<String>,
    profile: ProviderProfile,
) -> IntellectProvider<tpt_intellect_backend::ReferenceBackend> {
    IntellectProvider::new(
        tpt_intellect_backend::ReferenceBackend::new(
            tpt_intellect_backend::ReferenceConfig::default(),
        ),
        candidates,
        profile,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_intellect_backend::{ReferenceBackend, ReferenceConfig};
    use tpt_thought_providers::{Answer, Locality, Privacy};

    fn profile() -> ProviderProfile {
        ProviderProfile::new(Locality::Local, Privacy::OnDevice)
    }

    fn reference() -> ReferenceBackend {
        ReferenceBackend::new(ReferenceConfig::default())
    }

    fn request(question: &str) -> IntelligenceRequest {
        IntelligenceRequest::new(question, Timestamp(7))
    }

    #[test]
    fn reference_backend_answers_with_one_of_the_candidates() {
        let provider =
            IntellectProvider::new(reference(), vec!["alpha".into(), "beta".into()], profile());
        let decision = provider.decide(&request("pick one")).unwrap();
        match decision.answer {
            Answer::Answered(StructuredValue::Text(chosen)) => {
                assert!(chosen == "alpha" || chosen == "beta", "got {chosen}");
            }
            other => panic!("expected an answer, got {other:?}"),
        }
        assert!(
            decision.confidence.raw.is_some(),
            "sibling confidence stays raw"
        );
        assert_eq!(
            decision.source,
            DecisionSource::Provider(provider.id().clone())
        );
    }

    #[test]
    fn capability_discovery_reports_choice_support() {
        let provider = IntellectProvider::new(reference(), vec!["x".into()], profile());
        assert!(provider.supports_choice());
    }

    #[test]
    fn no_candidates_is_a_configuration_error() {
        let provider = IntellectProvider::new(reference(), Vec::new(), profile());
        let error = provider.decide(&request("pick one")).unwrap_err();
        assert!(matches!(error.kind, ProviderErrorKind::Other(_)));
    }
}
