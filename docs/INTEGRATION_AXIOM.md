# Integration: Axiom

**Status: implemented in part** in `tpt-thought-integrations`, behind the `axiom` feature. The
sibling is `tpt-axiom`, pinned at `42c495f7751db5acc3c571bb890da41a153f75ba`. The adapter uses only
`tpt-axiom-core`, which is free of async runtimes.

The pinned commit moved the cargo-generate template out of the crate tree. The template's
placeholder manifest had broken cargo's scan of the repository, so git dependencies on `tpt-axiom`
could not resolve before that commit.

## Mapping

| Axiom | Thought | Function |
|---|---|---|
| `Probability` | `Probability` (same value, validated to 0..=1) | `probability` |
| `Confidence` | raw `Confidence` | `confidence` |
| `Fuzzy<f64>` variance | `Uncertainty::epistemic` | `uncertainty` |

Axiom records no calibration provenance, so its confidence is recorded as raw. Axiom validates
probabilities, confidence and fuzzy variance when they are built, so the adapter needs no separate
checks. A `Fuzzy` estimate's variance is the uncertainty in its mean, so it is recorded as
epistemic uncertainty. That reading is a design choice, and it is documented here.

## Not mapped

- Formal constraints.
- Verification markers, such as ZK proof envelopes from `tpt-axiom-zk`. The adapter does not claim
  a proof it has not checked.

## Tests

The tests cover:

- probabilities keeping their value;
- Axiom rejecting out-of-range probabilities and confidence before adaptation;
- confidence being recorded as raw;
- fuzzy variance becoming epistemic uncertainty;
- a fuzzy interval bracketing its mean.
