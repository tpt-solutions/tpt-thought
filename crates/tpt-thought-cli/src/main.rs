// SPDX-License-Identifier: MIT OR Apache-2.0
//! `thought`: record, re-run and inspect cognitive runs from the command line.
//!
//! ```text
//! thought demo [--out RECORD.json]          run the reference scenario and record it
//! thought run INPUT.json [--out RECORD.json] run inputs (a config and a list of inputs)
//! thought replay RECORD.json                re-run a record and check it step for step
//! thought validate RECORD.json              check a record's structure and configuration
//! thought why RECORD.json CYCLE             explain the provenance of one cycle's thought
//! thought schema                            print the wire version and an input skeleton
//! ```
//!
//! Every command uses the reference setup in [`mod@reference`]. Exit codes: 0 on success, 1 when a
//! replay diverges from its record, and 2 for usage, I/O and parse errors.

mod reference;

use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fs;
use std::process::ExitCode;
use tpt_thought::cognitive_loop::{
    replay, CycleOutcome, CycleReport, Input, LoopConfig, ReplayError, ReplayRecord,
};
use tpt_thought::core::wire::WIRE_VERSION;

const USAGE: &str = "usage:
  thought demo [--out RECORD.json]
  thought run INPUT.json [--out RECORD.json]
  thought replay RECORD.json
  thought validate RECORD.json
  thought why RECORD.json CYCLE
  thought schema";

/// The file `run` reads: the configuration and the inputs, in order.
#[derive(Debug, Serialize, Deserialize)]
struct RunFile {
    config: LoopConfig,
    inputs: Vec<Input>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn dispatch(args: &[String]) -> Result<ExitCode, Box<dyn Error>> {
    let (command, rest) = match args.split_first() {
        Some(split) => split,
        None => return Err("no command given".into()),
    };
    match command.as_str() {
        "demo" => demo(rest),
        "run" => run(rest),
        "replay" => replay_command(rest),
        "validate" => validate(rest),
        "why" => why(rest),
        "schema" => schema(),
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown command `{other}`").into()),
    }
}

/// Splits `args` into positional arguments and the value of an optional `--out` flag.
type Positional<'a> = (Vec<&'a str>, Option<&'a str>);

fn parse_out(args: &[String]) -> Result<Positional<'_>, Box<dyn Error>> {
    let mut positional = Vec::new();
    let mut out = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--out" {
            let value = iter.next().ok_or("--out needs a file name")?;
            out = Some(value.as_str());
        } else if arg.starts_with("--") {
            return Err(format!("unknown flag `{arg}`").into());
        } else {
            positional.push(arg.as_str());
        }
    }
    Ok((positional, out))
}

fn write_record(record: &ReplayRecord, out: Option<&str>) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string_pretty(record)?;
    match out {
        Some(path) => {
            fs::write(path, json)?;
            println!("record written to {path}");
        }
        None => println!("{json}"),
    }
    Ok(())
}

fn read_record(path: &str) -> Result<ReplayRecord, Box<dyn Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn demo(args: &[String]) -> Result<ExitCode, Box<dyn Error>> {
    let (positional, out) = parse_out(args)?;
    if !positional.is_empty() {
        return Err("demo takes no positional arguments".into());
    }
    let record = ReplayRecord::capture(
        reference::default_config(),
        reference::metadata(),
        vec![reference::grasp_input(1_000)],
        reference::build,
    )?;
    if out.is_none() {
        println!("recorded one grasp decision");
    }
    write_record(&record, out)?;
    Ok(ExitCode::SUCCESS)
}

fn run(args: &[String]) -> Result<ExitCode, Box<dyn Error>> {
    let (positional, out) = parse_out(args)?;
    let [input_path] = positional.as_slice() else {
        return Err("run takes exactly one input file".into());
    };
    let file: RunFile = serde_json::from_str(&fs::read_to_string(input_path)?)?;
    let mut lp = reference::build(&file.config)?;
    for (number, input) in file.inputs.iter().enumerate() {
        let report = lp.run(input)?;
        println!("input {number}: {}", describe(&report));
    }
    let record = ReplayRecord::capture(
        file.config,
        reference::metadata(),
        file.inputs,
        reference::build,
    )?;
    if out.is_some() {
        write_record(&record, out)?;
    }
    Ok(ExitCode::SUCCESS)
}

fn describe(report: &CycleReport) -> String {
    match &report.outcome {
        CycleOutcome::Completed {
            action,
            error,
            level,
        } => {
            format!("completed `{action}` (prediction error {error:.2}, {level:?})")
        }
        CycleOutcome::Unresolved { reason } => format!("unresolved: {reason}"),
    }
}

fn replay_command(args: &[String]) -> Result<ExitCode, Box<dyn Error>> {
    let (positional, _) = parse_out(args)?;
    let [path] = positional.as_slice() else {
        return Err("replay takes exactly one record file".into());
    };
    let record = read_record(path)?;
    match replay(&record, reference::build) {
        Ok(reports) => {
            println!(
                "reproduced {} cycle(s) of {} input(s)",
                reports.len(),
                record.inputs.len()
            );
            Ok(ExitCode::SUCCESS)
        }
        Err(ReplayError::Diverged { index, .. }) => {
            println!("replay diverged at step {index}");
            Ok(ExitCode::from(1))
        }
        Err(ReplayError::Length { expected, actual }) => {
            println!("replay produced {actual} steps, the record has {expected}");
            Ok(ExitCode::from(1))
        }
        Err(error) => Err(error.into()),
    }
}

fn validate(args: &[String]) -> Result<ExitCode, Box<dyn Error>> {
    let (positional, _) = parse_out(args)?;
    let [path] = positional.as_slice() else {
        return Err("validate takes exactly one record file".into());
    };
    let record = read_record(path)?;
    // Building the loop checks the configuration's thresholds.
    reference::build(&record.config)?;
    println!(
        "structure ok: {} input(s), {} trajectory step(s), seed {}",
        record.inputs.len(),
        record.trajectory.len(),
        record.config.seed
    );
    println!("run `thought replay` for the full check against the reference setup");
    Ok(ExitCode::SUCCESS)
}

fn why(args: &[String]) -> Result<ExitCode, Box<dyn Error>> {
    let (positional, _) = parse_out(args)?;
    let [path, cycle] = positional.as_slice() else {
        return Err("why takes a record file and a cycle number".into());
    };
    let record = read_record(path)?;
    let cycle: usize = cycle.parse().map_err(|_| "the cycle must be a number")?;

    let mut lp = reference::build(&record.config)?;
    let mut reports = Vec::new();
    for input in &record.inputs {
        reports.push(lp.run(input)?);
    }
    let report = reports
        .get(cycle)
        .ok_or_else(|| format!("the record has {} cycle(s)", reports.len()))?;

    println!("cycle {cycle}: {}", describe(report));
    let records = lp.provenance().explain(&report.thought);
    if records.is_empty() {
        println!("no provenance recorded for thought {}", report.thought);
    }
    for entry in records {
        println!(
            "  {:?} at {:?}, provider {}, path {}",
            entry.body.source,
            entry.body.timestamp,
            entry.body.provider.as_deref().unwrap_or("none"),
            entry.body.reasoning_path.join(" > ")
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn schema() -> Result<ExitCode, Box<dyn Error>> {
    println!("wire version: {WIRE_VERSION}");
    println!("run file skeleton (for `thought run`):");
    let skeleton = RunFile {
        config: reference::default_config(),
        inputs: vec![reference::grasp_input(1_000)],
    };
    println!("{}", serde_json::to_string_pretty(&skeleton)?);
    Ok(ExitCode::SUCCESS)
}
