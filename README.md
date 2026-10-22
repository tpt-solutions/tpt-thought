# TPT Thought

Cognitive process orchestration and thought representation, in Rust, by TPT Solutions.

A *thought* is a dynamically evolving cognitive state that can receive stimuli, retrieve
memories, form hypotheses, invoke intelligence, evaluate alternatives, form intentions, observe
outcomes and update itself. TPT Thought owns that trajectory, not the intelligence used to
generate individual conclusions.

## What it is not

Thought is **not** an LLM, a prompt framework, a chatbot, a memory database, a reasoning
engine, an inference runtime, a model router or an agent framework. Those belong to Anima,
Eve, Intellect, Augur, Axiom and Infer. See `spec.txt` section 111.

## Layout

| Crate | Purpose |
|---|---|
| `tpt-thought-core` | Core types: IDs, thoughts, confidence, uncertainty, time, wire format. No TPT or backend dependencies. |
| `tpt-thought-state` | Lifecycle state machine and transition validation. |
| `tpt-thought-provenance` | Hash-linked, machine-readable provenance and "why" queries. |
| `tpt-thought-attention` | Attention scoring, cognitive demand and working memory. |
| `tpt-thought-graph` | Typed relationships between thoughts, beliefs and evidence, and bounded spawning. |
| `tpt-thought-hypothesis` | Hypothesis branches, collapse, contradictions and the belief promotion pipeline. |
| `tpt-thought-prediction` | Predictions, prediction errors and the response ladder. |
| `tpt-thought-providers` | Provider-neutral intelligence traits, registry, calibration and cognition modes. |
| `tpt-thought-deliberation` | `DeliberationEngine` trait, metacognition and bounded investigation. |
| `tpt-thought-policy` | Escalation levels L0 to L6, expected value of cognition, budgets and termination. |
| `tpt-thought-workspace` | In-process global workspace, memory references, episodes and the thought bubble. |
| `tpt-thought-safety` | Circuit breakers, capability sandbox, resource limits and integrity checks. |
| `tpt-thought-reflection` | Bounded reflection sessions and structured review artifacts. |
| `tpt-thought-consolidation` | Sleep cycle, consolidation plan, replay and skill formation. |
| `tpt-thought-observatory` | Telemetry events, metrics, sinks and snapshot data. |
| `tpt-thought-intention` | Intentions with validated construction, lifecycle and set queries. |
| `tpt-thought-loop` | The closed cognitive cycle, modes, modulation, planning, curiosity and replay. |
| `tpt-thought-tests` | Acceptance, adversarial and replay tests, and benchmarks. |
| `tpt-thought` | Facade re-exporting the public API. |

`tpt-thought-integrations` (adapters to sibling repositories) is on hold. See `docs/ARCHITECTURE.md`.

More crates are added only when a real API or dependency boundary exists. See `todo.md`.

## Development

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo deny check
```

## Licence

Licensed under either of MIT or Apache-2.0 at your option. Source files carry
`// SPDX-License-Identifier: MIT OR Apache-2.0`.
