# Integration: Anima

**Status: implemented in part** in `tpt-thought-integrations`, behind the `anima` feature. The
sibling is `tpt-anima`, pinned at `5e9da51b4e13b79204bcfeeabaf915cb224d7bdf`. The adapter uses
`tpt-anima-sse`, `tpt-anima-inp` and `tpt-anima-ck`, all 0.1.0.

## Mapping

| Thought | Anima | Adapter |
|---|---|---|
| `MemoryStore`: IDs and tags only, no duplicated storage (§10) | `tpt-anima-sse` `InMemoryStorage` | `AnimaMemory` |
| Memory ranking by tag | tag overlap with the cue, then identifier, so the order is deterministic | `AnimaMemory::retrieve` |
| Memory ranking by meaning (§10) | cosine similarity over the SSE approximate nearest-neighbour index | `AnimaMemory::recall_similar` |
| Reference resolution | only references into this store, with canonical lowercase hex ids, resolve | `AnimaMemory::text_of` |
| Compute-budget signal (§35) | `tpt-anima-ck` `BudgetManager` | `AnimaBudget` |
| Compute, energy and memory state for sleep (§48.3) | CK's flops and power headroom, plus a caller-supplied memory pressure | `AnimaBudget::resource_snapshot` |
| Intention negotiation (§23, §39) | `tpt-anima-inp` `IntentBroadcaster` over a `CapabilityRegistry`: providers found, bids collected, winner selected | `AnimaBroadcast::negotiate` |
| Bubble execution boundary (§42, §48.4) | `tpt-anima-ck` `ThoughtBubbleManager`: open, quarantine, teardown | `AnimaBubbles` |

`AnimaMemory::remember` takes the caller's embedding. Thought has no embedder (§111), so the caller
supplies one. The store fixes the embedding length from its first memory and refuses other lengths,
as well as empty, non-finite and all-zero embeddings. The SSE index accepts only one length, and a
zero vector has no direction to rank by, so admitting either would corrupt semantic recall.

`AnimaBudget` refuses non-finite and negative costs, so they hold nothing. A release only frees
flops that were admitted on that compute unit, and reports whether it did.

Anima's ids are 32-byte arrays in the memory layer and UUIDs in the broadcast layer. The adapter
keeps the two apart and does not convert between them.

## Partial mapping

`AnimaBroadcast::negotiate` covers the bid round for an intention. An offer from a provider not
registered for the action is ignored, and a round is always closed, so no round is left open.

`AnimaBroadcast::open_intention` goes further. It runs the bid round through INP's
`NegotiationSession`, so the winner is executing and `AnimaIntention` tracks the lifecycle. The
provider does the work, and Thought records what it reports: `Completed` or `PartiallyCompleted`
completes the intention, `Failed`, `TimedOut` and `Cancelled` end it in their own states, and an
intention accepts one outcome. Fallback strategies are not used, because no intention is created
with any.

The workspace's publish, subscribe, withdraw and request operations have no counterpart in
`tpt-anima`, so they are not mapped.

`AnimaBudget::resource_snapshot` takes memory pressure from the caller because CK reports no memory
figures. Thermal state is applied through the sleep grant rather than the snapshot.

## Not provided by `tpt-anima`

These are not invented here:

- `retrieve_context`. The same role is covered by `recall_similar` over `semantic_search`;
- a global workspace type.

Episode persistence (§14, §48.2) was missing and is now provided by `tpt-anima-sse`'s `EpisodeStore`.
It stores opaque payloads under a kind tag, and `AnimaEpisodes` encodes Thought episodes as JSON.

ZKPA is used with `default-features = false`, which removes the tokio runtime that `async_proofs`
needs. The `zkpa` feature wraps sessions and delegation in `AnimaIdentity`. Attestation, proofs and
the agency ledger are not mapped.

## Blocked

- Episode persistence cannot be mapped until `tpt-anima-sse` has an episode type.

## Tests

The tests use the real SSE store, the real CK budget manager, and the real INP broadcaster and
capability registry. They cover tag and semantic ranking, refusal of degenerate and mismatched
embeddings, reference resolution in other stores and in non-canonical forms, budget admission and
release, refusal of invalid costs, sleep grants, the resource snapshot, provider discovery, and
intention negotiation with unregistered offers and empty rounds.
