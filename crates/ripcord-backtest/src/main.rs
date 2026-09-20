//! `ripcord-backtest` — replay historical Solana transactions through the
//! production policy engine.
//!
//! Two commands:
//!
//! ```text
//! ripcord-backtest verify                     check every configured program ID against the chain
//! ripcord-backtest run [options]              replay the incident windows and write a report
//! ```
//!
//! `verify` exists because a program ID typed from memory that happens to look
//! plausible is the easiest way to publish a confidently wrong report. Nothing
//! is watched until the chain confirms the account exists, is executable, and
//! says who may upgrade it.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Result};
use ripcord_backtest::cache::Cache;
use ripcord_backtest::engine::{self, WindowResult};
use ripcord_backtest::incidents::{self, Incident};
use ripcord_backtest::report::{self, ReportInput};
use ripcord_backtest::rpc::RpcClient;

struct Options {
    incident: Option<String>,
    limit: Option<usize>,
    page_limit: usize,
    cache_dir: PathBuf,
    out: Option<PathBuf>,
    throttle_ms: u64,
    /// Length of the `smoke-recent` self-test window, in minutes.
    smoke_minutes: i64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            incident: None,
            limit: None,
            page_limit: 1000,
            cache_dir: PathBuf::from("cache"),
            out: None,
            throttle_ms: 250,
            smoke_minutes: 5,
        }
    }
}

fn main() -> Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().map(String::as_str).unwrap_or("help");

    match command {
        "verify" => verify(),
        "run" => run(parse_options(&arguments[1..])?),
        _ => {
            print_usage();
            Ok(())
        }
    }
}

fn print_usage() {
    eprintln!(
        "ripcord-backtest

  verify                       check configured program IDs against the chain
  run [options]                replay incident windows and write a report

options for run:
  --incident <id>              one window only (default: all)
  --limit <n>                  stop after n transactions per window
  --page-limit <n>             signatures per RPC page (default 1000)
  --cache <dir>                transaction cache directory (default ./cache)
  --out <file>                 write the report here (default: stdout only)
  --throttle-ms <n>            pause between RPC calls (default 250)
  --smoke-minutes <n>          length of the smoke-recent self-test window (default 5)

  --incident smoke-recent      live self-test over recent Kamino traffic;
                               needs no archival history

environment:
  RIPCORD_RPC_URL              RPC endpoint (default: public mainnet)"
    );
}

fn parse_options(arguments: &[String]) -> Result<Options> {
    let mut options = Options::default();
    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index].as_str();
        let mut value = || -> Result<String> {
            index += 1;
            arguments
                .get(index)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{flag} needs a value"))
        };
        match flag {
            "--incident" => options.incident = Some(value()?),
            "--limit" => options.limit = Some(value()?.parse()?),
            "--page-limit" => options.page_limit = value()?.parse()?,
            "--cache" => options.cache_dir = PathBuf::from(value()?),
            "--out" => options.out = Some(PathBuf::from(value()?)),
            "--throttle-ms" => options.throttle_ms = value()?.parse()?,
            "--smoke-minutes" => options.smoke_minutes = value()?.parse()?,
            other => bail!("unknown option {other}"),
        }
        index += 1;
    }
    Ok(options)
}

/// Check every configured program against the chain.
///
/// Prints, per program: whether the account exists, whether it is executable,
/// its ProgramData account and the current upgrade authority. An immutable
/// program is called out, because an authority-change policy on a program that
/// has no authority left is a policy that can never fire.
fn verify() -> Result<()> {
    let client = RpcClient::from_env();
    println!("endpoint: {}", client.endpoint());
    println!();

    let mut programs: Vec<&str> = Vec::new();
    for incident in incidents::incidents() {
        for program in incident.programs.iter().chain(incident.watch_programs) {
            if !programs.contains(program) {
                programs.push(program);
            }
        }
    }

    for program in programs {
        print!("{program}  ");
        match client.program_authority(program) {
            Ok(Some(info)) => {
                println!("OK");
                println!("  executable:       {}", info.executable);
                println!("  program data:     {}", info.program_data);
                println!(
                    "  upgrade authority: {}",
                    info.upgrade_authority
                        .unwrap_or_else(|| "none — program is immutable".to_string())
                );
            }
            Ok(None) => println!("NOT FOUND on this endpoint — do not watch this program"),
            Err(error) => println!("ERROR: {error}"),
        }
        println!();
    }
    Ok(())
}

fn run(options: Options) -> Result<()> {
    let client = RpcClient::from_env().with_throttle(Duration::from_millis(options.throttle_ms));
    let cache = Cache::new(&options.cache_dir)?;

    let selected: Vec<Incident> = match options.incident.as_deref() {
        Some("smoke-recent") => vec![incidents::smoke_window(
            chrono::Utc::now().timestamp(),
            options.smoke_minutes,
        )],
        Some(id) => vec![incidents::by_id(id)
            .ok_or_else(|| anyhow::anyhow!("no incident with id {id}"))?],
        None => incidents::incidents(),
    };

    eprintln!("endpoint: {}", client.endpoint());
    eprintln!("cache:    {} ({} transactions)", options.cache_dir.display(), cache.len());
    eprintln!();

    let mut windows: Vec<(Incident, WindowResult)> = Vec::new();
    for incident in selected {
        eprintln!("{} ({})", incident.id, incident.protocol);

        // Resolve each watched program to its ProgramData account from the
        // chain. A watch entry that cannot be resolved is dropped with a
        // message rather than silently matching nothing.
        let mut program_data = Vec::new();
        for program in incident.watch_programs {
            match client.program_authority(program)? {
                Some(info) => program_data.push((program.to_string(), info.program_data)),
                None => eprintln!("  warning: {program} not found; it will not be watched"),
            }
        }

        let policy = engine::policy_for(&incident, &program_data);
        let result = engine::run_window(
            &client,
            &cache,
            &incident,
            &policy,
            options.page_limit,
            options.limit,
            |message| eprintln!("{message}"),
        )?;

        eprintln!(
            "  {} examined, {} detections, {} refusals, {} skipped",
            result.examined,
            result.detections.len(),
            result.refusals,
            result.skipped.len()
        );
        eprintln!();
        windows.push((incident, result));
    }

    let command = rebuild_command(&options);
    let rendered = report::render(&ReportInput {
        endpoint: client.endpoint(),
        command: &command,
        generated_unix: chrono::Utc::now().timestamp(),
        windows,
    });

    if let Some(path) = &options.out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &rendered)?;
        eprintln!("report written to {}", path.display());
    }
    println!("{rendered}");
    Ok(())
}

/// The command that reproduces this run, printed into the report.
fn rebuild_command(options: &Options) -> String {
    let mut parts = vec!["cargo run -p ripcord-backtest -- run".to_string()];
    if let Some(incident) = &options.incident {
        parts.push(format!("--incident {incident}"));
    }
    if let Some(limit) = options.limit {
        parts.push(format!("--limit {limit}"));
    }
    if options.page_limit != 1000 {
        parts.push(format!("--page-limit {}", options.page_limit));
    }
    parts.push(format!("--cache {}", options.cache_dir.display()));
    if let Some(out) = &options.out {
        parts.push(format!("--out {}", out.display()));
    }
    parts.join(" ")
}
