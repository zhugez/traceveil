use crate::{
    Error, MAX_INPUT_BYTES, MAX_RECORD_BYTES, MAX_RECORDS, Result, SCHEMA_VERSION,
    diagnostics::{Engine, Settings},
    digest,
    events::{Data, Event, SourcedEvent},
    io_error,
    report::Report,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
};

struct Scan {
    digest: String,
    lines: usize,
}
fn scan<R: BufRead>(
    mut reader: R,
    mut consume: impl FnMut(usize, &[u8]) -> Result<()>,
) -> Result<Scan> {
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut line = Vec::new();
    let mut lines = 0;
    loop {
        let chunk = reader.fill_buf().map_err(io_error)?;
        if chunk.is_empty() {
            break;
        }
        let take = chunk
            .iter()
            .position(|b| *b == b'\n')
            .map_or(chunk.len(), |i| i + 1);
        total += take as u64;
        if total > MAX_INPUT_BYTES || line.len() + take > MAX_RECORD_BYTES {
            return Err(Error::new(
                "INPUT_TOO_LARGE",
                "The input or a record exceeds the documented safety limit.",
            ));
        }
        let ends = chunk[take - 1] == b'\n';
        hash.update(&chunk[..take]);
        line.extend_from_slice(&chunk[..take]);
        reader.consume(take);
        if ends {
            lines += 1;
            if lines > MAX_RECORDS {
                return Err(Error::new("INPUT_TOO_LARGE", "Too many input records."));
            }
            consume(lines, &line)?;
            line.clear();
        }
    }
    if !line.is_empty() {
        lines += 1;
        if lines > MAX_RECORDS {
            return Err(Error::new("INPUT_TOO_LARGE", "Too many input records."));
        }
        consume(lines, &line)?;
    }
    Ok(Scan {
        digest: hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        lines,
    })
}
fn blank(line: &[u8]) -> bool {
    line.iter().all(u8::is_ascii_whitespace)
}

pub fn jsonl(reader: impl BufRead, settings: Settings) -> Result<Report> {
    let mut engine = Engine::new("jsonl", settings)?;
    let mut seen = HashSet::new();
    let mut previous = None;
    let meta = scan(reader, |line, bytes| {
        if blank(bytes) {
            return Ok(());
        }
        let event: Event = match serde_json::from_slice(bytes) {
            Ok(event) => event,
            Err(_) => {
                engine.report.coverage.malformed_records += 1;
                engine.warning("INVALID_EVENT", Some(line));
                engine.gap();
                previous = None;
                return Ok(());
            }
        };
        if event.event_id.is_empty() || event.event_id.len() > 128 {
            return Err(Error::input());
        }
        let id = digest(&event.event_id);
        if !seen.insert(id.clone()) {
            return Err(Error::new(
                "DUPLICATE_EVENT_ID",
                "Event identifiers must be unique.",
            ));
        }
        if let Some(parent) = &event.parent_event_id
            && (parent.len() > 128 || previous.as_ref() != Some(&digest(parent)))
        {
            return Err(Error::new(
                "EVENT_LINEAGE_INVALID",
                "Canonical input must be one ordered, linear session. Select a branch before export.",
            ));
        }
        previous = Some(id);
        engine.report.coverage.selected_records += 1;
        match engine.push(SourcedEvent {
            event,
            line,
            block: 0,
        }) {
            Err(e) if e.code == "INVALID_EVENT" => {
                engine.report.coverage.malformed_records += 1;
                engine.warning("INVALID_EVENT", Some(line));
                engine.gap();
            }
            result => result?,
        }
        Ok(())
    })?;
    engine.report.coverage.records = meta.lines;
    if seen.is_empty() {
        return Err(Error::new(
            "INVALID_EVENT",
            "No valid canonical events were found.",
        ));
    }
    Ok(engine.finish(meta.digest, None))
}

pub fn open_source(path: &Path) -> Result<File> {
    let before = std::fs::symlink_metadata(path).map_err(io_error)?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(Error::new(
            "SOURCE_NOT_REGULAR",
            "The input must be a regular file, without a final symlink.",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(io_error)?;
    let after = file.metadata().map_err(io_error)?;
    if !after.is_file() {
        return Err(Error::new(
            "SOURCE_NOT_REGULAR",
            "The input must be a regular file.",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            return Err(Error::new(
                "SOURCE_CHANGED",
                "The source changed while opening it.",
            ));
        }
    }
    if after.len() > MAX_INPUT_BYTES {
        return Err(Error::new(
            "INPUT_TOO_LARGE",
            "The input exceeds the documented safety limit.",
        ));
    }
    Ok(file)
}
#[derive(Debug)]
struct Node {
    parent: Option<String>,
    line: usize,
}
fn identifier(value: Option<&Value>, max: usize) -> Result<String> {
    value
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= max)
        .map(str::to_owned)
        .ok_or_else(Error::input)
}

pub fn pi(path: &Path, leaf: Option<&str>, settings: Settings) -> Result<Report> {
    pi_reader(open_source(path)?, leaf, settings)
}
fn pi_reader<R: std::io::Read + Seek>(
    mut file: R,
    leaf: Option<&str>,
    settings: Settings,
) -> Result<Report> {
    let mut nodes = HashMap::<String, Node>::new();
    let mut last = None;
    let mut session = None;
    let mut engine = Engine::new("pi", settings)?;
    let first = scan(BufReader::new(&mut file), |line, bytes| {
        if blank(bytes) {
            return Ok(());
        }
        let value: Value = match serde_json::from_slice(bytes) {
            Ok(value) => value,
            Err(_) => {
                engine.report.coverage.malformed_records += 1;
                engine.warning("INVALID_RECORD", Some(line));
                return Ok(());
            }
        };
        if session.is_none() {
            if value["type"] != "session" {
                return Err(Error::new(
                    "SESSION_FORMAT_UNSUPPORTED",
                    "A Pi session v3 header must be the first parsed record.",
                ));
            }
            if value["version"].as_u64() != Some(3) {
                return Err(Error::new(
                    "SCHEMA_VERSION_UNSUPPORTED",
                    "Only Pi session format v3 is supported.",
                ));
            }
            let id = identifier(value.get("id"), 128)?;
            engine.set_session(&id)?;
            session = Some(id);
            return Ok(());
        }
        if value["type"] == "session" {
            return Err(Error::new(
                "MULTIPLE_SESSIONS",
                "A Pi input must contain exactly one session header.",
            ));
        }
        let id = identifier(value.get("id"), 128)?;
        let parent = match value.get("parentId") {
            Some(Value::Null) => None,
            Some(v) => Some(identifier(Some(v), 128)?),
            None => {
                return Err(Error::new(
                    "EVENT_LINEAGE_INVALID",
                    "A Pi entry is missing its parent link.",
                ));
            }
        };
        if let Some(parent) = &parent
            && !nodes.contains_key(parent)
        {
            return Err(Error::new(
                "EVENT_LINEAGE_INVALID",
                "A Pi parent must refer to an earlier entry.",
            ));
        }
        if nodes.insert(id.clone(), Node { parent, line }).is_some() {
            return Err(Error::new(
                "DUPLICATE_EVENT_ID",
                "Pi entry identifiers must be unique.",
            ));
        }
        last = Some(id);
        Ok(())
    })?;
    let session = session.ok_or_else(|| {
        Error::new(
            "SESSION_FORMAT_UNSUPPORTED",
            "No Pi session header was found.",
        )
    })?;
    let chosen = match leaf {
        Some(id) => {
            if id.len() > 128 || !nodes.contains_key(id) {
                return Err(Error::new(
                    "LEAF_NOT_FOUND",
                    "The requested Pi leaf does not exist.",
                ));
            }
            Some(id.to_owned())
        }
        None => last,
    };
    let mut selected = HashSet::new();
    let mut current = chosen.as_ref();
    while let Some(id) = current {
        let node = nodes.get(id).ok_or_else(|| {
            Error::new("EVENT_LINEAGE_INVALID", "A selected ancestor is missing.")
        })?;
        if !selected.insert(node.line) {
            return Err(Error::new(
                "EVENT_LINEAGE_INVALID",
                "A cycle was found in session lineage.",
            ));
        }
        current = node.parent.as_ref();
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let second = scan(BufReader::new(&mut file), |line, bytes| {
        if blank(bytes) {
            return Ok(());
        }
        let value: Value = match serde_json::from_slice(bytes) {
            Ok(v) => v,
            Err(_) => {
                engine.gap();
                return Ok(());
            }
        };
        if !selected.contains(&line) {
            return Ok(());
        }
        engine.report.coverage.selected_records += 1;
        normalize_pi(value, line, &session, &mut engine)
    })?;
    if first.digest != second.digest {
        return Err(Error::new(
            "SOURCE_CHANGED",
            "The source changed during analysis. Retry on a stable offline copy.",
        ));
    }
    engine.report.coverage.records = first.lines;
    Ok(engine.finish(first.digest, chosen.as_deref()))
}
fn pi_event(
    session: &str,
    id: &str,
    event_type: &str,
    data: Data,
    line: usize,
    block: usize,
    engine: &mut Engine,
) -> Result<()> {
    engine.push(SourcedEvent {
        event: Event {
            schema_version: SCHEMA_VERSION.to_owned(),
            event_id: id.to_owned(),
            session_id: session.to_owned(),
            parent_event_id: None,
            timestamp: None,
            event_type: event_type.to_owned(),
            data,
        },
        line,
        block,
    })
}
fn text_content(value: &Value) -> Option<String> {
    let blocks = value.as_array()?;
    if !blocks
        .iter()
        .all(|b| b["type"] == "text" && b["text"].is_string())
    {
        return None;
    }
    Some(
        blocks
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join(
                "
",
            ),
    )
}
fn normalize_pi(value: Value, line: usize, session: &str, engine: &mut Engine) -> Result<()> {
    if value.get("timestamp").is_some() && !value["timestamp"].is_string() {
        engine.warning("TIMESTAMP_INVALID", Some(line));
        engine.gap();
    }
    engine.observe_timestamp(value["timestamp"].as_str(), line);
    let id = value["id"].as_str().ok_or_else(Error::input)?;
    let kind = value["type"].as_str().unwrap_or("");
    match kind {
        "message" => {
            let message = &value["message"];
            match message["role"].as_str() {
                Some("assistant") => {
                    let Some(blocks) = message["content"].as_array() else {
                        engine.warning("MESSAGE_CONTENT_UNAVAILABLE", Some(line));
                        engine.gap();
                        return Ok(());
                    };
                    for (i, block) in blocks.iter().enumerate() {
                        if !matches!(
                            block["type"].as_str(),
                            Some("toolCall" | "text" | "thinking" | "image")
                        ) {
                            engine.report.coverage.unknown_records += 1;
                            engine.warning("CONTENT_BLOCK_UNSUPPORTED", Some(line));
                            engine.gap();
                        }
                        if block["type"] == "toolCall" {
                            let data = Data {
                                tool: block["name"].as_str().map(str::to_owned),
                                call_id: block["id"].as_str().map(str::to_owned),
                                arguments: block.get("arguments").cloned(),
                                ..Data::default()
                            };
                            let eid = format!("{}:call:{i}", &digest(id)[..24]);
                            pi_event(
                                session,
                                &eid,
                                "tool.execution.started",
                                data,
                                line,
                                i,
                                engine,
                            )?;
                        }
                    }
                    if matches!(message["stopReason"].as_str(), Some("error" | "aborted")) {
                        engine.gap();
                    }
                    Ok(())
                }
                Some("toolResult") => {
                    if message
                        .get("nestedCalls")
                        .is_some_and(|value| !value.is_null())
                    {
                        engine.warning("NESTED_TOOL_CALLS_NOT_ANALYZED", Some(line));
                    }
                    let output = text_content(&message["content"]);
                    let complete = output.as_ref().is_some_and(|text| {
                        message.pointer("/details/truncation/truncated") != Some(&Value::Bool(true))
                            && !text.contains("more lines in file.")
                            && !text.contains("[Showing ")
                            && !text.contains("exceeds ")
                            && !text.contains("[Output truncated")
                    });
                    let data = Data {
                        tool: message["toolName"].as_str().map(str::to_owned),
                        call_id: message["toolCallId"].as_str().map(str::to_owned),
                        arguments: None,
                        success: message["isError"].as_bool().map(|e| !e),
                        exit_code: None,
                        output,
                        output_complete: Some(complete),
                    };
                    match pi_event(
                        session,
                        id,
                        "tool.execution.completed",
                        data,
                        line,
                        0,
                        engine,
                    ) {
                        Err(e) if e.code == "INVALID_EVENT" => {
                            engine.warning("INVALID_TOOL_RESULT", Some(line));
                            engine.gap();
                            Ok(())
                        }
                        result => result,
                    }
                }
                Some("user") => pi_event(
                    session,
                    id,
                    "message.user",
                    Data::default(),
                    line,
                    0,
                    engine,
                ),
                Some(
                    "system" | "bashExecution" | "custom" | "branchSummary" | "compactionSummary",
                ) => {
                    engine.gap();
                    Ok(())
                }
                _ => {
                    engine.report.coverage.unknown_records += 1;
                    engine.warning("MESSAGE_ROLE_UNSUPPORTED", Some(line));
                    engine.gap();
                    Ok(())
                }
            }
        }
        "model_change"
        | "thinking_level_change"
        | "compaction"
        | "branch_summary"
        | "context_edit"
        | "custom_message"
        | "custom" => {
            engine.gap();
            Ok(())
        }
        "label" | "session_info" | "usage" => Ok(()),
        _ => {
            engine.report.coverage.unknown_records += 1;
            engine.warning("PI_ENTRY_TYPE_UNSUPPORTED", Some(line));
            engine.gap();
            Ok(())
        }
    }
}

pub fn discover(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    for (i, entry) in std::fs::read_dir(directory).map_err(io_error)?.enumerate() {
        if i >= 10_000 {
            return Err(Error::new(
                "INPUT_TOO_LARGE",
                "Too many directory entries. Select a narrower session directory.",
            ));
        }
        let entry = entry.map_err(io_error)?;
        if entry.file_type().map_err(io_error)?.is_file()
            && entry.path().extension().is_some_and(|s| s == "jsonl")
        {
            let modified = entry
                .metadata()
                .map_err(io_error)?
                .modified()
                .map_err(io_error)?;
            entries.push((modified, entry.path()));
        }
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(entries.into_iter().map(|(_, path)| path).collect())
}

pub fn default_pi_directory() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("PI_CODING_AGENT_SESSION_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| {
            Error::new(
                "CONFIGURATION_INVALID",
                "Set --session-dir explicitly when the home directory is unavailable.",
            )
        })?;
    let cwd = std::env::current_dir().map_err(io_error)?;
    let cwd = cwd.to_str().ok_or_else(|| {
        Error::new(
            "CONFIGURATION_INVALID",
            "Set --session-dir for a working directory that is not UTF-8.",
        )
    })?;
    let encoded = cwd
        .trim_start_matches(['/', '\\'])
        .replace(['/', '\\', ':'], "-");
    Ok(PathBuf::from(home)
        .join(".pi/agent/sessions")
        .join(format!("--{encoded}--")))
}

#[cfg(test)]
#[path = "adapter_tests.rs"]
mod tests;
