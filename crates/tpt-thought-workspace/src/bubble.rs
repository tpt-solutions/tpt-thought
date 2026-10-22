// SPDX-License-Identifier: MIT OR Apache-2.0
//! The thought bubble (spec section 42): the execution boundary for experimental cognition.
//!
//! A bubble works on a snapshot of durable state and records its own writes in an overlay.
//! Durable state is untouched until [`ThoughtBubble::commit`], which copies only the kinds a
//! [`CommitPolicy`] allows. Discarding a bubble is just dropping it: nothing it wrote can have
//! reached durable state.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use tpt_thought_core::ThoughtContent;

/// The kinds of temporary cognition a bubble may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BubbleKind {
    /// Temporary memory traces.
    Memory,
    /// Speculative hypotheses.
    Hypothesis,
    /// Tools used only inside the bubble. Never committed.
    Tool,
    /// Exploratory branches. Never committed.
    Branch,
    /// Temporary models. Never committed.
    Model,
}

impl BubbleKind {
    /// Whether this kind may ever be committed to durable state. Tools, branches and models
    /// are execution scaffolding and are refused unconditionally.
    pub const fn committable(self) -> bool {
        matches!(self, Self::Memory | Self::Hypothesis)
    }
}

/// Address of an entry: kind plus a key chosen by the bubble's owner.
pub type BubbleKey = (BubbleKind, String);

/// Errors from configuring a bubble.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BubbleError {
    /// The kind can never leave a bubble.
    NeverCommittable(BubbleKind),
}

impl fmt::Display for BubbleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NeverCommittable(kind) => {
                write!(f, "{kind:?} entries never leave a thought bubble")
            }
        }
    }
}

impl std::error::Error for BubbleError {}

/// Which kinds may be copied into durable state on commit. The default allows every committable
/// kind; a caller may narrow it, but can never widen it past [`BubbleKind::committable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitPolicy {
    allowed: BTreeSet<BubbleKind>,
}

impl Default for CommitPolicy {
    fn default() -> Self {
        Self {
            allowed: [BubbleKind::Memory, BubbleKind::Hypothesis].into(),
        }
    }
}

impl CommitPolicy {
    /// A policy that commits nothing.
    pub fn none() -> Self {
        Self {
            allowed: BTreeSet::new(),
        }
    }

    /// Permit committing `kind`. Fails for kinds that can never be committed.
    pub fn allow(&mut self, kind: BubbleKind) -> Result<&mut Self, BubbleError> {
        if !kind.committable() {
            return Err(BubbleError::NeverCommittable(kind));
        }
        self.allowed.insert(kind);
        Ok(self)
    }

    /// Whether `kind` may be committed under this policy.
    pub fn permits(&self, kind: BubbleKind) -> bool {
        kind.committable() && self.allowed.contains(&kind)
    }
}

/// Durable state a bubble can read and commit into.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DurableStore {
    entries: BTreeMap<BubbleKey, ThoughtContent>,
}

impl DurableStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// The entry at `key`, if any.
    pub fn get(&self, key: &BubbleKey) -> Option<&ThoughtContent> {
        self.entries.get(key)
    }

    /// Write an entry directly. Bypasses bubbles, so it is for the durable owner only.
    pub fn put(&mut self, key: BubbleKey, payload: ThoughtContent) {
        self.entries.insert(key, payload);
    }

    /// Number of durable entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// What a commit did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitReport {
    /// Entries copied into durable state.
    pub committed: Vec<BubbleKey>,
    /// Entries left behind because the policy refused their kind.
    pub skipped: Vec<BubbleKey>,
}

/// An isolated workspace for experimental cognition.
#[derive(Debug, Clone)]
pub struct ThoughtBubble {
    snapshot: DurableStore,
    overlay: BTreeMap<BubbleKey, ThoughtContent>,
}

impl ThoughtBubble {
    /// Enter a bubble over a snapshot of `durable`. Later changes to `durable` are not visible
    /// to the bubble, and the bubble's writes are not visible to `durable` until committed.
    pub fn enter(durable: &DurableStore) -> Self {
        Self {
            snapshot: durable.clone(),
            overlay: BTreeMap::new(),
        }
    }

    /// Read an entry, preferring the bubble's own writes.
    pub fn get(&self, kind: BubbleKind, key: &str) -> Option<&ThoughtContent> {
        let k = (kind, key.to_owned());
        self.overlay.get(&k).or_else(|| self.snapshot.get(&k))
    }

    /// Write an entry inside the bubble.
    pub fn put(&mut self, kind: BubbleKind, key: impl Into<String>, payload: ThoughtContent) {
        self.overlay.insert((kind, key.into()), payload);
    }

    /// Copy the bubble's permitted entries into `durable`. Entries the policy refuses are left
    /// behind and reported. The bubble is consumed either way.
    pub fn commit(self, durable: &mut DurableStore, policy: &CommitPolicy) -> CommitReport {
        let mut report = CommitReport::default();
        for (key, payload) in self.overlay {
            if policy.permits(key.0) {
                durable.put(key.clone(), payload);
                report.committed.push(key);
            } else {
                report.skipped.push(key);
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::TextContent;

    fn text(s: &str) -> ThoughtContent {
        ThoughtContent::Text(TextContent {
            text: s.into(),
            language: None,
        })
    }

    fn key(kind: BubbleKind, k: &str) -> BubbleKey {
        (kind, k.to_owned())
    }

    #[test]
    fn bubble_writes_do_not_reach_durable_state_until_commit() {
        let durable = DurableStore::new();
        let mut bubble = ThoughtBubble::enter(&durable);
        bubble.put(BubbleKind::Hypothesis, "h1", text("maybe"));
        assert_eq!(
            bubble.get(BubbleKind::Hypothesis, "h1"),
            Some(&text("maybe"))
        );
        assert!(
            durable.is_empty(),
            "durable state untouched by bubble writes"
        );
        drop(bubble);
        assert!(
            durable.is_empty(),
            "discarding leaves durable state unchanged"
        );
    }

    #[test]
    fn bubble_sees_snapshot_not_later_durable_changes() {
        let mut durable = DurableStore::new();
        durable.put(key(BubbleKind::Memory, "m1"), text("old"));
        let bubble = ThoughtBubble::enter(&durable);
        durable.put(key(BubbleKind::Memory, "m1"), text("new"));
        durable.put(key(BubbleKind::Memory, "m2"), text("added"));
        assert_eq!(bubble.get(BubbleKind::Memory, "m1"), Some(&text("old")));
        assert_eq!(bubble.get(BubbleKind::Memory, "m2"), None);
    }

    #[test]
    fn bubble_overlay_shadows_snapshot() {
        let mut durable = DurableStore::new();
        durable.put(key(BubbleKind::Memory, "m"), text("durable"));
        let mut bubble = ThoughtBubble::enter(&durable);
        bubble.put(BubbleKind::Memory, "m", text("local"));
        assert_eq!(bubble.get(BubbleKind::Memory, "m"), Some(&text("local")));
    }

    #[test]
    fn commit_copies_only_permitted_kinds() {
        let mut durable = DurableStore::new();
        let mut bubble = ThoughtBubble::enter(&durable);
        bubble.put(BubbleKind::Memory, "keep", text("m"));
        bubble.put(BubbleKind::Tool, "scratch-tool", text("t"));
        bubble.put(BubbleKind::Branch, "branch-1", text("b"));
        bubble.put(BubbleKind::Model, "tmp-model", text("x"));
        let report = bubble.commit(&mut durable, &CommitPolicy::default());
        assert_eq!(report.committed, vec![key(BubbleKind::Memory, "keep")]);
        assert_eq!(report.skipped.len(), 3);
        assert_eq!(durable.len(), 1);
        assert!(durable
            .get(&key(BubbleKind::Tool, "scratch-tool"))
            .is_none());
    }

    #[test]
    fn policy_cannot_widen_past_committable_kinds() {
        let mut policy = CommitPolicy::none();
        assert_eq!(
            policy.allow(BubbleKind::Model).map(|_| ()),
            Err(BubbleError::NeverCommittable(BubbleKind::Model))
        );
        assert!(!policy.permits(BubbleKind::Model));
        policy.allow(BubbleKind::Hypothesis).unwrap();
        assert!(policy.permits(BubbleKind::Hypothesis));
        assert!(!policy.permits(BubbleKind::Memory));
    }

    #[test]
    fn no_permission_means_nothing_is_committed() {
        let mut durable = DurableStore::new();
        let mut bubble = ThoughtBubble::enter(&durable);
        bubble.put(BubbleKind::Memory, "m", text("x"));
        let report = bubble.commit(&mut durable, &CommitPolicy::none());
        assert!(report.committed.is_empty());
        assert!(durable.is_empty());
    }
}
