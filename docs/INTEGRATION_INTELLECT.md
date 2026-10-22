# Integration: Intellect

**Status: implemented** in `tpt-thought-integrations`, in three features:

- `intellect`: `IntellectProvider`, which wraps any Intellect backend over string candidates.
  `reference_provider` builds one on Intellect's reference backend.
- `remote`: `remote::loopback` and `remote::over_transport`, which put a remote backend behind the
  same provider.
- `local`: `local::LocalClassifier`, which answers through the `tpt-infer` classifier bridge.

The sibling is `tpt-intellect`, pinned at `a502c0139a7e5cc12b7adf8fb9bdbbae31f69ffc`. The `remote`
feature uses its `tpt-intellect-remote` crate with default features. The `local` feature enables
that crate's `infer` feature, which pulls in `tpt-infer`. `tpt-infer` has no pin in its own
manifest, so `Cargo.lock` is the only pin for it.

## Mapping (`intellect`)

| Thought | Intellect |
|---|---|
| `IntelligenceProvider` | `IntelligenceBackend<String>`, wrapped by `IntellectProvider` |
| `IntelligenceRequest` question | `IntelligenceRequest` with `StructuredInput` text and `DecisionKind::Choice` |
| Candidate answers | `Candidate<String>`, fixed when the provider is built |
| `Constraints::max_latency_nanos` | `RequestConstraints::max_latency` |
| Answer | `DecisionOutcome::Choice`, returned as `StructuredValue::Text` |
| Abstention | `DecisionOutcome::Abstain`, recorded as an abstained `Decision` |
| Confidence | the sibling's confidence value, recorded as raw |
| Provider identity | the backend's `BackendId` |
| Backend errors | `Unavailable`, `Timeout` and `UnsupportedRequest` map to the matching `ProviderErrorKind`; other errors map to `Other` |
| Capability discovery | `supports_choice()`, from `BackendCapabilities::supports(DecisionKind::Choice)` |

A provider with an empty candidate list returns a configuration error rather than sending an empty
choice to the backend.

Sibling confidence is recorded as raw. Calibrated and empirical values come from Thought's
`CalibrationTracker`, not from Intellect.

## Remote and local

- `remote::loopback` serves the reference backend over the sibling's in-process protocol transport.
  `remote::over_transport` accepts any transport the caller supplies. The sibling's subprocess
  transport is not wrapped, because it needs a child process. Thought adds no HTTP client.
- `local::LocalClassifier` wraps the sibling's classification bridge. The bridge takes numeric
  features, so the caller supplies a function from question text to features. The labels are fixed
  when the graph is built, and a question is answered with one of them.

## Not mapped

- Outcomes other than choice and abstention (binary, score, ranking and so on) produce an
  abstention whose reason names the outcome. They are not converted to answers.
- Candidate lists are fixed at construction. Thought questions do not carry candidates.
- Required capabilities in `Constraints` are not forwarded to the sibling.
- The batch and stream crates are not used.
- Ollama and `tpt-spark` adapters are not built. Neither appears in the sibling survey.

## Tests

- `intellect`: the reference backend answers with one of its candidates, and capability discovery
  reports choice support. Both use the real `ReferenceBackend`, not a mock.
- `remote`: a loopback round trip through the real protocol transport, and a failed connection.
- `local`: a two-class graph built in code, so no model file is needed, and a clear error for a
  wrong input width.
- The acceptance test for criterion D runs the real Intellect provider through the loop, and
  checks that the provider was consulted.
