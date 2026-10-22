// SPDX-License-Identifier: MIT OR Apache-2.0
//! The thought graph (spec sections 10, 75-77, 85): typed relationships between thoughts,
//! hypotheses, beliefs, observations, predictions and actions, plus bounded recursive spawning.
//!
//! The graph refers to Anima memory by ID only ([`NodeRef::Memory`]); it never duplicates
//! storage.

mod tree;

pub use tree::{SpawnError, SpawnLimits, ThoughtTree};

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use tpt_thought_core::{
    ContentReference, HypothesisId, ObservationId, PredictionId, ProvenanceId, ThoughtId,
};

/// A node in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeRef {
    /// A thought.
    Thought(ThoughtId),
    /// A hypothesis.
    Hypothesis(HypothesisId),
    /// An observation.
    Observation(ObservationId),
    /// A prediction.
    Prediction(PredictionId),
    /// A belief, named by its owner's identifier.
    Belief(ContentReference),
    /// An action issued through Anima/INP.
    Action(ContentReference),
    /// A memory held in Anima, referenced by ID only.
    Memory(ContentReference),
}

macro_rules! node_from {
    ($($t:ty => $v:ident),* $(,)?) => {$(
        impl From<$t> for NodeRef { fn from(x: $t) -> Self { NodeRef::$v(x) } }
    )*};
}
node_from!(ThoughtId => Thought, HypothesisId => Hypothesis, ObservationId => Observation, PredictionId => Prediction);

/// How two nodes relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EdgeKind {
    /// `from` supports `to`.
    Supports,
    /// `from` contradicts `to` (treated symmetrically in queries).
    Contradicts,
    /// `from` caused `to`. Directional; requires causal provenance (spec section 75).
    Causes,
    /// `from` was derived from `to`.
    DerivedFrom,
    /// `from` generalises `to`.
    Generalises,
    /// `from` specialises `to`.
    Specialises,
    /// `from` triggered `to`.
    Triggers,
    /// `from` is analogous to `to` (validity is for Eve to decide). Symmetric.
    AnalogousTo,
    /// `from` and `to` are associated. Symmetric, and never causal (spec section 76).
    Associates,
    /// `from` happened before `to`. Says nothing about causation.
    PrecededBy,
}

impl EdgeKind {
    /// True if the relationship holds in both directions.
    pub const fn is_symmetric(self) -> bool {
        matches!(
            self,
            Self::Contradicts | Self::AnalogousTo | Self::Associates
        )
    }

    /// True if the `from` node is downstream of the `to` node (`A DerivedFrom B`: B is the origin).
    const fn flows_backwards(self) -> bool {
        matches!(self, Self::DerivedFrom)
    }

    /// True if edges of this kind must not form cycles.
    const fn is_acyclic(self) -> bool {
        matches!(self, Self::Causes | Self::DerivedFrom | Self::PrecededBy)
    }
}

/// Why a causal claim is believed (spec section 75). Causal claims originate from Eve, Augur,
/// experimentation or external evidence, never from timing alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausalProvenance {
    /// Origin of the claim (e.g. `eve`, `augur`, `experiment`, `external`).
    pub origin: String,
    /// Supporting provenance records.
    pub records: Vec<ProvenanceId>,
    /// Estimated strength, if known.
    pub strength: Option<f64>,
}

/// A typed, directed relationship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Source node.
    pub from: NodeRef,
    /// Target node.
    pub to: NodeRef,
    /// Relationship.
    pub kind: EdgeKind,
    /// Causal provenance. Required for [`EdgeKind::Causes`], forbidden otherwise.
    pub causal: Option<CausalProvenance>,
}

impl Edge {
    /// An edge without causal provenance.
    pub fn new(from: impl Into<NodeRef>, kind: EdgeKind, to: impl Into<NodeRef>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            kind,
            causal: None,
        }
    }

    /// A causal edge `from -> to` with its provenance.
    pub fn causal(from: impl Into<NodeRef>, to: impl Into<NodeRef>, why: CausalProvenance) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            kind: EdgeKind::Causes,
            causal: Some(why),
        }
    }
}

/// Why an edge was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    /// An edge from a node to itself.
    SelfLoop,
    /// A `Causes` edge had no causal provenance.
    CausalProvenanceRequired,
    /// Causal provenance was supplied on a non-causal edge.
    CausalProvenanceNotAllowed,
    /// The edge would create a cycle in an acyclic relationship.
    WouldCreateCycle,
    /// An identical edge already exists.
    Duplicate,
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SelfLoop => "self-loops are not allowed",
            Self::CausalProvenanceRequired => "causal edges require causal provenance",
            Self::CausalProvenanceNotAllowed => "only causal edges carry causal provenance",
            Self::WouldCreateCycle => "edge would create a cycle",
            Self::Duplicate => "edge already exists",
        })
    }
}

impl std::error::Error for GraphError {}

/// A directed, typed multigraph with invariants enforced on insertion.
#[derive(Debug, Default, Clone)]
pub struct ThoughtGraph {
    edges: Vec<Edge>,
    out: HashMap<NodeRef, Vec<usize>>,
    inc: HashMap<NodeRef, Vec<usize>>,
}

impl ThoughtGraph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// All edges, in insertion order.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Add `edge`, enforcing the graph invariants.
    pub fn add_edge(&mut self, edge: Edge) -> Result<(), GraphError> {
        if edge.from == edge.to {
            return Err(GraphError::SelfLoop);
        }
        match (edge.kind, &edge.causal) {
            (EdgeKind::Causes, None) => return Err(GraphError::CausalProvenanceRequired),
            (EdgeKind::Causes, Some(_)) => {}
            (_, Some(_)) => return Err(GraphError::CausalProvenanceNotAllowed),
            _ => {}
        }
        if self.has_edge(&edge.from, edge.kind, &edge.to)
            || (edge.kind.is_symmetric() && self.has_edge(&edge.to, edge.kind, &edge.from))
        {
            return Err(GraphError::Duplicate);
        }
        if edge.kind.is_acyclic() && self.reaches(&edge.to, &edge.from, edge.kind) {
            return Err(GraphError::WouldCreateCycle);
        }
        let idx = self.edges.len();
        self.out.entry(edge.from.clone()).or_default().push(idx);
        self.inc.entry(edge.to.clone()).or_default().push(idx);
        self.edges.push(edge);
        Ok(())
    }

    fn has_edge(&self, from: &NodeRef, kind: EdgeKind, to: &NodeRef) -> bool {
        self.out.get(from).is_some_and(|v| {
            v.iter()
                .any(|&i| self.edges[i].kind == kind && &self.edges[i].to == to)
        })
    }

    /// True if `target` is reachable from `start` following only `kind` edges forwards.
    fn reaches(&self, start: &NodeRef, target: &NodeRef, kind: EdgeKind) -> bool {
        self.walk(start, |n| self.successors(n, Some(kind)))
            .iter()
            .any(|n| n == target)
            || start == target
    }

    /// Nodes one hop away along `from -> to` edges (both ways for symmetric kinds), optionally
    /// restricted to `kind`.
    fn successors(&self, node: &NodeRef, kind: Option<EdgeKind>) -> Vec<NodeRef> {
        let mut v = Vec::new();
        for &i in self.out.get(node).into_iter().flatten() {
            let e = &self.edges[i];
            if kind.is_none_or(|k| k == e.kind) {
                v.push(e.to.clone());
            }
        }
        for &i in self.inc.get(node).into_iter().flatten() {
            let e = &self.edges[i];
            if e.kind.is_symmetric() && kind.is_none_or(|k| k == e.kind) {
                v.push(e.from.clone());
            }
        }
        v
    }

    /// One hop along `kinds` edges. `upstream` steps towards origins (causes, supporters,
    /// sources derived from); otherwise towards consequences. Symmetric kinds go both ways.
    fn step(&self, node: &NodeRef, kinds: &[EdgeKind], upstream: bool) -> Vec<NodeRef> {
        let mut v = Vec::new();
        for &i in self.out.get(node).into_iter().flatten() {
            let e = &self.edges[i];
            if kinds.contains(&e.kind)
                && (e.kind.is_symmetric() || e.kind.flows_backwards() == upstream)
            {
                v.push(e.to.clone());
            }
        }
        for &i in self.inc.get(node).into_iter().flatten() {
            let e = &self.edges[i];
            if kinds.contains(&e.kind)
                && (e.kind.is_symmetric() || e.kind.flows_backwards() != upstream)
            {
                v.push(e.from.clone());
            }
        }
        v
    }

    /// Breadth-first traversal from `start` (excluded), each node once, nearest first.
    fn walk(&self, start: &NodeRef, next: impl Fn(&NodeRef) -> Vec<NodeRef>) -> Vec<NodeRef> {
        let mut seen: HashSet<NodeRef> = HashSet::from([start.clone()]);
        let mut queue: VecDeque<NodeRef> = VecDeque::from([start.clone()]);
        let mut out = Vec::new();
        while let Some(n) = queue.pop_front() {
            for m in next(&n) {
                if seen.insert(m.clone()) {
                    out.push(m.clone());
                    queue.push_back(m);
                }
            }
        }
        out
    }

    /// Everything `node` derives from or is caused or supported by, following `kinds` edges
    /// towards their origins. `A DerivedFrom B` makes `B` an ancestor of `A`.
    pub fn ancestors(&self, node: &NodeRef, kinds: &[EdgeKind]) -> Vec<NodeRef> {
        self.walk(node, |n| self.step(n, kinds, true))
    }

    /// Everything downstream of `node`, following `kinds` edges towards their consequences.
    pub fn descendants(&self, node: &NodeRef, kinds: &[EdgeKind]) -> Vec<NodeRef> {
        self.walk(node, |n| self.step(n, kinds, false))
    }

    /// Nodes that contradict `node` (contradiction is symmetric).
    pub fn contradictions(&self, node: &NodeRef) -> Vec<NodeRef> {
        self.successors(node, Some(EdgeKind::Contradicts))
    }

    /// Every chain of `Supports` edges ending at `conclusion`, each starting at a node with no
    /// further support (a piece of ground evidence). Paths are listed evidence-first.
    pub fn evidence_paths(&self, conclusion: &NodeRef) -> Vec<Vec<NodeRef>> {
        let mut paths = Vec::new();
        let mut stack = vec![vec![conclusion.clone()]];
        while let Some(path) = stack.pop() {
            let head = path.last().expect("paths are non-empty").clone();
            let supporters = self.step(&head, &[EdgeKind::Supports], true);
            let fresh: Vec<_> = supporters
                .into_iter()
                .filter(|s| !path.contains(s))
                .collect();
            if fresh.is_empty() {
                if path.len() > 1 {
                    let mut p = path;
                    p.reverse();
                    paths.push(p);
                }
                continue;
            }
            for s in fresh {
                let mut p = path.clone();
                p.push(s);
                stack.push(p);
            }
        }
        paths
    }

    /// The causal provenance for the edge `from -> to`, if a causal edge exists.
    pub fn causal_provenance(&self, from: &NodeRef, to: &NodeRef) -> Option<&CausalProvenance> {
        self.out
            .get(from)?
            .iter()
            .map(|&i| &self.edges[i])
            .find(|e| e.kind == EdgeKind::Causes && &e.to == to)
            .and_then(|e| e.causal.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::{IdGenerator, SeededEntropy};

    fn ids() -> (IdGenerator, Vec<ThoughtId>) {
        let g = IdGenerator::new(SeededEntropy::new(5));
        let v = (0..5).map(|_| g.thought()).collect();
        (g, v)
    }

    fn why() -> CausalProvenance {
        CausalProvenance {
            origin: "eve".into(),
            records: vec![],
            strength: Some(0.9),
        }
    }

    fn n(t: ThoughtId) -> NodeRef {
        t.into()
    }

    #[test]
    fn causal_edges_need_provenance_and_others_forbid_it() {
        let (_, t) = ids();
        let mut g = ThoughtGraph::new();
        let bare = Edge {
            from: n(t[0]),
            to: n(t[1]),
            kind: EdgeKind::Causes,
            causal: None,
        };
        assert_eq!(g.add_edge(bare), Err(GraphError::CausalProvenanceRequired));
        let sneaky = Edge {
            from: n(t[0]),
            to: n(t[1]),
            kind: EdgeKind::Associates,
            causal: Some(why()),
        };
        assert_eq!(
            g.add_edge(sneaky),
            Err(GraphError::CausalProvenanceNotAllowed)
        );
        g.add_edge(Edge::causal(t[0], t[1], why())).unwrap();
        assert_eq!(
            g.causal_provenance(&n(t[0]), &n(t[1])).unwrap().origin,
            "eve"
        );
        // Association and causation are distinct: the association query finds no causal claim.
        g.add_edge(Edge::new(t[2], EdgeKind::Associates, t[3]))
            .unwrap();
        assert!(g.causal_provenance(&n(t[2]), &n(t[3])).is_none());
    }

    #[test]
    fn rejects_self_loops_duplicates_and_cycles() {
        let (_, t) = ids();
        let mut g = ThoughtGraph::new();
        assert_eq!(
            g.add_edge(Edge::new(t[0], EdgeKind::Supports, t[0])),
            Err(GraphError::SelfLoop)
        );
        g.add_edge(Edge::causal(t[0], t[1], why())).unwrap();
        g.add_edge(Edge::causal(t[1], t[2], why())).unwrap();
        assert_eq!(
            g.add_edge(Edge::causal(t[2], t[0], why())),
            Err(GraphError::WouldCreateCycle)
        );
        assert_eq!(
            g.add_edge(Edge::causal(t[0], t[1], why())),
            Err(GraphError::Duplicate)
        );
        g.add_edge(Edge::new(t[0], EdgeKind::Associates, t[1]))
            .unwrap();
        assert_eq!(
            g.add_edge(Edge::new(t[1], EdgeKind::Associates, t[0])),
            Err(GraphError::Duplicate)
        );
    }

    #[test]
    fn supports_may_cycle_but_traversal_terminates() {
        let (_, t) = ids();
        let mut g = ThoughtGraph::new();
        g.add_edge(Edge::new(t[0], EdgeKind::Supports, t[1]))
            .unwrap();
        g.add_edge(Edge::new(t[1], EdgeKind::Supports, t[0]))
            .unwrap();
        assert_eq!(g.ancestors(&n(t[0]), &[EdgeKind::Supports]), vec![n(t[1])]);
    }

    #[test]
    fn ancestors_and_descendants_follow_kinds() {
        let (_, t) = ids();
        let mut g = ThoughtGraph::new();
        g.add_edge(Edge::new(t[1], EdgeKind::DerivedFrom, t[0]))
            .unwrap();
        g.add_edge(Edge::new(t[2], EdgeKind::DerivedFrom, t[1]))
            .unwrap();
        g.add_edge(Edge::new(t[3], EdgeKind::Triggers, t[2]))
            .unwrap();
        assert_eq!(
            g.ancestors(&n(t[2]), &[EdgeKind::DerivedFrom]),
            vec![n(t[1]), n(t[0])]
        );
        assert_eq!(
            g.descendants(&n(t[2]), &[EdgeKind::Triggers]),
            Vec::<NodeRef>::new()
        );
        assert_eq!(
            g.descendants(&n(t[3]), &[EdgeKind::Triggers]),
            vec![n(t[2])]
        );
    }

    #[test]
    fn contradictions_are_symmetric() {
        let (_, t) = ids();
        let mut g = ThoughtGraph::new();
        g.add_edge(Edge::new(t[0], EdgeKind::Contradicts, t[1]))
            .unwrap();
        assert_eq!(g.contradictions(&n(t[1])), vec![n(t[0])]);
        assert_eq!(g.contradictions(&n(t[0])), vec![n(t[1])]);
        assert!(g.contradictions(&n(t[2])).is_empty());
    }

    #[test]
    fn evidence_paths_run_from_ground_evidence_to_conclusion() {
        let (_, t) = ids();
        let mut g = ThoughtGraph::new();
        // t0 -> t1 -> t3 and t2 -> t3 support the conclusion t3.
        g.add_edge(Edge::new(t[0], EdgeKind::Supports, t[1]))
            .unwrap();
        g.add_edge(Edge::new(t[1], EdgeKind::Supports, t[3]))
            .unwrap();
        g.add_edge(Edge::new(t[2], EdgeKind::Supports, t[3]))
            .unwrap();
        let mut paths = g.evidence_paths(&n(t[3]));
        paths.sort_by_key(|p| p.len());
        assert_eq!(
            paths,
            vec![vec![n(t[2]), n(t[3])], vec![n(t[0]), n(t[1]), n(t[3])]]
        );
        assert!(g.evidence_paths(&n(t[0])).is_empty());
    }

    #[test]
    fn memory_nodes_are_references_only() {
        let (_, t) = ids();
        let mem = NodeRef::Memory(ContentReference {
            store: "anima-sse".into(),
            id: "M91".into(),
        });
        let mut g = ThoughtGraph::new();
        g.add_edge(Edge {
            from: mem.clone(),
            to: n(t[0]),
            kind: EdgeKind::Supports,
            causal: None,
        })
        .unwrap();
        assert_eq!(g.ancestors(&n(t[0]), &[EdgeKind::Supports]), vec![mem]);
    }
}
