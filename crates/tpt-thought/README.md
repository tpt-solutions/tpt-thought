# tpt-thought

The facade for TPT Thought. It re-exports the public API of the Thought crates under one name, so
a program can depend on a single crate and reach every subsystem through a module path.

> **Status:** pre-release (v0.1.1). Distributed from git only, not on crates.io.

TPT Thought is cognitive process orchestration and thought representation. A *thought* is a
dynamically evolving cognitive state that receives stimuli, retrieves memories, forms hypotheses,
invokes intelligence, evaluates alternatives, forms intentions, observes outcomes and updates
itself. Thought owns that trajectory. It does not own the intelligence used to reach individual
conclusions.

## Installation

```toml
[dependencies]
tpt-thought = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

To enable adapters for sibling TPT repositories, turn on the matching feature:

```toml
[dependencies]
tpt-thought = { git = "https://github.com/TPT-Solutions/tpt-thought", features = ["eve"] }
```

## What this crate does

It contains no logic of its own. It re-exports these crates:

| Module | Crate | Purpose |
|---|---|---|
| `core` | `tpt-thought-core` | Thoughts, IDs, confidence, uncertainty, time, wire format |
| `state` | `tpt-thought-state` | Lifecycle state machine |
| `provenance` | `tpt-thought-provenance` | Hash-linked provenance and "why" queries |
| `attention` | `tpt-thought-attention` | Attention scoring, cognitive demand, working memory |
| `graph` | `tpt-thought-graph` | Typed relationships and bounded spawning |
| `hypothesis` | `tpt-thought-hypothesis` | Hypothesis branches, collapse, contradictions, belief promotion |
| `prediction` | `tpt-thought-prediction` | Predictions, prediction errors and the response ladder |
| `providers` | `tpt-thought-providers` | Provider-neutral intelligence traits, registry, calibration |
| `deliberation` | `tpt-thought-deliberation` | `DeliberationEngine` trait, metacognition, bounded investigation |
| `policy` | `tpt-thought-policy` | Escalation levels L0 to L6, budgets, termination |
| `workspace` | `tpt-thought-workspace` | In-process global workspace, memory references, episodes, thought bubble |
| `safety` | `tpt-thought-safety` | Circuit breakers, sandboxing, loop detection, integrity checks |
| `reflection` | `tpt-thought-reflection` | Bounded reflection sessions and review artifacts |
| `consolidation` | `tpt-thought-consolidation` | Interruptible sleep, consolidation plans, replay, skills |
| `observatory` | `tpt-thought-observatory` | Telemetry events, metrics, sinks, snapshots |
| `intention` | `tpt-thought-intention` | Intentions, their lifecycle and set queries |
| `cognitive_loop` | `tpt-thought-loop` | The closed cognitive cycle, modes, planning, curiosity, replay |
| `integrations` | `tpt-thought-integrations` | Sibling-repository adapters (only with a feature enabled) |

It also provides a `prelude` module with the types most programs need:

```rust
use tpt_thought::prelude::*;
```

## Usage

Create a thought, then drive its lifecycle with the state machine:

```rust
use tpt_thought::{core::*, state};

let ids = IdGenerator::new(SeededEntropy::new(1));
let mut t = Thought::new(
    ids.thought(),
    ThoughtOrigin::Internal,
    ThoughtContent::Text(TextContent { text: "hello".into(), language: None }),
    Timestamp(0),
);
state::apply(&mut t, state::ThoughtEvent::Receive, Timestamp(1)).unwrap();
assert_eq!(t.state, ThoughtState::Received);
```

## Features

Adapters are off by default. Each sibling feature pulls in one sibling repository, pinned to a
revision:

| Feature | Adds |
|---|---|
| `intellect` | `integrations::intellect` (choice providers) |
| `eve` | `integrations::eve` (`DeliberationEngine`) |
| `anima` | `integrations::anima` (memory, budget, broadcast, sleep scheduling) |
| `augur` | `integrations::augur` (posteriors with residual uncertainty) |
| `axiom` | `integrations::axiom` (probability, confidence, fuzzy uncertainty) |
| `remote` | `integrations::remote` (loopback and caller-supplied transports) |
| `local` | `integrations::local` (`tpt-infer` classifier); implies `remote` |

Without a feature, `tpt_thought::integrations` does not exist. The rest of the facade builds with
no sibling repositories.

## Design notes

- **No logic here.** Every behaviour lives in a sub-crate, so each crate can be read, tested and
  replaced on its own.
- **Dependencies point downward.** Nothing in the sub-crates depends on this facade.
- **Siblings stay behind features.** Only `tpt-thought-integrations` may depend on sibling
  repositories, and no library crate may pull in an async runtime.
- **Thought does not execute actions.** Intentions are handed to an action sink. Execution is
  Anima's job.

See `docs/ARCHITECTURE.md` for the layers and `spec.txt` section 111 for what Thought is not.

## Related crates

Every module above. Start with `tpt-thought-core` for the vocabulary and `tpt-thought-loop` for
the cognitive cycle.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
