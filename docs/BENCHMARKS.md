# BENCHMARKS

The benchmark harness is `crates/tpt-thought-tests/benches/pipeline.rs`. Run it with:

```
cargo bench -p tpt-thought-tests
```

It uses `harness = false` and `std::time::Instant`, with no criterion dependency. It drives the
closed loop (`tpt-thought-loop`) for 500 cycles with a deterministic seed.

## What the numbers mean

The counted metrics come from a synthetic, deterministic scenario. They show that the loop
behaves as specified on that scenario and that the bounds hold. They are not evidence of
decision quality on real-world data. Timings are machine-dependent and informational only.

## Results (last run)

| Metric (§96) | Result |
|---|---|
| Decision quality | 500/500 correct actions |
| Prediction accuracy | 500/500 exact matches |
| Compute cost | 1.00 provider calls per cycle |
| Unnecessary escalations | 0 of 500 reliable cases |
| Missed escalations | 0 of 500 unreliable cases |
| Memory utilisation | 100% of relevant memories recalled |
| Branch explosion | 8 admitted of 200 offered; 8 active (bound 8) |
| Replay trajectory | 5500 steps, reproduced exactly |
| Time to decision | about 13.5 µs per cycle (informational) |

## Not yet measured

- Calibration error on real provider outputs (needs recorded outcomes from a real provider).
- Prediction error reduction over time (needs a learning scenario, not a fixed one).
- Energy cost (no energy accounting exists in the core budget).
- Time to decision against real sibling backends (the integrations crate is on hold).
