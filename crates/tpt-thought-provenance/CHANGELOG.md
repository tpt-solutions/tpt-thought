# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `ProvenanceRecord` and `ProvenanceBody`, with sources, parents, evidence, memory, provider and
  model labels, structured reasoning steps and verification.
- SHA-256 sealing (`Hash256`) over the CBOR body and the parent hashes.
- `ProvenanceStore`: an append-only, acyclic store with `insert`, `get`, `ancestors`,
  `descendants`, `for_thought`, `explain` and `verify`.
- `IntegrityError` and `InsertError`.
