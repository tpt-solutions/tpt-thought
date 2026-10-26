// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thought sharing over INP (spec sections 23, 39).
//!
//! [`ThoughtBroadcast::share`] sends a [`SharedThought`] as an INP intent for [`SHARE_ACTION`],
//! with the thought as the intent's JSON payload. Every peer offered for that action is asked in
//! turn through a [`PeerLink`] that the caller supplies, and each reply is held until the next
//! peer is asked. A peer therefore sees only the thought, never another peer's reply. A peer
//! whose link fails, whose reply is for another intent or agent, or whose result is not
//! `Completed` is recorded as missing, not as an assessment.
//!
//! The merge is not here. [`ShareRound`] goes to `tpt_thought_providers::merge_round`, which
//! uses no INP types. No HTTP is used, and the link is the caller's transport.

use std::fmt;
use std::time::Duration;

use tpt_anima_inp::{
    AgentId as InpAgentId, Capability, CapabilityRegistry, EconomicTerms, ExecutionResult,
    ExecutionStatus, Intent, IntentBroadcaster, PerformanceProfile, QoSConstraints,
};
use tpt_thought_providers::{
    evaluate_thought, Assessment, CalibrationTracker, PeerId, ProviderRegistry, ShareRound,
    SharedThought,
};

use tpt_anima_inp::BroadcastConfig;

/// The INP action under which thoughts are shared.
pub const SHARE_ACTION: &str = "thought.share";

/// A peer's reply to one shared thought: the INP result and its assessments.
#[derive(Debug, Clone)]
pub struct PeerReply {
    /// The INP execution result. Its intent id and provider id must match the request.
    pub result: ExecutionResult,
    /// The peer's assessments. Any whose peer id is not the replying peer's are dropped.
    pub assessments: Vec<Assessment>,
}

/// The link to a peer could not carry the thought or the reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkError {
    /// What went wrong.
    pub reason: String,
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "peer link failed: {}", self.reason)
    }
}

impl std::error::Error for LinkError {}

/// Carries an intent to one peer and returns its reply. The caller owns the transport, which may
/// be in-process, a protocol transport or a network link. Thought does not do HTTP.
pub trait PeerLink {
    /// Deliver `intent` to `peer` and wait for its reply.
    fn send(&mut self, peer: InpAgentId, intent: &Intent) -> Result<PeerReply, LinkError>;
}

/// The thought peer id for an INP agent.
pub fn peer_id(agent: InpAgentId) -> PeerId {
    PeerId::new(agent.to_string())
}

/// Broadcasts thoughts to the peers offered for [`SHARE_ACTION`].
pub struct ThoughtBroadcast {
    broadcaster: IntentBroadcaster,
    registry: CapabilityRegistry,
    origin: InpAgentId,
}

impl ThoughtBroadcast {
    /// An empty broadcast. `origin` identifies this agent. It is never asked to evaluate its own
    /// thought.
    pub fn new(origin: InpAgentId) -> Self {
        Self {
            broadcaster: IntentBroadcaster::new(BroadcastConfig::default()),
            registry: CapabilityRegistry::new(),
            origin,
        }
    }

    /// Register `peer` as able to evaluate shared thoughts.
    pub fn offer(&mut self, peer: InpAgentId) {
        let capability = Capability::new(
            vec![SHARE_ACTION.to_string()],
            PerformanceProfile {
                typical_latency: Duration::from_millis(100),
                max_latency: Duration::from_secs(5),
                typical_accuracy: 0.9,
                throughput_per_second: 10.0,
            },
            EconomicTerms::FixedPrice { credits: 0.0 },
            peer,
        );
        self.registry.register(capability);
    }

    /// Broadcast `thought` to every offered peer except the origin, one at a time, and collect
    /// the replies. The result is the [`ShareRound`] to merge.
    pub fn share(&mut self, thought: &SharedThought, link: &mut impl PeerLink) -> ShareRound {
        let payload = serde_json::to_value(thought).unwrap_or(serde_json::Value::Null);
        let intent = Intent::new(
            SHARE_ACTION.to_string(),
            payload,
            QoSConstraints::default(),
            EconomicTerms::FixedPrice { credits: 0.0 },
            self.origin,
        );
        let offered = self.broadcaster.begin_broadcast(&intent, &self.registry);

        let mut round = ShareRound {
            thought: thought.clone(),
            assessments: Vec::new(),
            missing: Vec::new(),
        };
        let mut asked: Vec<InpAgentId> = Vec::new();
        for peer in offered {
            if peer == self.origin || asked.contains(&peer) {
                continue;
            }
            asked.push(peer);
            match link.send(peer, &intent) {
                Ok(reply) => match accept_reply(peer, &intent, reply) {
                    Some(assessments) => round.assessments.extend(assessments),
                    None => round.missing.push(peer_id(peer)),
                },
                Err(_) => round.missing.push(peer_id(peer)),
            }
        }
        // The round closes whether or not any peer replied.
        let _ = self.broadcaster.finalize_broadcast(&intent.id);
        round
    }
}

/// The assessments of a reply, if the reply is for this intent and peer and completed.
fn accept_reply(peer: InpAgentId, intent: &Intent, reply: PeerReply) -> Option<Vec<Assessment>> {
    let valid = reply.result.intent_id == intent.id
        && reply.result.provider_id == peer
        && reply.result.status == ExecutionStatus::Completed;
    if !valid {
        return None;
    }
    let owner = peer_id(peer);
    Some(
        reply
            .assessments
            .into_iter()
            .filter(|assessment| assessment.peer == owner)
            .collect(),
    )
}

/// The peer's side of a share. The peer reads the thought from the intent's payload, evaluates
/// it with its own providers and calibration, and returns the reply. A payload that is not a
/// thought, or an intent for another action, yields a failed result and no assessments.
pub fn answer_share(
    peer: InpAgentId,
    intent: &Intent,
    registry: &ProviderRegistry,
    calibration: &CalibrationTracker,
) -> PeerReply {
    let (status, assessments) = if intent.action != SHARE_ACTION {
        (
            ExecutionStatus::Failed {
                reason: format!("unsupported action {}", intent.action),
            },
            Vec::new(),
        )
    } else {
        match serde_json::from_value::<SharedThought>(intent.payload.clone()) {
            Ok(thought) => (
                ExecutionStatus::Completed,
                evaluate_thought(&thought, &peer_id(peer), registry, calibration),
            ),
            Err(error) => (
                ExecutionStatus::Failed {
                    reason: format!("malformed thought: {error}"),
                },
                Vec::new(),
            ),
        }
    };
    PeerReply {
        result: ExecutionResult {
            intent_id: intent.id,
            provider_id: peer,
            actual_cost: 0.0,
            actual_latency: Duration::ZERO,
            actual_accuracy: 0.0,
            proofs: Vec::new(),
            status,
        },
        assessments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{Probability, StructuredValue, Timestamp};
    use tpt_thought_providers::{
        merge_round, Locality, MergePolicy, Privacy, ProviderProfile, ReferenceProvider,
    };

    const QUESTION: &str = "is the claim supported";

    fn agent(n: u128) -> InpAgentId {
        InpAgentId::from_u128(n)
    }

    /// A peer hosting one provider that answers the question with `yes` at `confidence`.
    fn peer_with(
        provider: &str,
        yes: bool,
        confidence: f64,
    ) -> (ProviderRegistry, CalibrationTracker) {
        let mut registry = ProviderRegistry::new();
        let profile = ProviderProfile::new(Locality::Local, Privacy::OnDevice);
        let confidence = Probability::new(confidence).expect("test probability in range");
        registry
            .register(ReferenceProvider::new(provider, profile).with_answer(
                QUESTION,
                StructuredValue::Bool(yes),
                confidence,
            ))
            .expect("provider ids are unique within the peer");
        (registry, CalibrationTracker::default())
    }

    /// Peers reached in-process. Each answers from its own registry and sees only the intent it
    /// is sent. Peers listed in `down` cannot be reached.
    struct Peers {
        nodes: Vec<(InpAgentId, ProviderRegistry, CalibrationTracker)>,
        down: Vec<InpAgentId>,
        received: Vec<(InpAgentId, serde_json::Value)>,
    }

    impl Peers {
        fn new() -> Self {
            Self {
                nodes: Vec::new(),
                down: Vec::new(),
                received: Vec::new(),
            }
        }

        fn add(
            mut self,
            id: InpAgentId,
            (registry, calibration): (ProviderRegistry, CalibrationTracker),
        ) -> Self {
            self.nodes.push((id, registry, calibration));
            self
        }
    }

    impl PeerLink for Peers {
        fn send(&mut self, peer: InpAgentId, intent: &Intent) -> Result<PeerReply, LinkError> {
            self.received.push((peer, intent.payload.clone()));
            if self.down.contains(&peer) {
                return Err(LinkError {
                    reason: "unreachable".into(),
                });
            }
            let (_, registry, calibration) = self
                .nodes
                .iter()
                .find(|(id, _, _)| *id == peer)
                .ok_or_else(|| LinkError {
                    reason: "unknown peer".into(),
                })?;
            Ok(answer_share(peer, intent, registry, calibration))
        }
    }

    fn thought(origin: InpAgentId) -> SharedThought {
        SharedThought::new(
            peer_id(origin),
            QUESTION,
            StructuredValue::Bool(true),
            Timestamp(7),
        )
        .with_evidence("sensor reading 4.2")
    }

    fn broadcast_to(peers: &[InpAgentId], origin: InpAgentId) -> ThoughtBroadcast {
        let mut broadcast = ThoughtBroadcast::new(origin);
        for peer in peers {
            broadcast.offer(*peer);
        }
        broadcast
    }

    #[test]
    fn broadcast_reaches_every_offered_peer_except_the_origin() {
        let origin = agent(1);
        let peers = [agent(2), agent(3), agent(4)];
        let mut link = Peers::new()
            .add(agent(1), peer_with("origin-model", true, 0.9))
            .add(agent(2), peer_with("model-a", true, 0.9))
            .add(agent(3), peer_with("model-b", true, 0.9))
            .add(agent(4), peer_with("model-c", true, 0.9));
        let mut offered = peers.to_vec();
        offered.push(origin);
        let round = broadcast_to(&offered, origin).share(&thought(origin), &mut link);

        let mut reached: Vec<InpAgentId> = link.received.iter().map(|(peer, _)| *peer).collect();
        reached.sort();
        assert_eq!(reached, peers.to_vec());
        assert_eq!(round.assessments.len(), 3);
        assert!(round.missing.is_empty());
    }

    #[test]
    fn every_peer_receives_only_the_thought() {
        let origin = agent(1);
        let shared = thought(origin);
        let mut link = Peers::new()
            .add(agent(2), peer_with("model-a", true, 0.9))
            .add(agent(3), peer_with("model-b", false, 0.9));
        broadcast_to(&[agent(2), agent(3)], origin).share(&shared, &mut link);

        let expected = serde_json::to_value(&shared).expect("thought serialises");
        assert_eq!(link.received.len(), 2);
        // Identical payloads: no peer's reply is carried to another peer.
        for (_, payload) in &link.received {
            assert_eq!(payload, &expected);
        }
    }

    #[test]
    fn agreement_merges_to_the_shared_value() {
        let origin = agent(1);
        let mut link = Peers::new()
            .add(agent(2), peer_with("model-a", true, 0.9))
            .add(agent(3), peer_with("model-b", true, 0.8));
        let round = broadcast_to(&[agent(2), agent(3)], origin).share(&thought(origin), &mut link);
        let merged = merge_round(&round, MergePolicy::default());

        assert_eq!(merged.value, Some(StructuredValue::Bool(true)));
        assert!(merged.disagreement.is_none());
        assert_eq!(merged.counted, 2);
    }

    #[test]
    fn disagreement_is_reported_as_an_event() {
        let origin = agent(1);
        let mut link = Peers::new()
            .add(agent(2), peer_with("model-a", true, 0.9))
            .add(agent(3), peer_with("model-b", true, 0.9))
            .add(agent(4), peer_with("model-c", false, 0.9));
        let round = broadcast_to(&[agent(2), agent(3), agent(4)], origin)
            .share(&thought(origin), &mut link);
        let merged = merge_round(&round, MergePolicy::default());

        assert_eq!(merged.value, None);
        let disagreement = merged.disagreement.expect("disagreement is reported");
        assert_eq!(disagreement.positions.len(), 2);
        assert_eq!(disagreement.positions[0].sources.len(), 2);
    }

    #[test]
    fn peers_hosting_the_same_provider_count_once() {
        let origin = agent(1);
        let mut link = Peers::new()
            .add(agent(2), peer_with("shared-model", true, 0.9))
            .add(agent(3), peer_with("shared-model", true, 0.9))
            .add(agent(4), peer_with("other-model", false, 0.9));
        let round = broadcast_to(&[agent(2), agent(3), agent(4)], origin)
            .share(&thought(origin), &mut link);
        let merged = merge_round(&round, MergePolicy::default());

        assert_eq!(round.assessments.len(), 3);
        assert_eq!(merged.counted, 2);
    }

    #[test]
    fn unreachable_peer_is_recorded_as_missing_not_assessed() {
        let origin = agent(1);
        let mut link = Peers::new()
            .add(agent(2), peer_with("model-a", true, 0.9))
            .add(agent(3), peer_with("model-b", true, 0.9));
        link.down.push(agent(3));
        let round = broadcast_to(&[agent(2), agent(3)], origin).share(&thought(origin), &mut link);

        assert_eq!(round.missing, vec![peer_id(agent(3))]);
        assert_eq!(round.assessments.len(), 1);
        assert!(round
            .assessments
            .iter()
            .all(|a| a.peer == peer_id(agent(2))));
    }

    #[test]
    fn malformed_thought_gets_a_failed_reply_and_no_assessments() {
        let intent = Intent::new(
            SHARE_ACTION.to_string(),
            serde_json::Value::Null,
            QoSConstraints::default(),
            EconomicTerms::FixedPrice { credits: 0.0 },
            agent(1),
        );
        let (registry, calibration) = peer_with("model-a", true, 0.9);
        let reply = answer_share(agent(2), &intent, &registry, &calibration);

        assert!(matches!(
            reply.result.status,
            ExecutionStatus::Failed { .. }
        ));
        assert!(reply.assessments.is_empty());
    }
}
