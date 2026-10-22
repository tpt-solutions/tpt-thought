# Integration: Eve

**Status: implemented in part** in `tpt-thought-integrations`, behind the `eve` feature. The
sibling is `tpt-eve`, pinned at `c230d0e83633bd0a7f8d3e6a5d830f48236427ad`. The adapter uses the
`tpt-eve-core`, `tpt-eve-metacognition`, `tpt-eve-causal`, `tpt-eve-abstraction` and
`tpt-eve-active-learning` crates, all 0.1.0.

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
| `analyse_contradictions` | `tpt-eve-symbolic` | unsupported; see Blocked |
| `rank_queries` | none in `tpt-eve` | unsupported |
| `reason`, `investigate` | none; `tpt-eve` has no deliberation entry point | unsupported |

## Blocked

- **Contradiction analysis.** `tpt-eve-symbolic` is free of tokio, but it depends on `dirs`, which
  pulls in `option-ext`. That crate is MPL-2.0, which the licence policy in `deny.toml` does not
  allow. The adapter reports `Unsupported` until Eve drops or replaces that dependency.
- `tpt-eve-neural` depends on tokio and async-trait, so it is excluded.
- `tpt-eve` has no deliberation entry point, so `reason` and `investigate` have no Eve source.

## Tests

The tests drive the real Eve layers:

- reflection on asserted facts;
- a causal effect recovered from 20 linear observations (about 2.0);
- an intervention that changes the predicted effect, a counterfactual, and rejected inputs;
- experiment design from a generated hypothesis, and rejection of unknown hypotheses;
- regeneration that does not duplicate hypotheses, and rejection of a cost cap;
- contradiction analysis reporting `Unsupported`.

The acceptance test for criterion F runs the real Eve adapter through the loop. Eve does not
implement `investigate`, so the test checks that the slow path skips Eve. It does not show Eve
deliberating.
