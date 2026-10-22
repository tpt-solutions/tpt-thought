# Integration: Intellect

**Status: not yet implemented.** The adapter belongs in `tpt-thought-integrations`, which is on
hold. Adding it requires a decision about depending on sibling-repository code, which has not been
made.

## Planned mapping

| Thought | Intellect |
|---|---|
| `IntelligenceProvider` | `IntelligenceBackend` |
| `IntelligenceRequest` and `Constraints` | `IntelligenceRequest`, `ConfidenceRequirement` |
| `Decision` (answer or abstention) | `Decision<T>`, `DecisionOutcome`, `AbstentionReason` |
| Provider profile and discovery | `BackendInfo`, `BackendCapabilities`, `capability_check` |
| Raw, calibrated and empirical confidence | `ConfidenceSemantics`, `CalibrationReport` |
| Provenance | `DecisionProvenance`, `ProvenanceRecorder` |
| Escalation and abstention policy | `DecisionPolicy`, `PolicyChain`, `Cascade` |

## What exists in Intellect

The surface is synchronous and has no async runtime. It is the most coherent sibling API. Relevant
items: `IntelligenceBackend` with `decide`, `BackendInfo`, `BackendCapabilities`,
`DecisionPolicy`, `CalibrationReport`, `BudgetLedger`, `verify_conformance`, and `ReferenceBackend`.

## Dependency concerns

- `tpt-intellect-core` has an optional git dependency on `tpt-axiom-core`.
- `tpt-intellect-remote` has git dependencies on `tpt-infer` and `tpt-infer-ops`. Their licences
  have not been checked.
- The remote crate must not be used if it brings in an async runtime.
