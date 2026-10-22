# Integration: Intellect

**Status: implemented** behind the `intellect` feature of `tpt-thought-integrations`. The
sibling dependency is `tpt-intellect` pinned at revision `a502c0139a7e5cc12b7adf8fb9bdbbae31f69ffc`
(crates `tpt-intellect-core` and `tpt-intellect-backend`, both 0.1.0).

## Mapping

| Thought | Intellect |
|---|---|
| `IntelligenceProvider` | `IntelligenceBackend<String>` (wrapped by `IntellectProvider`) |
| `IntelligenceRequest` question | `IntelligenceRequest` with `StructuredInput` text and `DecisionKind::Choice` |
| Candidate answers | `Candidate<String>` fixed when the provider is built |
| `Constraints::max_latency_nanos` | `RequestConstraints::max_latency` |
| `Decision` answer (choice) | `DecisionOutcome::Choice`, answered with `StructuredValue::Text` |
| Abstention | `DecisionOutcome::Abstain`, recorded as an abstained `Decision` |
| Confidence | `Confidence` raw value, from the sibling's `Confidence::value()` |
| Provider identity | `BackendId` |
| Backend errors | `Unavailable`, `Timeout`, `UnsupportedRequest` and other variants map to `ProviderErrorKind` |
| Capability discovery | `BackendCapabilities::supports(DecisionKind::Choice)` via `supports_choice()` |

Sibling confidence is recorded as raw. Calibrated and empirical values come from Thought's own
`CalibrationTracker`, not from Intellect.

## Not mapped

- Outcomes other than choice and abstention (binary, score, ranking and so on) produce an
  abstention whose reason names the outcome. They are not converted to answers.
- Candidate lists are fixed at construction. Thought questions do not carry candidates yet.
- Required capabilities in `Constraints` are not forwarded to the sibling.
- The remote, batch and stream crates are not used.

## Tests

`reference_backend_answers_with_one_of_the_candidates` and
`capability_discovery_reports_choice_support` exercise the real `ReferenceBackend` from
`tpt-intellect-backend`, not a mock.
