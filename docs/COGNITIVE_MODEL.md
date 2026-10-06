# Cognitive model (first draft)

## Thought lifecycle

`Created -> Received -> Attending -> Contextualising -> Associating -> Hypothesising ->
Evaluating -> (Accepted | Uncertain)`. `Uncertain` loops through `Investigating` and
`ReEvaluating` until accepted. `Accepted -> Intending -> Acting -> Observing -> Learning ->
Consolidating -> Complete`.

Loops are allowed: `Evaluating/Uncertain -> Hypothesising`, `ReEvaluating -> Investigating`,
and `Intending/Observing -> Evaluating` (reconsider after a changed intention or outcome).
The full table is `tpt_thought_state::TRANSITIONS`; transitions are pure functions of
`(state, event)`.

## Distinctions kept explicit

- Observation, hypothesis, belief, prediction, intention, action and outcome are separate types.
  A thought is not a belief merely because it was considered.
- Confidence is raw, calibrated or empirical reliability, never one number.
- Uncertainty separates confidence, probability, epistemic, aleatoric, unknown, contradiction
  and abstention. Unknown is not zero.
- Epistemic status: informal < probabilistic < verified < proven.
- Transitions are tagged deterministic or probabilistic to support replay.

## Provenance

Every significant step can emit a `ProvenanceRecord`, hash-linked to its parents. `explain`
returns the full derivation of a thought without any generated explanation.
