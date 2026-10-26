# tpt-thought-provenance

Machine-readable, hash-linked provenance for cognition. Each record states where a piece of
thinking came from, links to its parents, and is sealed with a SHA-256 digest, so tampering with
any ancestor can be detected. The store can answer "why did the agent think this?" from structure
alone.

> **Status:** pre-release (v0.1.0). Distributed from git only, not on crates.io.

## Installation

```toml
[dependencies]
tpt-thought-provenance = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## What this crate does

A `ProvenanceRecord` pairs a `ProvenanceBody` with a `Hash256`. The body records:

- what kind of source it is (`ProvenanceSource`: observation, memory, decision, reasoning,
  inference, verification, intention, human, or a named `Other`);
- which thought it explains, if any;
- its parent records, the evidence and memories it used;
- the provider, model and version that produced it, as opaque labels;
- structured reasoning steps and the verification applied;
- a timestamp.

The hash covers the CBOR encoding of the body and the hashes of its parents. A record therefore
commits to its whole derivation.

The store is append-only. A record can only be inserted after its parents, so the graph is
acyclic by construction and cycles are impossible.

What it deliberately does not do:

- It does not generate explanations. `explain` returns records, not prose.
- It does not record a model's private chain of thought. `reasoning_path` holds structured steps
  only, as the threat model requires.
- It does not decide what is trustworthy. Trust is classified elsewhere (`SourcePolicy` in
  `tpt-thought-core`).

## Main API

| Item | Purpose |
|---|---|
| `ProvenanceStore` | Append-only store. `new`, `len`, `is_empty`, `get`, `insert` |
| `ProvenanceStore::insert(body)` | Seals and appends a record. Fails with `InsertError` on a duplicate ID or a missing parent. |
| `ancestors(id)` | Every record the given record derives from |
| `descendants(id)` | Every record derived from the given record |
| `for_thought(thought)` | The records that explain one thought |
| `explain(thought)` | The full derivation of a thought: its records and all their ancestors, oldest first |
| `verify()` | Recomputes every hash and checks every parent link. Returns `IntegrityError` on failure. |
| `ProvenanceRecord` | A sealed body with its `hash` |
| `Hash256` | A SHA-256 digest, serialised as lowercase hex |
| `IntegrityError` | `MissingParent` or `HashMismatch`, naming the record |
| `InsertError` | `Duplicate` or `MissingParent` |

## Usage

Insert a root record, then a derived record, then ask why a thought holds:

```rust
use tpt_thought_core::{IdGenerator, SeededEntropy, ThoughtId, Timestamp};
use tpt_thought_provenance::{ProvenanceBody, ProvenanceSource, ProvenanceStore};

let ids = IdGenerator::new(SeededEntropy::new(7));
let thought: ThoughtId = ids.thought();
let mut store = ProvenanceStore::new();

let observed = store
    .insert(ProvenanceBody {
        id: ids.provenance(),
        source: ProvenanceSource::Observation,
        thought: Some(thought),
        parents: vec![],
        evidence: vec![],
        memory: vec![],
        provider: None,
        model: None,
        version: None,
        reasoning_path: vec![],
        verification: None,
        timestamp: Timestamp(1),
    })
    .unwrap()
    .body
    .id;

store
    .insert(ProvenanceBody {
        id: ids.provenance(),
        source: ProvenanceSource::Reasoning,
        thought: Some(thought),
        parents: vec![observed],
        evidence: vec![],
        memory: vec![],
        provider: Some("reference".into()),
        model: None,
        version: None,
        reasoning_path: vec!["compared the reading with the expected range".into()],
        verification: None,
        timestamp: Timestamp(2),
    })
    .unwrap();

assert_eq!(store.explain(&thought).len(), 2);
store.verify().unwrap();
```

## Design notes

- **Tampering is detectable.** Changing any field of any record changes its hash, and the change
  propagates to every descendant through the parent hashes. `verify` finds it.
- **Explanations are structural.** `explain` follows links. No generated text is involved.
- **Provider labels are opaque.** `provider`, `model` and `version` are strings that the crate
  never branches on, which keeps the layer provider-neutral.
- **Hashing uses the wire format.** Bodies are encoded with `tpt_thought_core::wire::to_cbor`,
  so the hash depends on the wire schema version.
- **`verify` does not stop the cognition.** It is a check that callers run at audit points. The
  integrity checks that gate actions are in `tpt-thought-safety`
  (`verify_provenance`).

## Dependencies

Depends on `tpt-thought-core`, `serde` and `sha2`. No async runtime.

## Related crates

- `tpt-thought-core`: `ThoughtId`, `ProvenanceId`, `ContentReference` and the wire format.
- `tpt-thought-safety`: `verify_provenance` builds on `ProvenanceStore::verify` and the
  per-thought check.
- `tpt-thought` (facade): re-exports this crate as `tpt_thought::provenance`.

Threats this crate answers are listed in `docs/THREAT_MODEL.md`.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
