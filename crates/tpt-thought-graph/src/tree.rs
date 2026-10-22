// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bounded recursive thought (spec section 85). Anima circuit breakers terminate runaway
//! cognition; these limits stop it from being created in the first place.

use std::collections::HashMap;
use std::fmt;
use tpt_thought_core::{ThoughtId, Timestamp};

/// Limits on a tree of spawned thoughts. `None` means unlimited.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SpawnLimits {
    /// Maximum depth below the root (the root is depth 0).
    pub max_depth: Option<u32>,
    /// Maximum direct children of any one thought.
    pub max_children: Option<u32>,
    /// Maximum compute summed over the whole tree, in provider-defined units.
    pub max_total_compute: Option<f64>,
    /// Maximum nanoseconds since the tree was created.
    pub max_duration_nanos: Option<i64>,
}

/// Why a spawn was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// The parent is not in the tree.
    UnknownParent,
    /// The child ID is already in the tree.
    DuplicateChild,
    /// `max_depth` would be exceeded.
    DepthExceeded,
    /// `max_children` would be exceeded.
    TooManyChildren,
    /// `max_total_compute` would be exceeded.
    ComputeExceeded,
    /// `max_duration` has elapsed.
    DurationExceeded,
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownParent => "unknown parent thought",
            Self::DuplicateChild => "thought already in tree",
            Self::DepthExceeded => "max depth exceeded",
            Self::TooManyChildren => "max children exceeded",
            Self::ComputeExceeded => "max total compute exceeded",
            Self::DurationExceeded => "max duration exceeded",
        })
    }
}

impl std::error::Error for SpawnError {}

#[derive(Debug, Clone, Copy)]
struct Node {
    depth: u32,
    children: u32,
}

/// Tracks a tree of thoughts and refuses spawns that break the limits.
#[derive(Debug, Clone)]
pub struct ThoughtTree {
    limits: SpawnLimits,
    root: ThoughtId,
    started_at: Timestamp,
    compute_used: f64,
    nodes: HashMap<ThoughtId, Node>,
}

impl ThoughtTree {
    /// A tree containing only `root`, started at `now`.
    pub fn new(root: ThoughtId, limits: SpawnLimits, now: Timestamp) -> Self {
        let nodes = HashMap::from([(
            root,
            Node {
                depth: 0,
                children: 0,
            },
        )]);
        Self {
            limits,
            root,
            started_at: now,
            compute_used: 0.0,
            nodes,
        }
    }

    /// The root thought.
    pub fn root(&self) -> ThoughtId {
        self.root
    }

    /// Number of thoughts in the tree, including the root.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Always false: a tree has a root.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Depth of `id` below the root.
    pub fn depth(&self, id: &ThoughtId) -> Option<u32> {
        self.nodes.get(id).map(|n| n.depth)
    }

    /// Compute charged so far.
    pub fn compute_used(&self) -> f64 {
        self.compute_used
    }

    /// Spawn `child` under `parent`, charging `compute`. On error nothing changes.
    pub fn spawn(
        &mut self,
        parent: &ThoughtId,
        child: ThoughtId,
        compute: f64,
        now: Timestamp,
    ) -> Result<(), SpawnError> {
        let p = *self.nodes.get(parent).ok_or(SpawnError::UnknownParent)?;
        if self.nodes.contains_key(&child) {
            return Err(SpawnError::DuplicateChild);
        }
        let l = &self.limits;
        if l.max_depth.is_some_and(|m| p.depth + 1 > m) {
            return Err(SpawnError::DepthExceeded);
        }
        if l.max_children.is_some_and(|m| p.children + 1 > m) {
            return Err(SpawnError::TooManyChildren);
        }
        if l.max_total_compute
            .is_some_and(|m| self.compute_used + compute > m)
        {
            return Err(SpawnError::ComputeExceeded);
        }
        if l.max_duration_nanos
            .is_some_and(|m| now.as_nanos() - self.started_at.as_nanos() > m)
        {
            return Err(SpawnError::DurationExceeded);
        }
        self.nodes.get_mut(parent).expect("checked above").children += 1;
        self.nodes.insert(
            child,
            Node {
                depth: p.depth + 1,
                children: 0,
            },
        );
        self.compute_used += compute;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    fn gen() -> IdGenerator {
        IdGenerator::new(SeededEntropy::new(11))
    }

    #[test]
    fn enforces_depth() {
        let g = gen();
        let root = g.thought();
        let mut t = ThoughtTree::new(
            root,
            SpawnLimits {
                max_depth: Some(2),
                ..Default::default()
            },
            Timestamp(0),
        );
        let (a, b, c) = (g.thought(), g.thought(), g.thought());
        t.spawn(&root, a, 0.0, Timestamp(0)).unwrap();
        t.spawn(&a, b, 0.0, Timestamp(0)).unwrap();
        assert_eq!(
            t.spawn(&b, c, 0.0, Timestamp(0)),
            Err(SpawnError::DepthExceeded)
        );
        assert_eq!(t.depth(&b), Some(2));
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn enforces_children_compute_and_duration() {
        let g = gen();
        let root = g.thought();
        let limits = SpawnLimits {
            max_children: Some(2),
            max_total_compute: Some(5.0),
            max_duration_nanos: Some(100),
            ..Default::default()
        };
        let mut t = ThoughtTree::new(root, limits, Timestamp(0));
        t.spawn(&root, g.thought(), 2.0, Timestamp(10)).unwrap();
        t.spawn(&root, g.thought(), 2.0, Timestamp(20)).unwrap();
        assert_eq!(
            t.spawn(&root, g.thought(), 0.0, Timestamp(30)),
            Err(SpawnError::TooManyChildren)
        );
        let kid = g.thought();
        let first_child = t.nodes.keys().find(|k| **k != root).copied().unwrap();
        assert_eq!(
            t.spawn(&first_child, kid, 2.0, Timestamp(30)),
            Err(SpawnError::ComputeExceeded)
        );
        assert_eq!(
            t.spawn(&first_child, kid, 1.0, Timestamp(101)),
            Err(SpawnError::DurationExceeded)
        );
        t.spawn(&first_child, kid, 1.0, Timestamp(100)).unwrap();
        assert_eq!(t.compute_used(), 5.0);
    }

    #[test]
    fn rejects_unknown_parent_and_duplicates() {
        let g = gen();
        let root = g.thought();
        let mut t = ThoughtTree::new(root, SpawnLimits::default(), Timestamp(0));
        assert_eq!(
            t.spawn(&g.thought(), g.thought(), 0.0, Timestamp(0)),
            Err(SpawnError::UnknownParent)
        );
        let c = g.thought();
        t.spawn(&root, c, 0.0, Timestamp(0)).unwrap();
        assert_eq!(
            t.spawn(&root, c, 0.0, Timestamp(0)),
            Err(SpawnError::DuplicateChild)
        );
    }

    proptest! {
        /// Branch explosion is bounded: a chain-or-fan of spawns never exceeds the limits.
        #[test]
        fn tree_size_is_bounded(depth in 0u32..5, kids in 0u32..4, picks in proptest::collection::vec(0usize..64, 0..200)) {
            let g = gen();
            let root = g.thought();
            let limits = SpawnLimits { max_depth: Some(depth), max_children: Some(kids), ..Default::default() };
            let mut t = ThoughtTree::new(root, limits, Timestamp(0));
            let mut all = vec![root];
            for p in picks {
                let parent = all[p % all.len()];
                let child = g.thought();
                if t.spawn(&parent, child, 0.0, Timestamp(0)).is_ok() {
                    all.push(child);
                }
            }
            // Upper bound on a (depth, kids)-bounded tree: sum_{d=0..=depth} kids^d.
            let bound: usize = (0..=depth).map(|d| (kids as usize).pow(d)).sum();
            prop_assert!(t.len() <= bound);
            prop_assert!(all.iter().all(|i| t.depth(i).unwrap() <= depth));
        }
    }
}
