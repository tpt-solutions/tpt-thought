# Integration: Augur

**Status: implemented in part** in `tpt-thought-integrations`, behind the `augur` feature. The
sibling is `tpt-augur`, pinned at `634f67a5ed632f9bba082cafeeb921c22dc18195`. The adapter uses
`tpt-augur-frontend`, `tpt-augur-ir` and `tpt-augur-runtime`, all 0.1.0.

## Mapping

`AugurAdapter::new(options)` takes Augur's inference options. `infer(source)` runs the whole chain:
it parses the Augur model, lowers it, then runs inference. It returns one `PosteriorRecord` per
parameter, with:

| Field | Source |
|---|---|
| `name`, `mean` | the parameter's name and posterior mean |
| `sd` | the posterior standard deviation: the residual uncertainty of the estimate |
| `interval95` | the central 95% credible interval |
| `rhat` | split-chain potential scale reduction; values near 1.0 mean the chains agree |
| `ess` | effective sample size |

`PosteriorRecord::interval_width` gives the width of the 95% interval.

## Not provided by `tpt-augur`

- No evidence-update API. This adapter cannot feed new observations into an existing posterior:
  each call to `infer` runs from the model source.
- No residual-uncertainty API. The residual values are derived here from the posterior summaries.

## Tests

A normal-mean model is run through the real Augur pipeline, and the posterior mean is checked
against the closed-form value.
