# Integration: Eve

**Status: implemented in part** in `tpt-thought-integrations`, behind the `eve` feature. The
sibling is `tpt-eve`, pinned at `db6e8df72344dc820bcf5fac5b9aaff8e438ef48`. The adapter uses the
`tpt-eve-core`, `tpt-eve-metacognition`, `tpt-eve-causal`, `tpt-eve-abstraction`,
`tpt-eve-active-learning` and `tpt-eve-symbolic` crates, all 0.1.0.

## Mapping

`EveAdapter` implements Thought's `DeliberationEngine`. Each capability maps onto one Eve layer.
Facts, observations and causal relations are supplied through `assert_fact`, `observe`, `relate`,
`fit` and `add_example` before deliberation.

| Thought operation | Eve layer | Status |
|---|---|---|
| `reflect` (metacognition) | `MetaCognitionEngine::detect_gaps`, `calibration_error` | implemented |
| `generate_hypotheses` | `ActiveLearningEngine::generate_hypotheses` | implemented; regenerating over the same facts reuses existing hypothesis ids |
| `design_experiment` | `ActiveLearningEngine::design_experiment` | implemented; a request with a `max_cost` cap is refused, because Eve does not estimate cost |
| `analyse_causation` (effect estimate) | `CausalReasoningEngine::estimate_causal_effect` | implemented |
| `analyse_causation` (intervention) | `CausalReasoningEngine::predict_intervention` | implemented; the intervention is `variable=value`, and the baseline is the root variables of the most recent observation |
| `analyse_causation` (counterfactual) | `CausalReasoningEngine::simulate_counterfactual` | implemented; the baseline is the most recent observation, and the effect is read from the counterfactual world |
| `validate_abstraction` | `AbstractionEngine::mine_principles` | implemented; principles with partial support are reported as `Conditional`; analogy targets are noted but not validated |
| `analyse_contradictions` | `tpt-eve-symbolic` `ConsistencyChecker` | implemented; each claim must be `subject predicate object`, with the object as the rest of the statement. Only `is` and `was` are exclusive, so only they conflict, and identical repeats do not |
| `rank_queries` | none in `tpt-eve` | unsupported |
| `reason`, `investigate` | none; `tpt-eve` has no deliberation entry point | unsupported |

## Blocked

- `tpt-eve-neural` depends on tokio and async-trait, so it is excluded.
- `tpt-eve` has no deliberation entry point, so `reason` and `investigate` have no Eve source.

## Tests

The tests drive the real Eve layers:

- reflection on asserted facts;
- a causal effect recovered from 20 linear observations (about 2.0);
- an intervention that changes the predicted effect, a counterfactual, and rejected inputs;
- experiment design from a generated hypothesis, and rejection of unknown hypotheses;
- regeneration that does not duplicate hypotheses, and rejection of a cost cap;
- contradiction analysis: a conflict between two `is` claims, no conflict for identical repeats or
  for non-exclusive predicates, and a malformed claim refused rather than skipped.

`tpt-eve-symbolic` used to depend on `dirs`, which pulled in `option-ext` (MPL-2.0). `tpt-eve`
commit `db6e8df` removes that dependency, so the licence policy is met.

The acceptance test for criterion F runs the real Eve adapter through the loop. Eve does not
implement `investigate`, so the test checks that the slow path skips Eve. It does not show Eve
deliberating.
