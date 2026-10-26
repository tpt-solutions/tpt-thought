# tpt-thought-attention

Attention scoring, cognitive demand and working memory. This crate calculates how much a thought
matters and how much cognition it asks for. It never schedules compute itself.

> **Status:** pre-release (v0.1.0). Distributed from git only, not on crates.io.

## Installation

```toml
[dependencies]
tpt-thought-attention = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## What this crate does

Three things, in three modules:

1. **Scoring** (`formula`). `AttentionFactors` holds ten inputs, each nominally in `0.0..=1.0`:
   salience, novelty, urgency, goal relevance, risk, uncertainty, information gain, value
   relevance, recency and resource cost. An `AttentionFormula` combines them into one score.
   The default `WeightedSum` weights every factor 1.0 and subtracts the resource cost.
2. **Demand** (`demand`). `CognitiveDemand` is what a thought asks of the scheduler: a priority,
   the earliest deadline among its intentions, an estimated cost, the capabilities its
   intentions require, risk and expected information gain.
3. **Working memory** (`working_memory`). A strictly bounded working set of thoughts. Each item
   has a weight that decays with a half-life, and items that fall below an expiry threshold are
   removed.

What it deliberately does not do:

- It does not schedule CPU or any other resource. Anima CK turns `CognitiveDemand` into
  scheduling decisions.
- It does not hold persistent memory. Durable memory is Anima's, and this crate keeps only the
  working set.
- It does not decide what a thought should do next. That is `tpt-thought-policy`.

## Main API

| Item | Purpose |
|---|---|
| `AttentionFactors` | The ten scoring inputs |
| `AttentionFormula` | Trait: `score(&factors) -> f64`. Implement it to replace the default formula. |
| `WeightedSum` | The default formula, with configurable per-factor weights |
| `apply_attention(&mut thought, &factors, &formula)` | Scores a thought, and records the score, `salience` and `novelty` on it |
| `CognitiveDemand` | The demand record |
| `demand(&thought, &factors, &formula)` | Builds the demand for a thought from its intentions and its score |
| `WorkingMemory` | The bounded working set: `new`, `insert`, `reinforce`, `remove`, `decay`, `items`, `len`, `is_empty` |
| `WorkingMemoryConfig` | `capacity` (default 7), `half_life_nanos` (default 30 seconds), `expiry_threshold` (default 0.01) |
| `WorkingMemoryItem` | One entry: the thought, its weight and its `ForgettingState` |
| `ForgettingState` | `Temporary`, `Weak`, `Strong`, `Consolidated`, `Expired`, `Pruned` |
| `InsertOutcome` | Whether an insert fitted, or evicted an item to make room |

## Usage

Score a thought and read its demand:

```rust
use tpt_thought_attention::{apply_attention, demand, AttentionFactors, WeightedSum};
use tpt_thought_core::{IdGenerator, SeededEntropy, TextContent, Thought, ThoughtContent,
    ThoughtOrigin, Timestamp};

let ids = IdGenerator::new(SeededEntropy::new(3));
let mut thought = Thought::new(
    ids.thought(),
    ThoughtOrigin::Internal,
    ThoughtContent::Text(TextContent { text: "a loud noise".into(), language: None }),
    Timestamp(0),
);

let factors = AttentionFactors { salience: 0.9, urgency: 0.6, ..Default::default() };
let formula = WeightedSum::default();

let score = apply_attention(&mut thought, &factors, &formula);
let d = demand(&thought, &factors, &formula);
assert_eq!(d.priority, score);
```

Keep a bounded working set. Capacity is strict:

```rust
use tpt_thought_attention::{WorkingMemory, WorkingMemoryConfig};

let memory = WorkingMemory::new(WorkingMemoryConfig { capacity: 3, ..Default::default() });
assert!(memory.is_empty());
```

When the set is full, an insert either evicts the item with the lowest retention (if the new item
retains more) or is rejected. `InsertOutcome` reports which happened.

Call `decay(now)` periodically. It returns the items that expired, so the caller can decide what
happens to them, for example by handing them to consolidation.

## Design notes

- **Demand, not decisions.** The crate states what a thought asks for and leaves scheduling to
  the sibling that owns the resources.
- **Pluggable scoring.** Replace `WeightedSum` by implementing `AttentionFormula`. The factors
  and the score type do not change.
- **Forgetting is a feature.** Working memory drops items. Strict capacity and decay are what keep
  cognition bounded.
- **Time moving backwards is ignored.** `decay` does nothing when `now` precedes the last update.
- **Scores are not clamped.** Callers are expected to supply factors in `0.0..=1.0`. The default
  formula does not check this.

## Dependencies

Depends on `tpt-thought-core` and `serde`. `proptest` is a dev-dependency only.

## Related crates

- `tpt-thought-core`: `Thought`, `Intention` and `Timestamp`.
- `tpt-thought-consolidation`: consumes what working memory lets go, for durable learning.
- `tpt-thought-policy`: decides what to do with a thought once it has a priority.
- `tpt-thought` (facade): re-exports this crate as `tpt_thought::attention`.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
