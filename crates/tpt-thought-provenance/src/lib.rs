// SPDX-License-Identifier: MIT OR Apache-2.0
//! Machine-readable cognitive provenance (spec sections 31, 70, 71).
//!
//! Each [`ProvenanceRecord`] states where a piece of cognition came from. Records are linked to
//! their parents and carry a SHA-256 hash covering their content and their parents' hashes,
//! so tampering with any ancestor is detectable. [`ProvenanceStore::explain`] answers "why did
//! the agent think this?" from structure alone, with no generated explanation.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt;
use tpt_thought_core::wire;
use tpt_thought_core::{ContentReference, ProvenanceId, ThoughtId, Timestamp};

/// A SHA-256 digest, serialised as lowercase hex.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash256(pub [u8; 32]);

impl fmt::Debug for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl Serialize for Hash256 {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Hash256 {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s.len() != 64 || !s.is_ascii() {
            return Err(serde::de::Error::custom("expected 64 hex characters"));
        }
        let mut out = [0u8; 32];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(serde::de::Error::custom)?;
        }
        Ok(Self(out))
    }
}

/// The kind of source a record describes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceSource {
    /// An observation of the world.
    Observation,
    /// A memory retrieval.
    Memory,
    /// A decision from an intelligence provider.
    Decision,
    /// A deliberation or reasoning step.
    Reasoning,
    /// A probabilistic inference.
    Inference,
    /// A verification step.
    Verification,
    /// An intention formed.
    Intention,
    /// A human contribution.
    Human,
    /// Anything else, named by the integrator.
    Other(String),
}

/// The content of a provenance record, before hashing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceBody {
    /// Identity of this record.
    pub id: ProvenanceId,
    /// What kind of source this is.
    pub source: ProvenanceSource,
    /// The thought this record explains, if any.
    pub thought: Option<ThoughtId>,
    /// Records this one derives from.
    pub parents: Vec<ProvenanceId>,
    /// Evidence used.
    pub evidence: Vec<ContentReference>,
    /// Memories used.
    pub memory: Vec<ContentReference>,
    /// Provider that produced the result (opaque label; never branched on).
    pub provider: Option<String>,
    /// Model identifier.
    pub model: Option<String>,
    /// Provider or model version.
    pub version: Option<String>,
    /// Structured reasoning steps (not a model's private chain of thought).
    pub reasoning_path: Vec<String>,
    /// Verification applied and its result.
    pub verification: Option<String>,
    /// When it happened.
    pub timestamp: Timestamp,
}

/// A provenance body sealed with an integrity hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    /// The content.
    pub body: ProvenanceBody,
    /// Hash of `body` and the hashes of its parents.
    pub hash: Hash256,
}

impl ProvenanceRecord {
    fn compute_hash(body: &ProvenanceBody, parent_hashes: &[Hash256]) -> Hash256 {
        let mut h = Sha256::new();
        // Encoding a plain struct to CBOR cannot fail.
        h.update(wire::to_cbor(body).expect("provenance body encodes"));
        for p in parent_hashes {
            h.update(p.0);
        }
        Hash256(h.finalize().into())
    }
}

/// Problems detected by [`ProvenanceStore::verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityError {
    /// A record names a parent that is not in the store.
    MissingParent {
        /// The record.
        record: ProvenanceId,
        /// The absent parent.
        parent: ProvenanceId,
    },
    /// A record's stored hash does not match its recomputed hash.
    HashMismatch {
        /// The tampered record.
        record: ProvenanceId,
    },
}

/// Error inserting a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertError {
    /// A record with this ID already exists.
    Duplicate(ProvenanceId),
    /// A named parent does not exist yet. Parents must be inserted first, which also makes
    /// cycles impossible.
    MissingParent(ProvenanceId),
}

impl fmt::Display for InsertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(id) => write!(f, "duplicate provenance record {id}"),
            Self::MissingParent(id) => write!(f, "missing parent provenance record {id}"),
        }
    }
}

impl std::error::Error for InsertError {}

/// An append-only, acyclic store of provenance records.
#[derive(Debug, Default, Clone)]
pub struct ProvenanceStore {
    records: HashMap<ProvenanceId, ProvenanceRecord>,
    order: Vec<ProvenanceId>,
}

impl ProvenanceStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// True if there are no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Look up a record.
    pub fn get(&self, id: &ProvenanceId) -> Option<&ProvenanceRecord> {
        self.records.get(id)
    }

    /// Seal and insert `body`. Parents must already be present.
    pub fn insert(&mut self, body: ProvenanceBody) -> Result<&ProvenanceRecord, InsertError> {
        if self.records.contains_key(&body.id) {
            return Err(InsertError::Duplicate(body.id));
        }
        let mut parent_hashes = Vec::with_capacity(body.parents.len());
        for p in &body.parents {
            let rec = self.records.get(p).ok_or(InsertError::MissingParent(*p))?;
            parent_hashes.push(rec.hash);
        }
        let hash = ProvenanceRecord::compute_hash(&body, &parent_hashes);
        let id = body.id;
        self.order.push(id);
        Ok(self
            .records
            .entry(id)
            .or_insert(ProvenanceRecord { body, hash }))
    }

    /// All ancestors of `id` (excluding `id`), nearest first, each once.
    pub fn ancestors(&self, id: &ProvenanceId) -> Vec<&ProvenanceRecord> {
        let mut out = Vec::new();
        let mut seen: HashSet<ProvenanceId> = HashSet::new();
        let mut frontier: Vec<ProvenanceId> = self
            .records
            .get(id)
            .map(|r| r.body.parents.clone())
            .unwrap_or_default();
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for pid in frontier {
                if !seen.insert(pid) {
                    continue;
                }
                if let Some(rec) = self.records.get(&pid) {
                    next.extend(rec.body.parents.iter().copied());
                    out.push(rec);
                }
            }
            frontier = next;
        }
        out
    }

    /// Direct and transitive descendants of `id` (records that derive from it).
    pub fn descendants(&self, id: &ProvenanceId) -> Vec<&ProvenanceRecord> {
        let mut known: HashSet<ProvenanceId> = HashSet::from([*id]);
        let mut out = Vec::new();
        // `order` is insertion order, and parents precede children, so one pass suffices.
        for rid in &self.order {
            let rec = &self.records[rid];
            if rec.body.parents.iter().any(|p| known.contains(p)) {
                known.insert(*rid);
                out.push(rec);
            }
        }
        out
    }

    /// Records explaining `thought` (those whose `thought` field matches), oldest first.
    pub fn for_thought(&self, thought: &ThoughtId) -> Vec<&ProvenanceRecord> {
        self.order
            .iter()
            .map(|id| &self.records[id])
            .filter(|r| r.body.thought.as_ref() == Some(thought))
            .collect()
    }

    /// "Why did the agent think this?" The full derivation of `thought`: its own records
    /// followed by every ancestor, deduplicated and ordered oldest first. Purely structural.
    pub fn explain(&self, thought: &ThoughtId) -> Vec<&ProvenanceRecord> {
        let mut seen: HashSet<ProvenanceId> = HashSet::new();
        let mut out: Vec<&ProvenanceRecord> = Vec::new();
        for rec in self.for_thought(thought) {
            for r in self.ancestors(&rec.body.id).into_iter().chain([rec]) {
                if seen.insert(r.body.id) {
                    out.push(r);
                }
            }
        }
        let position: HashMap<_, _> = self
            .order
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();
        out.sort_by_key(|r| position[&r.body.id]);
        out
    }

    /// Recompute every hash and check parent links.
    pub fn verify(&self) -> Result<(), IntegrityError> {
        for id in &self.order {
            let rec = &self.records[id];
            let mut parent_hashes = Vec::with_capacity(rec.body.parents.len());
            for p in &rec.body.parents {
                let parent = self.records.get(p).ok_or(IntegrityError::MissingParent {
                    record: *id,
                    parent: *p,
                })?;
                parent_hashes.push(parent.hash);
            }
            if ProvenanceRecord::compute_hash(&rec.body, &parent_hashes) != rec.hash {
                return Err(IntegrityError::HashMismatch { record: *id });
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn tamper(&mut self, id: &ProvenanceId, f: impl FnOnce(&mut ProvenanceBody)) {
        f(&mut self.records.get_mut(id).unwrap().body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    fn body(
        ids: &IdGenerator,
        parents: Vec<ProvenanceId>,
        thought: Option<ThoughtId>,
    ) -> ProvenanceBody {
        ProvenanceBody {
            id: ids.provenance(),
            source: ProvenanceSource::Decision,
            thought,
            parents,
            evidence: vec![],
            memory: vec![],
            provider: Some("specialist-vision-v3".into()),
            model: None,
            version: Some("3".into()),
            reasoning_path: vec!["classify".into()],
            verification: None,
            timestamp: Timestamp(1),
        }
    }

    /// obs <- decision <- intention, all explaining one thought.
    fn chain() -> (ProvenanceStore, ThoughtId, [ProvenanceId; 3]) {
        let ids = IdGenerator::new(SeededEntropy::new(9));
        let t = ids.thought();
        let mut s = ProvenanceStore::new();
        let a = s.insert(body(&ids, vec![], None)).unwrap().body.id;
        let b = s.insert(body(&ids, vec![a], None)).unwrap().body.id;
        let c = s.insert(body(&ids, vec![b], Some(t))).unwrap().body.id;
        (s, t, [a, b, c])
    }

    #[test]
    fn explain_returns_full_derivation_oldest_first() {
        let (s, t, [a, b, c]) = chain();
        let ids: Vec<_> = s.explain(&t).iter().map(|r| r.body.id).collect();
        assert_eq!(ids, vec![a, b, c]);
    }

    #[test]
    fn ancestors_and_descendants() {
        let (s, _, [a, b, c]) = chain();
        let anc: Vec<_> = s.ancestors(&c).iter().map(|r| r.body.id).collect();
        assert_eq!(anc, vec![b, a]);
        let desc: Vec<_> = s.descendants(&a).iter().map(|r| r.body.id).collect();
        assert_eq!(desc, vec![b, c]);
    }

    #[test]
    fn verify_passes_then_detects_tampering() {
        let (mut s, _, [a, ..]) = chain();
        assert_eq!(s.verify(), Ok(()));
        s.tamper(&a, |b| b.provider = Some("forged".into()));
        assert!(matches!(
            s.verify(),
            Err(IntegrityError::HashMismatch { .. })
        ));
    }

    #[test]
    fn rejects_missing_parent_and_duplicates() {
        let ids = IdGenerator::new(SeededEntropy::new(3));
        let mut s = ProvenanceStore::new();
        let orphan = body(&ids, vec![ids.provenance()], None);
        assert!(matches!(
            s.insert(orphan),
            Err(InsertError::MissingParent(_))
        ));
        let b = body(&ids, vec![], None);
        s.insert(b.clone()).unwrap();
        assert!(matches!(s.insert(b), Err(InsertError::Duplicate(_))));
    }

    #[test]
    fn hash_covers_parent_hashes() {
        let (s, _, [a, _, c]) = chain();
        // Same body content under a different parent chain yields a different hash.
        assert_ne!(s.get(&a).unwrap().hash, s.get(&c).unwrap().hash);
    }

    #[test]
    fn record_json_round_trip() {
        let (s, _, [a, ..]) = chain();
        let rec = s.get(&a).unwrap();
        let back: ProvenanceRecord = wire::from_json(&wire::to_json(rec).unwrap()).unwrap();
        assert_eq!(&back, rec);
    }
}
