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
needs. The `zkpa` feature wraps the following in `AnimaIdentity`:

- sessions and delegation (§40);
- attestation evidence through `AttestationService`, and its check into provenance. The check is
  structural only: it verifies a 32-byte digest and the verified flag, and does not recompute the
  digest or check a platform signature;
- Groth16 authorization proofs over BN254, verified with `ProofGenerator::verify`. The check uses the
  verification key embedded in the proof, so it shows internal consistency, not a trusted setup. The
  proof is not bound to a particular agent.

Ownership states and the agency ledger have no counterpart in the pinned `tpt-anima-zkpa`, so they
are not mapped. Delegation grants permissions, not responsibility, so it is not treated as
`ThoughtOwnership::Delegated`.

## Resource admission and homeostasis

`AnimaBudget` implements `ResourceGate`, so a cycle run with `CognitiveLoop::run_gated` asks CK
before any provider is consulted. A denied cycle returns `CycleOutcome::Deferred` with CK's reason,
runs nothing and holds nothing. Only the estimated cost reaches CK. Priority, deadline and risk are
carried in the requirement but not used, because CK's budget manager has no path for them here.

Resource strain raises arousal only, and never lowers it. Urgency, risk, uncertainty and the rest
still come from the thought. Uncertain thoughts still investigate under pressure.

## Consolidation persistence

`AnimaConsolidation` persists consolidated knowledge and history summaries to SSE through
`CognitiveLoop::persist_consolidation`. Each item is stored once, under a content-addressed id, so a
repeated consolidation of the same result stores nothing. Thought keeps only the references. Episodes,
confidence bins, the next-session state and working state stay where they are: episodes are already in
SSE, and the rest are re-derived or Thought-owned.

## Action execution through INP

`AnimaActionSink` implements the loop's `ActionSink`. Each action is offered to the providers
registered for it, INP picks the winner, and the winner's executor runs the action. The loop never
sees INP. INP does not run the work, does not enforce QoS deadlines, and does not use fallbacks.

## Multi-agent thought sharing

`share::ThoughtBroadcast` sends a shared thought to each offered peer over INP, one peer at a time.
Each peer evaluates it with its own providers, and the replies are merged by `providers::sharing`.
The merge rule and the transport are documented in those modules.

## Blocked

- None in the Anima mapping. Ownership states and the agency ledger are not in the pinned sibling.

## Tests

The tests use the real SSE store, the real CK budget manager, and the real INP broadcaster and
capability registry. They cover tag and semantic ranking, refusal of degenerate and mismatched
embeddings, reference resolution in other stores and in non-canonical forms, budget admission and
release, refusal of invalid costs, sleep grants, the resource snapshot, provider discovery, and
intention negotiation with unregistered offers and empty rounds.
