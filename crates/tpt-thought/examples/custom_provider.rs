// SPDX-License-Identifier: MIT OR Apache-2.0
//! A custom intelligence provider. Thought asks providers questions through a trait and never
//! depends on a backend, so any program can add one without changing the cognitive model.
//!
//! This provider answers from a small rule table. A real one might call a model, a database or
//! a sibling service. Run with `cargo run -p tpt-thought --example custom_provider`.

use tpt_thought::core::{Confidence, Probability, StructuredValue, Timestamp};
use tpt_thought::prelude::*;
use tpt_thought::providers::{
    CognitionMode, Decision, DecisionSource, IntelligenceProvider, IntelligenceRequest, Locality,
    Privacy, ProviderError, ProviderId, ProviderProfile,
};

/// Answers a question only when a rule matches, and abstains otherwise.
struct RuleProvider {
    id: ProviderId,
    profile: ProviderProfile,
    rules: Vec<(&'static str, &'static str)>,
}

impl IntelligenceProvider for RuleProvider {
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
        let now = Timestamp(0);
        let matched = self
            .rules
            .iter()
            .find(|(keyword, _)| request.question.contains(keyword));
        Ok(match matched {
            Some((_, answer)) => Decision::answered(
                source,
                StructuredValue::Text((*answer).into()),
                Confidence::from_raw(Probability::new(0.9).expect("valid probability")),
                now,
            ),
            None => Decision::abstained(source, "no rule matched", now),
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile =
        ProviderProfile::new(Locality::Local, Privacy::OnDevice).with_capability("answer");
    let provider = RuleProvider {
        id: ProviderId::new("cup-rules"),
        profile,
        rules: vec![
            ("cup", "use a power grip"),
            ("battery", "check the charge first"),
        ],
    };

    let mut registry = ProviderRegistry::new();
    registry.register(provider)?;

    let request = IntelligenceRequest::new("how should the cup be held?", Timestamp(0));
    let step = tpt_thought::providers::FastPath::default();
    let calibration = tpt_thought::providers::CalibrationTracker::new(5);
    match step.respond(&request, &registry, &calibration) {
        tpt_thought::providers::Step::Accept(decision) => {
            println!(
                "accepted from {}: {:?}",
                decision.source.provider_id().as_str(),
                decision.value()
            );
        }
        tpt_thought::providers::Step::Escalate(escalation) => {
            println!("escalated: {}", escalation.reason);
        }
    }
    Ok(())
}
