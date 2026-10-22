# Providers

Thought asks questions of intelligence through provider traits. It never depends on a concrete
backend. The crate is `tpt-thought-providers`.

## Traits and types

- **`IntelligenceProvider`**: has an id, a `ProviderProfile` and a `decide` method that takes an
  `IntelligenceRequest` and returns a `Decision`.
- **`IntelligenceRequest`**: the question, plus `Constraints`: required confidence, maximum cost,
  maximum latency, risk tolerance, acceptable providers and required capabilities.
- **`Decision`**: an answer or an abstention, with three confidence kinds (raw, calibrated and
  empirical), the source, provenance, context and timestamp. Abstention is a first-class outcome.
- **`ProviderError`**: a failure to answer. A failure is not an abstention.

## Registry and selection

`ProviderRegistry` holds providers. `candidates` filters them by the request's constraints, then
ranks by reliability, cost, latency and id, in that order. Ties are broken by id, so the ranking
is deterministic.

## Calibration

`CalibrationTracker` records each provider's outcomes against what it claimed. It reports:

- calibrated confidence, binned by claimed confidence;
- empirical reliability;
- a Brier score.

Calibrated and empirical values are withheld until there are enough samples. A provider with no
track record is not presented as reliable.

## Cognition modes

`CognitionMode` decides how providers are used. Modes are pluggable. Nothing is hard-coded as
"System 1" or "System 2".

- **`FastPath`** accepts the first answer that meets the required confidence and never asks the
  remaining providers.
- **`Deliberative`** asks the candidates and combines their answers.

`combine` returns one of three results: `Agreed`, `Disagreed` (with the positions and five probes
into why: ask why, assumptions, evidence, calibration, missing variable), or `Insufficient`.
Disagreement is a cognitive event. It is recorded, not discarded.

## Human provider

`HumanProvider` answers with identity, timestamp, context and confidence, and returns a `Decision`
whose source is the human. It is an ordinary provider, so the cognitive model does not special-case
it. The escalation level L6 uses it.

## Reference and test providers

- `ReferenceProvider` answers from a fixed table. It needs no model, so tests are deterministic.
- `FailingProvider` always fails. It exercises failover.

## Adding a backend

1. Implement `IntelligenceProvider` for the new backend.
2. Register it in a `ProviderRegistry` with an accurate `ProviderProfile`.
3. Nothing else changes. The modes and calibration see only the trait.

A backend that cannot answer returns `ProviderError` or abstains. It gets no special handling in
the core.

## Sibling adapters

Adapters to Intellect, Eve, Augur and Axiom are planned for `tpt-thought-integrations`. That crate
is on hold. See `INTEGRATION_INTELLECT.md`.
