# tpt-thought-intention

Intentions for TPT Thought: the goals a thought commits to, with their constraints, risk
tolerance, priority, deadline, required capabilities and provenance, plus a lifecycle and a set
type for priority and deadline queries.

**Status:** pre-release, v0.1.0. Distributed from git only; not published to crates.io. See the
repository's `docs/DEPENDENCY_POLICY.md`.

## What it does

- **Validated construction.** `ManagedIntention::new` checks an `Intention` (from
  `tpt-thought-core`) before it enters the system. It rejects a blank goal, a non-finite priority
  and blank constraint or capability entries. Constraints and capabilities are sorted and
  de-duplicated, so equal content compares equal.
- **A lifecycle.** `IntentionStatus` has six states: `Formed`, `Dispatched`, `Executing`,
  `Achieved`, `Failed` and `Abandoned`. `Achieved` and `Abandoned` are terminal. Retrying is legal:
  a `Failed` intention may be dispatched again, so the transition table contains a loop. The
  permitted transitions are listed in `EDGES`, and `IntentionStatus::can_transition_to` checks them.
  `ManagedIntention::advance` applies a transition and records it in the history.
- **Provenance.** Each managed intention carries the `ProvenanceId`s that explain why it exists.
  `add_provenance` appends more.
- **Capability checks.** `require_capabilities` confirms that every capability an intention
  requires was granted. Nothing is granted by default.
- **Sets.** `IntentionSet` holds intentions by ID and answers `by_priority`, `due_before`,
  `overdue` and `next_deadline`. Its `advance` applies a transition by ID.

## What it deliberately does not do

- It never executes anything. Thought does not run actions (spec section 24). Dispatch and
  execution belong to Anima's INP, and their outcomes come back as status transitions.
- It does not decide which intentions to form. That is the cognitive loop's job.

## Usage

```rust
use tpt_thought_core::{IdGenerator, Intention, Probability, SeededEntropy, Timestamp};
use tpt_thought_intention::{require_capabilities, IntentionSet, IntentionStatus, ManagedIntention};

let ids = IdGenerator::new(SeededEntropy::new(7));
let intention = Intention {
    id: ids.intention(),
    goal: "pick up the cup".into(),
    desired_outcome: Some("cup held".into()),
    constraints: vec!["do not tip the cup".into()],
    risk_tolerance: Some(Probability::new(0.1).expect("valid probability")),
    priority: 2.0,
    required_capabilities: vec!["gripper".into()],
    deadline: Some(Timestamp(5_000)),
};

require_capabilities(&intention, ["gripper"])?;

let managed = ManagedIntention::new(intention, vec![])?;
let id = managed.id();

let mut set = IntentionSet::new();
set.insert(managed)?;
set.advance(&id, IntentionStatus::Dispatched)?;

assert_eq!(set.overdue(Timestamp(6_000)).len(), 1);
```

## Module layout

| Module | Contents |
|---|---|
| crate root | `ManagedIntention`, `InvalidIntention`, `MissingCapabilities`, `require_capabilities` |
| `status` | `IntentionStatus`, `IllegalIntentionTransition`, `EDGES` |
| `set` | `IntentionSet`, `SetError` |

## Installation

```toml
[dependencies]
tpt-thought-intention = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## Design notes

- Intentions are data. The crate validates and sequences them but does not act on them.
- A deadline counts as overdue only for an active intention. A terminal intention is never overdue.
- `IntentionSet` returns intentions in a stable order (priority, or earliest deadline first), so
  queries are deterministic.

## Related crates

- `tpt-thought-core` defines the `Intention` data type and its IDs.
- `tpt-thought-loop` holds an `IntentionSet` and forms intentions during a cycle.
- `tpt-thought-integrations` (`anima` feature) connects dispatch to Anima's INP.

## Licence

Licensed under either of MIT or Apache-2.0, at your option.

## Minimum supported Rust version

1.85.
