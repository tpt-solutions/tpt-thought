# Integration: Axiom

**Status: implemented in part** in `tpt-thought-integrations`, behind the `axiom` feature. The
sibling is `tpt-axiom`, pinned at `42c495f7751db5acc3c571bb890da41a153f75ba`. The adapter uses
`tpt-axiom-core` (value types), `tpt-axiom-ir` (constraint systems) and `tpt-axiom-verify`
(polynomial checks). None of them depends on an async runtime.

The pinned commit moved the cargo-generate template out of the crate tree. The template's
placeholder manifest had broken cargo's scan of the repository, so git dependencies on `tpt-axiom`
could not resolve before that commit.

## Mapping

| Axiom | Thought | Function |
|---|---|---|
| `Probability` | `Probability` (same value, validated to 0..=1) | `probability` |
| `Confidence` | raw `Confidence` | `confidence` |
| `Fuzzy<f64>` variance | `Uncertainty::epistemic` | `uncertainty` |
| `ConstraintSystem` constraint, checked against `lhs <cmp> rhs` | `EpistemicStatus::Verified` | `verify_comparison` |

Axiom records no calibration provenance on its `Confidence`, so that confidence is recorded as raw.
Axiom validates probabilities, confidence and fuzzy variance when they are built, so the adapter
needs no separate checks. A `Fuzzy` estimate's variance is the uncertainty in its mean, so it is
recorded as epistemic uncertainty. That reading is a design choice, and it is documented here.

## Epistemic status (spec §50)

- Values from Axiom's probabilistic types are `Probabilistic`. The adapter returns no status for
  them, because the value types carry no status of their own.
- `verify_comparison` checks that constraint `i` of a system encodes `lhs >= rhs`, `lhs > rhs`,
  `lhs <= rhs` or `lhs < rhs`, using `tpt_axiom_verify::check_comparison`. A passing check gives
  `Verified`. A failing check returns `ConstraintError::Mismatch` with both polynomials, so the
  caller can see the disagreement. A missing constraint or expression is also an error.
- `verify_proven` gives `Proven` when a `ProofClaim` verifies. It takes the backend and the verifying
  key from the caller, never from the claim, and rejects a claim made against a different circuit
  (the claim's IR digest must match). A rejected proof is `ProofError::Rejected`, and a backend
  failure is `ProofError::Backend`. Thought ships no trusted key and selects no backend, so
  `Proven` is reachable only when a caller supplies both.

## Not mapped

- Axiom's `decision_types` (`Calibration`, `Decision`, `Hypotheses`). They are not yet adapted.
## Tests

The tests cover:

- probabilities keeping their value;
- Axiom rejecting out-of-range probabilities and confidence before adaptation;
- confidence being recorded as raw;
- fuzzy variance becoming epistemic uncertainty;
- a fuzzy interval bracketing its mean;
- a constraint that encodes the claimed comparison being `Verified`;
- a constraint that encodes a different comparison being a mismatch;
- a missing constraint and a missing expression being reported.
