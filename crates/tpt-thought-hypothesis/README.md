# tpt-thought-hypothesis

Branching hypotheses, their lifecycle, collapse to a single choice, contradictions between them,
expected information gain, and the evidence-gated pipeline by which a claim may become a belief.

> **Status:** pre-release (v0.1.0). This crate is distributed from git only and is not published
> to crates.io. See `docs/DEPENDENCY_POLICY.md` in the repository.

## What it does

- **Branches.** A `BranchSet` holds the live alternative explanations for one question. It bounds
  how many can be active at once, which is the guard against branch explosion. Branches can be
  explored from a parent, suspended, rejected, promoted, pruned or merged.
- **Lifecycle.** `HypothesisStatus` moves through a fixed transition table (`EDGES`). The same
  `(from, to)` pair always has the same answer.
- **Collapse.** `collapse` chooses one surviving branch by a named `CollapseStrategy`: probability,
  a caller-supplied utility, evidence count, formal verification, ensemble agreement, or an
  explicit human choice. A tie is an error, not a silent pick.
- **Contradictions.** A `ContradictionLedger` records conflicts between hypotheses, raises their
  attention, and resolves each one by new evidence, by separating the contexts in which each side
  holds, or by making one side conditionally valid.
- **Information gain.** `shannon_bits`, `expected_information_gain` and `should_investigate` decide
  whether testing a set of hypotheses is worth its cost.
- **Belief pipeline.** A `Claim` moves one stage at a time through `Thought`, `Hypothesis`,
  `SupportedHypothesis`, `BeliefCandidate` and `Belief`, and demotes one stage at a time. Every
  stage change is recorded in its history.

## What it deliberately does not do

- It does not make a hypothesis a belief by considering it. Promotion is sequential and gated by
  evidence, so there is no path from `Thought` to `Belief` that skips the intermediate stages.
- It does not gather evidence or call providers. The caller builds an `EvidenceSummary` from
  observations and providers and passes it in.
- It does not run experiments. Experiment design is an engine capability in
  `tpt-thought-deliberation`.

## Main types

| Item | Role |
|---|---|
| `Branch` | One alternative: its `Hypothesis`, status, parent, evidence, cost, risk, expected information gain and verification level. |
| `BranchSet` | Live branches for one question, bounded by `max_active`. Methods include `explore`, `suspend`, `reject`, `promote`, `prune`, `merge` and `record_test`. |
| `HypothesisStatus` | `Generated`, `Untested`, `Testing`, `Supported`, `Weakened`, `Updated`, `Accepted`, `Rejected` and `Suspended`. |
| `CollapseStrategy` | How to choose the surviving branch. |
| `Contradiction` and `ContradictionLedger` | Conflicts between two hypotheses, with attention and resolution. |
| `Resolution` | `NewEvidence`, `ContextSeparation` or `ConditionalValidity`. |
| `Claim` | A statement moving through the belief pipeline. `promote`, `demote`, `stage`, `history`, `is_belief`. |
| `PromotionPolicy` | A set of `Criterion` values, of which `required` must hold. |
| `Criterion` | `RepeatedObservation`, `Proof`, `Experiment`, `HighConfidenceIntelligence`, `ModelAgreement` or `HumanConfirmation`. |
| `EvidenceSummary` | What is known about a claim, gathered by the caller. |

## Module layout

- `belief.rs`: the claim pipeline, criteria, promotion policy and evidence summary.
- `branch.rs`: branches and the bounded branch set.
- `collapse.rs`: choosing one branch by a named strategy.
- `contradiction.rs`: the contradiction ledger and its resolutions.
- `information.rs`: entropy and expected information gain, in bits.
- `lifecycle.rs`: the hypothesis status table.

## Usage

```rust
use tpt_thought_hypothesis::{
    Claim, ClaimStage, Criterion, EvidenceSummary, PromotionPolicy,
};

let policy = PromotionPolicy::new(
    vec![
        Criterion::RepeatedObservation { min: 3 },
        Criterion::HumanConfirmation,
    ],
    1,
)?;

let mut claim = Claim::new("the drawer is locked");
let summary = EvidenceSummary { confirming_observations: 1, ..Default::default() };

claim.promote(&EvidenceSummary::default(), &policy, "considered")?;     // Thought -> Hypothesis
claim.promote(&summary, &policy, "first observation")?;                // -> SupportedHypothesis
assert_eq!(claim.stage(), ClaimStage::SupportedHypothesis);
```

Promotion is refused when the evidence does not justify the next stage. A claim that is already a
belief cannot be promoted further.

## Design notes

- `PromotionPolicy::new` rejects a policy that requires zero criteria (it would believe everything)
  and one that requires more criteria than it supplies (it could never be met).
- `Claim::demote` returns `None` when the claim is already a thought. Each demotion keeps its note
  in the history, so provenance survives promotion and demotion.
- Collapse refuses to choose when branches tie. The error carries the tied IDs, so the caller can
  gather more evidence or ask a human.
- `EpistemicStatus` on a branch records verification (for example, formal proof) separately from
  probability, so a verified branch is not treated as merely probable.

## Related crates

- `tpt-thought-core`: `Hypothesis`, `Probability` and `EpistemicStatus`.
- `tpt-thought-graph`: stores the hypotheses and contradictions as nodes and edges.
- `tpt-thought-providers`: supplies the intelligence evidence that informs `EvidenceSummary`.
- `tpt-thought-deliberation`: investigation and experiment design for contested branches.
- `tpt-thought-consolidation`: re-evaluates beliefs during sleep.

## Minimum supported Rust version

1.85.

## Licence

Licensed under either of MIT or Apache-2.0, at your option.
