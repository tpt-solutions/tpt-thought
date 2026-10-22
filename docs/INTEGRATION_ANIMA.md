# Integration: Anima

**Status: implemented in part** behind the `anima` feature of `tpt-thought-integrations`. The
sibling dependencies are `tpt-anima` crates pinned at revision `408c2e0065ba99e619fa7277d3801e0c9abad0ff`
(`tpt-anima-sse`, `tpt-anima-inp`, `tpt-anima-ck`, all 0.1.0).

## Mapping

| Thought | Anima | Adapter |
|---|---|---|
| `MemoryStore` (IDs and tags only, no duplicated storage, §10) | `tpt-anima-sse` `InMemoryStorage`, `retrieve_by_tag` | `AnimaMemory` |
| Memory ranking | tag overlap with the cue, then identifier for determinism | `AnimaMemory::retrieve` |
| Compute-budget signal (§35) | `tpt-anima-ck` `BudgetManager::allocate_flops`, `release_flops`, `thermal_throttle_factor` | `AnimaBudget` |
| Provider discovery for an intention (§12, §39) | `tpt-anima-inp` `IntentBroadcaster::begin_broadcast` over a `CapabilityRegistry` | `AnimaBroadcast` |

Anima's ids are 32-byte arrays in the memory layer and UUIDs in the broadcast layer. The adapter
keeps the two apart and does not convert between them.

## Partial mapping

`AnimaBroadcast` covers provider discovery only. The workspace's publish, subscribe, withdraw and
request operations have no counterpart in `tpt-anima`, so they are not mapped.

## Not provided by `tpt-anima`

These are not invented here:

- `retrieve_context`
- episode persistence (§14, §48.2)
- a global workspace type
- a ZKPA identity type (`tpt-anima-zkpa` pulls in tokio, so it is excluded)

## Blocked

- Thought's `ThoughtBubble` is not connected to Anima's `thought_bubble` module.
- CK scheduling of sleep (§35) is not connected. Thought's `SleepScheduler` trait has no Anima
  implementation.

## Tests

The tests use the real SSE store and CK budget manager, and the real INP broadcaster and capability
registry. Memory ranking, budget admission and release, and provider discovery are each covered.
