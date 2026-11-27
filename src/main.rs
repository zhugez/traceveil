mod discovery;
mod local_files;
mod presentation;
mod prompt;
mod skill_install;
mod user_config;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::{
    io::{self, BufReader, Cursor, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};
use traceveil::{
    Error, Result, SCHEMA_VERSION, VERSION, adapters,
    diagnostics::{RULES, Settings},
    report::{Limits, Report},
};

#[derive(Parser)]
#[command(
    version,
    about = "Debug your coding agents.",
    long_about = "Find repeated failures, execution loops, and redundant reads in coding-agent sessions. Local, read-only and evidence-backed.",
    after_help = "Try: traceveil demo\nThen: traceveil analyze --last\nAutomation: traceveil analyze SESSION --agent pi --json"
)]
struct Cli {
    #[arg(long,global=true,conflicts_with_all=["compact","quiet"],help="Emit one versioned JSON envelope; never open a menu")]
    json: bool,
    #[arg(
        long,
        global = true,
        conflicts_with = "json",
        help = "Show a shorter human report"
    )]
    compact: bool,
    #[arg(short,long,global=true,conflicts_with_all=["json","verbose"],help="Suppress successful human output; keep exit status")]
    quiet: bool,
    #[arg(
        short,
        long,
        global = true,
        conflicts_with = "quiet",
        help = "Explain command and policy selection on stderr"
    )]
    verbose: bool,
    #[arg(long,global=true,value_parser=["auto","always","never"],default_value="auto",help="Terminal colors; NO_COLOR always disables color")]
    color: String,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    #[command(about = "Analyze a Pi session, with safe project discovery when no path is supplied")]
    Analyze(Analysis),
    #[command(about = "Analyze an explicit source (compatible with the original CLI)")]
    Diagnose(Analysis),
    #[command(about = "Try a synthetic session through the real engine, completely offline")]
    Demo,
    #[command(
        about = "List local Pi sessions; JSON paths-only mode preserves the original interface"
    )]
    Sessions {
        #[arg(long)]
        session_dir: Option<PathBuf>,
        #[arg(long, value_enum)]
        agent: Option<Agent>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(
            long,
            help = "Read bounded session headers for format, project and activity hints"
        )]
        metadata: bool,
    },
    #[command(about = "Inspect a scoped reference; use --demo for short demo finding IDs")]
    Inspect {
        reference: String,
        #[command(flatten)]
        input: Input,
    },
    #[command(about = "Describe adapter coverage, privacy behavior and resource limits")]
    Doctor,
    #[command(about = "Show agent detection separately from adapter support")]
    Agents,
    #[command(about = "Configure an explicitly authorized Pi session directory")]
    Init {
        #[arg(long)]
        session_dir: Option<PathBuf>,
    },
    #[command(about = "View or update private user configuration")]
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    #[command(about = "Describe deterministic rules and their limitations")]
    Rules,
    #[command(about = "Read offline guidance or install the optional Pi skill")]
    Skills {
        #[command(subcommand)]
        action: Option<SkillCommand>,
    },
    #[command(about = "Generate shell completions on stdout")]
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}
#[derive(Args)]
struct Analysis {
    #[command(flatten)]
    input: Input,
    #[arg(
        long,
        value_enum,
        help = "Exit 1 when a finding meets or exceeds this severity"
    )]
    fail_on: Option<Severity>,
    #[arg(long, help = "Exit 3 for missing evidence or skipped enabled rules")]
    require_complete: bool,
}
#[derive(Args, Default)]
#[group(skip)]
struct Input {
    #[arg(value_name="SESSION",conflicts_with_all=["last","stdin","demo"])]
    session: Option<PathBuf>,
    #[arg(long,conflicts_with_all=["stdin","demo"],help="Authorize selection of the newest matching Pi session")]
    last: bool,
    #[arg(
        long,
        conflicts_with = "demo",
        help = "Read canonical JSONL from stdin"
    )]
    stdin: bool,
    #[arg(long, help = "Use the embedded synthetic source")]
    demo: bool,
    #[arg(long, value_enum, help = "Default: pi for files, jsonl for stdin")]
    agent: Option<Agent>,
    #[arg(long, help = "Authorize one explicit Pi session directory")]
    session_dir: Option<PathBuf>,
    #[arg(
        long,
        help = "Select the project whose Pi sessions should be considered"
    )]
    project: Option<PathBuf>,
    #[arg(long, help = "Select one recorded Pi ancestry ending at this entry")]
    leaf: Option<String>,
    #[arg(long,default_value_t=3,value_parser=clap::value_parser!(u8).range(3..=16))]
    repeat_threshold: u8,
    #[arg(long,default_value_t=600,value_parser=clap::value_parser!(u32).range(1..=86400))]
    max_gap_seconds: u32,
    #[arg(
        long,
        value_name = "RULE=COUNT",
        help = "Override one rule threshold, for example RF001=4"
    )]
    rule_threshold: Vec<String>,
    #[arg(
        long,
        value_name = "RULE",
        help = "Explicitly suppress a rule; repeat for multiple rules"
    )]
    suppress: Vec<String>,
}
#[derive(Clone, Copy, ValueEnum)]
enum Agent {
    Pi,
    Jsonl,
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
#[derive(Subcommand)]
enum ConfigAction {
    Show,
    Path,
    Set { key: String, value: String },
}
#[derive(Subcommand)]
enum SkillCommand {
    List,
    Get {
        #[arg(value_parser=["core"])]
        name: String,
    },
    Install {
        #[arg(long,value_parser=["pi"],default_value="pi")]
        agent: String,
        #[arg(long,value_parser=["project","user"],default_value="project")]
        scope: String,
        #[arg(
            long,
            help = "Authorize replacing SKILL.md in this scope after review; a receipt is not ownership proof"
        )]
        force: bool,
    },
    Uninstall {
        #[arg(long,value_parser=["pi"],default_value="pi")]
        agent: String,
        #[arg(long,value_parser=["project","user"],default_value="project")]
        scope: String,
    },
}
fn settings(input: &Input, product: bool) -> Result<Settings> {
    let mut config = if product {
        user_config::load()?
    } else {
        user_config::Config::default()
    };
    for value in &input.rule_threshold {
        let (id, n) = value.split_once('=').ok_or_else(|| {
            Error::new(
                "CONFIGURATION_INVALID",
                "Use --rule-threshold RULE=COUNT, such as RF001=4.",
            )
        })?;
        user_config::rule_index(id)?;
        config
            .rule_thresholds
            .insert(id.into(), n.parse().map_err(|_| Error::input())?);
    }
    for id in &input.suppress {
        user_config::rule_index(id)?;
        config.suppressed_rules.push(id.clone());
    }
    config.settings(input.repeat_threshold as usize, input.max_gap_seconds)
}
fn demo(settings: Settings) -> Result<Report> {
    adapters::jsonl(
        Cursor::new(include_bytes!("../benchmarks/fixtures/demo.jsonl")),
        settings,
    )
}
fn source(input: Input, product: bool, interactive: bool) -> Result<Report> {
    let settings = settings(&input, product)?;
    source_with_settings(input, product, interactive, settings)
}
fn source_with_settings(
    mut input: Input,
    product: bool,
    interactive: bool,
    settings: Settings,
) -> Result<Report> {
    if input.demo {
        return demo(settings);
    }
    let agent = input
        .agent
        .unwrap_or(if input.stdin { Agent::Jsonl } else { Agent::Pi });
    if input.leaf.is_some() && matches!(agent, Agent::Jsonl) {
        return Err(Error::new(
            "INVALID_ARGUMENTS",
            "--leaf applies only to Pi sessions.",
        ));
    }
    if input.stdin {
        if matches!(agent, Agent::Pi) {
            return Err(Error::new(
                "INVALID_ARGUMENTS",
                "Pi analysis needs a stable file. Use canonical JSONL for stdin.",
            ));
        }
        return adapters::jsonl(io::stdin().lock(), settings);
    }
    if matches!(agent, Agent::Jsonl) && input.session.is_none() {
        return Err(Error::new(
            "INVALID_ARGUMENTS",
            "Canonical JSONL requires an explicit source file or --stdin.",
        ));
    }
    if !product && input.session_dir.is_some() && !input.last {
        return Err(Error::new(
            "INVALID_ARGUMENTS",
            "--session-dir requires --last.",
        ));
    }
    if input.session.is_none() {
        if !product && !input.last {
            return Err(Error::new(
                "INVALID_ARGUMENTS",
                "Select a source file, --last, or --stdin. Try traceveil demo.",
            ));
        }
        let project = input
            .project
            .unwrap_or(std::env::current_dir().map_err(traceveil::io_error)?)
            .canonicalize()
            .map_err(traceveil::io_error)?;
        let config = if product {
            user_config::load()?
        } else {
            user_config::Config::default()
        };
        let explicit = input.session_dir.is_some() || config.session_dir.is_some() || input.last;
        let directory = if product {
            discovery::directory(
                &project,
                input
                    .session_dir
                    .as_deref()
                    .or(config.session_dir.as_deref()),
            )?
            .authorize(interactive)?
        } else {
            match input.session_dir {
                Some(p) => p,
                None => adapters::default_pi_directory()?,
            }
        };
        if !explicit {
            if !interactive {
                return Err(Error::new(
                    "DISCOVERY_AUTHORIZATION_REQUIRED",
                    "Use analyze --last to authorize this project’s Pi directory, or pass --session-dir DIR. No source was read.",
                ));
            }
            eprintln!(
                "Pi source directory: {}",
                presentation::label(&directory.display().to_string())
            );
            if !prompt::ask(|| {
                dialoguer::Confirm::new()
                    .with_prompt("Read local session headers and analyze a selected session?")
                    .default(false)
                    .interact()
            })? {
                return Err(Error::new("CANCELLED", "No session was read."));
            }
        }
        input.session = Some(if product {
            discovery::select(&directory, &project, input.last, interactive)?
        } else {
            adapters::discover(&directory)?.into_iter().next().ok_or_else(||Error::new("SESSION_NOT_FOUND","No regular .jsonl files found. Try traceveil demo or select --session-dir DIR."))?
        });
    }
    let path = input.session.ok_or_else(Error::input)?;
    match agent {
        Agent::Pi => adapters::pi(&path, input.leaf.as_deref(), settings),
        Agent::Jsonl => adapters::jsonl(BufReader::new(adapters::open_source(&path)?), settings),
    }
}
enum Output {
    Report(Box<Report>),
    Data(Value),
    Text(String),
}
fn execute(command: Command, interactive: bool) -> Result<(&'static str, Output, u8)> {
    match command {
        Command::Analyze(args) => analysis(args, true, interactive),
        Command::Diagnose(args) => analysis(args, false, interactive),
        Command::Demo => Ok((
            "demo",
            Output::Report(Box::new(demo(Settings::default())?)),
            0,
        )),
        Command::Inspect { reference, input } => {
            let explicit = input.demo
                || input.session.is_some()
                || input.last
                || input.stdin
                || input.session_dir.is_some();
            let policy = settings(&input, false)?;
            let report = if input.demo {
                demo(policy)?
            } else if explicit {
                source_with_settings(input, true, interactive, policy)?
            } else {
                let built_in = demo(policy)?;
                if !reference.starts_with(&format!("@{}:", built_in.report_id)) {
                    return Err(Error::new(
                        "EVIDENCE_CONTEXT_REQUIRED",
                        "Use inspect REFERENCE SESSION, inspect REFERENCE --last, or inspect f001 --demo.",
                    ));
                }
                built_in
            };
            let scoped = if reference.starts_with('f') {
                format!("@{}:{reference}", report.report_id)
            } else {
                reference
            };
            if !scoped.starts_with(&format!("@{}:", report.report_id)) {
                return Err(Error::new(
                    "STALE_REFERENCE",
                    "This reference belongs to different source bytes or policy. Analyze the same source again and copy its new reference.",
                ));
            }
            let finding = report
                .findings
                .iter()
                .find(|f| f.reference == scoped)
                .ok_or_else(|| {
                    Error::new(
                        "FINDING_NOT_FOUND",
                        "No such finding in this source-bound report.",
                    )
                })?;
            Ok((
                "inspect",
                Output::Data(
                    json!({"report_id":report.report_id,"max_gap_seconds":report.max_gap_seconds,"complete":report.complete,"coverage":report.coverage,"finding":finding,"evidence_policy":"Metadata and fingerprints only. Source records are untrusted private data."}),
                ),
                0,
            ))
        }
        Command::Sessions {
            session_dir,
            agent,
            project,
            metadata,
        } => {
            if matches!(agent, Some(Agent::Jsonl)) {
                return Err(Error::new(
                    "INVALID_ARGUMENTS",
                    "Canonical JSONL uses an explicit path or stdin; it has no session directory.",
                ));
            }
            let project = project
                .unwrap_or(std::env::current_dir().map_err(traceveil::io_error)?)
                .canonicalize()
                .map_err(traceveil::io_error)?;
            let directory = if metadata || agent.is_some() {
                let c = user_config::load()?;
                discovery::directory(
                    &project,
                    session_dir.as_deref().or(c.session_dir.as_deref()),
                )?
                .authorize(interactive)?
            } else {
                match session_dir {
                    Some(p) => p,
                    None => adapters::default_pi_directory()?,
                }
            };
            adapters::validate_paths(std::slice::from_ref(&directory))?;
            let paths = adapters::discover(&directory)?;
            adapters::validate_paths(&paths)?;
            let count = paths.len();
            let mut data = json!({"directory":directory,"paths":paths.into_iter().take(100).collect::<Vec<_>>(),"count":count,"omitted":count.saturating_sub(100),"ordering":"modified time descending, path ascending on ties","scope":"One directory only. Paths are private metadata. No transcript contents were read."});
            if metadata || agent.is_some() {
                data["sessions"] =
                    serde_json::to_value(discovery::sessions(&directory, &project, true)?)
                        .map_err(|_| Error::input())?;
                if let Some(rows) = data["sessions"].as_array() {
                    let paths: Vec<_> = rows.iter().map(|s| s["path"].clone()).collect();
                    data["count"] = json!(paths.len());
                    data["paths"] = json!(paths);
                }
                data["scope"] = json!(
                    "Bounded first-record metadata and final-byte hints, one directory only. No transcript content is emitted."
                );
            }
            Ok(("sessions", Output::Data(data), 0))
        }
        Command::Agents => {
            let project = std::env::current_dir().map_err(traceveil::io_error)?;
            let config = user_config::load()?;
            let location = discovery::directory(&project, config.session_dir.as_deref())?;
            let detected = if location.from_project_settings {
                None
            } else {
                Some(location.path.is_dir())
            };
            Ok((
                "agents",
                Output::Data(
                    json!({"agents":[{"name":"Pi","detected":detected,"adapter":"verified","format":"session v3","coverage":"partial for nested Code Mode"},{"name":"Canonical JSONL","detected":false,"adapter":"supported","format":"1.0","coverage":"depends on recorded fields"},{"name":"Claude Code / Codex / OpenCode","detected":null,"adapter":"unsupported","coverage":"No adapters implemented; directories are not evidence of support."}]}),
                ),
                0,
            ))
        }
        Command::Doctor => Ok((
            "doctor",
            Output::Data(
                json!({"version":VERSION,"event_schema":SCHEMA_VERSION,"adapters":{"pi":"Session v3, selected recorded lineage.","jsonl":"Canonical schema1.0, one linear session."},"platform":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),"limits":Limits::default(),"network":"No runtime network client or telemetry.","storage":"Private user configuration only when explicitly saved; no report or transcript cache.","privacy":"Raw prompts, arguments and outputs omitted. Fingerprints are not anonymization.","next":"Run traceveil demo; then traceveil analyze --last."}),
            ),
            0,
        )),
        Command::Init { session_dir } => {
            let dir = match session_dir {
                Some(d) => d,
                None => {
                    if !interactive {
                        return Err(Error::new(
                            "INVALID_ARGUMENTS",
                            "Use init --session-dir DIR in non-interactive mode.",
                        ));
                    }
                    PathBuf::from(prompt::ask(|| {
                        dialoguer::Input::<String>::new()
                            .with_prompt("Authorized Pi session directory")
                            .interact_text()
                    })?)
                }
            };
            let dir = dir.canonicalize().map_err(traceveil::io_error)?;
            if !dir.is_dir() {
                return Err(Error::new(
                    "CONFIGURATION_INVALID",
                    "The session location must be an existing directory.",
                ));
            }
            let mut c = user_config::load()?;
            c.session_dir = Some(dir);
            let path = user_config::save(&c)?;
            Ok((
                "init",
                Output::Data(
                    json!({"configured":true,"config_path":path,"next":"traceveil analyze --last"}),
                ),
                0,
            ))
        }
        Command::Config { action } => match action.unwrap_or(ConfigAction::Show) {
            ConfigAction::Path => Ok((
                "config",
                Output::Data(json!({"path":user_config::path()?})),
                0,
            )),
            ConfigAction::Show => Ok((
                "config",
                Output::Data(
                    serde_json::to_value(user_config::load()?).map_err(|_| Error::input())?,
                ),
                0,
            )),
            ConfigAction::Set { key, value } => {
                let mut c = user_config::load()?;
                if key == "session-dir" {
                    let p = PathBuf::from(value)
                        .canonicalize()
                        .map_err(traceveil::io_error)?;
                    if !p.is_dir() {
                        return Err(Error::input());
                    }
                    c.session_dir = Some(p);
                } else if let Some(id) = key.strip_prefix("threshold.") {
                    user_config::rule_index(id)?;
                    c.rule_thresholds
                        .insert(id.into(), value.parse().map_err(|_| Error::input())?);
                } else if let Some(id) = key.strip_prefix("suppress.") {
                    user_config::rule_index(id)?;
                    let yes = value.parse::<bool>().map_err(|_| Error::input())?;
                    c.suppressed_rules.retain(|s| s != id);
                    if yes {
                        c.suppressed_rules.push(id.into());
                    }
                } else {
                    return Err(Error::new(
                        "CONFIGURATION_INVALID",
                        "Keys: session-dir, threshold.RF001, suppress.RF001 (or another known rule).",
                    ));
                }
                let path = user_config::save(&c)?;
                Ok(("config", Output::Data(json!({"saved":true,"path":path})), 0))
            }
        },
        Command::Rules => Ok((
            "rules",
            Output::Data(
                json!({"rules":RULES.map(|(id,method)|json!({"rule_id":id,"methodology":method})),"default_repeat_threshold":3,"configuration":"--rule-threshold RF001=4; --suppress READ001. Explicit policy is shown in coverage.","confidence":"Matching recorded observations, not calibrated cause probability."}),
            ),
            0,
        )),
        Command::Skills { action } => match action.unwrap_or(SkillCommand::List) {
            SkillCommand::List => Ok((
                "skills",
                Output::Data(
                    json!({"version":VERSION,"modules":["core"],"install":"traceveil skills install --agent pi --scope project"}),
                ),
                0,
            )),
            SkillCommand::Get { name: _ } => Ok((
                "skills",
                Output::Text(include_str!("../skill-data/core.md").into()),
                0,
            )),
            SkillCommand::Install {
                agent: _,
                scope,
                force,
            } => Ok((
                "skills",
                Output::Data(skill_install::install(&scope, force)?),
                0,
            )),
            SkillCommand::Uninstall { agent: _, scope } => {
                Ok(("skills", Output::Data(skill_install::uninstall(&scope)?), 0))
            }
        },
        Command::Completions { shell } => {
            let mut bytes = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "traceveil", &mut bytes);
            Ok((
                "completions",
                Output::Text(String::from_utf8(bytes).map_err(|_| Error::input())?),
                0,
            ))
        }
    }
}
fn analysis(
    args: Analysis,
    product: bool,
    interactive: bool,
) -> Result<(&'static str, Output, u8)> {
    let is_demo = args.input.demo;
    let report = source(args.input, product, interactive)?;
    let status = if args.require_complete
        && (!report.complete
            || report
                .coverage
                .rules
                .iter()
                .any(|r| r.status == "skipped" && !r.suppressed))
    {
        3
    } else if args.fail_on.is_some_and(|n| {
        report
            .findings
            .iter()
            .any(|f| severity_rank(f.severity) >= n.rank())
    }) {
        1
    } else {
        0
    };
    Ok((
        if is_demo {
            "demo"
        } else if product {
            "analyze"
        } else {
            "diagnose"
        },
        Output::Report(Box::new(report)),
        status,
    ))
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
fn human_data(command: &str, data: &Value) -> String {
    let mut text = String::new();
    match command {
        "sessions" => {
            text.push_str(
                "PI SESSIONS\nPaths are private metadata. Newest modified file first.\n\n",
            );
            if let Some(items) = data["sessions"].as_array() {
                for s in items {
                    text.push_str(&format!(
                        "{}  {}{}\n",
                        s["format"].as_str().unwrap_or("unknown"),
                        presentation::label(s["path"].as_str().unwrap_or("")),
                        if s["incomplete_tail"] == true {
                            " [incomplete final record]"
                        } else if s["possibly_active"] == true {
                            " [possibly active]"
                        } else {
                            ""
                        }
                    ));
                }
            } else if let Some(paths) = data["paths"].as_array() {
                for p in paths {
                    text.push_str(&presentation::label(p.as_str().unwrap_or("")));
                    text.push('\n');
                }
            }
            if data["count"] == 0 {
                text.push_str("No session found. Run traceveil demo, or configure traceveil init --session-dir DIR.\n");
            }
            text.push_str("\nNext: traceveil analyze --last\n");
        }
        "agents" => {
            text.push_str("CODING AGENTS\nDetection is separate from adapter support.\n\n");
            if let Some(items) = data["agents"].as_array() {
                for a in items {
                    text.push_str(&format!(
                        "{}: {} — {}\n",
                        a["name"].as_str().unwrap_or(""),
                        a["adapter"].as_str().unwrap_or(""),
                        if a["detected"] == true {
                            "source directory detected"
                        } else {
                            "not detected / explicit source required"
                        }
                    ));
                }
            }
        }
        "inspect" => {
            let f = &data["finding"];
            text.push_str(&format!("EVIDENCE — {}\n{}\n\nObserved: {} matching observations\n\nWhy flagged\n{}\n\nInvestigate\n{}\n\nLimits\n{}\n\nSource records\n",f["rule_id"].as_str().unwrap_or(""),f["description"].as_str().unwrap_or(""),f["occurrences"],f["methodology"].as_str().unwrap_or(""),f["recommendation"].as_str().unwrap_or(""),f["limitations"].as_str().unwrap_or("")));
            if let Some(rows) = f["evidence"].as_array() {
                for row in rows {
                    text.push_str(&format!(
                        "  record {} · {} · call record {}\n",
                        row["record_line"],
                        row["tool"].as_str().unwrap_or("other"),
                        row["call_record_line"]
                    ));
                }
            }
            text.push_str("\nRaw source text is private and is not displayed.\n");
        }
        "doctor" => {
            text.push_str(&format!("TRACEVEIL {}\nDebug your coding agents.\n\nPlatform: {}\nAdapters: Pi v3, canonical JSONL1.0\nPrivacy: local analysis; no telemetry or external model.\nCoverage: nested Code Mode remains partial.\n\nTry: traceveil demo\nSetup: traceveil init --session-dir DIR\nDetails: traceveil doctor --json\n",VERSION,data["platform"].as_str().unwrap_or("unknown")));
        }
        "rules" => {
            text.push_str("DIAGNOSTIC RULES\n\n");
            for (id, method) in RULES {
                text.push_str(&format!("{id}\n  {method}\n\n"));
            }
            text.push_str("Default: 3 repetitions. Configure with --rule-threshold RF001=4 or --suppress READ001.\n");
        }
        _ => text.push_str(&presentation::safe(
            &serde_json::to_string_pretty(data).unwrap_or_default(),
        )),
    }
    text
}
fn render(command: &str, output: Output, cli: &Cli) -> Result<Vec<u8>> {
    if cli.json {
        let mut data = match output {
            Output::Report(r) => serde_json::to_value(r).map_err(|_| Error::input())?,
            Output::Data(v) => v,
            Output::Text(t) => json!({"version":VERSION,"text":t}),
        };
        if command == "demo" {
            data["synthetic"] = json!(true);
        }
        let mut b = serde_json::to_vec(&envelope(command, data)).map_err(|_| Error::input())?;
        b.push(b'\n');
        return Ok(presentation::safe(&String::from_utf8_lossy(&b)).into_bytes());
    }
    if cli.quiet {
        return Ok(Vec::new());
    }
    let mut text = match output {
        Output::Report(r) => presentation::report(
            &r,
            cli.compact,
            presentation::color(&cli.color),
            command == "demo",
        ),
        Output::Data(v) => human_data(command, &v),
        Output::Text(t) => t,
    };
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text.into_bytes())
}
fn emit(bytes: &[u8]) -> Result<()> {
    match io::stdout().lock().write_all(bytes) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(traceveil::io_error(e)),
    }
}
fn error(e: Error, json_output: bool) -> ExitCode {
    let code = if e.code == "CANCELLED" { 130 } else { 2 };
    if json_output {
        let _ = emit(
            format!(
                "{}\n",
                json!({"schema_version":SCHEMA_VERSION,"ok":false,"error":e})
            )
            .as_bytes(),
        );
    } else {
        eprintln!("{e}");
        if e.code != "CANCELLED" {
            eprintln!("Try traceveil demo or traceveil --help.");
        }
    }
    ExitCode::from(code)
}
fn home() -> Result<Option<Command>> {
    let project = std::env::current_dir().map_err(traceveil::io_error)?;
    eprintln!("TRACEVEIL\nDebug your coding agents.\n");
    if let Ok(dir) =
        user_config::load().and_then(|c| discovery::directory(&project, c.session_dir.as_deref()))
    {
        eprintln!(
            "Pi source: {}\nOnly the selected directory will be read.\n",
            presentation::label(&dir.path.display().to_string())
        );
    }
    let choice = prompt::ask(|| {
        dialoguer::Select::new()
            .with_prompt("What would you like to do? (Esc to cancel)")
            .items([
                "Analyze latest Pi session",
                "Browse Pi sessions",
                "Run synthetic demo",
                "Configure session location",
                "Exit",
            ])
            .default(0)
            .interact_opt()
    })?;
    Ok(match choice {
        Some(0) => Some(Command::Analyze(Analysis {
            input: Input {
                last: true,
                repeat_threshold: 3,
                max_gap_seconds: 600,
                ..Input::default()
            },
            fail_on: None,
            require_complete: false,
        })),
        Some(1) => Some(Command::Sessions {
            session_dir: None,
            agent: Some(Agent::Pi),
            project: None,
            metadata: true,
        }),
        Some(2) => Some(Command::Demo),
        Some(3) => Some(Command::Init { session_dir: None }),
        _ => None,
    })
}
fn main() -> ExitCode {
    let wants_json = std::env::args_os()
        .take_while(|a| a != "--")
        .any(|a| a == "--json");
    let mut cli = match Cli::try_parse() {
        Ok(c) => c,
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
    let interactive = io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && io::stderr().is_terminal()
        && !cli.json
        && !cli.quiet;
    let mut command = match cli.command.take() {
        Some(c) => c,
        None => {
            if cli.json {
                let _=emit(format!("{}\n",envelope("welcome",json!({"version":VERSION,"next":["traceveil demo","traceveil analyze --last","traceveil --help"],"session_analyzed":false}))).as_bytes());
                return ExitCode::SUCCESS;
            }
            if !interactive {
                if !cli.quiet {
                    let _ = emit(presentation::welcome().as_bytes());
                }
                return ExitCode::SUCCESS;
            }
            match home() {
                Ok(Some(c)) => c,
                Ok(None) => return ExitCode::SUCCESS,
                Err(e) => return error(e, false),
            }
        }
    };
    if let Command::Sessions { metadata, .. } = &mut command {
        *metadata |= !cli.json;
    }
    if cli.verbose && !cli.json {
        eprintln!(
            "Traceveil {VERSION}: local read-only analysis. User policy is loaded only for analyze/explicit configuration; diagnose keeps original defaults."
        );
    }
    match execute(command, interactive)
        .and_then(|(name, output, status)| Ok((render(name, output, &cli)?, status)))
    {
        Ok((bytes, status)) => match emit(&bytes) {
            Ok(()) => ExitCode::from(status),
            Err(e) => error(e, cli.json),
        },
        Err(e) => error(e, cli.json),
    }
}
