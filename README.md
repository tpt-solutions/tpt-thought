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
| `tpt-thought` | Facade re-exporting the public API. |

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
