# Integration: Anima

**Status: implemented in part** in `tpt-thought-integrations`, behind the `anima` feature. The
sibling is `tpt-anima`, pinned at `408c2e0065ba99e619fa7277d3801e0c9abad0ff`. The adapter uses
`tpt-anima-sse`, `tpt-anima-inp` and `tpt-anima-ck`, all 0.1.0.

## Mapping

| Thought | Anima | Adapter |
|---|---|---|
| `MemoryStore`: IDs and tags only, no duplicated storage (§10) | `tpt-anima-sse` `InMemoryStorage` | `AnimaMemory` |
| Memory ranking | tag overlap with the cue, then identifier, so the order is deterministic | `AnimaMemory::retrieve` |
| Reference resolution | only references into this store, with canonical lowercase hex ids, resolve | `AnimaMemory::text_of` |
| Compute-budget signal (§35) | `tpt-anima-ck` `BudgetManager` | `AnimaBudget` |
| Provider discovery for an intention (§12, §39) | `tpt-anima-inp` `IntentBroadcaster` over a `CapabilityRegistry` | `AnimaBroadcast` |

`AnimaBudget` refuses non-finite and negative costs, so they hold nothing. A release only frees
flops that were admitted on that compute unit, and reports whether it did.

Anima's ids are 32-byte arrays in the memory layer and UUIDs in the broadcast layer. The adapter
keeps the two apart and does not convert between them.

## Partial mapping

`AnimaBroadcast` covers provider discovery only. The workspace's publish, subscribe, withdraw and
request operations have no counterpart in `tpt-anima`, so they are not mapped.

## Not provided by `tpt-anima`

These are not invented here:

- `retrieve_context`;
- episode persistence (§14, §48.2);
- a global workspace type;
- a ZKPA identity type. `tpt-anima-zkpa` pulls in tokio, so it is excluded.

## Blocked

- Thought's `ThoughtBubble` is not connected to Anima's `thought_bubble` module.
- CK scheduling of sleep (§35) is connected through `AnimaBudget`, which implements
  `SleepScheduler` from the consolidation crate using CK's thermal throttle factor. CK provides
  no memory-pressure or energy signal, so those values still come from the caller.

## Tests

The tests use the real SSE store, the real CK budget manager, and the real INP broadcaster and
capability registry. They cover memory ranking, reference resolution in other stores and in
non-canonical forms, budget admission and release, refusal of invalid costs, and provider
discovery.
