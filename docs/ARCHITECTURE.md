# Architecture

Thought is the cognitive *process* layer. It owns the trajectory of a thought and delegates
intelligence, memory, reasoning and scheduling to other systems through provider traits.

```text
Anima (substrate) -> THOUGHT -> Intellect / Eve / Augur / Axiom
```

## Crate layers

Crates are listed bottom-up. A crate may depend only on crates in lower layers.

| Layer | Crate | Purpose |
|---|---|---|
| 0 | `tpt-thought-core` | Vocabulary: IDs, thoughts, observations, hypotheses, beliefs, predictions, intentions, outcomes, confidence, uncertainty, time, wire format. No TPT or backend dependencies. |
| 1 | `tpt-thought-state` | Lifecycle state machine and transition table. |
| 1 | `tpt-thought-provenance` | Hash-linked provenance records and "why" queries. |
| 1 | `tpt-thought-attention` | Attention scoring, cognitive demand and working memory. |
| 1 | `tpt-thought-graph` | Typed relationships between nodes, and bounded recursive spawning. |
| 2 | `tpt-thought-hypothesis` | Branches, collapse, contradictions, information gain, belief promotion and demotion. |
| 2 | `tpt-thought-prediction` | Predictions, matching, prediction errors and the response ladder. |
| 2 | `tpt-thought-providers` | Provider-neutral intelligence traits, registry, calibration, cognition modes, human provider. |
| 2 | `tpt-thought-deliberation` | `DeliberationEngine` trait, metacognition, bounded investigation loop. |
| 2 | `tpt-thought-policy` | Escalation levels L0 to L6, expected value of cognition, resource budgets, termination. |
| 2 | `tpt-thought-workspace` | In-process global workspace model, memory references, episodic records, thought bubble. |
| 2 | `tpt-thought-safety` | Circuit breakers, capability sandbox, resource limits, integrity checks. |
| 2 | `tpt-thought-reflection` | Bounded reflection sessions and structured review artifacts. |
| 2 | `tpt-thought-consolidation` | Sleep cycle, consolidation plan, replay and skill formation. |
| 2 | `tpt-thought-observatory` | Telemetry events, metrics, sinks and snapshot data. No UI. |
| 3 | `tpt-thought-intention` | Intentions with validated construction, lifecycle and set queries. |
| 3 | `tpt-thought-loop` | The closed cognitive cycle, modes, modulation, planning, curiosity, slow path and replay. |
| 3 | `tpt-thought-tests` | Acceptance, adversarial, replay and resource tests, plus benchmarks. |
| top | `tpt-thought` | Facade that re-exports the public API. |

`tpt-thought-integrations` is **on hold**. It would hold the only adapters to sibling repositories
(Intellect, Eve, Anima, Augur, Axiom). It is not in the build yet. See the `INTEGRATION_*` documents.

## Dependency rules

- Dependencies point downward only. `tpt-thought-core` depends on nothing TPT.
- Cognition crates depend on the core types and on the other crates they need, never on a backend.
- Sibling-repository dependencies are allowed only in `tpt-thought-integrations`, each behind its
  own feature.
- No library crate depends on an async runtime. Tokio is denied in `deny.toml`.
- Nothing is branched on a specific backend. Everything goes through provider traits.

## Principles

- Never branch on a specific backend; go through provider traits.
- Libraries are runtime-agnostic. Asynchronous work is the caller's concern.
- Provenance is the canonical explanation; generated text is presentation only.
- Thought decides *when* and *why* to think. It does not execute actions. Actions go through an
  `ActionSink` supplied by the caller.
- Bounded by construction: every recursive or repeated process has an explicit limit.
