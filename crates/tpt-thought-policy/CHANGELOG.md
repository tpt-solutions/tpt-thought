# Changelog

All notable changes to `tpt-thought-policy` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project uses semantic versioning.

## [Unreleased]

### Added

- Escalation levels `L0` to `L6` (`EscalationLevel`) and a validated, strictly ascending
  `EscalationLadder` that lets a deployment skip levels it does not have.
- `Decision` (`Accept`, `Escalate`, `Terminate`) with `combine`: termination overrides everything,
  otherwise the higher escalation wins.
- `Policy` trait, `PolicyContext` and `PolicySet`. Any `Fn(&PolicyContext) -> Decision` is a
  policy.
- Built-in policies: `EscalationGate`, `RiskCeiling`, `HumanEscalation`, `BranchPolicy` and
  `BudgetGuard`.
- `CognitionValue`: expected value of further cognition from the probability of improvement, the
  gain and the cost, with validated construction.
- `ResourceBudget`, `ResourceUsage`, `Dimension` and `BudgetExhausted` for budgets over time,
  compute, energy, money, memory and branches.
- `EscalationConfig`: serialisable thresholds with defaults and `build()` for the standard policy
  set.
