# Threat model

Scope: the TPT Thought library (spec section 41). Thought does the cognition; identity, permissions
and attestation belong to Anima ZKPA, and transport to INP. This document covers the threats Thought
itself must resist, and where each mitigation lives.

Principles
- Deny by default: capabilities, sources and actors are refused unless explicitly granted.
- Provenance is canonical: every conclusion must trace to sealed, verifiable records.
- Failure stops, not drifts: circuit breakers stay open until a caller resets them.
- Thought never executes actions. Action goes through Intellect and Anima/INP.

| Threat | Description | Mitigation | Where |
| --- | --- | --- | --- |
| Memory injection | Forged content planted in memory is recalled as fact. | Recalled content carries provenance and a trust level; intentions require a minimum trust. | `tpt-thought-core` (`SourceTrust`, `SourcePolicy`); `ObservationGate` in `tpt-thought-safety` |
| Prompt injection | Text from an untrusted input steers reasoning or actions. | Untrusted sources are classified as untrusted by default; an `ObservationGate` at `Authenticated` keeps them out of action paths. Structured fields are never executed as instructions. | `SourcePolicy::classify`, `ObservationGate::admit` |
| Thought manipulation | A hostile process inflates, loops or redirects thinking. | Circuit breakers for repetition, explosion, escalation, oscillation, contradiction loops and resource exhaustion. Breakers stay open until reset. | `CircuitBreakers` (`breaker.rs`) |
| False provenance | A thought cites records that do not explain it, or records that were altered. | Provenance is SHA-256 sealed and chained. `verify_provenance` checks the whole store and that every cited record explains this thought. | `verify_provenance` (`integrity.rs`), `ProvenanceStore::verify` in `tpt-thought-provenance` |
| Fabricated observations | An observation claims a trust level or source it does not have. | Trust is assigned by the source policy, not by the observation. Unlisted sources are untrusted. Gates enforce a minimum. Identity attestation is delegated to ZKPA. | `SourcePolicy`, `ObservationGate` |
| Replay attacks | A captured observation or thought is submitted again to repeat its effect. | Each identifier is admitted once. Repeats are refused. Timestamps are kept on every record for audit. | `ReplayGuard` (`integrity.rs`) |
| Identity substitution | An actor acts on a thought it does not own or has not been delegated. | Only the owner (or the delegate, for delegated thoughts) may act. Archived thoughts admit no actor. The identity itself is resolved by ZKPA. | `admit_actor` (`integrity.rs`), `ThoughtOwnership` in core |
| Contaminated knowledge | Bad facts persist and are reused, or contradictory knowledge accumulates. | Provenance lets contamination be traced. Contradiction loops trip a breaker. Promotion from thought to belief requires configured criteria (Phase 2, `tpt-thought-hypothesis`). Consolidation re-evaluates beliefs (Phase 9). | `CircuitBreakers::record_contradiction`; see `tpt-thought-hypothesis` and `tpt-thought-consolidation` |
| Malicious loops | Thoughts spawn each other, or a cycle of parent links forms, without end. | Parent-cycle detection, depth and child limits, compute and duration limits, and repetition breakers. | `find_parent_cycle`, `Sandbox::check_usage` (`sandbox.rs`), `CircuitBreakers::record_step` |
| Capability abuse | A thought uses a capability (sensor, actuator, tool) it was never granted. | Capabilities are granted explicitly per sandbox. Matching is exact. An intention is authorised only if every required capability is granted. | `Sandbox` (`sandbox.rs`) |

Residual risks and out-of-scope items
- Identity and attestation are trusted to ZKPA. A compromised identity provider defeats `admit_actor`.
- Cryptographic hashes detect tampering with a store; they do not prove who wrote a record. Signing
  belongs to the integration layer.
- Resource limits bound the work Thought schedules. Anima CK enforces the actual CPU and memory.
- Breaker thresholds are defaults. They must be tuned per deployment and measured (spec section 96).
- Fuzzing of serialization decoders is tracked separately in `todo.md` (Phase 12).
