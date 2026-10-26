# tpt-thought-providers

The provider-neutral intelligence layer: a trait that any answering source implements, a registry
that discovers and ranks providers under request constraints, calibration of their track records,
cognition modes that decide when to ask more, ensemble disagreement, and a human provider.

> **Status:** pre-release (v0.1.1). This crate is distributed from git only and is not published
> to crates.io. See `docs/DEPENDENCY_POLICY.md` in the repository.

## What it does

- **Asks through one trait.** `IntelligenceProvider` answers an `IntelligenceRequest` with a
  `Decision`. A provider may be a local model, a remote service, a rule engine, a verifier or a
  person. Abstaining is an `Ok` result, while `Err(ProviderError)` means no decision at all.
- **Discovers and ranks.** `ProviderRegistry` filters providers by the request's `Constraints`
  and ranks the survivors by reliability, then cost, then latency, then identity, so the order is
  deterministic.
- **Calibrates.** `CalibrationTracker` records what happened after a provider made a claim. It
  maps raw confidence to calibrated values through binned hit rates (`BINS`, default 10), and
  reports empirical reliability and the Brier score. Values stay `None` until a provider has
  `DEFAULT_MIN_SAMPLES` outcomes.
- **Chooses how hard to think.** `CognitionMode` is a pluggable strategy. `FastPath` accepts the
  first answer that clears `accept_at`. `Deliberative` asks providers, up to an optional limit, and
  combines their answers.
- **Treats disagreement as an event.** `combine` returns `Consensus::Agreed` or
  `Consensus::Disagreed(Disagreement)`, and a disagreement lists the `Probe`s that should
  investigate it (ask why, surface assumptions, compare evidence, check calibration, look for a
  missing variable).
- **Includes people.** `HumanProvider` wraps a `HumanResponder` supplied by the host application.
  Its decisions keep the person's identity, timestamp, reasoning and stated confidence.

## What it deliberately does not do

- It does not name a concrete backend. Core and the cognitive modes depend only on the trait, so
  Intellect, Eve, Augur or a remote model are adapters in `tpt-thought-integrations`.
- It does not pick the final answer when several providers disagree. It reports the positions and
  the probes, and leaves the choice to the caller or to a deeper investigation.
- It does not own a user interface. The host application implements `HumanResponder`.

## Main types

| Item | Role |
|---|---|
| `IntelligenceProvider` | The trait every answering source implements: `id`, `profile`, `decide`. |
| `IntelligenceRequest` | The question, `Constraints`, context labels and time. |
| `ProviderProfile` | Capabilities, modalities, accuracy, latency, cost, confidence range, locality, privacy and verification. |
| `Constraints` | Minimum confidence, risk tolerance, maximum cost and latency, acceptable providers, required capabilities and required verification. |
| `Decision<T>` | `answer` (`Answer::Answered` or `Answer::Abstained`), confidence, uncertainty, source, provenance and decision time. |
| `ProviderError` and `ProviderErrorKind` | `Unavailable`, `Timeout`, `Refused(String)` or `Other(String)`. |
| `ProviderRegistry` and `Candidate` | Registration, lookup and constraint-filtered, ranked candidates. `register` refuses duplicate IDs. |
| `CalibrationTracker` | Outcome records, calibrated and empirical reliability, and Brier score. |
| `CognitionMode`, `Step`, `Escalation` | The mode trait, its result (accept or escalate) and the reason for escalating. |
| `FastPath`, `Deliberative` | The two built-in modes. |
| `combine`, `Consensus`, `Disagreement`, `Position`, `Probe` | Ensemble agreement and disagreement. |
| `HumanProvider`, `HumanResponder`, `HumanAnswer` | A person as a provider. |
| `ReferenceProvider`, `FailingProvider` | Deterministic providers for tests and examples. |

## Module layout

- `provider.rs`: the `IntelligenceProvider` trait.
- `types.rs`: `ProviderId`, `Capability`, `ProviderProfile`, `Constraints` and `IntelligenceRequest`.
- `registry.rs`: `ProviderRegistry` and `Candidate`.
- `decision.rs`: `Decision`, `Answer`, `DecisionSource`, `ProviderError`.
- `calibration.rs`: `CalibrationTracker`.
- `mode.rs`: `CognitionMode`, `FastPath`, `Deliberative`.
- `ensemble.rs`: `combine` and the disagreement types.
- `human.rs`: `HumanProvider` and `HumanResponder`.
- `reference.rs`: `ReferenceProvider` and `FailingProvider`.

## Usage

```rust
use tpt_thought_core::{Probability, StructuredValue, Timestamp};
use tpt_thought_providers::{
    CalibrationTracker, CognitionMode, FastPath, IntelligenceRequest, Locality, Privacy,
    ProviderProfile, ProviderRegistry, ReferenceProvider, Step, DEFAULT_MIN_SAMPLES,
};

fn ask() -> Result<(), Box<dyn std::error::Error>> {
    let profile = ProviderProfile::new(Locality::Local, Privacy::OnDevice);
    let provider = ReferenceProvider::new("reference", profile).with_answer(
        "is the drawer locked?",
        StructuredValue::Bool(true),
        Probability::new(0.9)?,
    );

    let mut registry = ProviderRegistry::new();
    registry.register(provider)?;

    let calibration = CalibrationTracker::new(DEFAULT_MIN_SAMPLES);
    let request = IntelligenceRequest::new("is the drawer locked?", Timestamp(0));

    match FastPath::default().respond(&request, &registry, &calibration) {
        Step::Accept(decision) => { /* use the decision */ }
        Step::Escalate(escalation) => { /* ask a deeper mode or a human */ }
    }
    Ok(())
}
```

`ReferenceProvider` is a fixed-answer provider for tests and examples. Production providers
implement `IntelligenceProvider` directly and are registered the same way.

## Design notes

- Abstention is a result, not an error. An abstained decision is never counted as a hit or a miss.
- Unknown profile values never exclude a provider. A provider is excluded only when a known value
  breaks a constraint.
- Calibration only learns from answered decisions with a raw confidence. Human sources are recorded
  under `human:<identity>`.
- `ProviderRegistry::candidates` exposes the ranking inputs so that a selector such as Intellect
  can make the final choice with full information.

## Related crates

- `tpt-thought-core`: `Probability`, `Confidence`, `Uncertainty` and `StructuredValue`.
- `tpt-thought-deliberation`: the deeper engine trait, used when a mode escalates.
- `tpt-thought-hypothesis`: `EvidenceSummary` is built partly from provider confidence.
- `tpt-thought-integrations`: adapters that implement `IntelligenceProvider` for sibling
  repositories.
- `tpt-thought-loop`: invokes modes during the slow path.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0, at your option.
