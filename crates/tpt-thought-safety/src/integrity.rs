// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integrity, trust and identity checks (spec section 41). Hashing is not reimplemented here:
//! provenance seals and verification come from `tpt-thought-provenance`, and source classification
//! comes from `tpt-thought-core`.

use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use tpt_thought_core::{
    Observation, ProvenanceId, SourceTrust, Thought, ThoughtId, ThoughtOwnership,
};
use tpt_thought_provenance::{IntegrityError, ProvenanceStore};

/// A thought's provenance does not check out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityFault {
    /// The store failed verification (a hash mismatch or a missing parent).
    Store(IntegrityError),
    /// The thought cites a provenance record the store does not hold.
    MissingRecord(ProvenanceId),
    /// The thought cites a record that explains a different thought: false provenance.
    ForeignRecord {
        /// The record cited.
        record: ProvenanceId,
        /// The thought the record actually explains.
        explains: ThoughtId,
    },
}

impl fmt::Display for IntegrityFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => write!(f, "provenance store failed verification: {e:?}"),
            Self::MissingRecord(id) => write!(f, "provenance record {id:?} is not in the store"),
            Self::ForeignRecord { record, explains } => write!(
                f,
                "provenance record {record:?} explains {explains:?}, not this thought"
            ),
        }
    }
}

impl std::error::Error for IntegrityFault {}

/// Verify the whole provenance store, then check that every record `thought` cites exists and
/// explains that thought. Catches tampered records, dangling links and false provenance.
pub fn verify_provenance(store: &ProvenanceStore, thought: &Thought) -> Result<(), IntegrityFault> {
    store.verify().map_err(IntegrityFault::Store)?;
    for id in &thought.provenance.records {
        let record = store.get(id).ok_or(IntegrityFault::MissingRecord(*id))?;
        if let Some(explains) = &record.body.thought {
            if *explains != thought.id {
                return Err(IntegrityFault::ForeignRecord {
                    record: *id,
                    explains: *explains,
                });
            }
        }
    }
    Ok(())
}

/// An observation was refused because its source is trusted less than required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustRejected {
    /// The observation's trust level.
    pub trust: SourceTrust,
    /// The minimum the gate requires.
    pub minimum: SourceTrust,
}

impl fmt::Display for TrustRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "observation trust {:?} is below required {:?}",
            self.trust, self.minimum
        )
    }
}

impl std::error::Error for TrustRejected {}

fn rank(trust: SourceTrust) -> u8 {
    match trust {
        SourceTrust::Untrusted => 0,
        SourceTrust::Authenticated => 1,
        SourceTrust::Trusted => 2,
    }
}

/// Admits only observations at or above a minimum trust level. Untrusted content (fabricated
/// observations, prompt or memory injection) can be kept out of anything that drives action by
/// setting the minimum to [`SourceTrust::Authenticated`] or higher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationGate {
    minimum: SourceTrust,
}

impl ObservationGate {
    /// A gate requiring at least `minimum`.
    pub fn new(minimum: SourceTrust) -> Self {
        Self { minimum }
    }

    /// The minimum trust level admitted.
    pub fn minimum(&self) -> SourceTrust {
        self.minimum
    }

    /// Succeed only if the observation's trust meets the minimum.
    pub fn admit(&self, observation: &Observation) -> Result<(), TrustRejected> {
        if rank(observation.trust) >= rank(self.minimum) {
            Ok(())
        } else {
            Err(TrustRejected {
                trust: observation.trust,
                minimum: self.minimum,
            })
        }
    }
}

/// Rejects a key the second time it is seen, so a captured message cannot be replayed.
/// Keys are typically observation or thought identifiers.
#[derive(Debug, Clone)]
pub struct ReplayGuard<K> {
    seen: HashSet<K>,
}

impl<K: Hash + Eq> ReplayGuard<K> {
    /// An empty guard.
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    /// Record `key`. Returns `true` if it is new, `false` if it is a replay.
    pub fn admit(&mut self, key: K) -> bool {
        self.seen.insert(key)
    }

    /// Number of keys remembered.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// True if no keys have been recorded.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

impl<K: Hash + Eq> Default for ReplayGuard<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// An actor tried to act on a thought it has no right to act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityMismatch {
    /// The actor that tried.
    pub actor: String,
    /// Identities permitted to act. Empty when nobody may act (archived thoughts).
    pub permitted: Vec<String>,
}

impl fmt::Display for IdentityMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.permitted.is_empty() {
            write!(f, "'{}' may not act: thought is archived", self.actor)
        } else {
            write!(
                f,
                "'{}' may not act; permitted: {}",
                self.actor,
                self.permitted.join(", ")
            )
        }
    }
}

impl std::error::Error for IdentityMismatch {}

/// Check that `actor` may act on a thought with `ownership` held by `owner`. Identities are
/// resolved by Anima ZKPA; this only compares the identifiers it is given. Delegation moves the
/// right to act to the delegate alone, and archived thoughts can be acted on by nobody.
pub fn admit_actor(
    ownership: &ThoughtOwnership,
    owner: &str,
    actor: &str,
) -> Result<(), IdentityMismatch> {
    let permitted: Vec<String> = match ownership {
        ThoughtOwnership::Private | ThoughtOwnership::Shared | ThoughtOwnership::Published => {
            vec![owner.to_owned()]
        }
        ThoughtOwnership::Delegated { to } => vec![to.clone()],
        ThoughtOwnership::Archived => vec![],
    };
    if permitted.iter().any(|p| p == actor) {
        Ok(())
    } else {
        Err(IdentityMismatch {
            actor: actor.to_owned(),
            permitted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{
        IdGenerator, ObservationId, ProvenanceId as Pid, SeededEntropy, StructuredValue,
        ThoughtContent, ThoughtOrigin, Timestamp,
    };
    use tpt_thought_provenance::{ProvenanceBody, ProvenanceSource};

    fn ids() -> IdGenerator {
        IdGenerator::new(SeededEntropy::new(7))
    }

    fn observation(trust: SourceTrust, id: ObservationId) -> Observation {
        Observation {
            id,
            content: Box::new(ThoughtContent::Structured(StructuredValue::Null)),
            source: "test".into(),
            trust,
            observed_at: Timestamp(0),
        }
    }

    #[test]
    fn gate_rejects_below_minimum() {
        let g = ids();
        let gate = ObservationGate::new(SourceTrust::Authenticated);
        let oid = g.observation();
        assert!(gate
            .admit(&observation(SourceTrust::Untrusted, oid))
            .is_err());
        assert!(gate
            .admit(&observation(SourceTrust::Authenticated, oid))
            .is_ok());
        assert!(gate.admit(&observation(SourceTrust::Trusted, oid)).is_ok());
        let err = gate
            .admit(&observation(SourceTrust::Untrusted, oid))
            .unwrap_err();
        assert_eq!(err.minimum, SourceTrust::Authenticated);
    }

    #[test]
    fn replay_is_detected() {
        let g = ids();
        let mut guard = ReplayGuard::new();
        let oid = g.observation();
        assert!(guard.admit(oid));
        assert!(!guard.admit(oid), "second sighting is a replay");
        assert_eq!(guard.len(), 1);
    }

    #[test]
    fn identity_substitution_is_refused() {
        assert!(admit_actor(&ThoughtOwnership::Private, "anima:a", "anima:a").is_ok());
        assert!(admit_actor(&ThoughtOwnership::Private, "anima:a", "anima:b").is_err());
        let delegated = ThoughtOwnership::Delegated {
            to: "anima:b".into(),
        };
        assert!(admit_actor(&delegated, "anima:a", "anima:b").is_ok());
        assert!(admit_actor(&delegated, "anima:a", "anima:a").is_err());
        let archived = admit_actor(&ThoughtOwnership::Archived, "anima:a", "anima:a");
        assert_eq!(archived.unwrap_err().permitted, Vec::<String>::new());
    }

    #[test]
    fn provenance_is_verified_and_false_links_are_caught() {
        let g = ids();
        let mut store = ProvenanceStore::new();
        let mut thought = Thought::new(
            g.thought(),
            ThoughtOrigin::Observation,
            ThoughtContent::Structured(StructuredValue::Null),
            Timestamp(0),
        );
        let pid: ProvenanceId = g.provenance();
        let body = |id: Pid, thought: Option<ThoughtId>| ProvenanceBody {
            id,
            source: ProvenanceSource::Observation,
            thought,
            parents: vec![],
            evidence: vec![],
            memory: vec![],
            provider: None,
            model: None,
            version: None,
            reasoning_path: vec![],
            verification: None,
            timestamp: Timestamp(0),
        };
        store.insert(body(pid, Some(thought.id))).unwrap();
        thought.provenance.records.push(pid);
        assert!(verify_provenance(&store, &thought).is_ok());

        let other = g.thought();
        let foreign = g.provenance();
        store.insert(body(foreign, Some(other))).unwrap();
        thought.provenance.records.push(foreign);
        assert!(matches!(
            verify_provenance(&store, &thought),
            Err(IntegrityFault::ForeignRecord { .. })
        ));

        thought.provenance.records = vec![g.provenance()];
        assert!(matches!(
            verify_provenance(&store, &thought),
            Err(IntegrityFault::MissingRecord(_))
        ));
    }
}
