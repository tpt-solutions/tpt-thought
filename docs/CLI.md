# The `thought` command line

The `tpt-thought-cli` crate builds a binary called `thought`. It records a cognitive run to a JSON
file, re-runs it, checks it, and explains one of its decisions.

```
cargo run -p tpt-thought-cli -- <command> [arguments]
```

## Commands

| Command | What it does |
|---|---|
| `thought demo [--out RECORD.json]` | Runs the reference scenario and records it. Without `--out`, the record goes to stdout. |
| `thought run INPUT.json [--out RECORD.json]` | Runs the inputs in the file, in order, and prints each outcome. With `--out`, also records the run. |
| `thought replay RECORD.json` | Re-runs the record's inputs and checks every step against the record. |
| `thought validate RECORD.json` | Checks the record's structure and that its configuration is valid. Does not replay. |
| `thought why RECORD.json CYCLE` | Re-runs the record and prints the provenance of one cycle's thought. `CYCLE` is zero-based. |
| `thought schema` | Prints the wire version and an example input file. |

Exit codes: `0` on success, `1` when a replay diverges from its record, and `2` for a usage,
file or parse error.

## Input files

`thought run` reads a configuration and a list of inputs. `thought schema` prints a valid example:

```json
{
  "config": { "seed": 1, "...": "..." },
  "inputs": [
    {
      "at": 1000,
      "source": "camera",
      "observation": { "Text": "cup within reach" },
      "question": "can the gripper grasp the cup?",
      "goal": "pick up the cup",
      "constraints": [],
      "memories": [],
      "candidates": [{ "name": "grasp", "predicted_outcome": { "Text": "cup held" }, "cost": 1.0, "risk": 0.1 }]
    }
  ]
}
```

The `config` object is the full `LoopConfig`, so `thought schema` is the reliable source for its
shape.

## Records

A record holds the configuration, the metadata, the inputs and the trajectory the run produced.
`thought replay` compares a fresh run with the trajectory step for step. A record that has been
edited shows a divergence at the first changed step:

```
$ thought replay tampered.json
replay diverged at step 3
```

## The reference setup

Every command uses one fixed setup: a single reference provider named `intellect`, which answers
the question "can the gripper grasp the cup?" with 95% reliability, and a simulated executor that
reports the expected outcome. A replay can only be checked against the setup that produced it, so
the CLI does not load custom providers.

To run your own providers or executors, build the loop in Rust. See `docs/COOKBOOK.md` and the
examples in `crates/tpt-thought/examples/`.

## Sample session

```
$ thought demo --out record.json
record written to record.json
$ thought replay record.json
reproduced 1 cycle(s) of 1 input(s)
$ thought why record.json 0
cycle 0: completed `grasp` (prediction error 0.00, Small)
  Decision at Timestamp(1000), provider intellect, path mode:deliberative
  Inference at Timestamp(1000), provider none, path prediction error 0.000
```
