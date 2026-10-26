# tpt-thought-safety

Cognitive safety for TPT Thought: circuit breakers, capability sandboxing, loop detection,
resource limits and provenance integrity checks.

`tpt-thought-safety` answers one question for each step of cognition: may this proceed? It does
not run cognition, execute actions or talk to a backend. Callers ask the types here whether a step
is allowed, and a refusal comes back as an error that names the reason. The threats each module
answers are listed in `docs/THREAT_MODEL.md`.

> **Status:** pre-release, `v0.1.1`. Distributed from git only, not published on crates.io.

## What it does

- **Circuit breakers.** `CircuitBreakers` watches six failure modes and trips when a threshold is
  crossed. A tripped breaker stays open until a caller resets it, so a runaway process cannot
  recover quietly. Each mode is a `BreakerKind`:
  - `Repetition`: a step fingerprint recurs too often in a window (`record_step`).
  - `Explosion`: too many thoughts or hypotheses are alive (`observe_population`).
  - `RecursiveEscalation`: escalation climbs past the allowed level (`observe_escalation`).
  - `ConfidenceOscillation`: confidence keeps changing direction (`observe_confidence`).
  - `ContradictionLoop`: the same contradiction is raised without resolution
    (`record_contradiction` and `resolve_contradiction`).
  - `ResourceExhaustion`: accumulated compute passes its limit (`charge`).

  Each check returns `Result<(), Trip>`. A `Trip` names the breaker and the observed values.
  `check`, `is_open`, `trip`, `trips`, `reset` and `reset_all` inspect and clear breakers.
- **Capability sandbox.** `Sandbox` grants capabilities explicitly. Everything is denied by
  default and matching is exact. `check`, `authorize` and `authorize_intention` return
  `CapabilityDenied` for anything not granted. An intention is authorised only if every required
  capability is granted.
- **Resource limits.** `ResourceLimits` bounds spawn depth, children per thought, total compute
  and duration. `Sandbox::check_usage` compares a `Usage` snapshot against them and returns a
  `LimitViolation` naming the first limit exceeded: `Depth`, `Children`, `Compute` or `Duration`.
- **Loop detection.** `find_parent_cycle` takes a map from thought to parent and returns the cycle
  if one exists, so a thought cannot be its own ancestor.
- **Integrity.**
  - `verify_provenance` verifies the whole provenance store and checks that every record a thought
    cites exists and explains that thought. It catches tampered records, dangling links and false
    provenance, as `IntegrityFault`.
  - `ObservationGate` refuses observations whose `SourceTrust` is below a minimum (`TrustRejected`).
  - `ReplayGuard<K>` admits each key once and refuses repeats, so a captured message cannot be
    replayed.
  - `admit_actor` checks that an actor may act on a thought given its `ThoughtOwnership`. Only the
    owner may act, a delegate acts alone, and archived thoughts admit no one. A mismatch returns
    `IdentityMismatch`.

## What it deliberately does not do

- It does not decide what to do after a trip. The caller chooses how to halt, escalate or ask for
  help.
- It does not resolve identities. `admit_actor` compares identifiers it is given. Identity
  resolution belongs to Anima ZKPA.
- It does not verify cryptographic signatures. Hash-chain verification comes from
  `tpt-thought-provenance`.
- It does not schedule compute or enforce wall-clock time itself. It checks the figures callers
  report.

## Usage

```rust
use tpt_thought_safety::{BreakerConfig, CircuitBreakers, ResourceLimits, Sandbox};

let mut breakers = CircuitBreakers::new(BreakerConfig::default());
breakers.record_step(fingerprint)?;

let mut sandbox = Sandbox::new(ResourceLimits::default());
sandbox.grant("sensor.camera");
sandbox.check("sensor.camera")?;
sandbox.check("actuator.arm")?; // Err(CapabilityDenied), never granted
```

`record_step` and the other observers return `Trip` as their error type, so `?` stops the caller
at the first tripped breaker.

## Module layout

| Module | Contents |
|---|---|
| `breaker` | `BreakerKind`, `Trip`, `BreakerConfig`, `CircuitBreakers` |
| `sandbox` | `Sandbox`, `ResourceLimits`, `Usage`, `LimitViolation`, `CapabilityDenied`, `find_parent_cycle` |
| `integrity` | `verify_provenance`, `ObservationGate`, `ReplayGuard`, `admit_actor`, `IntegrityFault`, `TrustRejected`, `IdentityMismatch` |

`SourcePolicy` and `SourceTrust` are re-exported from `tpt-thought-core`, so callers can configure
trust without a second import.

## Design notes

- Breakers are latched. Reset is an explicit call, which makes every trip visible in the caller's
  logs.
- `BreakerConfig` is serialisable with defaults. Thresholds include a window of 16 steps for
  repetition, a population of 64, an escalation level of 4 and a compute budget of 1,000.
- Capabilities are opt-in per sandbox. A new `Sandbox` can do nothing until it is granted something.
- `ReplayGuard` keeps every key it has admitted, so memory grows with the number of distinct keys.
  Bound the keys a long-lived process admits if that matters.

## Related crates

- `tpt-thought-provenance` supplies the hash-linked store that `verify_provenance` checks.
- `tpt-thought-policy` makes the escalation decisions that breakers constrain.
- `tpt-thought-core` supplies `Intention`, `Thought`, `SourceTrust` and `ThoughtOwnership`.

## Installation

```toml
[dependencies]
tpt-thought-safety = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
