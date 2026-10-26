# tpt-thought-reflection

Bounded reflection over past thoughts for TPT Thought.

Reflection is a deliberate, bounded review of what has already happened: decisions, outcomes,
prediction errors, contradictions and knowledge gaps. TPT Thought decides *when* to reflect, *why*
and *on what*. The reasoning itself is supplied by a `ReflectionReasoner`, which is Eve in
production and a stub in tests.

Reflection sits between two other modes. Online thought is latency-bound and cannot change durable
belief. Sleep is offline consolidation, which `tpt-thought-consolidation` owns.

> **Status:** pre-release, `v0.1.0`. Distributed from git only, not published on crates.io.

## What it does

- **Triggers.** `TriggerSignal` is a raw signal: an error, uncertainty, a contradiction, a goal
  change or an outcome mismatch. `ReflectionEngine::decide` turns signals into a
  `ReflectionTrigger` using the thresholds in `ReflectionPolicy`. When several conditions hold,
  the priority is error, outcome mismatch, contradiction, goal change, uncertainty, then the
  periodic schedule. Nothing fires, so the call returns `None`.
- **Sessions.** `ReflectionSession` runs under explicit `ReflectionLimits`: a maximum number of
  steps, a maximum number of artifacts and a wall-clock duration. A session is `Running`,
  `Completed` or `Halted(Halt)`, where `Halt` names the limit that ended it. A closed session
  refuses further steps with `SessionError`.
- **Reasoner.** The `ReflectionReasoner` trait produces `ReflectionArtifact`s for one thought at a
  time. A failure returns `ReasonerError` with a short diagnostic, and the run records it as the
  review's `failure`.
- **Artifacts.** `ReflectionArtifact` is a closed set of structured records: decisions, evidence,
  outcomes, predictions, prediction errors, confidence, contradictions, causal explanations,
  knowledge gaps, strategy updates, memory updates and belief revisions. Free text is limited to
  `Label`, which is one line of at most `MAX_LABEL_LEN` (160) bytes.
- **Review.** `ReflectionEngine::run` returns a `CognitiveReview`. It holds the trigger, the final
  status, the thoughts reviewed, any failure, and the analyses: `Review` counts,
  `Metacognition`, `KnowledgeGapAnalysis`, `ErrorAnalysis` and `StrategyRevision`. `Metacognition`
  carries the mean certainty and a `Recommendation`: `Continue`, `Act` or `AskForHelp`.
- **Modes.** `CognitiveMode` (`Online`, `Reflection`, `Sleep`) has `ModeRules` that state whether
  it is latency-bound, interruptible, allowed durable updates, and allowed to read past thoughts.
  Reflection reads the past and is interruptible; online thought is latency-bound and cannot
  update durably.

## What it deliberately does not do

- It does not record a model's private chain-of-thought. Artifacts hold identifiers, references,
  numbers and labels only.
- It does not reason. The reasoner does, and this crate checks what comes back.
- It does not write durable beliefs during online thought. Belief changes go through the
  consolidation and hypothesis pipelines.
- It does not schedule itself. `decide` is pure: the caller supplies the current time and the
  last reflection time.

## Usage

```rust
use tpt_thought_reflection::{ReflectionEngine, ReflectionPolicy, TriggerSignal};

let engine = ReflectionEngine::new(ReflectionPolicy::default());

let signals = [TriggerSignal::Uncertainty {
    thought: thought_id,
    uncertainty: 0.9,
}];

if let Some(trigger) = engine.decide(&signals, &recent, last_reflection, now) {
    let review = engine.run(trigger, &thoughts, &mut reasoner, &clock);
    if review.failure.is_some() {
        // the session ended early; inspect `review.status`
    }
}
```

`ReflectionPolicy::default()` fires uncertainty at 0.7, outcome mismatch at 0.5, reflects
periodically every 60 seconds, and uses `ReflectionLimits::default()`: 16 steps, 64 artifacts and
5 seconds.

## Module layout

| Module | Contents |
|---|---|
| `artifact` | `ReflectionArtifact` and its record types, `Label`, `LabelError` |
| `engine` | `ReflectionEngine`, `ReflectionPolicy`, `ReflectionReasoner`, `ReflectionStep`, `ReasonerError` |
| `mode` | `CognitiveMode`, `ModeRules` |
| `review` | `CognitiveReview`, `Review`, `Metacognition`, `Recommendation` and the analyses |
| `session` | `ReflectionSession`, `ReflectionLimits`, `SessionStatus`, `Halt`, `SessionError` |
| `trigger` | `TriggerSignal`, `ReflectionTrigger` |

## Design notes

- Every run terminates. Each step is checked against the session limits before it starts, and a
  breached limit halts the session. The review then reports the `Halted` status instead of the
  caller receiving an error.
- Artifacts are a closed enum, which keeps reflection output inspectable and stops it carrying
  free-form reasoning text.
- The clock is injected, so tests and replays see the same durations.

## Related crates

- `tpt-thought-core` supplies `Thought`, `ThoughtId`, `Clock` and `Timestamp`.
- `tpt-thought-consolidation` handles the offline counterpart, sleep.
- `tpt-thought-loop` triggers reflection from the cognitive cycle.

## Installation

```toml
[dependencies]
tpt-thought-reflection = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
