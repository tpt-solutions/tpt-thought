# tpt-thought-graph

Typed relationships between thoughts, hypotheses, beliefs, observations, predictions and actions,
with the graph invariants enforced on insertion and bounded recursive spawning of thoughts.

> **Status:** pre-release (v0.1.1). This crate is distributed from git only and is not published
> to crates.io. See `docs/DEPENDENCY_POLICY.md` in the repository.

## What it does

- Stores a directed graph of `Edge`s between `NodeRef`s, where a node is a thought, hypothesis,
  observation, prediction, belief, action or memory.
- Distinguishes causation from association. `EdgeKind::Causes` must carry `CausalProvenance`
  (origin, supporting provenance records, optional strength). Every other kind must not.
- Refuses edges that break the graph invariants, reporting a `GraphError` that names the reason.
- Answers structural queries: ancestors, descendants, contradictions of a node, evidence paths to
  a conclusion, and the causal provenance of a specific edge.
- Tracks a tree of spawned thoughts under `SpawnLimits` (depth, children per node, total compute
  and wall-clock duration), so recursive cognition is bounded at creation time.

## What it deliberately does not do

- It does not store memory content. `NodeRef::Memory` holds an ID only, because durable memory
  lives in Anima.
- It does not decide whether a causal claim is true. Causal claims come from Eve, Augur,
  experiments or external evidence, never from timing alone.
- It does not run cognition or execute actions. Circuit breakers that stop runaway cognition
  belong to `tpt-thought-safety` and Anima; the spawn limits here stop it being created.

## Main types

| Item | Role |
|---|---|
| `NodeRef` | A node: `Thought`, `Hypothesis`, `Observation`, `Prediction`, `Belief`, `Action` or `Memory`. |
| `EdgeKind` | `Supports`, `Contradicts`, `Causes`, `DerivedFrom`, `Generalises`, `Specialises`, `Triggers`, `AnalogousTo`, `Associates`, `PrecededBy`. Symmetric kinds are marked by `is_symmetric()`. |
| `Edge` | A directed edge with an optional `CausalProvenance`. Built with `Edge::new` or `Edge::causal`. |
| `CausalProvenance` | Origin, supporting `ProvenanceId`s and optional strength. |
| `ThoughtGraph` | The graph: `add_edge`, `edges`, `ancestors`, `descendants`, `contradictions`, `evidence_paths`, `causal_provenance`. |
| `GraphError` | Why an edge was refused. |
| `ThoughtTree` | Spawned thoughts with depth, child and compute accounting: `new`, `spawn`, `depth`, `compute_used`. |
| `SpawnLimits` | `max_depth`, `max_children`, `max_total_compute` and `max_duration_nanos`. `None` means unlimited. |
| `SpawnError` | Why a spawn was refused (unknown parent, duplicate child, or a limit exceeded). |

## Module layout

- `lib.rs`: node and edge types, the `ThoughtGraph` and its queries.
- `tree.rs`: the bounded spawn tree (`ThoughtTree`, `SpawnLimits`, `SpawnError`).

## Usage

```rust
use tpt_thought_core::{IdGenerator, SeededEntropy};
use tpt_thought_graph::{Edge, EdgeKind, ThoughtGraph};

let ids = IdGenerator::new(SeededEntropy::new(1));
let hypothesis = ids.hypothesis();
let observation = ids.observation();

let mut graph = ThoughtGraph::new();
graph.add_edge(Edge::new(observation, EdgeKind::Supports, hypothesis))?;

let support = graph.ancestors(&hypothesis.into(), &[EdgeKind::Supports]);
```

## Design notes

- Edges are checked on insertion. Acyclicity is enforced for `Causes`, `DerivedFrom` and
  `PrecededBy`, so those relations can be walked without cycle checks.
- Symmetric kinds (`Contradicts`, `AnalogousTo`, `Associates`) are treated the same in both
  directions by queries.
- `DerivedFrom` flows backwards: in `A DerivedFrom B`, `B` is the origin.
- `AnalogousTo` says two things are analogous. Whether the analogy is valid is for Eve to decide.

## Related crates

- `tpt-thought-core`: the ID and node types the graph refers to.
- `tpt-thought-hypothesis` and `tpt-thought-prediction`: produce the hypotheses and predictions
  that appear as nodes.
- `tpt-thought-provenance`: the hash-linked records that `CausalProvenance` points at.
- `tpt-thought-safety`: circuit breakers for cognition that is already running.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0, at your option.
