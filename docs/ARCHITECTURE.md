# Architecture

Thought is the cognitive *process* layer. It owns the trajectory of a thought and delegates
intelligence, memory, reasoning and scheduling to other systems through provider traits.

```text
Anima (substrate) -> THOUGHT -> Intellect / Eve / Augur / Axiom
```

## Crate dependency graph

```text
tpt-thought (facade)
  |-- tpt-thought-state ------> tpt-thought-core
  |-- tpt-thought-provenance -> tpt-thought-core
  '-- tpt-thought-core         (no TPT or backend dependencies)
```

Adapters to Anima, Eve, Intellect, Augur, Axiom and Pulse will live only in
`tpt-thought-integrations`. See `DEPENDENCY_POLICY.md` and `spec.txt` section 102.

## Principles

- Never branch on a specific backend; go through provider traits.
- Libraries are runtime-agnostic.
- Provenance is the canonical explanation; generated text is presentation only.
