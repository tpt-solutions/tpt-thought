# Cognitive model

## Thought lifecycle

`Created -> Received -> Attending -> Contextualising -> Associating -> Hypothesising ->
Evaluating -> (Accepted | Uncertain)`. `Uncertain` loops through `Investigating` and
`ReEvaluating` until accepted. `Accepted -> Intending -> Acting -> Observing -> Learning ->
Consolidating -> Complete`.

Loops are allowed: `Evaluating/Uncertain -> Hypothesising`, `ReEvaluating -> Investigating`,
and `Intending/Observing -> Evaluating` (reconsider after a changed intention or outcome).
The full table is `tpt_thought_state::TRANSITIONS`; transitions are pure functions of
`(state, event)`.

## Cognitive cycle

One pass of the closed loop (`tpt-thought-loop`) runs eleven phases in order:

observe, contextualise, associate, hypothesise, evaluate, decide, intend, act, observe the
result, learn, consolidate.

Each phase is recorded in a trajectory. Acting goes through an `ActionSink` supplied by the
caller. The loop never performs an action itself.

## Modes and modulation

Cognitive modes: Reactive, Deliberative, Investigative, Creative, Reflective, Planning, Learning,
Consolidation, Emergency.

The mode is selected from a modulatory state: urgency, risk, novelty, reward, threat,
confidence, uncertainty and arousal. Each field is validated to the range 0 to 1.

## Escalation

Escalation levels, lowest first:

| Level | Name | Meaning |
|---|---|---|
| L0 | Reflex | Answer without deliberation. |
| L1 | Structured intelligence | A structured provider answer. |
| L2 | Specialist intelligence | A domain specialist answer. |
| L3 | Probabilistic inference | Inference with explicit probabilities. |
| L4 | Deliberate reasoning | Engine reasoning, such as Eve. |
| L5 | Multi-hypothesis investigation | Competing hypotheses investigated in turn. |
| L6 | Human | Escalate to a human provider. |

When the fast or deliberative path does not accept its answer, the loop asks its policy set what
to do. The policy decides and the loop executes:

- `Escalate(level)` runs the slow stages that level allows. Intellect always runs. Eve
  (investigation) needs L4 or higher, and Augur (the estimate) needs L3 or higher.
- `Accept` ends the cycle without further cognition, and the cycle is reported as unresolved.
- `Terminate` ends the cycle the same way, with the termination reason in the report.

The default policy escalates to L5 on every rejected answer, which is the full slow path. The
policy's context carries the best confidence among the rejected answers and the level. Risk, branch
count and value are zero or absent at this point, because they are not known until a plan is made.
`Termination` takes precedence over escalation in the policy crate's `Decision::combine`.

## Beliefs

Observation, hypothesis, belief, prediction, intention, action and outcome are separate types.

**A thought is not a belief merely because it was considered.** A claim moves through these
stages, one per step:

`Thought -> Hypothesis -> SupportedHypothesis -> BeliefCandidate -> Belief`

Each step is gated by evidence. Promotion is recorded in the claim's history, and demotion
steps back one stage at a time. Provenance is carried through every step.

## Prediction errors

A prediction is matched against a later observation. The error is classified and mapped to a
response:

| Error | Response |
|---|---|
| Small | Continue. |
| Moderate | Update the belief. |
| Large | Raise attention and investigate. |
| Catastrophic | Suspend the model and re-evaluate assumptions. |

Responses are not cumulative. A large error raises attention and investigates; it does not also
update belief. Thresholds are configurable.

## Distinctions kept explicit

- Confidence is raw, calibrated or empirical reliability, never one number.
- Uncertainty separates confidence, probability, epistemic, aleatoric, unknown, contradiction
  and abstention. Unknown is not zero.
- Epistemic status: informal < probabilistic < verified < proven.
- Transitions are tagged deterministic or probabilistic to support replay.

## Provenance

Every significant step can emit a `ProvenanceRecord`, hash-linked to its parents. `explain`
returns the full derivation of a thought without any generated explanation.

A captured replay record carries the provenance of each step. `replay` verifies the hashes of that
chain before it runs anything, then compares the re-run's provenance with the recorded chain field
by field. A record without provenance replays on its trajectory alone, so the provenance checks
cannot be proven present on such a record.

## Reflection, sleep and learning

- **Online thought** is latency-bound and makes no durable belief change.
- **Reflection** is a bounded review of past thoughts, with structured artifacts only. It never
  stores or exposes private chain-of-thought.
- **Sleep** is an interruptible consolidation cycle. High-priority events wake it. Anima decides
  when sleep is granted, and owns memory decay. Thought owns relevance.
