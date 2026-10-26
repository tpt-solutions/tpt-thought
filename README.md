# TPT Thought

Cognitive process orchestration and thought representation, in Rust, by TPT Solutions.

**Status:** pre-release, v0.1.0. Distributed from git only; not published to crates.io. See
`docs/DEPENDENCY_POLICY.md`.

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
| `tpt-thought` | Facade re-exporting the public API, with a `prelude` and the examples. |
| `tpt-thought-cli` | The `thought` command: demo, run, replay, validate, why and schema for recorded runs. |

`tpt-thought-integrations` holds the adapters to sibling repositories. Each is behind its own
feature on the `tpt-thought` facade: `intellect`, `eve`, `anima`, `augur`, `axiom`, `remote` and
`local`. None is enabled by default. For example:

```
cargo build -p tpt-thought --features augur
```

More crates are added only when a real API or dependency boundary exists. See `todo.md`.

## Quickstart

```
cargo run -p tpt-thought --example quickstart
cargo run -p tpt-thought-cli -- demo --out record.json
cargo run -p tpt-thought-cli -- replay record.json
```

The first runs one observation through the closed loop and checks that a replay reproduces it.
The second records that run to a file and the third re-runs it.

The facade also has examples for a custom provider, hypotheses, Augur evidence, and reflection
and sleep. Run them with `cargo run -p tpt-thought --example <name>`.

The `thought` CLI has six commands:

| Command | Purpose |
|---|---|
| `demo [--out RECORD.json]` | Run the built-in scenario and optionally record it |
| `run INPUT.json [--out RECORD.json]` | Run inputs from a file and optionally record them |
| `replay RECORD.json` | Re-run a recording and check it reproduces |
| `validate RECORD.json` | Check a recording's structure |
| `why RECORD.json CYCLE` | Explain a cycle from a recording |
| `schema` | Print the schema for recorded runs |

## Documentation

- `docs/ARCHITECTURE.md`: how the crates fit together
- `docs/COGNITIVE_MODEL.md`: the model of thought and the cognitive cycle
- `docs/QUICKSTART.md`: code to get started
- `docs/CLI.md`: the `thought` commands
- `docs/COOKBOOK.md`: common tasks
- `docs/PROVIDERS.md`: writing and registering providers
- `docs/INTEGRATION_*.md`: the sibling-repository adapters
- `docs/BENCHMARKS.md`, `docs/THREAT_MODEL.md`, `docs/DEPENDENCY_POLICY.md`

`spec.txt` is the specification and `CONTRIBUTING.md` covers contributions.

## Development

`scripts/check.sh` (or `scripts/check.ps1` on Windows) runs the same checks as CI:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

`scripts/bench.sh` (or `scripts/bench.ps1`) runs the pipeline benchmark and updates
`docs/BENCHMARKS.md`.

## Licence

Licensed under either of MIT or Apache-2.0 at your option. Source files carry
`// SPDX-License-Identifier: MIT OR Apache-2.0`.
