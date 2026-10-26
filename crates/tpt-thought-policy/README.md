# tpt-thought-policy

Escalation, branching, resource and termination policies for TPT Thought.

`tpt-thought-policy` decides what a thought should do next: accept the current answer, escalate
to a deeper level of cognition, or stop. It does not perform cognition itself. Every policy is a
pure function of a `PolicyContext`, so the same inputs always produce the same decision and a run
can be replayed.

> **Status:** pre-release, `v0.1.1`. Distributed from git only, not published on crates.io.

## What it does

- **Escalation ladder.** `EscalationLevel` names the architectural levels `L0` (reflex) to `L6`
  (human). `EscalationLadder` is the ordered subset of levels a deployment actually has, so a
  system without a probabilistic provider can skip `L3`.
- **Decisions.** `Decision` is `Accept`, `Escalate(level)` or `Terminate(termination)`.
  `Decision::combine` merges decisions from several policies: termination overrides everything
  (the first termination wins), otherwise the higher escalation wins.
- **Policies.** The `Policy` trait takes a `PolicyContext` and returns a `Decision`. Any
  `Fn(&PolicyContext) -> Decision` is a policy. The built-in policies are:
  - `EscalationGate`: escalates while confidence is below `accept_confidence`, unless the expected
    value of further cognition is at or below `value_floor`. Stops at the top of the ladder.
  - `RiskCeiling`: escalates to `L6` (human) when risk is above `max_risk`, unless already there.
  - `HumanEscalation`: escalates to `L6` once `failed_escalations` reaches the configured count.
  - `BranchPolicy`: terminates when more branches are open than `max_branches` allows.
  - `BudgetGuard`: terminates when the usage in the context exceeds its budget, naming the
    exhausted `Dimension`.
- **Policy sets.** `PolicySet` holds any number of policies and returns their combined decision.
- **Expected value of cognition.** `CognitionValue` holds the probability that more cognition
  improves the answer, the gain if it does, and its cost. `expected_value()` is
  `p_improve * gain - cost`. Inputs must be valid, or construction fails with
  `InvalidCognitionValue`.
- **Resource budgets.** `ResourceBudget` sets limits over six `Dimension`s: `Time`, `Compute`,
  `Energy`, `Money`, `Memory` and `Branches`. `ResourceUsage` is the spend so far. `charge` adds a
  delta and returns `BudgetExhausted` if the result would exceed the budget, without changing
  the usage.
- **Configuration.** `EscalationConfig` holds the tunable thresholds (`accept_confidence`,
  `value_floor`, `max_risk`, `human_after_failed_escalations`, `max_branches` and `ladder`). It
  implements `Serialize` and `Deserialize`, and `build()` turns it into a `PolicySet`. Missing
  fields take their defaults.

## What it deliberately does not do

- It does not run providers, investigations or deliberation. The cognitive loop in
  `tpt-thought-loop` asks these policies what to do and then does it.
- It does not measure confidence, risk or cost. Callers supply those in the `PolicyContext`.
- It does not map levels to biological claims. The levels are architectural.

## Usage

```rust
use tpt_thought_policy::{
    CognitionValue, Decision, EscalationConfig, EscalationLevel, Policy, PolicyContext,
    ResourceBudget, ResourceUsage,
};

let policies = EscalationConfig::default().build()?;

let ctx = PolicyContext {
    level: EscalationLevel::Reflex,
    confidence: 0.4,
    risk: 0.1,
    branches: 1,
    failed_escalations: 0,
    value: Some(CognitionValue::new(0.6, 1.0, 0.1)?),
    usage: ResourceUsage::default(),
    budget: ResourceBudget::default(),
};

match policies.decide(&ctx) {
    Decision::Accept => { /* use the current answer */ }
    Decision::Escalate(level) => { /* run cognition at `level` */ }
    Decision::Terminate(reason) => { /* stop without an answer */ }
}
```

Every field of `PolicyContext` is read by at least one built-in policy, so fill them all from the
cognitive loop's current state.

## Module layout

| Module | Contents |
|---|---|
| `budget` | `Dimension`, `ResourceBudget`, `ResourceUsage`, `BudgetExhausted` |
| `config` | `EscalationConfig` |
| `decision` | `Decision`, `Termination`, `Policy`, `PolicyContext`, `PolicySet` |
| `level` | `EscalationLevel`, `EscalationLadder`, `LadderError` |
| `policies` | `EscalationGate`, `RiskCeiling`, `HumanEscalation`, `BranchPolicy`, `BudgetGuard` |
| `value` | `CognitionValue`, `InvalidCognitionValue` |

## Design notes

- Policies are data plus a `decide` method, so they can be stored, serialised and compared.
- `Decision::combine` is the only rule for merging policies. Termination is final, so a single
  policy can stop cognition even when others would escalate.
- A ladder must be non-empty and strictly ascending. `EscalationLadder::new` rejects anything
  else with `LadderError`, and `EscalationConfig::build` returns the same error.

## Related crates

- `tpt-thought-loop` calls these policies from the cognitive cycle.
- `tpt-thought-providers` supplies the cognition modes that escalation selects between.
- `tpt-thought-safety` provides the circuit breakers that stop runaway escalation independently.

## Installation

```toml
[dependencies]
tpt-thought-policy = { git = "https://github.com/TPT-Solutions/tpt-thought" }
```

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0 at your option.
