# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The crate uses semantic versioning.
Repository-wide changes are recorded in the root `CHANGELOG.md`.

## [Unreleased]

### Added

- Provider-neutral `IntelligenceProvider` trait, with requests, constraints and provider profiles
  (`IntelligenceRequest`, `Constraints`, `ProviderProfile`, `ProviderId`, `Capability`).
- `ProviderRegistry` that filters providers by constraints and ranks them deterministically, with
  duplicate registration refused (`ProviderRegistry`, `Candidate`, `RegistryError`).
- Decisions that either answer or abstain, with confidence, uncertainty, source and provenance
  (`Decision`, `Answer`, `DecisionSource`, `ProviderError`, `ProviderErrorKind`).
- `CalibrationTracker` with binned calibration, empirical reliability and Brier score.
- Cognition modes: the `CognitionMode` trait, `FastPath`, `Deliberative`, `Step` and `Escalation`.
- Ensemble agreement and disagreement, with investigation probes (`combine`, `Consensus`,
  `Disagreement`, `Position`, `Probe`).
- `HumanProvider` and `HumanResponder` for human-in-the-loop answers.
- Deterministic `ReferenceProvider` and `FailingProvider` for tests and examples.
