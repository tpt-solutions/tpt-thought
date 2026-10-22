# Integration: Anima

**Status: not yet implemented.** The adapter belongs in `tpt-thought-integrations`, which is on
hold. Adding it requires a decision about depending on sibling-repository code, which has not been
made.

## Planned mapping

| Thought | Anima |
|---|---|
| `MemoryStore` (IDs and tags only, no duplicated storage) | `tpt-anima-sse` in-memory storage: `semantic_search`, `attention_retrieve`, `hybrid_retrieve`, `retrieve_by_tag` |
| Shared workspace publish, attend, broadcast, request | `tpt-anima-inp` `IntentBroadcaster` (nearest equivalent) |
| Scheduling budget and sleep grant (`SleepScheduler`) | `tpt-anima-ck` `BudgetManager`, `CircuitBreaker` |
| Identity and attestation | `tpt-anima-zkpa` `AttestationService` (see gap below) |

The Thought-side global workspace is in-process (`tpt-thought-workspace`). The shared workspace is
Anima's concern and sits behind that crate's `GlobalWorkspace` trait.

## Gaps in Anima

- No `retrieve_context` function.
- No episode persistence API. Episodic records are defined on the Thought side only.
- No global workspace type. The closest is the INP broadcaster.
- No ZKPA identity type. Only proofs, attestation and delegation exist.
- Storage is in-memory only. `InMemoryStorage` is the only store.

## Runtime concerns

- `tpt-anima-zkpa` depends on tokio with full features, and its async proof path is async. **It
  must not be used.**
- Co-processor, core-hypervisor, edge-gateway, edge-runtime and observatory also depend on tokio.
- `tpt-anima-sse`, `tpt-anima-inp` and `tpt-anima-ck` are synchronous and free of tokio.
