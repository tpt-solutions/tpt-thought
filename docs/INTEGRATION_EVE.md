# Integration: Eve

**Status: not yet implemented.** The adapter belongs in `tpt-thought-integrations`, which is on
hold. Adding it requires a decision about depending on sibling-repository code, which has not been
made.

## Planned mapping

Eve implements Thought's `DeliberationEngine` (in `tpt-thought-deliberation`). Thought's operations
map to Eve layers:

| Thought operation | Eve layer |
|---|---|
| Metacognition: certainty, gaps, assumptions, missing evidence | `MetaCognitionEngine` (`tpt-eve-metacognition`) |
| Hypothesis generation | `ActiveLearningEngine::generate_hypotheses` (`tpt-eve-active-learning`) |
| Experiment design | `ActiveLearningEngine::design_experiment` |
| Causal reasoning: interventions, counterfactuals, effect estimation | `CausalReasoningEngine` (`tpt-eve-causal`) |
| Abstraction and analogy validation | `AbstractionEngine` (`tpt-eve-abstraction`) |
| Contradiction analysis | `SymbolicReasoningLayer::check_consistency` (`tpt-eve-symbolic`), if it is free of async runtimes |

## Gaps in Eve

- No deliberation entry point exists. The word appears only in prose.
- `tpt-eve-neural` declares tokio and async-trait as normal dependencies, and its JSON-RPC code uses
  tokio networking. **It must not be used.**
- `tpt-eve-symbolic` relies on `redb` and has an unfinished backtracking stub.

## Dependency concerns

Only the non-neural layers are candidates. Each needs its runtime and licence checked before use.
