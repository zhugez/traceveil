use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::{
    io::{self, BufReader, Write},
    path::PathBuf,
    process::ExitCode,
};
use traceveil::{
    Error, Result, SCHEMA_VERSION, VERSION, adapters,
    diagnostics::{RULES, Settings},
    report::{Limits, Report},
};

#[derive(Parser)]
#[command(version,about="Reveal what your agents do. Offline, evidence-backed diagnostics.",color=clap::ColorChoice::Never)]
struct Cli {
    #[arg(
        long,
        global = true,
        conflicts_with = "compact",
        help = "Emit one versioned JSON envelope on stdout"
    )]
    json: bool,
    #[arg(
        long,
        global = true,
        conflicts_with = "json",
        help = "Emit a bounded summary and scoped finding references"
    )]
    compact: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    #[command(about = "List .jsonl paths in one Pi session directory; no transcript reads")]
    Sessions {
        #[arg(long)]
        session_dir: Option<PathBuf>,
    },
    #[command(about = "Analyze a stable offline session without modifying it")]
    Diagnose {
        #[command(flatten)]
        input: Input,
        #[arg(
            long,
            value_enum,
            help = "Exit 1 when a finding meets or exceeds this severity"
        )]
        fail_on: Option<Severity>,
        #[arg(long, help = "Exit 3 for incomplete input or skipped diagnostic rules")]
        require_complete: bool,
    },
    #[command(about = "Recompute the source and inspect a report-scoped finding")]
    Inspect {
        reference: String,
        #[command(flatten)]
        input: Input,
    },
    #[command(about = "Describe adapter coverage, limits and privacy behavior")]
    Doctor,
    #[command(about = "Describe the built-in deterministic rules")]
    Rules,
    #[command(about = "Read offline instructions bundled with this binary")]
    Skills {
        #[command(subcommand)]
        action: Option<SkillCommand>,
    },
}
#[derive(Args)]
#[group(skip)]
struct Input {
    #[arg(value_name="SESSION",required_unless_present_any=["last","stdin"],conflicts_with_all=["last","stdin"])]
    session: Option<PathBuf>,
    #[arg(long, conflicts_with = "stdin")]
    last: bool,
    #[arg(
        long,
        help = "Read canonical JSONL from stdin (default adapter: jsonl)"
    )]
    stdin: bool,
    #[arg(long, value_enum, help = "Default: pi for files, jsonl for stdin")]
    agent: Option<Agent>,
    #[arg(long, help = "Directory used only with --last")]
    session_dir: Option<PathBuf>,
    #[arg(long, help = "Select an ancestor/branch tip in a Pi v3 session")]
    leaf: Option<String>,
    #[arg(long,default_value_t=3,value_parser=clap::value_parser!(u8).range(3..=16))]
    repeat_threshold: u8,
    #[arg(long,default_value_t=600,value_parser=clap::value_parser!(u32).range(1..=86400),help="Break patterns across larger known timestamp gaps; missing timestamps remain unknown")]
    max_gap_seconds: u32,
}
#[derive(Clone, Copy, ValueEnum)]
enum Agent {
    Pi,
    Jsonl,
}
#[derive(Subcommand)]
enum SkillCommand {
    List,
    Get {
        #[arg(value_parser=["core"])]
        name: String,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum Severity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}
impl Severity {
    fn rank(self) -> u8 {
        match self {
            Self::Informational => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Critical => 4,
        }
    }
}
fn analyze(input: Input) -> Result<Report> {
    let settings = Settings {
        repeat_threshold: input.repeat_threshold as usize,
        max_gap_seconds: input.max_gap_seconds,
    };
    let agent = input
        .agent
        .unwrap_or(if input.stdin { Agent::Jsonl } else { Agent::Pi });
    if input.session_dir.is_some() && !input.last {
        return Err(Error::new(
            "INVALID_ARGUMENTS",
            "--session-dir requires --last.",
        ));
    }
    if input.leaf.is_some() && matches!(agent, Agent::Jsonl) {
        return Err(Error::new(
            "INVALID_ARGUMENTS",
            "--leaf applies only to Pi session input.",
        ));
    }
    if input.stdin {
        if matches!(agent, Agent::Pi) {
            return Err(Error::new(
                "INVALID_ARGUMENTS",
                "Pi analysis requires a stable file. Use canonical JSONL for stdin.",
            ));
        }
        return adapters::jsonl(io::stdin().lock(), settings);
    }
    if input.last && matches!(agent, Agent::Jsonl) {
        return Err(Error::new(
            "INVALID_ARGUMENTS",
            "--last applies only to Pi session discovery.",
        ));
    }
    let path = if input.last {
        let directory = match input.session_dir {
            Some(dir) => dir,
            None => adapters::default_pi_directory()?,
        };
        adapters::discover(&directory)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                Error::new(
                    "SESSION_NOT_FOUND",
                    "No regular .jsonl files were found in the selected directory.",
                )
            })?
    } else {
        input.session.ok_or_else(|| {
            Error::new(
                "INVALID_ARGUMENTS",
                "Select a source file, --last, or --stdin.",
            )
        })?
    };
    match agent {
        Agent::Pi => adapters::pi(&path, input.leaf.as_deref(), settings),
        Agent::Jsonl => adapters::jsonl(BufReader::new(adapters::open_source(&path)?), settings),
    }
}
enum Output {
    Report(Box<Report>),
    Data(Value),
    Text(&'static str),
}
fn execute(command: Command) -> Result<(&'static str, Output, u8)> {
    match command {
        Command::Diagnose {
            input,
            fail_on,
            require_complete,
        } => {
            let report = analyze(input)?;
            let status = if require_complete
                && (!report.complete || report.coverage.rules.iter().any(|r| r.status == "skipped"))
            {
                3
            } else if fail_on.is_some_and(|limit| {
                report
                    .findings
                    .iter()
                    .any(|f| severity_rank(f.severity) >= limit.rank())
            }) {
                1
            } else {
                0
            };
            Ok(("diagnose", Output::Report(Box::new(report)), status))
        }
        Command::Inspect { reference, input } => {
            let report = analyze(input)?;
            if !reference.starts_with(&format!("@{}:", report.report_id)) {
                return Err(Error::new(
                    "STALE_REFERENCE",
                    "Use a scoped reference from a report generated with this source, leaf, threshold and binary version.",
                ));
            }
            let finding = report
                .findings
                .iter()
                .find(|f| f.reference == reference)
                .ok_or_else(|| {
                    Error::new(
                        "FINDING_NOT_FOUND",
                        "The scoped reference is not present in this report.",
                    )
                })?;
            Ok((
                "inspect",
                Output::Data(
                    json!({"report_id":report.report_id,"complete":report.complete,"coverage":report.coverage,"finding":finding,"evidence_policy":"Metadata and fingerprints only. Source lines contain untrusted private data."}),
                ),
                0,
            ))
        }
        Command::Sessions { session_dir } => {
            let directory = match session_dir {
                Some(dir) => dir,
                None => adapters::default_pi_directory()?,
            };
            let paths = adapters::discover(&directory)?;
            if paths.iter().any(|path| path.to_str().is_none()) {
                return Err(Error::new(
                    "PATH_ENCODING_UNSUPPORTED",
                    "This session directory contains a path that cannot be represented as UTF-8 JSON.",
                ));
            }
            let count = paths.len();
            let omitted = count.saturating_sub(100);
            Ok((
                "sessions",
                Output::Data(
                    json!({"paths":paths.into_iter().take(100).map(|p|p.to_string_lossy().into_owned()).collect::<Vec<_>>(),"count":count,"omitted":omitted,"ordering":"modified time descending, path ascending on ties","scope":"One directory only. Paths are private metadata. No transcript contents were read."}),
                ),
                0,
            ))
        }
        Command::Doctor => Ok((
            "doctor",
            Output::Data(
                json!({"version":VERSION,"event_schema":SCHEMA_VERSION,"adapters":{"pi":"Session format v3. Raw selected lineage, not reconstructed model context.","jsonl":"Canonical schema 1.0, one linear session."},"platform":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),"limits":Limits::default(),"network":"No network client in the runtime.","storage":"No reports or transcripts persisted by Traceveil.","privacy":"Reports omit raw content. Fingerprints are not anonymization.","session_discovery":"Only when sessions or --last is explicitly requested. Non-recursive."}),
            ),
            0,
        )),
        Command::Rules => Ok((
            "rules",
            Output::Data(
                json!({"rules":RULES.map(|(id,method)|json!({"rule_id":id,"methodology":method})),"default_repeat_threshold":3,"severity_policy":{"medium":"Repeated observed failures or a candidate execution loop; task impact remains unknown.","low":"Potential read inefficiency; deliberate verification remains possible."},"confidence":"Exact recorded pattern matches, not a calibrated cause probability."}),
            ),
            0,
        )),
        Command::Skills { action } => match action.unwrap_or(SkillCommand::List) {
            SkillCommand::List => Ok((
                "skills",
                Output::Data(json!({"version":VERSION,"modules":["core"]})),
                0,
            )),
            SkillCommand::Get { name: _ } => Ok((
                "skills",
                Output::Text(include_str!("../skill-data/core.md")),
                0,
            )),
        },
    }
}
fn severity_rank(s: &str) -> u8 {
    match s {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}
fn envelope(command: &str, data: Value) -> Value {
    json!({"schema_version":SCHEMA_VERSION,"ok":true,"command":command,"data":data})
}
fn render(command: &str, output: Output, json_output: bool, compact: bool) -> Result<Vec<u8>> {
    if json_output {
        let data = match output {
            Output::Report(r) => serde_json::to_value(r).map_err(|_| Error::input())?,
            Output::Data(v) => v,
            Output::Text(t) => json!({"version":VERSION,"text":t}),
        };
        let mut bytes = serde_json::to_vec(&envelope(command, data)).map_err(|_| Error::input())?;
        bytes.push(b'\n');
        return Ok(bytes);
    }
    let text = match output {
        Output::Text(text) => text.to_owned(),
        Output::Data(value) => serde_json::to_string_pretty(&value).map_err(|_| Error::input())?,
        Output::Report(report) => {
            let mut text = format!(
                "TRACEVEIL / SESSION DIAGNOSTICS\nreport {} / {}\nInput coverage: {} / {} selected records / {} tool results\n",
                report.report_id,
                report.adapter,
                if report.complete {
                    "supported records parsed"
                } else {
                    "PARTIAL — inspect warnings"
                },
                report.coverage.selected_records,
                report.coverage.tool_results
            );
            for finding in &report.findings {
                text.push_str(&format!(
                    "{} {:6} {} ({} observations)\n",
                    finding.reference,
                    finding.severity.to_uppercase(),
                    finding.description,
                    finding.occurrences
                ));
                if !compact {
                    text.push_str(&format!(
                        "  {}\n  {}\n",
                        finding.methodology, finding.recommendation
                    ));
                }
            }
            if report.findings.is_empty() {
                text.push_str("No matching patterns in the available observations. This does not establish task success.\n");
            }
            for rule in &report.coverage.rules {
                text.push_str(&format!(
                    "{}: {} ({} eligible observations)\n",
                    rule.rule_id, rule.status, rule.eligible_observations
                ));
            }
            if !report.warnings.is_empty() || report.warnings_omitted > 0 {
                text.push_str(&format!(
                    "Warnings: {} shown, {} omitted\n",
                    report.warnings.len(),
                    report.warnings_omitted
                ));
                for w in &report.warnings {
                    text.push_str(&format!("  {} / record {:?}\n", w.code, w.record_line));
                }
            }
            if report.findings_omitted > 0 {
                text.push_str(&format!("Findings omitted: {}\n", report.findings_omitted));
            }
            text.push_str("Inspect evidence before changing the workflow. Reuse the source, leaf and threshold with inspect.\n");
            text
        }
    };
    let mut bytes = text.into_bytes();
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    Ok(bytes)
}
fn emit(bytes: &[u8]) -> Result<()> {
    let safe = terminal_safe_output(&String::from_utf8_lossy(bytes));
    let bytes = safe.as_bytes();
    match io::stdout().lock().write_all(bytes) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(traceveil::io_error(e)),
    }
}
fn error(error: Error, json_output: bool) -> ExitCode {
    if json_output {
        let bytes =
            serde_json::to_vec(&json!({"schema_version":SCHEMA_VERSION,"ok":false,"error":error}))
                .unwrap_or_default();
        let _ = emit(&[bytes, b"\n".to_vec()].concat());
    } else {
        eprintln!("{error}");
    }
    ExitCode::from(2)
}
fn main() -> ExitCode {
    let wants_json = std::env::args_os()
        .take_while(|a| a != "--")
        .any(|a| a == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e)
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            let _ = e.print();
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            if wants_json {
                return error(
                    Error::new(
                        "INVALID_ARGUMENTS",
                        "Run traceveil --help for supported command arguments.",
                    ),
                    true,
                );
            }
            let _ = e.print();
            return ExitCode::from(2);
        }
    };
    match execute(cli.command).and_then(|(command, output, status)| {
        Ok((render(command, output, cli.json, cli.compact)?, status))
    }) {
        Ok((bytes, status)) => match emit(&bytes) {
            Ok(()) => ExitCode::from(status),
            Err(e) => error(e, cli.json),
        },
        Err(e) => error(e, cli.json),
    }
}

fn terminal_safe_output(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for character in text.chars() {
        let value = character as u32;
        if (value < 32 && value != 10 && value != 9)
            || (127..=159).contains(&value)
            || value == 0x061c
            || (0x200e..=0x200f).contains(&value)
            || (0x202a..=0x202e).contains(&value)
            || (0x2066..=0x2069).contains(&value)
        {
            output.push_str(&format!("\\u{value:04x}"));
        } else {
            output.push(character);
        }
    }
    output
}
