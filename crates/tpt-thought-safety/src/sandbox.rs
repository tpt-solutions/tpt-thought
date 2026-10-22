// SPDX-License-Identifier: MIT OR Apache-2.0
//! Thought sandboxing, capability restriction, loop detection and resource limits (spec section
//! 41). A thought may use only the capabilities granted to its sandbox, and spawning is bounded.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use tpt_thought_core::{Intention, ThoughtId};

/// A capability was requested but never granted. Capabilities are denied by default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDenied {
    /// The capability that was refused.
    pub capability: String,
}

impl fmt::Display for CapabilityDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "capability '{}' is not granted", self.capability)
    }
}

impl std::error::Error for CapabilityDenied {}

/// Bounds on how far a thought may spread, mirroring the recursion limits of the thought graph
/// (spec section 85). A `None` bound is not enforced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Deepest allowed spawn chain.
    pub max_depth: Option<u32>,
    /// Most children a single thought may spawn.
    pub max_children: Option<u32>,
    /// Compute allowed across the whole tree.
    pub max_total_compute: Option<f64>,
    /// Wall-clock duration allowed, in nanoseconds.
    pub max_duration_nanos: Option<i64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_depth: Some(8),
            max_children: Some(16),
            max_total_compute: Some(1_000.0),
            max_duration_nanos: Some(60_000_000_000),
        }
    }
}

/// Current usage of a spawn tree, checked against [`ResourceLimits`].
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Depth of the thought being spawned.
    pub depth: u32,
    /// Children the parent already has, including the one being spawned.
    pub children: u32,
    /// Compute consumed so far across the tree.
    pub compute: f64,
    /// Nanoseconds since the tree started.
    pub elapsed_nanos: i64,
}

/// A limit was exceeded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LimitViolation {
    /// Spawn depth exceeded `max`.
    Depth {
        /// Observed depth.
        depth: u32,
        /// Allowed depth.
        max: u32,
    },
    /// Child count exceeded `max`.
    Children {
        /// Observed count.
        children: u32,
        /// Allowed count.
        max: u32,
    },
    /// Compute exceeded `max`.
    Compute {
        /// Observed compute.
        compute: f64,
        /// Allowed compute.
        max: f64,
    },
    /// Duration exceeded `max` nanoseconds.
    Duration {
        /// Observed duration.
        elapsed_nanos: i64,
        /// Allowed duration.
        max: i64,
    },
}

impl fmt::Display for LimitViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Depth { depth, max } => write!(f, "depth {depth} exceeds limit {max}"),
            Self::Children { children, max } => {
                write!(f, "{children} children exceeds limit {max}")
            }
            Self::Compute { compute, max } => write!(f, "compute {compute} exceeds limit {max}"),
            Self::Duration { elapsed_nanos, max } => {
                write!(f, "duration {elapsed_nanos}ns exceeds limit {max}ns")
            }
        }
    }
}

impl std::error::Error for LimitViolation {}

/// The sandbox a thought runs in: an explicit capability allow-list and resource limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sandbox {
    granted: BTreeSet<String>,
    limits: ResourceLimits,
}

impl Sandbox {
    /// A sandbox that grants no capabilities and enforces `limits`.
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            granted: BTreeSet::new(),
            limits,
        }
    }

    /// Grant `capability`. Matching is exact; there are no wildcards.
    pub fn grant(&mut self, capability: impl Into<String>) -> &mut Self {
        self.granted.insert(capability.into());
        self
    }

    /// Withdraw `capability`.
    pub fn revoke(&mut self, capability: &str) -> &mut Self {
        self.granted.remove(capability);
        self
    }

    /// True if `capability` is granted.
    pub fn is_granted(&self, capability: &str) -> bool {
        self.granted.contains(capability)
    }

    /// Succeed only if `capability` is granted.
    pub fn check(&self, capability: &str) -> Result<(), CapabilityDenied> {
        if self.is_granted(capability) {
            Ok(())
        } else {
            Err(CapabilityDenied {
                capability: capability.to_owned(),
            })
        }
    }

    /// Succeed only if every capability in `required` is granted. Reports the first missing one.
    pub fn authorize(&self, required: &[String]) -> Result<(), CapabilityDenied> {
        required.iter().try_for_each(|c| self.check(c))
    }

    /// Succeed only if every capability the intention requires is granted.
    pub fn authorize_intention(&self, intention: &Intention) -> Result<(), CapabilityDenied> {
        self.authorize(&intention.required_capabilities)
    }

    /// The resource limits in force.
    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// Succeed only if `usage` is within every bounded limit.
    pub fn check_usage(&self, usage: &Usage) -> Result<(), LimitViolation> {
        let l = &self.limits;
        if let Some(max) = l.max_depth.filter(|&max| usage.depth > max) {
            return Err(LimitViolation::Depth {
                depth: usage.depth,
                max,
            });
        }
        if let Some(max) = l.max_children.filter(|&max| usage.children > max) {
            return Err(LimitViolation::Children {
                children: usage.children,
                max,
            });
        }
        if let Some(max) = l.max_total_compute.filter(|&max| usage.compute > max) {
            return Err(LimitViolation::Compute {
                compute: usage.compute,
                max,
            });
        }
        if let Some(max) = l
            .max_duration_nanos
            .filter(|&max| usage.elapsed_nanos > max)
        {
            return Err(LimitViolation::Duration {
                elapsed_nanos: usage.elapsed_nanos,
                max,
            });
        }
        Ok(())
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new(ResourceLimits::default())
    }
}

/// Find a cycle in a child-to-parent map, such as `Thought::parent` links. A thought whose
/// ancestry loops back on itself would recurse forever, so the first cycle found is returned,
/// starting and ending at the same thought.
pub fn find_parent_cycle(parents: &BTreeMap<ThoughtId, ThoughtId>) -> Option<Vec<ThoughtId>> {
    for &start in parents.keys() {
        let mut path = vec![start];
        let mut current = start;
        while let Some(&parent) = parents.get(&current) {
            if let Some(pos) = path.iter().position(|&t| t == parent) {
                let mut cycle = path[pos..].to_vec();
                cycle.push(parent);
                return Some(cycle);
            }
            path.push(parent);
            current = parent;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, IntentionId, SeededEntropy};

    fn ids() -> IdGenerator {
        IdGenerator::new(SeededEntropy::new(42))
    }

    fn intention(caps: &[&str]) -> Intention {
        Intention {
            id: IntentionId::from_bytes([1; 16]),
            goal: "g".into(),
            desired_outcome: None,
            constraints: vec![],
            risk_tolerance: None,
            priority: 1.0,
            required_capabilities: caps.iter().map(|c| (*c).to_owned()).collect(),
            deadline: None,
        }
    }

    #[test]
    fn capabilities_are_denied_by_default() {
        let s = Sandbox::default();
        assert_eq!(
            s.check("move:arm"),
            Err(CapabilityDenied {
                capability: "move:arm".into()
            })
        );
    }

    #[test]
    fn granted_capability_passes_and_revoke_removes_it() {
        let mut s = Sandbox::default();
        s.grant("sense:thermal");
        assert!(s.check("sense:thermal").is_ok());
        assert!(s.check("sense:thermal2").is_err(), "matching is exact");
        s.revoke("sense:thermal");
        assert!(s.check("sense:thermal").is_err());
    }

    #[test]
    fn intention_needs_every_required_capability() {
        let mut s = Sandbox::default();
        s.grant("sense:thermal");
        let i = intention(&["sense:thermal", "move:arm"]);
        assert_eq!(
            s.authorize_intention(&i).unwrap_err().capability,
            "move:arm"
        );
        s.grant("move:arm");
        assert!(s.authorize_intention(&i).is_ok());
    }

    #[test]
    fn depth_limit_is_enforced() {
        let s = Sandbox::new(ResourceLimits {
            max_depth: Some(2),
            ..Default::default()
        });
        assert!(s
            .check_usage(&Usage {
                depth: 2,
                ..Default::default()
            })
            .is_ok());
        assert_eq!(
            s.check_usage(&Usage {
                depth: 3,
                ..Default::default()
            }),
            Err(LimitViolation::Depth { depth: 3, max: 2 })
        );
    }

    #[test]
    fn children_compute_and_duration_limits_are_enforced() {
        let s = Sandbox::new(ResourceLimits {
            max_depth: None,
            max_children: Some(1),
            max_total_compute: Some(10.0),
            max_duration_nanos: Some(100),
        });
        assert!(matches!(
            s.check_usage(&Usage {
                children: 2,
                ..Default::default()
            }),
            Err(LimitViolation::Children {
                children: 2,
                max: 1
            })
        ));
        assert!(matches!(
            s.check_usage(&Usage {
                compute: 10.5,
                ..Default::default()
            }),
            Err(LimitViolation::Compute { .. })
        ));
        assert!(matches!(
            s.check_usage(&Usage {
                elapsed_nanos: 101,
                ..Default::default()
            }),
            Err(LimitViolation::Duration { max: 100, .. })
        ));
    }

    #[test]
    fn unbounded_limits_are_not_enforced() {
        let s = Sandbox::new(ResourceLimits {
            max_depth: None,
            max_children: None,
            max_total_compute: None,
            max_duration_nanos: None,
        });
        assert!(s
            .check_usage(&Usage {
                depth: u32::MAX,
                children: u32::MAX,
                compute: f64::MAX,
                elapsed_nanos: i64::MAX,
            })
            .is_ok());
    }

    #[test]
    fn detects_parent_cycle() {
        let g = ids();
        let (a, b, c, d) = (g.thought(), g.thought(), g.thought(), g.thought());
        let mut parents = BTreeMap::new();
        parents.insert(a, b);
        parents.insert(b, c);
        parents.insert(d, a);
        assert_eq!(find_parent_cycle(&parents), None, "a chain is not a cycle");
        parents.insert(c, a);
        let cycle = find_parent_cycle(&parents).expect("a -> b -> c -> a is a cycle");
        assert_eq!(cycle.first(), cycle.last());
        assert_eq!(cycle.len(), 4);
    }

    #[test]
    fn self_parent_is_a_cycle() {
        let g = ids();
        let a = g.thought();
        let parents = BTreeMap::from([(a, a)]);
        assert_eq!(find_parent_cycle(&parents), Some(vec![a, a]));
    }
}
