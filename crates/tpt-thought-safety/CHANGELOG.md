# Changelog

All notable changes to `tpt-thought-safety` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- `CircuitBreakers` with six latched breakers: repetition, explosion, recursive escalation,
  confidence oscillation, contradiction loop and resource exhaustion. A tripped breaker stays open
  until reset.
- `BreakerConfig` with serialisable thresholds and defaults, and `Trip` values that name the
  observed figures.
- `Sandbox` capability grants, denied by default, with `authorize_intention` for intentions that
  need several capabilities.
- `ResourceLimits`, `Usage` and `LimitViolation` for spawn depth, children, compute and duration.
- `find_parent_cycle` for detecting cycles in parent links.
- `verify_provenance` and `IntegrityFault`: whole-store verification plus a check that every cited
  record explains the thought.
- `ObservationGate` and `TrustRejected`: observations below a minimum source trust are refused.
- `ReplayGuard<K>`: each key is admitted once.
- `admit_actor` and `IdentityMismatch`: actors may act only on thoughts they own or have been
  delegated.
- Re-exports of `SourcePolicy` and `SourceTrust` from `tpt-thought-core`.
