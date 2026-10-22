// SPDX-License-Identifier: MIT OR Apache-2.0
//! Remote Intellect backends over `tpt-intellect-remote`'s transports (spec section 93;
//! todo.md Phase 10, "Remote provider via `tpt-intellect-remote` only").
//!
//! Thought performs no HTTP and no process spawning itself. A remote backend is a
//! [`RemoteBackend`] discovered over a [`RemoteTransport`]; it is wrapped by the same
//! [`IntellectProvider`] used for local Intellect backends, so the Thought port is unchanged.
//!
//! Transports: [`loopback`] uses the sibling's in-process `ProtocolTransport`, which encodes
//! every request and response on the wire format. [`over_transport`] accepts any transport the
//! caller supplies. The sibling's `SubprocessTransport` is not wrapped here: it needs a child
//! process, and this crate does not spawn processes. An HTTP transport is the sibling's
//! documented seam and is not included, since the sibling deliberately has no HTTP client.

use std::io;
use std::sync::Arc;

use tpt_intellect_backend::{IntelligenceBackend, ReferenceBackend, ReferenceConfig};
use tpt_intellect_remote::{in_process, ModelInfo, RemoteBackend, RemoteTransport};
use tpt_thought_providers::ProviderProfile;

use crate::intellect::IntellectProvider;

/// A provider whose backend runs behind the sibling's loopback protocol. The served backend is
/// Intellect's reference backend, so answers come from a real backend over the real wire format.
///
/// # Panics
/// Never: the loopback discovery is performed on in-memory bytes and cannot fail.
#[must_use]
pub fn loopback(
    candidates: Vec<String>,
    profile: ProviderProfile,
) -> IntellectProvider<RemoteBackend<String>> {
    let served: Arc<dyn IntelligenceBackend<String>> =
        Arc::new(ReferenceBackend::new(ReferenceConfig::default()));
    let backend = in_process(served, ModelInfo::new("reference", "0.1.0"));
    IntellectProvider::new(backend, candidates, profile)
}

/// A provider whose backend is reached over `transport`. Capability discovery runs during
/// construction, so an unreachable or incompatible service is reported here rather than on the
/// first decision.
///
/// # Errors
/// Transport failures and malformed discovery responses surface as [`io::Error`].
pub fn over_transport(
    transport: Box<dyn RemoteTransport>,
    candidates: Vec<String>,
    profile: ProviderProfile,
) -> io::Result<IntellectProvider<RemoteBackend<String>>> {
    let backend = RemoteBackend::connect(transport)?;
    Ok(IntellectProvider::new(backend, candidates, profile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_intellect_remote::ProtocolTransport;
    use tpt_thought_core::{StructuredValue, Timestamp};
    use tpt_thought_providers::{
        Answer, IntelligenceProvider, IntelligenceRequest, Locality, Privacy,
    };

    fn profile() -> ProviderProfile {
        ProviderProfile::new(Locality::Remote, Privacy::External)
    }

    #[test]
    fn loopback_round_trip_answers_with_a_candidate() {
        let provider = loopback(vec!["alpha".into(), "beta".into()], profile());
        let request = IntelligenceRequest::new("pick one", Timestamp(3));
        let decision = provider.decide(&request).expect("answer over loopback");
        match decision.answer {
            Answer::Answered(StructuredValue::Text(chosen)) => {
                assert!(chosen == "alpha" || chosen == "beta", "got {chosen}");
            }
            other => panic!("expected an answer over the wire, got {other:?}"),
        }
    }

    #[test]
    fn discovery_over_loopback_reports_choice_support() {
        let provider = loopback(vec!["x".into()], profile());
        assert!(provider.supports_choice());
    }

    #[test]
    fn an_unreachable_service_fails_at_connection() {
        let transport = ProtocolTransport::new(|_| Err(io::Error::other("connection refused")));
        let result = over_transport(Box::new(transport), vec!["x".into()], profile());
        assert!(
            result.is_err(),
            "discovery must fail when the service is down"
        );
    }
}
