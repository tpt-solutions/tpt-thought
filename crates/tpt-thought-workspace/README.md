# tpt-thought-workspace

Global workspace references, memory identifiers, episodic records and the thought bubble for
TPT Thought.

`tpt-thought-workspace` is the Thought side of shared state. It defines the contracts a thought
codes against: a global workspace for publishing and requesting items, a memory store that
returns identifiers, a record of what happened in an episode, and a sandbox for experimental
cognition. Durable memory and the shared workspace itself belong to Anima. This crate ships an
in-process workspace for tests and standalone use, and nothing else that persists.

> **Status:** pre-release, `v0.1.1`. Distributed from git only, not published on crates.io.

## What it does

- **Global workspace.** The `GlobalWorkspace` trait covers `subscribe`, `unsubscribe`, `publish`,
  `broadcast`, `request`, `respond`, `attend`, `inbox`, `withdraw` and `get`. `InProcessWorkspace`
  is a deterministic reference implementation: items are ordered by identifier and subscriptions
  are kept in sorted sets.
  - `Publication`: visible on the workspace but pushed to no inbox.
  - `Broadcast`: pushed to every subscriber of the topic except the publisher.
  - `Request`: asks subscribers for a reply. `respond` answers it and delivers the response to the
    requester's inbox.
  - Only the publisher may `withdraw` an item. Attempts by anyone else fail with
    `WorkspaceError::NotPublisher`.
- **Memory by identifier.** The `MemoryStore` trait retrieves `MemoryRef`s for a tag cue, best
  match first, ties broken by identifier. `MemoryIndex` maps identifiers to tags. It holds no
  memory content, so the content stays in Anima.
- **Episodic records.** `Episode` collects an `EpisodeEvent` stream, `PredictionRecord`s and
  `ActionRecord`s, then closes with an `EpisodeOutcome`. Recording after close returns
  `EpisodeError`. Anima persists episodes; this crate only produces them.
- **Thought bubble.** `ThoughtBubble` is the execution boundary for experimental cognition. It is
  entered against a `DurableStore` snapshot and keeps its writes in an overlay. Nothing reaches
  durable state until `commit`, which copies only the kinds a `CommitPolicy` allows. Dropping a
  bubble discards it.

## Bubble kinds and commit rules

| `BubbleKind` | Committable | Meaning |
|---|---|---|
| `Memory` | yes | Temporary memory traces |
| `Hypothesis` | yes | Speculative hypotheses |
| `Tool` | never | Tools used only inside the bubble |
| `Branch` | never | Exploratory branches |
| `Model` | never | Temporary models |

`BubbleKind::committable` is the fixed rule. `CommitPolicy::allow` refuses the non-committable
kinds with `BubbleError::NeverCommittable`, so a policy cannot widen what may leave a bubble.
`CommitPolicy::none()` permits nothing. A commit returns a `CommitReport` listing what was copied
and what was skipped.

## What it deliberately does not do

- It does not hold memory content. `MemoryStore` returns identifiers only.
- It does not implement the shared, multi-agent workspace. That is Anima's `GlobalWorkspace`.
  Anima-backed implementations belong in `tpt-thought-integrations`.
- It does not depend on any Anima crate.
- It does not execute actions or call providers.

## Usage

```rust
use tpt_thought_core::{ThoughtContent, TextContent, Timestamp};
use tpt_thought_workspace::{GlobalWorkspace, InProcessWorkspace};

let mut ws = InProcessWorkspace::new();
ws.subscribe("planner", "sensors");

let text = ThoughtContent::Text(TextContent {
    text: "obstacle ahead".into(),
    language: None,
});
ws.broadcast("camera", "sensors", text, Timestamp(1));

let inbox = ws.inbox("planner");
let item = ws.attend("planner", inbox[0])?;
```

Use a `ThoughtBubble` for speculative work:

```rust
use tpt_thought_workspace::{BubbleKind, CommitPolicy, DurableStore, ThoughtBubble};

let mut durable = DurableStore::new();
let mut bubble = ThoughtBubble::enter(&durable);
bubble.put(BubbleKind::Hypothesis, "h1", payload);

let mut policy = CommitPolicy::none();
policy.allow(BubbleKind::Hypothesis)?;
let report = bubble.commit(&mut durable, &policy);
```

## Module layout

| Module | Contents |
|---|---|
| `workspace` | `GlobalWorkspace`, `InProcessWorkspace`, `Item`, `ItemId`, `ItemKind`, `WorkspaceError` |
| `memory` | `MemoryStore`, `MemoryIndex` |
| `episode` | `Episode`, `EpisodeEvent`, `PredictionRecord`, `ActionRecord`, `EpisodeOutcome`, `EpisodeError` |
| `bubble` | `ThoughtBubble`, `BubbleKind`, `BubbleKey`, `BubbleError`, `CommitPolicy`, `CommitReport`, `DurableStore` |

## Design notes

- Every ordering is deterministic (`BTreeMap` and `BTreeSet` throughout), so replaying the same
  calls yields the same inboxes and retrieval results.
- The bubble copies its durable snapshot on entry and writes only to its overlay, so a half-finished
  experiment cannot corrupt durable state.
- Memory references are identifiers and cues, never content, which keeps Anima as the only owner
  of stored memory.

## Related crates

- `tpt-thought-core` supplies `ThoughtContent`, `Timestamp` and `MemoryRef`.
- `tpt-thought-consolidation` replays the episodes this crate records.
- `tpt-thought-loop` drives workspace publication and bubble use during a cycle.

## Installation

```toml
[dependencies]
tpt-thought-workspace = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
