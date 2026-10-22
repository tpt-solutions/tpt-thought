# Integration: Eve

**Status: implemented in part** behind the `eve` feature of `tpt-thought-integrations`. The
sibling dependencies are `tpt-eve` crates pinned at revision `c230d0e83633bd0a7f8d3e6a5d830f48236427ad`
(`tpt-eve-core`, `tpt-eve-metacognition`, `tpt-eve-causal`, `tpt-eve-abstraction`,
`tpt-eve-active-learning`, all 0.1.0).

## Mapping

`EveAdapter` implements Thought's `DeliberationEngine`. Each capability maps onto one Eve layer.
Facts, observations and causal relations are supplied through `assert_fact`, `observe`, `relate`,
`fit` and `add_example` before deliberation.

| Thought operation | Eve layer | Status |
|---|---|---|
| `reflect` (metacognition) | `MetaCognitionEngine::detect_gaps`, `calibration_error` | implemented |
| `generate_hypotheses` | `ActiveLearningEngine::generate_hypotheses` | implemented |
| `design_experiment` | `ActiveLearningEngine::design_experiment` | implemented; `estimated_cost` is 0.0 because Eve does not estimate cost |
| `analyse_causation` (effect estimate) | `CausalReasoningEngine::estimate_causal_effect` | implemented |
| `analyse_causation` (intervention, counterfactual) | `predict_intervention`, `simulate_counterfactual` | unsupported; needs Eve's intervention and counterfactual request types |
| `validate_abstraction` | `AbstractionEngine::mine_principles` | implemented; analogy targets are noted but not validated |
| `analyse_contradictions` | none in `tpt-eve` that is tokio-free | unsupported |
| `rank_queries` | none | unsupported |
| `reason`, `investigate` | no deliberation entry point in `tpt-eve` | unsupported |

## Blocked

- `tpt-eve-symbolic` (contradiction analysis) was not used. It depends on `redb`, and no
  licence or runtime check of that dependency has been done for this feature.
- `tpt-eve-neural` pulls in tokio and async-trait, so it is excluded.
- `tpt-eve` has no deliberation entry point, so `reason` and `investigate` have no Eve source.

## Tests

The tests exercise the real Eve layers: reflection on asserted facts, a causal effect recovered
from 20 linear observations (expected about 2.0), unsupported intervention and contradiction
requests, experiment design from a generated hypothesis, and rejection of unknown hypotheses.
