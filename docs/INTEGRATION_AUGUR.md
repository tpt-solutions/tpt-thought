# Integration: Augur

**Status: implemented.** In `tpt-thought-integrations`, behind the `augur` feature. The sibling is
`tpt-augur`, pinned at `c29ecd1781f9f2b8d04b971329340c44349fcbfb`, which adds evidence sessions and
residual uncertainty to `tpt-augur-runtime`. The adapter uses `tpt-augur-frontend`, `tpt-augur-ir`
and `tpt-augur-runtime`.

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

## Evidence sessions

`AugurSession::new(source, options)` holds a model. `observe(statement)` appends one `observe`
statement, such as `observe Normal(mu, 1) = 0.5`, after checking that the extended model parses and
lowers. A rejected statement leaves the session unchanged. `infer()` re-runs the whole model from
source. Seeds are fixed, so the same evidence gives the same posterior. Each re-run is from source,
not an incremental update, so the cost grows with the number of observations.

## Possible states

Spec section 49 asks what the possible states are. For a discrete parameter, `PosteriorRecord::states`
lists each state seen in the posterior draws, with its probability, most probable first. The
probabilities sum to one. A continuous parameter has no states (`None`). A parameter with a
non-integer draw, or with more than `MAX_STATES` (64) distinct values, is treated as continuous.
A value that was never drawn is not listed, so a very improbable state can be missing.

Augur's `run` returns only summaries, so the adapter runs the same engine dispatch itself to keep
the draws. A test checks that its summaries agree with `run` for the same seed.

## Residual uncertainty

`tpt-augur-runtime::residual_of` gives a `Residual` per parameter: the posterior `sd`, the 95%
interval width, `sd / |mean|` (or `None` at mean zero), and `converged`, which is true when R-hat is
at most 1.1 and the effective sample size is at least 100.

`augur::uncertainty_of` maps a residual to Thought's `Uncertainty`. The `sd` becomes `epistemic`,
since it is reducible uncertainty from lack of knowledge. A posterior that has not converged is
marked `unknown`: its summary cannot be trusted. Augur reports no confidence or probability, so
neither is set.

## Tests

A normal-mean model is run through the real Augur pipeline, and the posterior mean is checked
against the closed-form value.
