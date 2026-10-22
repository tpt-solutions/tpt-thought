// SPDX-License-Identifier: MIT OR Apache-2.0
//! Memory retrieval by identifier (spec section 10). Durable memory lives in Anima; Thought
//! asks for memory references and never copies memory content.

use std::collections::BTreeSet;
use tpt_thought_core::MemoryRef;

/// Retrieval of memory references for a cue. Implementations return identifiers only.
pub trait MemoryStore {
    /// Memories matching `cue` (tags), best match first, at most `limit` of them. Ties are broken
    /// by identifier so results are deterministic.
    fn retrieve(&self, cue: &[String], limit: usize) -> Vec<MemoryRef>;
}

/// An index that maps memory identifiers to retrieval tags. It holds no memory content: the
/// content stays in Anima, and the index is only a way to find identifiers.
#[derive(Debug, Clone, Default)]
pub struct MemoryIndex {
    entries: Vec<(MemoryRef, BTreeSet<String>)>,
}

impl MemoryIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `memory` under `tags`, replacing any earlier tags for the same identifier.
    pub fn insert(&mut self, memory: MemoryRef, tags: impl IntoIterator<Item = String>) {
        let tags: BTreeSet<String> = tags.into_iter().collect();
        match self.entries.iter_mut().find(|(m, _)| *m == memory) {
            Some(entry) => entry.1 = tags,
            None => self.entries.push((memory, tags)),
        }
    }

    /// Number of indexed memories.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if nothing is indexed.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl MemoryStore for MemoryIndex {
    fn retrieve(&self, cue: &[String], limit: usize) -> Vec<MemoryRef> {
        let mut scored: Vec<(usize, &MemoryRef)> = self
            .entries
            .iter()
            .filter_map(|(memory, tags)| {
                let hits = cue.iter().filter(|c| tags.contains(c.as_str())).count();
                (hits > 0).then_some((hits, memory))
            })
            .collect();
        scored.sort_by(|(a_hits, a), (b_hits, b)| {
            b_hits
                .cmp(a_hits)
                .then_with(|| a.store.cmp(&b.store))
                .then_with(|| a.id.cmp(&b.id))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, memory)| memory.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem(id: &str) -> MemoryRef {
        MemoryRef {
            store: "anima-sse".into(),
            id: id.into(),
        }
    }

    fn cue(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn ranks_by_matching_tags_then_identifier() {
        let mut idx = MemoryIndex::new();
        idx.insert(mem("b"), cue(&["kettle", "hot"]));
        idx.insert(mem("a"), cue(&["kettle"]));
        idx.insert(mem("c"), cue(&["kettle"]));
        idx.insert(mem("d"), cue(&["unrelated"]));
        let found = idx.retrieve(&cue(&["kettle", "hot"]), 10);
        let ids: Vec<&str> = found.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a", "c"]);
    }

    #[test]
    fn respects_limit_and_no_match_is_empty() {
        let mut idx = MemoryIndex::new();
        idx.insert(mem("a"), cue(&["x"]));
        idx.insert(mem("b"), cue(&["x"]));
        assert_eq!(idx.retrieve(&cue(&["x"]), 1).len(), 1);
        assert!(idx.retrieve(&cue(&["y"]), 5).is_empty());
    }

    #[test]
    fn reinsert_replaces_tags_without_duplicating() {
        let mut idx = MemoryIndex::new();
        idx.insert(mem("a"), cue(&["old"]));
        idx.insert(mem("a"), cue(&["new"]));
        assert_eq!(idx.len(), 1);
        assert!(idx.retrieve(&cue(&["old"]), 5).is_empty());
        assert_eq!(idx.retrieve(&cue(&["new"]), 5), vec![mem("a")]);
    }
}
