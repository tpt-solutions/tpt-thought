# tpt-thought-state

The lifecycle state machine for a thought. It defines which events may move a thought from one
state to another, and it refuses every other move.

> **Status:** pre-release (v0.1.0). Distributed from git only, not on crates.io.

## Installation

```toml
[dependencies]
tpt-thought-state = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## What this crate does

A thought moves through states such as `Received`, `Hypothesising`, `Evaluating`, `Acting` and
`Complete`. Cognition is not linear: a thought can re-hypothesise after a setback, re-investigate,
or reconsider after an outcome. The transition table permits those loops and nothing else.

Transitions are driven by `ThoughtEvent`s. A transition is a pure function of
`(state, event)`, so the same input always produces the same result.

What it deliberately does not do:

- It does not decide when to fire an event. That belongs to `tpt-thought-loop`.
- It does not store or persist anything. It changes the `state` field of a `Thought` and
  nothing else, apart from `temporal.updated_at`.
- It does not know what the states mean for content. Content rules belong to the crates that
  own that content.

## Main API

| Item | Purpose |
|---|---|
| `ThoughtEvent` | The sixteen events that drive transitions (`Receive`, `Attend`, `Contextualise`, `Associate`, `Hypothesise`, `Evaluate`, `Accept`, `MarkUncertain`, `Investigate`, `ReEvaluate`, `Intend`, `Act`, `Observe`, `Learn`, `Consolidate`, `Complete`) |
| `TRANSITIONS` | The complete transition table as `(from, event, to)` triples |
| `next(from, event)` | The state reached by an event, or `IllegalTransition` |
| `can_transition(from, to)` | Whether any event moves `from` directly to `to` |
| `permitted_events(from)` | The events that are legal in a given state |
| `kind(event)` | The `TransitionKind` recorded for an event |
| `apply(&mut thought, event, now)` | Applies an event to a `Thought`, updating `state` and `temporal.updated_at`. On error the thought is unchanged. |
| `ThoughtLifecycle` | A trait with one method per step (`receive`, `attend`, `contextualise`, `associate`, and so on) that applies exactly one event each |
| `IllegalTransition` | Error carrying the `from` state and the rejected event |

`ThoughtState` itself lives in `tpt-thought-core`.

## Usage

Apply events to a thought. Each call either succeeds or leaves the thought untouched:

```rust
use tpt_thought_core::{IdGenerator, SeededEntropy, TextContent, Thought, ThoughtContent,
    ThoughtOrigin, ThoughtState, Timestamp};
use tpt_thought_state::{apply, ThoughtEvent};

let ids = IdGenerator::new(SeededEntropy::new(1));
let mut t = Thought::new(
    ids.thought(),
    ThoughtOrigin::Internal,
    ThoughtContent::Text(TextContent { text: "hello".into(), language: None }),
    Timestamp(0),
);
apply(&mut t, ThoughtEvent::Receive, Timestamp(1)).unwrap();
assert_eq!(t.state, ThoughtState::Received);
```

Check a move before attempting it:

```rust
use tpt_thought_core::ThoughtState;
use tpt_thought_state::{next, ThoughtEvent};

assert!(next(ThoughtState::Complete, ThoughtEvent::Learn).is_err());
```

## Design notes

- **Loops are legal.** For example, `Uncertain` can return to `Hypothesising` or `Investigating`,
  and `Intending` can return to `Evaluating` when an intention changes. Anything not in
  `TRANSITIONS` is refused.
- **`Complete` is terminal.** No event leaves it.
- **Errors are values.** `IllegalTransition` reports the state and the event, so callers can log
  or escalate the refusal.
- **The table is data.** `TRANSITIONS` is public, so tests and documentation can read the same
  table the code enforces.

## Related crates

- `tpt-thought-core`: defines `Thought`, `ThoughtState` and `TransitionKind`.
- `tpt-thought-loop`: drives the state machine through a full cognitive cycle.
- `tpt-thought` (facade): re-exports this crate as `tpt_thought::state`.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
