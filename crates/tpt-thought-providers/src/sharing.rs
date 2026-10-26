// SPDX-License-Identifier: MIT OR Apache-2.0
//! Multi-agent thought sharing (spec section 39).
//!
//! An originating agent broadcasts a [`SharedThought`]: a question, the evidence cited for it and
//! a claim about it. Each peer evaluates the thought with its own providers, using its own
//! calibration, and returns [`Assessment`]s. Peers share no state, so no peer sees another's
//! assessment before its own reply is complete. [`merge_round`] combines the replies of one
//! [`ShareRound`] into a single [`Merged`] result.
//!
//! This module is pure and does not depend on any transport. The INP transport lives in
//! `tpt-thought-integrations` behind the `anima` feature. Disagreement uses the §66 types in
//! [`crate::ensemble`], so it can be investigated with the same [`Probe`](crate::ensemble::Probe)s.

use crate::calibration::CalibrationTracker;
use crate::decision::{Answer, Decision, DecisionSource};
use crate::ensemble::{Disagreement, Position, Probe};
use crate::registry::ProviderRegistry;
use crate::types::{Constraints, IntelligenceRequest, ProviderId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tpt_thought_core::{Probability, StructuredValue, Timestamp};

/// The weight of an answer whose reliability is unknown. It is neutral: neither trusted nor
/// ignored.
pub const UNKNOWN_WEIGHT: f64 = 0.5;

/// The default share of weight that may dissent before the merged value is withheld.
pub const DEFAULT_DISAGREEMENT_THRESHOLD: f64 = 0.2;

/// Identifies an agent that takes part in a share round.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PeerId(String);

impl PeerId {
    /// Wrap an identifier string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The identifier as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A thought broadcast to peers for independent evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedThought {
    /// The agent that broadcast the thought.
    pub origin: PeerId,
    /// The question the thought answers, as a stable key.
    pub question: String,
    /// Evidence the originator cites.
    pub evidence: Vec<String>,
    /// The originator's claim about the question.
    pub claim: StructuredValue,
    /// When the thought was broadcast.
    pub at: Timestamp,
}

impl SharedThought {
    /// A thought with no evidence yet.
    pub fn new(
        origin: PeerId,
        question: impl Into<String>,
        claim: StructuredValue,
        at: Timestamp,
    ) -> Self {
        Self {
            origin,
            question: question.into(),
            evidence: Vec::new(),
            claim,
            at,
        }
    }

    /// Cite one piece of evidence.
    pub fn with_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence.push(evidence.into());
        self
    }

    /// The request a peer puts to its providers. Evidence and the claim travel as context.
    pub fn request(&self) -> IntelligenceRequest {
        let mut request = IntelligenceRequest::new(self.question.clone(), self.at);
        request.context = self.evidence.clone();
        request.context.push(format!("claim: {:?}", self.claim));
        request
    }
}

/// One peer's answer from one of its providers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    /// The peer that evaluated the thought.
    pub peer: PeerId,
    /// The provider's decision, with its answer or abstention, confidence and provenance.
    pub decision: Decision<StructuredValue>,
}

/// Evaluate a thought with one peer's own providers. Each provider is asked once. A provider
/// that fails produces no assessment, because a failure is not an answer.
pub fn evaluate_thought(
    thought: &SharedThought,
    peer: &PeerId,
    registry: &ProviderRegistry,
    calibration: &CalibrationTracker,
) -> Vec<Assessment> {
    let request = thought.request();
    registry
        .candidates(&Constraints::default(), calibration)
        .into_iter()
        .filter_map(|candidate| candidate.provider.decide(&request).ok())
        .map(|decision| Assessment {
            peer: peer.clone(),
            decision,
        })
        .collect()
}

/// What came back from one broadcast of a thought.
#[derive(Debug, Clone, PartialEq)]
pub struct ShareRound {
    /// The thought that was broadcast.
    pub thought: SharedThought,
    /// Assessments, in the order the replies arrived.
    pub assessments: Vec<Assessment>,
    /// Peers that were asked and gave no usable reply. They are missing, not assessed.
    pub missing: Vec<PeerId>,
}

/// How much dissent a merge tolerates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MergePolicy {
    /// The largest share of weight that may disagree with the leading answer before the merged
    /// value is withheld.
    pub disagreement_threshold: Probability,
}

impl Default for MergePolicy {
    fn default() -> Self {
        Self {
            disagreement_threshold: Probability::new(DEFAULT_DISAGREEMENT_THRESHOLD)
                .unwrap_or(Probability::ZERO),
        }
    }
}

/// The merged result of a share round.
#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    /// The agreed answer. `None` if nothing answered, if the leading answer is tied, or if the
    /// dissent is above the policy threshold.
    pub value: Option<StructuredValue>,
    /// The leading answer's share of the counted weight, from 0 to 1.
    pub agreement: Probability,
    /// How many assessments were counted after abstentions and duplicates were removed.
    pub counted: usize,
    /// Peers that gave no usable reply.
    pub missing: Vec<PeerId>,
    /// Set whenever two or more distinct answers were counted. It is reported even when the
    /// dissent is below the threshold and a value is still returned.
    pub disagreement: Option<Disagreement>,
}

/// Merge the assessments of one round.
///
/// The rule, in order:
///
/// 1. Abstentions are ignored. They neither agree nor disagree.
/// 2. Each provider identity counts once. Two assessments with the same provider id, from the same
///    peer or from different peers, are counted at the first position in the round. This stops one
///    model hosted on several peers from voting several times.
/// 3. Each counted assessment has weight equal to its reliability (empirical, then calibrated, then
///    raw). An assessment with no reliability has weight [`UNKNOWN_WEIGHT`].
/// 4. Weights are summed per distinct answer. The leading answer is the one with the largest sum.
///    Its agreement is its sum divided by the total of all sums.
/// 5. The dissent is `1 - agreement`. If there is more than one distinct answer, a [`Disagreement`]
///    is reported, listing the positions and the [`Probe`]s to investigate them with.
/// 6. The merged value is the leading answer only if the leader is not tied and the dissent is not
///    above `policy.disagreement_threshold`. Otherwise `value` is `None` and the disagreement is
///    still reported.
///
/// Missing peers are not in the weights. They neither add agreement nor dissent, and they are
/// listed in [`Merged::missing`] so that the caller can see how much of the round was heard.
pub fn merge_round(round: &ShareRound, policy: MergePolicy) -> Merged {
    let mut seen: BTreeSet<ProviderId> = BTreeSet::new();
    let mut groups: Vec<(StructuredValue, f64, Vec<DecisionSource>)> = Vec::new();
    let mut counted = 0;
    for assessment in &round.assessments {
        let decision = &assessment.decision;
        let Answer::Answered(value) = &decision.answer else {
            continue;
        };
        if !seen.insert(decision.source.provider_id()) {
            continue;
        }
        counted += 1;
        let weight = decision
            .reliability()
            .map_or(UNKNOWN_WEIGHT, Probability::get);
        match groups.iter_mut().find(|(known, _, _)| known == value) {
            Some((_, total, sources)) => {
                *total += weight;
                sources.push(decision.source.clone());
            }
            None => groups.push((value.clone(), weight, vec![decision.source.clone()])),
        }
    }

    let total: f64 = groups.iter().map(|(_, weight, _)| weight).sum();
    groups.sort_by(|a, b| b.1.total_cmp(&a.1));
    let leader_weight = groups.first().map_or(0.0, |(_, weight, _)| *weight);
    let tied = groups
        .get(1)
        .is_some_and(|(_, weight, _)| weight.total_cmp(&leader_weight).is_eq());
    let share = if total > 0.0 {
        leader_weight / total
    } else {
        0.0
    };
    let dissent = 1.0 - share;
    let accepted = total > 0.0 && !tied && dissent <= policy.disagreement_threshold.get();

    let disagreement = (groups.len() > 1).then(|| Disagreement {
        positions: groups
            .iter()
            .map(|(value, _, sources)| Position {
                value: value.clone(),
                sources: sources.clone(),
            })
            .collect(),
        probes: Probe::ALL.to_vec(),
    });
    let value = accepted
        .then(|| groups.first().map(|(value, _, _)| value.clone()))
        .flatten();

    Merged {
        value,
        agreement: Probability::new(share).unwrap_or(Probability::ZERO),
        counted,
        missing: round.missing.clone(),
        disagreement,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::Confidence;

    fn p(x: f64) -> Probability {
        Probability::new(x).expect("test probability in range")
    }

    fn peer(name: &str) -> PeerId {
        PeerId::new(name)
    }

    fn vote(peer_name: &str, provider: &str, yes: bool, reliability: f64) -> Assessment {
        Assessment {
            peer: peer(peer_name),
            decision: Decision::answered(
                DecisionSource::Provider(ProviderId::new(provider)),
                StructuredValue::Bool(yes),
                Confidence::from_raw(p(reliability)),
                Timestamp(1),
            ),
        }
    }

    fn round(assessments: Vec<Assessment>, missing: Vec<PeerId>) -> ShareRound {
        ShareRound {
            thought: SharedThought::new(
                peer("origin"),
                "is the claim supported",
                StructuredValue::Bool(true),
                Timestamp(1),
            ),
            assessments,
            missing,
        }
    }

    #[test]
    fn agreement_returns_the_shared_value_without_disagreement() {
        let merged = merge_round(
            &round(
                vec![
                    vote("b", "model-x", true, 0.9),
                    vote("c", "model-y", true, 0.8),
                ],
                Vec::new(),
            ),
            MergePolicy::default(),
        );
        assert_eq!(merged.value, Some(StructuredValue::Bool(true)));
        assert_eq!(merged.agreement.get(), 1.0);
        assert_eq!(merged.counted, 2);
        assert!(merged.disagreement.is_none());
    }

    #[test]
    fn disagreement_above_threshold_is_reported_and_value_withheld() {
        let merged = merge_round(
            &round(
                vec![
                    vote("b", "model-x", true, 0.9),
                    vote("c", "model-y", true, 0.9),
                    vote("d", "model-z", false, 0.9),
                ],
                Vec::new(),
            ),
            MergePolicy::default(),
        );
        assert_eq!(merged.value, None);
        let disagreement = merged.disagreement.expect("disagreement is reported");
        assert_eq!(disagreement.positions.len(), 2);
        assert_eq!(disagreement.positions[0].value, StructuredValue::Bool(true));
        assert_eq!(disagreement.positions[0].sources.len(), 2);
        assert_eq!(disagreement.probes, Probe::ALL.to_vec());
    }

    #[test]
    fn dissent_below_threshold_keeps_the_value_but_still_reports_it() {
        let merged = merge_round(
            &round(
                vec![
                    vote("b", "model-x", true, 0.9),
                    vote("c", "model-y", true, 0.9),
                    vote("d", "model-z", true, 0.9),
                    vote("e", "model-w", false, 0.1),
                ],
                Vec::new(),
            ),
            MergePolicy::default(),
        );
        assert_eq!(merged.value, Some(StructuredValue::Bool(true)));
        assert!(merged.disagreement.is_some());
    }

    #[test]
    fn a_tie_withholds_the_value() {
        let merged = merge_round(
            &round(
                vec![
                    vote("b", "model-x", true, 0.8),
                    vote("c", "model-y", false, 0.8),
                ],
                Vec::new(),
            ),
            MergePolicy {
                disagreement_threshold: p(0.5),
            },
        );
        assert_eq!(merged.value, None);
        assert!(merged.disagreement.is_some());
    }

    #[test]
    fn duplicate_provider_ids_are_counted_once() {
        // Peers b and c both host model-x. Counted twice, true would have weight 1.8 against 0.9,
        // an agreement of 0.67. Counted once, the agreement is 0.5.
        let merged = merge_round(
            &round(
                vec![
                    vote("b", "model-x", true, 0.9),
                    vote("c", "model-x", true, 0.9),
                    vote("d", "model-y", false, 0.9),
                ],
                Vec::new(),
            ),
            MergePolicy::default(),
        );
        assert_eq!(merged.counted, 2);
        assert_eq!(merged.agreement.get(), 0.5);
    }

    #[test]
    fn abstentions_and_missing_peers_do_not_count_as_votes() {
        let abstention = Assessment {
            peer: peer("c"),
            decision: Decision::abstained(
                DecisionSource::Provider(ProviderId::new("model-y")),
                "not enough evidence",
                Timestamp(1),
            ),
        };
        let merged = merge_round(
            &round(
                vec![vote("b", "model-x", true, 0.9), abstention],
                vec![peer("d")],
            ),
            MergePolicy::default(),
        );
        assert_eq!(merged.counted, 1);
        assert_eq!(merged.value, Some(StructuredValue::Bool(true)));
        assert_eq!(merged.missing, vec![peer("d")]);
    }

    #[test]
    fn no_usable_assessment_gives_no_value() {
        let merged = merge_round(&round(Vec::new(), vec![peer("b")]), MergePolicy::default());
        assert_eq!(merged.value, None);
        assert_eq!(merged.agreement.get(), 0.0);
        assert_eq!(merged.counted, 0);
        assert!(merged.disagreement.is_none());
    }

    #[test]
    fn unknown_reliability_gets_the_neutral_weight() {
        let unknown = Assessment {
            peer: peer("c"),
            decision: Decision::answered(
                DecisionSource::Provider(ProviderId::new("model-y")),
                StructuredValue::Bool(true),
                Confidence::UNKNOWN,
                Timestamp(1),
            ),
        };
        let merged = merge_round(
            &round(vec![vote("b", "model-x", true, 0.5), unknown], Vec::new()),
            MergePolicy::default(),
        );
        assert_eq!(merged.value, Some(StructuredValue::Bool(true)));
        assert_eq!(merged.agreement.get(), 1.0);
    }
}
