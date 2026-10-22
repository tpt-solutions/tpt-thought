// SPDX-License-Identifier: MIT OR Apache-2.0
//! A local model provider over `tpt-infer`, through Intellect's `infer` bridge (spec section 94;
//! todo.md Phase 10, "Local provider support via Intellect").
//!
//! Thought sees only the [`IntelligenceProvider`] port. The graph, the arena and backend
//! selection belong to `tpt-infer`, reached through `tpt-intellect-remote`'s
//! `InferClassificationBackend`. The bridge takes numeric features rather than text, so the
//! caller supplies an encoder from question text to features.
//!
//! The bridge answers classification questions only. The labels are fixed when the graph is
//! built, so a question is answered with one of those labels.
//!
//! Not implemented: `tpt-spark` and Ollama adapters. Neither appears in the sibling survey, so
//! no adapter is written for them here.

use tpt_intellect_backend::{BackendInfo, IntelligenceBackend};
use tpt_intellect_core::{
    DecisionKind, DecisionOutcome, IntelligenceRequest as SiblingRequest, Probability,
    StructuredInput, Value,
};
use tpt_intellect_remote::InferClassificationBackend;
use tpt_thought_core::{Confidence, StructuredValue, Timestamp};
use tpt_thought_providers::{
    Decision, DecisionSource, IntelligenceProvider, IntelligenceRequest, ProviderError,
    ProviderErrorKind, ProviderId, ProviderProfile,
};

/// Turns question text into the numeric features the local model expects.
pub type Encoder = Box<dyn Fn(&str) -> Vec<f64> + Send + Sync>;

/// A `tpt-infer` classifier as a Thought provider.
pub struct LocalClassifier {
    id: ProviderId,
    profile: ProviderProfile,
    backend: InferClassificationBackend,
    encode: Encoder,
}

impl LocalClassifier {
    /// Wrap a classification backend. `encode` maps each question to the feature vector the
    /// backend's graph expects.
    pub fn new(
        backend: InferClassificationBackend,
        encode: Encoder,
        profile: ProviderProfile,
    ) -> Self {
        let id = ProviderId::new(backend.id().0.clone());
        Self {
            id,
            profile,
            backend,
            encode,
        }
    }

    fn failure(&self, kind: ProviderErrorKind) -> ProviderError {
        ProviderError {
            provider: self.id.clone(),
            kind,
        }
    }
}

impl IntelligenceProvider for LocalClassifier {
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
        let features = (self.encode)(&request.question)
            .into_iter()
            .map(Value::Number)
            .collect();
        let sibling = SiblingRequest::new(
            StructuredInput::new(Value::List(features)),
            DecisionKind::Classification,
        );
        let response = self
            .backend
            .decide(&sibling)
            .map_err(|error| self.failure(ProviderErrorKind::Other(error.to_string())))?;
        let value = response.decision.confidence.value();
        let raw = Probability::new(value).map_err(|_| {
            self.failure(ProviderErrorKind::Other(format!(
                "model confidence {value} is outside 0..=1"
            )))
        })?;
        let confidence = Confidence::from_raw(
            tpt_thought_core::Probability::new(raw.value())
                .map_err(|_| self.failure(ProviderErrorKind::Other("confidence".into())))?,
        );
        let source = DecisionSource::Provider(self.id.clone());
        let at = Timestamp(request.at.0);
        Ok(match response.decision.outcome {
            DecisionOutcome::Classification(classification) => {
                let best = classification
                    .labels
                    .iter()
                    .max_by(|a, b| a.1.value().total_cmp(&b.1.value()));
                match best {
                    Some((label, _)) => Decision::answered(
                        source,
                        StructuredValue::Text(label.clone()),
                        confidence,
                        at,
                    ),
                    None => Decision::<StructuredValue>::abstained(
                        source,
                        String::from("no labels returned"),
                        at,
                    ),
                }
            }
            other => Decision::<StructuredValue>::abstained(
                source,
                format!("outcome not used by this bridge: {other:?}"),
                at,
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_infer::prelude::{GraphBuilder, Sh};
    use tpt_thought_providers::{Answer, Locality, Privacy};

    fn profile() -> ProviderProfile {
        ProviderProfile::new(Locality::Local, Privacy::OnDevice)
    }

    /// A one-layer classifier over four features and two labels, built in code. Feature 0
    /// votes for "hot" (label 0); the other features carry no weight. No model file is read.
    fn hot_cold_backend() -> InferClassificationBackend {
        let mut builder = GraphBuilder::<Sh<1, 4>>::input("x");
        let weights = builder
            .add_initializer::<Sh<4, 2>>("w", vec![1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
            .expect("weights match the declared shape");
        let graph = builder.matmul(weights).into_graph();
        InferClassificationBackend::new(
            "local-hot-cold",
            graph,
            4,
            vec!["hot".into(), "cold".into()],
        )
    }

    /// Text mentioning "hot" maps to a positive first feature; anything else maps to negative.
    fn encoder() -> Encoder {
        Box::new(|text: &str| {
            let first = if text.contains("hot") { 1.0 } else { -1.0 };
            vec![first, 0.0, 0.0, 0.0]
        })
    }

    fn answer_of(decision: &Decision<StructuredValue>) -> Option<String> {
        match &decision.answer {
            Answer::Answered(StructuredValue::Text(label)) => Some(label.clone()),
            _ => None,
        }
    }

    #[test]
    fn the_local_graph_classifies_questions_through_tpt_infer() {
        let provider = LocalClassifier::new(hot_cold_backend(), encoder(), profile());
        let hot = provider
            .decide(&IntelligenceRequest::new("is the tea hot", Timestamp(1)))
            .expect("hot question");
        let cold = provider
            .decide(&IntelligenceRequest::new("is the tea cold", Timestamp(2)))
            .expect("cold question");
        assert_eq!(answer_of(&hot).as_deref(), Some("hot"));
        assert_eq!(answer_of(&cold).as_deref(), Some("cold"));
    }

    #[test]
    fn a_feature_count_the_graph_does_not_expect_is_a_clear_error() {
        let wrong: Encoder = Box::new(|_| vec![1.0, 2.0]);
        let provider = LocalClassifier::new(hot_cold_backend(), wrong, profile());
        let result = provider.decide(&IntelligenceRequest::new("anything", Timestamp(3)));
        assert!(
            result.is_err(),
            "a wrong input width must not produce an answer"
        );
    }
}
