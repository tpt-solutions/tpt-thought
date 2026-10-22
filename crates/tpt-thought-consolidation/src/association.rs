// SPDX-License-Identifier: MIT OR Apache-2.0
//! Association strengthening and weakening (spec sections 75, 76, todo Phase 9).
//!
//! A causal link must carry provenance (spec section 75): timing alone never creates one.
//! Causal links that are still being supported are strengthened; links that have gone stale
//! decay and are eventually pruned.

use crate::{extend_provenance, unit};
use std::collections::BTreeMap;
use std::fmt;
use tpt_thought_core::{ProvenanceId, Timestamp};

/// Strength gained by each supporting observation.
const SUPPORT_GAIN: f64 = 0.1;
/// Strength given to a link the first time it is seen.
const INITIAL_STRENGTH: f64 = 0.5;

/// A directed association between two topics.
#[derive(Debug, Clone, PartialEq)]
pub struct Association {
    /// Source topic.
    pub from: String,
    /// Target topic.
    pub to: String,
    /// True if the link is a causal claim backed by provenance.
    pub causal: bool,
    /// Strength, `0.0..=1.0`.
    pub strength: f64,
    /// Number of supporting observations.
    pub support: u32,
    /// When the link was last supported.
    pub last_supported: Timestamp,
    /// Provenance of the evidence for the link.
    pub provenance: Vec<ProvenanceId>,
}

/// Evidence for an association.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link<'a> {
    /// Source topic.
    pub from: &'a str,
    /// Target topic.
    pub to: &'a str,
    /// Whether this is a causal claim.
    pub causal: bool,
    /// Provenance supporting the claim.
    pub provenance: &'a [ProvenanceId],
}

/// Why an association update was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationError {
    /// A causal claim arrived without provenance.
    CausalWithoutProvenance,
}

impl fmt::Display for AssociationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CausalWithoutProvenance => {
                write!(
                    f,
                    "a causal association requires provenance (spec section 75)"
                )
            }
        }
    }
}

impl std::error::Error for AssociationError {}

/// How associations are reweighted during consolidation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReweightPolicy {
    /// A link not supported within this many nanoseconds is stale.
    pub stale_after_nanos: i64,
    /// Fraction of strength lost per pass while stale, `0.0..=1.0`.
    pub decay: f64,
    /// Strength gained per pass by a causal link that is still supported.
    pub causal_gain: f64,
    /// Links that decay below this strength are removed.
    pub prune_below: f64,
}

impl Default for ReweightPolicy {
    fn default() -> Self {
        Self {
            stale_after_nanos: 7 * 24 * 3600 * 1_000_000_000,
            decay: 0.2,
            causal_gain: 0.05,
            prune_below: 0.05,
        }
    }
}

/// Associations keyed by `(from, to)`.
#[derive(Debug, Clone, Default)]
pub struct AssociationStore {
    links: BTreeMap<(String, String), Association>,
}

impl AssociationStore {
    /// Record support for a link, creating it if needed.
    pub fn support(&mut self, link: Link<'_>, now: Timestamp) -> Result<(), AssociationError> {
        if link.causal && link.provenance.is_empty() {
            return Err(AssociationError::CausalWithoutProvenance);
        }
        let entry = self
            .links
            .entry((link.from.to_owned(), link.to.to_owned()))
            .or_insert_with(|| Association {
                from: link.from.to_owned(),
                to: link.to.to_owned(),
                causal: false,
                strength: INITIAL_STRENGTH,
                support: 0,
                last_supported: now,
                provenance: Vec::new(),
            });
        entry.strength = unit(entry.strength + SUPPORT_GAIN);
        entry.support += 1;
        entry.last_supported = now;
        entry.causal |= link.causal;
        extend_provenance(&mut entry.provenance, link.provenance);
        Ok(())
    }

    /// Reweight every link at `now`. Causal links supported within the stale window are
    /// strengthened; stale links decay. Links below `prune_below` are removed. Returns the
    /// removed `(from, to)` pairs.
    pub fn reweight(&mut self, now: Timestamp, policy: &ReweightPolicy) -> Vec<(String, String)> {
        let mut removed = Vec::new();
        self.links.retain(|key, link| {
            let age = now.0 - link.last_supported.0;
            if age <= policy.stale_after_nanos {
                if link.causal {
                    link.strength = unit(link.strength + policy.causal_gain);
                }
            } else {
                link.strength = unit(link.strength * (1.0 - policy.decay));
            }
            if link.strength < policy.prune_below {
                removed.push(key.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    /// The link from `from` to `to`, if any.
    pub fn get(&self, from: &str, to: &str) -> Option<&Association> {
        self.links.get(&(from.to_owned(), to.to_owned()))
    }

    /// All links, in key order.
    pub fn iter(&self) -> impl Iterator<Item = &Association> {
        self.links.values()
    }

    /// Links that are causal claims.
    pub fn causal(&self) -> Vec<&Association> {
        self.links.values().filter(|l| l.causal).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    fn link<'a>(from: &'a str, to: &'a str, causal: bool, prov: &'a [ProvenanceId]) -> Link<'a> {
        Link {
            from,
            to,
            causal,
            provenance: prov,
        }
    }

    #[test]
    fn causal_link_requires_provenance() {
        let mut s = AssociationStore::default();
        let r = s.support(link("a", "b", true, &[]), Timestamp(0));
        assert_eq!(r, Err(AssociationError::CausalWithoutProvenance));
        assert!(s.iter().next().is_none());
    }

    #[test]
    fn supported_causal_links_strengthen_and_stale_links_weaken() {
        let ids = IdGenerator::new(SeededEntropy::new(20));
        let prov = [ids.provenance()];
        let policy = ReweightPolicy {
            stale_after_nanos: 100,
            ..Default::default()
        };
        let mut s = AssociationStore::default();
        s.support(link("heat", "fault", true, &prov), Timestamp(0))
            .unwrap();
        s.support(link("x", "y", false, &[]), Timestamp(0)).unwrap();
        let before = s.get("heat", "fault").unwrap().strength;

        // Within the window: causal strengthens, plain link is unchanged.
        assert!(s.reweight(Timestamp(50), &policy).is_empty());
        assert!(s.get("heat", "fault").unwrap().strength > before);
        assert_eq!(
            s.get("x", "y").unwrap().strength,
            INITIAL_STRENGTH + SUPPORT_GAIN
        );

        // Past the window: both decay.
        let stale_strength = s.get("x", "y").unwrap().strength;
        s.reweight(Timestamp(1_000), &policy);
        assert!(s.get("x", "y").unwrap().strength < stale_strength);
    }

    #[test]
    fn stale_links_are_pruned_and_reported() {
        let policy = ReweightPolicy {
            stale_after_nanos: 10,
            decay: 0.9,
            prune_below: 0.2,
            ..Default::default()
        };
        let mut s = AssociationStore::default();
        s.support(link("a", "b", false, &[]), Timestamp(0)).unwrap();
        let removed = s.reweight(Timestamp(100), &policy);
        assert_eq!(removed, vec![("a".to_owned(), "b".to_owned())]);
        assert!(s.get("a", "b").is_none());
    }
}
