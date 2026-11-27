use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
use traceveil::{
    MAX_INPUT_BYTES, MAX_RECORD_BYTES, adapters, diagnostics::Settings, report::Report,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benchmarks/fixtures")
        .join(name)
}
fn parse(rows: &[Value]) -> Report {
    adapters::jsonl(
        Cursor::new(
            rows.iter()
                .map(|v| v.to_string() + "\n")
                .collect::<String>(),
        ),
        Settings::default(),
    )
    .unwrap()
}
fn op(i: usize, tool: &str, success: Option<bool>, text: Option<&str>) -> Value {
    json!({"schema_version":"1.0","event_id":format!("e{i}"),"session_id":"test","event_type":"tool.execution.completed","data":{"tool":tool,"arguments":{"path":"file"},"success":success,"output":text,"output_complete":true}})
}
fn boundary(i: usize, kind: &str) -> Value {
    json!({"schema_version":"1.0","event_id":format!("e{i}"),"session_id":"test","event_type":kind})
}
fn rules(r: &Report) -> Vec<&str> {
    r.findings.iter().map(|f| f.rule_id).collect()
}
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "traceveil-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self(path)
    }
    fn file(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, content).unwrap();
        path
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn cli(args: &[&str], stdin: Option<&[u8]>) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_traceveil"))
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(bytes) = stdin {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    child.wait_with_output().unwrap()
}
fn data(out: &std::process::Output) -> Value {
    assert!(
        out.stderr.is_empty(),
        "stderr was not empty: {:?}",
        out.stderr
    );
    serde_json::from_slice::<Value>(&out.stdout).unwrap()
}
#[test]
fn labeled_dev_and_heldout_cases_match_contract() {
    let labels: Value = serde_json::from_slice(&fs::read(fixture("labels.json")).unwrap()).unwrap();
    for case in labels["cases"].as_array().unwrap() {
        let path = fixture(case["file"].as_str().unwrap());
        let report =
            adapters::jsonl(Cursor::new(fs::read(&path).unwrap()), Settings::default()).unwrap();
        let mut found = rules(&report);
        found.sort();
        found.dedup();
        let expected: Vec<_> = case["expected_rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(found, expected, "case {}", path.display());
    }
}
#[test]
fn pi_and_canonical_demo_are_equivalent_and_read_only() {
    let path = fixture("demo.pi.jsonl");
    let before = fs::read(&path).unwrap();
    let pi = adapters::pi(&path, None, Settings::default()).unwrap();
    let canonical = adapters::jsonl(
        Cursor::new(fs::read(fixture("demo.jsonl")).unwrap()),
        Settings::default(),
    )
    .unwrap();
    assert_eq!(rules(&pi), vec!["RF001", "LOOP001", "READ001"]);
    assert_eq!(rules(&pi), rules(&canonical));
    assert_eq!(
        pi.findings
            .iter()
            .map(|f| f.occurrences)
            .collect::<Vec<_>>(),
        canonical
            .findings
            .iter()
            .map(|f| f.occurrences)
            .collect::<Vec<_>>()
    );
    assert_eq!(before, fs::read(path).unwrap());
    assert!(pi.complete);
    for f in pi.findings {
        for e in f.evidence {
            assert!(e.record_line > 1);
            assert!(e.call_record_line.unwrap() < e.record_line);
        }
    }
}
#[test]
fn branch_selection_excludes_abandoned_failures_and_metadata_leaf_is_valid() {
    let t = Temp::new();
    let original = fs::read_to_string(fixture("demo.pi.jsonl")).unwrap();
    let mut rows: Vec<Value> = original
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let previous = rows.last().unwrap()["id"].clone();
    rows.push(json!({"type":"message","id":"alternate","parentId":null,"message":{"role":"user","content":"New root"}}));
    rows.push(json!({"type":"label","id":"alternate-label","parentId":"alternate","targetId":"alternate","label":"new tip"}));
    let path = t.file(
        "branches.jsonl",
        rows.iter()
            .map(|v| v.to_string() + "\n")
            .collect::<String>(),
    );
    assert!(
        adapters::pi(&path, None, Settings::default())
            .unwrap()
            .findings
            .is_empty()
    );
    let old = adapters::pi(&path, previous.as_str(), Settings::default()).unwrap();
    assert_eq!(rules(&old), vec!["RF001", "LOOP001", "READ001"]);
    assert_eq!(
        adapters::pi(&path, Some("missing"), Settings::default())
            .unwrap_err()
            .code,
        "LEAF_NOT_FOUND"
    );
}
#[test]
fn broken_duplicate_and_future_pi_lineage_fail_closed() {
    let t = Temp::new();
    let header = json!({"type":"session","version":3,"id":"s"});
    for entries in [
        vec![json!({"type":"label","id":"a","parentId":"missing"})],
        vec![json!({"type":"label","id":"a","parentId":"a"})],
        vec![
            json!({"type":"label","id":"a","parentId":null}),
            json!({"type":"label","id":"a","parentId":null}),
        ],
    ] {
        let mut content = header.to_string() + "\n";
        for e in entries {
            content.push_str(&(e.to_string() + "\n"));
        }
        let path = t.file("bad.jsonl", &content);
        assert!(adapters::pi(&path, None, Settings::default()).is_err());
        assert_eq!(content.as_bytes(), fs::read(path).unwrap());
    }
    let path = t.file(
        "old.jsonl",
        "{\"type\":\"session\",\"version\":1,\"id\":\"legacy\"}\n",
    );
    assert_eq!(
        adapters::pi(&path, None, Settings::default())
            .unwrap_err()
            .code,
        "SCHEMA_VERSION_UNSUPPORTED"
    );
}
#[test]
fn object_key_order_is_irrelevant_but_array_order_and_types_are_not() {
    let mut rows = vec![
        op(0, "bash", Some(false), Some("error")),
        op(1, "bash", Some(false), Some("error")),
        op(2, "bash", Some(false), Some("error")),
    ];
    rows[0]["data"]["arguments"] = json!({"a":1,"b":2});
    rows[1]["data"]["arguments"] = serde_json::from_str("{\"b\":2,\"a\":1}").unwrap();
    rows[2]["data"]["arguments"] = json!({"a":1,"b":2});
    assert_eq!(rules(&parse(&rows)), vec!["RF001"]);
    rows[1]["data"]["arguments"] = json!({"a":"1","b":2});
    assert!(parse(&rows).findings.is_empty());
    for (i, row) in rows.iter_mut().enumerate() {
        row["data"]["arguments"] = json!({"array":if i==1 {vec![2,1]}else{vec![1,2]}});
    }
    assert!(parse(&rows).findings.is_empty());
}
#[test]
fn retries_read_verification_mutation_and_user_boundaries_are_conservative() {
    assert!(
        parse(&[
            op(0, "read", Some(true), Some("same")),
            op(1, "read", Some(true), Some("same"))
        ])
        .findings
        .is_empty()
    );
    let rows = vec![
        op(0, "read", Some(true), Some("same")),
        op(1, "read", Some(true), Some("same")),
        op(
            2,
            "bash",
            Some(false),
            Some("may have written before failing"),
        ),
        op(3, "read", Some(true), Some("same")),
        op(4, "read", Some(true), Some("same")),
    ];
    assert!(!rules(&parse(&rows)).contains(&"READ001"));
    let report = parse(&[
        op(0, "bash", Some(false), Some("error")),
        op(1, "bash", Some(false), Some("error")),
        boundary(2, "message.user"),
        op(3, "bash", Some(false), Some("error")),
    ]);
    assert!(report.findings.is_empty());
}
#[test]
fn missing_success_output_completeness_and_arguments_remain_unknown() {
    let report = parse(
        &(0..3)
            .map(|i| op(i, "bash", None, Some("error")))
            .collect::<Vec<_>>(),
    );
    assert!(report.findings.is_empty());
    assert_eq!(report.coverage.known_success, 0);
    assert!(!report.complete);
    assert!(report.coverage.rules.iter().all(|r| r.status == "skipped"));
    let mixed = parse(&[
        op(0, "bash", Some(true), Some("ok")),
        op(1, "bash", Some(false), Some("error")),
        op(2, "bash", None, Some("unknown")),
    ]);
    assert_eq!(mixed.coverage.known_success, 2);
    assert!(!mixed.complete);
    assert!(mixed.coverage.rules.iter().all(|r| r.status
        == if r.eligible_observations == 0 {
            "skipped"
        } else {
            "partial"
        }));
    let mut rows: Vec<_> = (0..3)
        .map(|i| op(i, "read", Some(true), Some("same")))
        .collect();
    for row in &mut rows {
        row["data"]["output_complete"] = Value::Null;
    }
    assert!(parse(&rows).findings.is_empty());
    for row in &mut rows {
        row["data"]["arguments"] = Value::Null;
    }
    let report = parse(&rows);
    assert!(!report.complete);
    assert!(report.findings.is_empty());
}
#[test]
fn gaps_unknown_types_and_malformed_records_break_pattern_windows() {
    let mut content = op(0, "bash", Some(false), Some("error")).to_string()
        + "\n"
        + &op(1, "bash", Some(false), Some("error")).to_string()
        + "\n{malformed\n";
    content += &(op(2, "bash", Some(false), Some("error")).to_string() + "\n");
    let report = adapters::jsonl(Cursor::new(content), Settings::default()).unwrap();
    assert!(!report.complete);
    assert_eq!(report.coverage.malformed_records, 1);
    assert!(report.findings.is_empty());
    let report = parse(&[
        op(0, "bash", Some(false), Some("error")),
        op(1, "bash", Some(false), Some("error")),
        boundary(2, "unknown.event"),
        op(3, "bash", Some(false), Some("error")),
    ]);
    assert!(!report.complete);
    assert!(report.findings.is_empty());
    assert_eq!(report.coverage.unknown_records, 1);
}
#[test]
fn canonical_branch_duplicate_and_mixed_session_inputs_are_rejected() {
    let mut rows = [
        op(0, "bash", Some(false), Some("error")),
        op(1, "bash", Some(false), Some("error")),
    ];
    rows[1]["parent_event_id"] = json!("missing");
    let bytes = rows
        .iter()
        .map(|r| r.to_string() + "\n")
        .collect::<String>();
    assert_eq!(
        adapters::jsonl(Cursor::new(bytes), Settings::default())
            .unwrap_err()
            .code,
        "EVENT_LINEAGE_INVALID"
    );
    rows[1] = rows[0].clone();
    let bytes = rows
        .iter()
        .map(|r| r.to_string() + "\n")
        .collect::<String>();
    assert_eq!(
        adapters::jsonl(Cursor::new(bytes), Settings::default())
            .unwrap_err()
            .code,
        "DUPLICATE_EVENT_ID"
    );
    rows[1]["event_id"] = json!("new");
    rows[1]["session_id"] = json!("other");
    let bytes = rows
        .iter()
        .map(|r| r.to_string() + "\n")
        .collect::<String>();
    assert_eq!(
        adapters::jsonl(Cursor::new(bytes), Settings::default())
            .unwrap_err()
            .code,
        "MULTIPLE_SESSIONS"
    );
}
#[test]
fn overlapping_calls_are_not_flattened_into_execution_sequences() {
    let mut rows = vec![];
    for i in 0..3 {
        rows.push(json!({"schema_version":"1.0","event_id":format!("start{i}"),"session_id":"test","event_type":"tool.execution.started","data":{"tool":"bash","call_id":format!("c{i}"),"arguments":{"command":"false"}}}));
    }
    for i in 0..3 {
        rows.push(json!({"schema_version":"1.0","event_id":format!("result{i}"),"session_id":"test","event_type":"tool.execution.completed","data":{"tool":"bash","call_id":format!("c{i}"),"success":false,"output":"error"}}));
    }
    let report = parse(&rows);
    assert!(report.findings.is_empty());
    assert!(!report.complete);
    assert_eq!(report.coverage.tool_calls, 3);
}
#[test]
fn resource_bounds_are_checked_before_unbounded_record_allocation() {
    let bytes = vec![b'x'; MAX_RECORD_BYTES + 1];
    assert_eq!(
        adapters::jsonl(Cursor::new(bytes), Settings::default())
            .unwrap_err()
            .code,
        "INPUT_TOO_LARGE"
    );
    let t = Temp::new();
    let path = t.file("large.jsonl", b"");
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(MAX_INPUT_BYTES + 1)
        .unwrap();
    assert_eq!(
        adapters::open_source(&path).unwrap_err().code,
        "INPUT_TOO_LARGE"
    );
    assert!(adapters::jsonl(Cursor::new(b""), Settings::default()).is_err());
    let mut row = op(0, "bash", Some(false), Some("error"));
    row["schema_version"] = json!("99.0");
    assert_eq!(
        adapters::jsonl(Cursor::new(row.to_string()), Settings::default())
            .unwrap_err()
            .code,
        "SCHEMA_VERSION_UNSUPPORTED"
    );
}
#[test]
fn malicious_contents_are_not_executed_or_emitted_on_any_report_path() {
    let t = Temp::new();
    let marker = t.0.join("MUST_NOT_EXIST");
    let secret = "sk-private-secret-DO-NOT-EMIT\u{1b}[2J\u{1b}]52;c;verification test vector\u{7}";
    let mut rows: Vec<_> = (0..3)
        .map(|i| op(i, secret, Some(false), Some(secret)))
        .collect();
    for row in &mut rows {
        row["session_id"] = json!(secret);
        row["data"]["arguments"] = json!({"command":format!("touch {}",marker.display()),"token":secret,"path":"/etc/shadow"});
    }
    let input = rows
        .iter()
        .map(|r| r.to_string() + "\n")
        .collect::<String>();
    let path = t.file("hostile.jsonl", input);
    let path = path.to_str().unwrap();
    let output = cli(&["diagnose", path, "--agent", "jsonl", "--json"], None);
    assert!(output.status.success());
    let report = data(&output);
    assert!(!marker.exists());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("sk-private"));
    assert!(!text.contains("/etc/shadow"));
    assert!(!text.contains("touch "));
    assert!(!text.contains('\u{1b}'));
    let reference = report["data"]["findings"][0]["reference"].as_str().unwrap();
    let inspected = cli(
        &["inspect", reference, path, "--agent", "jsonl", "--json"],
        None,
    );
    assert!(inspected.status.success());
    assert!(
        !String::from_utf8(inspected.stdout)
            .unwrap()
            .contains("sk-private")
    );
    let malformed = t.file("malformed.jsonl", secret);
    let out = cli(
        &[
            "diagnose",
            malformed.to_str().unwrap(),
            "--agent",
            "jsonl",
            "--json",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(
        !String::from_utf8(out.stdout)
            .unwrap()
            .contains("sk-private")
    );
}
#[test]
fn cli_stdin_inspection_stale_references_and_exit_policy_work() {
    let bytes = fs::read(fixture("demo.jsonl")).unwrap();
    let out = cli(&["diagnose", "--stdin", "--json"], Some(&bytes));
    assert!(out.status.success());
    let value = data(&out);
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["findings"].as_array().unwrap().len(), 3);
    let path = fixture("demo.jsonl");
    let path = path.to_str().unwrap();
    let reference = value["data"]["findings"][0]["reference"].as_str().unwrap();
    let out = cli(
        &["inspect", reference, path, "--agent", "jsonl", "--json"],
        None,
    );
    assert!(out.status.success());
    assert_eq!(data(&out)["data"]["finding"]["rule_id"], "RF001");
    let out = cli(
        &[
            "inspect",
            reference,
            path,
            "--agent",
            "jsonl",
            "--repeat-threshold",
            "4",
            "--json",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(data(&out)["error"]["code"], "STALE_REFERENCE");
    let out = cli(
        &[
            "diagnose",
            path,
            "--agent",
            "jsonl",
            "--fail-on",
            "medium",
            "--json",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(data(&out)["ok"], true);
    let out = cli(
        &[
            "diagnose",
            fixture("dev/failures.jsonl").to_str().unwrap(),
            "--agent",
            "jsonl",
            "--require-complete",
            "--json",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(data(&out)["ok"], true);
    let out = cli(&["diagnose", "--json"], None);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(data(&out)["error"]["code"], "INVALID_ARGUMENTS");
}
#[test]
fn discovery_is_scoped_and_last_uses_the_selected_directory() {
    let t = Temp::new();
    t.file("demo.jsonl", fs::read(fixture("demo.pi.jsonl")).unwrap());
    t.file("ignore.txt", b"not a session");
    fs::create_dir(t.0.join("nested")).unwrap();
    fs::write(t.0.join("nested/private.jsonl"), "private").unwrap();
    let dir = t.0.to_str().unwrap();
    let out = cli(&["sessions", "--session-dir", dir, "--json"], None);
    assert!(out.status.success());
    assert_eq!(data(&out)["data"]["count"], 1);
    let out = cli(
        &[
            "diagnose",
            "--agent",
            "pi",
            "--last",
            "--session-dir",
            dir,
            "--compact",
        ],
        None,
    );
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("Potential execution loop")
    );
}
#[test]
fn no_newline_valid_records_and_unmatched_calls_are_explicit() {
    let row = op(0, "bash", Some(false), Some("error"));
    let report = adapters::jsonl(Cursor::new(row.to_string()), Settings::default()).unwrap();
    assert_eq!(report.coverage.tool_results, 1);
    let row = json!({"schema_version":"1.0","event_id":"start","session_id":"test","event_type":"tool.execution.started","data":{"tool":"bash","call_id":"c","arguments":{}}});
    let report = parse(&[row]);
    assert!(!report.complete);
    assert_eq!(report.coverage.unpaired_calls, 1);
}
#[test]
fn output_caps_preserve_warning_and_evidence_omission_counts() {
    let mut rows = vec![];
    for i in 0..50 {
        rows.push(boundary(i, "unknown"));
    }
    let report = parse(&rows);
    assert_eq!(report.warnings.len(), traceveil::MAX_WARNINGS);
    assert!(report.warnings_omitted > 0);
    let report = parse(
        &(0..30)
            .map(|i| op(i, "bash", Some(false), Some("error")))
            .collect::<Vec<_>>(),
    );
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].occurrences, 30);
    assert_eq!(report.findings[0].evidence.len(), traceveil::MAX_EVIDENCE);
    assert_eq!(report.findings[0].evidence_omitted, 18);
}
#[cfg(unix)]
#[test]
fn final_symlinks_are_rejected_and_discovery_ignores_them() {
    use std::os::unix::fs::symlink;
    let t = Temp::new();
    let source = t.file("source.txt", b"private");
    let link = t.0.join("link.jsonl");
    symlink(&source, &link).unwrap();
    assert_eq!(
        adapters::open_source(&link).unwrap_err().code,
        "SOURCE_NOT_REGULAR"
    );
    assert!(adapters::discover(&t.0).unwrap().is_empty());
}
#[test]
fn runtime_skill_matches_binary_and_is_available_offline() {
    let out = cli(&["skills", "get", "core"], None);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(traceveil::VERSION));
    assert!(text.contains("inspect"));
    let out = cli(&["doctor", "--json"], None);
    assert!(out.status.success());
    assert_eq!(data(&out)["data"]["version"], traceveil::VERSION);
}

#[test]
fn timestamp_threshold_is_exact_and_discontinuities_break_patterns() {
    let mut rows: Vec<_> = (0..3)
        .map(|i| op(i, "bash", Some(false), Some("error")))
        .collect();
    for (row, stamp) in rows.iter_mut().zip([
        "2026-11-26T00:00:00Z",
        "2026-11-26T00:10:00Z",
        "2026-11-26T00:20:00Z",
    ]) {
        row["timestamp"] = json!(stamp);
    }
    assert_eq!(rules(&parse(&rows)), vec!["RF001"]);
    rows[1]["timestamp"] = json!("2026-11-26T00:10:00.001Z");
    rows[2]["timestamp"] = json!("2026-11-26T00:10:01Z");
    assert!(parse(&rows).findings.is_empty());
    rows[1]["timestamp"] = json!("invalid private timestamp");
    assert!(parse(&rows).findings.is_empty());
    assert!(!parse(&rows).complete);
    rows[1]["timestamp"] = json!("2026-11-25T23:59:59.999Z");
    assert!(parse(&rows).findings.is_empty());
    assert!(!parse(&rows).complete);
}

#[test]
fn tool_calls_spanning_a_time_boundary_do_not_seed_new_patterns() {
    let mut rows = vec![];
    for i in 0..3 {
        rows.push(json!({"schema_version":"1.0","event_id":format!("start{i}"),"session_id":"test","timestamp":if i==0 {"2026-11-26T00:00:00Z"} else {"2026-11-26T00:10:02Z"},"event_type":"tool.execution.started","data":{"tool":"bash","call_id":format!("c{i}"),"arguments":{"command":"false"}}}));
        rows.push(json!({"schema_version":"1.0","event_id":format!("end{i}"),"session_id":"test","timestamp":"2026-11-26T00:10:02Z","event_type":"tool.execution.completed","data":{"tool":"bash","call_id":format!("c{i}"),"success":false,"output":"error"}}));
    }
    assert!(parse(&rows).findings.is_empty());
}

#[test]
fn duplicate_results_and_corrupted_correlations_are_rejected() {
    let start = json!({"schema_version":"1.0","event_id":"start","session_id":"test","event_type":"tool.execution.started","data":{"tool":"bash","call_id":"call","arguments":{}}});
    let end = json!({"schema_version":"1.0","event_id":"end","session_id":"test","event_type":"tool.execution.completed","data":{"tool":"bash","call_id":"call","success":false,"output":"error"}});
    let mut duplicate = end.clone();
    duplicate["event_id"] = json!("duplicate");
    let input = [start.clone(), end, duplicate]
        .iter()
        .map(|r| r.to_string() + "\n")
        .collect::<String>();
    assert_eq!(
        adapters::jsonl(Cursor::new(input), Settings::default())
            .unwrap_err()
            .code,
        "DUPLICATE_TOOL_RESULT"
    );
    let wrong = json!({"schema_version":"1.0","event_id":"wrong","session_id":"test","event_type":"tool.execution.completed","data":{"tool":"read","call_id":"call","success":false,"output":"error"}});
    let input = [start, wrong]
        .iter()
        .map(|r| {
            r.to_string()
                + "
"
        })
        .collect::<String>();
    assert_eq!(
        adapters::jsonl(Cursor::new(input), Settings::default())
            .unwrap_err()
            .code,
        "TOOL_CORRELATION_INVALID"
    );
}

#[test]
fn finding_limits_and_outstanding_call_limits_are_enforced() {
    let mut rows = vec![];
    for group in 0..70 {
        for repeat in 0..3 {
            rows.push(op(
                group * 3 + repeat,
                "bash",
                Some(false),
                Some(&format!("error {group}")),
            ));
        }
    }
    let report = parse(&rows);
    assert_eq!(report.findings.len(), traceveil::MAX_FINDINGS);
    assert_eq!(report.findings_omitted, 6);
    assert!(!report.complete);
    for cross_boundary in [false, true] {
        let mut starts = vec![];
        for i in 0..traceveil::MAX_PENDING {
            starts.push(json!({"schema_version":"1.0","event_id":format!("s{i}"),"session_id":"test","event_type":"tool.execution.started","data":{"tool":"bash","call_id":format!("c{i}"),"arguments":{}}}));
            if cross_boundary {
                starts.push(boundary(10000 + i, "state.changed"));
            }
        }
        let at_limit = parse(&starts);
        assert_eq!(at_limit.coverage.tool_calls, traceveil::MAX_PENDING);
        assert_eq!(at_limit.coverage.unpaired_calls, traceveil::MAX_PENDING);
        starts.push(json!({"schema_version":"1.0","event_id":"overflow","session_id":"test","event_type":"tool.execution.started","data":{"tool":"bash","call_id":"overflow","arguments":{}}}));
        let input = starts
            .iter()
            .map(|row| row.to_string() + "\n")
            .collect::<String>();
        assert_eq!(
            adapters::jsonl(Cursor::new(input), Settings::default())
                .unwrap_err()
                .code,
            "INPUT_TOO_LARGE"
        );
    }
}
#[test]
fn session_paths_escape_terminal_and_bidi_controls_without_losing_json_values() {
    let t = Temp::new();
    let path = t.file("\u{009b}31m\u{202e}.jsonl", b"not read by discovery");
    let output = cli(
        &["sessions", "--session-dir", t.0.to_str().unwrap(), "--json"],
        None,
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(!text.contains('\u{009b}'));
    assert!(!text.contains('\u{202e}'));
    assert_eq!(data(&output)["data"]["paths"][0], path.to_str().unwrap());
}

#[test]
fn nested_pi_calls_are_reported_as_partial_instead_of_silent_full_coverage() {
    let t = Temp::new();
    let source = fs::read_to_string(fixture("demo.pi.jsonl")).unwrap();
    let mut records: Vec<Value> = source
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let result = records
        .iter_mut()
        .find(|record| record["message"]["role"] == "toolResult")
        .unwrap();
    result["message"]["nestedCalls"] = json!({"calls":[], "complete":false});
    let path = t.file(
        "nested.jsonl",
        records
            .iter()
            .map(|r| r.to_string() + "\n")
            .collect::<String>(),
    );
    let report = adapters::pi(&path, None, Settings::default()).unwrap();
    assert!(!report.complete);
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.code == "NESTED_TOOL_CALLS_NOT_ANALYZED")
    );
}
#[cfg(unix)]
#[test]
fn discovery_rejects_paths_that_would_be_lossily_reencoded() {
    use std::os::unix::ffi::OsStringExt;
    let t = Temp::new();
    let name = std::ffi::OsString::from_vec(vec![255, b'.', b'j', b's', b'o', b'n', b'l']);
    let path = t.0.join(name);
    assert_eq!(
        adapters::validate_paths(std::slice::from_ref(&path))
            .unwrap_err()
            .code,
        "PATH_ENCODING_UNSUPPORTED"
    );
    if let Err(error) = fs::write(&path, b"not read") {
        assert_eq!(error.raw_os_error(), Some(libc::EILSEQ));
        assert!(!path.exists());
        let out = cli(
            &["sessions", "--session-dir", t.0.to_str().unwrap(), "--json"],
            None,
        );
        assert_eq!(out.status.code(), Some(0));
        assert_eq!(data(&out)["data"]["count"], 0);
        return;
    }
    let out = cli(
        &["sessions", "--session-dir", t.0.to_str().unwrap(), "--json"],
        None,
    );
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(data(&out)["error"]["code"], "PATH_ENCODING_UNSUPPORTED");
}

#[test]
fn sibling_call_ids_and_timestamps_do_not_contaminate_selected_lineage() {
    let t = Temp::new();
    let records = [
        json!({"type":"session","version":3,"id":"s"}),
        json!({"type":"message","id":"old-call","parentId":null,"timestamp":"invalid off-branch timestamp","message":{"role":"assistant","content":[{"type":"toolCall","id":"shared","name":"bash","arguments":{"command":"false"}}]}}),
        json!({"type":"message","id":"old-result","parentId":"old-call","message":{"role":"toolResult","toolName":"bash","toolCallId":"shared","isError":true,"content":[{"type":"text","text":"old error"}]}}),
        json!({"type":"message","id":"new-call","parentId":null,"message":{"role":"assistant","content":[{"type":"toolCall","id":"shared","name":"bash","arguments":{"command":"false"}}]}}),
        json!({"type":"message","id":"new-result","parentId":"new-call","message":{"role":"toolResult","toolName":"bash","toolCallId":"shared","isError":true,"content":[{"type":"text","text":"new error"}]}}),
    ];
    let path = t.file(
        "siblings.jsonl",
        records
            .iter()
            .map(|record| record.to_string() + "\n")
            .collect::<String>(),
    );
    let selected = adapters::pi(&path, None, Settings::default()).unwrap();
    assert!(selected.complete);
    assert_eq!(selected.coverage.tool_results, 1);
    let old = adapters::pi(&path, Some("old-result"), Settings::default()).unwrap();
    assert!(!old.complete);
    assert_eq!(old.coverage.tool_results, 1);
}

#[test]
fn native_pi_custom_boundaries_preserve_tool_correlation() {
    let temp = Temp::new();
    let mut records = vec![
        json!({"type":"session","version":3,"id":"synthetic"}),
        json!({"type":"message","id":"call","parentId":null,"message":{"role":"assistant","content":[{"type":"toolCall","id":"c","name":"bash","arguments":{"command":"false"}}]}}),
        json!({"type":"custom","id":"metadata1","parentId":"call","customType":"test","data":{}}),
        json!({"type":"custom","id":"metadata2","parentId":"metadata1","customType":"test","data":{}}),
    ];
    for completed in [false, true] {
        if completed {
            records.push(json!({"type":"message","id":"result","parentId":"metadata2","message":{"role":"toolResult","toolName":"bash","toolCallId":"c","isError":true,"content":[{"type":"text","text":"error"}]}}));
        }
        let path = temp.file(
            "custom-boundary.jsonl",
            records
                .iter()
                .map(|r| r.to_string() + "\n")
                .collect::<String>(),
        );
        let report = adapters::pi(&path, None, Settings::default()).unwrap();
        assert_eq!(report.coverage.tool_calls, 1);
        assert_eq!(report.coverage.tool_results, usize::from(completed));
        assert_eq!(report.coverage.unpaired_calls, usize::from(!completed));
        assert!(report.findings.is_empty());
    }
}

#[test]
fn boundary_correlations_do_not_seed_any_rule_and_recover_after_overlap() {
    for kind in [
        "session.started",
        "message.user",
        "state.changed",
        "context.compacted",
        "session.ended",
        "configuration.changed",
        "unknown.event",
    ] {
        for (tool, success, expected) in [("bash", false, "RF001"), ("read", true, "READ001")] {
            for (interleaved, order) in
                [(false, ["a", "b"]), (true, ["a", "b"]), (true, ["b", "a"])]
            {
                let start = json!({"schema_version":"1.0","event_id":"start","session_id":"test","event_type":"tool.execution.started","data":{"tool":tool,"call_id":"a","arguments":{"path":"file"}}});
                let mut rows = vec![start.clone(), boundary(1000, kind)];
                if interleaved {
                    let mut second = start.clone();
                    second["event_id"] = json!("second-start");
                    second["data"]["call_id"] = json!("b");
                    rows.push(second);
                }
                for (i, id) in order
                    .iter()
                    .take(if interleaved { 2 } else { 1 })
                    .enumerate()
                {
                    let mut end = op(2000 + i, tool, Some(success), Some("same"));
                    end["data"]["call_id"] = json!(id);
                    end["data"]["arguments"] = Value::Null;
                    rows.push(end);
                }
                rows.extend([
                    op(1, tool, Some(success), Some("same")),
                    op(2, tool, Some(success), Some("same")),
                ]);
                let report = parse(&rows);
                assert_eq!(report.coverage.unpaired_calls, 0);
                assert_eq!(report.coverage.tool_calls, if interleaved { 4 } else { 3 });
                assert_eq!(report.coverage.tool_calls, report.coverage.tool_results);
                assert_eq!(
                    report
                        .coverage
                        .rules
                        .iter()
                        .find(|r| r.rule_id == expected)
                        .unwrap()
                        .eligible_observations,
                    2
                );
                assert!(report.findings.is_empty());
                rows.push(op(3, tool, Some(success), Some("same")));
                assert!(rules(&parse(&rows)).contains(&expected));
                let mut duplicate = start.clone();
                duplicate["event_id"] = json!("duplicate-start");
                let bytes = [start, boundary(9999, kind), duplicate]
                    .iter()
                    .map(|r| r.to_string() + "\n")
                    .collect::<String>();
                assert_eq!(
                    adapters::jsonl(Cursor::new(bytes), Settings::default())
                        .unwrap_err()
                        .code,
                    "DUPLICATE_TOOL_CALL"
                );
            }
        }
        let start = json!({"schema_version":"1.0","event_id":"start","session_id":"test","event_type":"tool.execution.started","data":{"tool":"read","call_id":"a","arguments":{"path":"file"}}});
        let mut end = op(0, "read", Some(true), Some("A"));
        end["data"]["call_id"] = json!("a");
        end["data"]["arguments"] = Value::Null;
        let mut rows = vec![start, boundary(1000, kind), end];
        for i in 1..=5 {
            rows.push(op(
                i,
                "read",
                Some(true),
                Some(if i % 2 == 0 { "A" } else { "B" }),
            ));
        }
        let report = parse(&rows);
        assert!(report.findings.is_empty());
        assert_eq!(
            report
                .coverage
                .rules
                .iter()
                .find(|r| r.rule_id == "LOOP001")
                .unwrap()
                .eligible_observations,
            5
        );
        rows.push(op(6, "read", Some(true), Some("A")));
        assert!(rules(&parse(&rows)).contains(&"LOOP001"));
    }
}

#[test]
fn mixed_missing_telemetry_is_partial_and_require_complete_rejects_it() {
    let mut unknown_range = op(3, "read", Some(true), Some("unknown range"));
    unknown_range["data"]["output_complete"] = Value::Null;
    let mut truncated = unknown_range.clone();
    truncated["data"]["output_complete"] = json!(false);
    let mut unknown_scope = op(2, "read", Some(true), Some("unknown path"));
    unknown_scope["data"]["arguments"] = json!({});
    let mut omitted_status = op(2, "bash", None, Some("unknown"));
    omitted_status["data"]
        .as_object_mut()
        .unwrap()
        .remove("success");
    let mut omitted_range = unknown_range.clone();
    omitted_range["data"]
        .as_object_mut()
        .unwrap()
        .remove("output_complete");
    let mut omitted_output = op(2, "bash", Some(false), None);
    omitted_output["data"]
        .as_object_mut()
        .unwrap()
        .remove("output");
    let cases = [
        vec![omitted_range],
        vec![omitted_output],
        vec![op(2, "bash", None, Some("unknown")), unknown_range.clone()],
        vec![omitted_status],
        vec![op(2, "bash", Some(false), None)],
        vec![op(2, "read", Some(true), None)],
        vec![unknown_range],
        vec![truncated],
        vec![unknown_scope],
    ];
    for missing in cases {
        let mut rows = vec![
            op(0, "bash", Some(false), Some("known failure")),
            op(1, "read", Some(true), Some("complete read")),
        ];
        rows.extend(missing);
        let bytes = rows
            .iter()
            .map(|r| r.to_string() + "\n")
            .collect::<String>();
        for require in [false, true] {
            let mut args = vec!["diagnose", "--stdin", "--json"];
            if require {
                args.push("--require-complete");
            }
            let out = cli(&args, Some(bytes.as_bytes()));
            assert_eq!(out.status.code(), Some(if require { 3 } else { 0 }));
            let envelope = data(&out);
            assert_eq!(envelope["ok"], true);
            let report = &envelope["data"];
            assert_eq!(report["complete"], false);
            assert!(!report["warnings"].as_array().unwrap().is_empty());
            assert!(report["findings"].as_array().unwrap().is_empty());
            assert!(
                report["coverage"]["rules"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|r| r["status"] == "partial")
            );
        }
    }
    let rows = [
        op(0, "bash", Some(false), Some("known failure")),
        op(1, "read", Some(true), Some("complete read")),
        op(2, "bash", Some(true), None),
    ];
    let bytes = rows
        .iter()
        .map(|r| r.to_string() + "\n")
        .collect::<String>();
    let out = cli(
        &["diagnose", "--stdin", "--json", "--require-complete"],
        Some(bytes.as_bytes()),
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(data(&out)["data"]["complete"], true);

    let mut failed_read = op(3, "read", Some(false), Some(""));
    failed_read["data"]
        .as_object_mut()
        .unwrap()
        .remove("output_complete");
    let rows = [
        op(0, "bash", Some(false), Some("")),
        op(1, "read", Some(true), Some("")),
        failed_read,
    ];
    let bytes = rows
        .iter()
        .map(|r| r.to_string() + "\n")
        .collect::<String>();
    let out = cli(
        &["diagnose", "--stdin", "--json", "--require-complete"],
        Some(bytes.as_bytes()),
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(data(&out)["data"]["complete"], true);

    for (tool, success) in [("bash", false), ("read", true)] {
        let report = parse(&[
            op(0, tool, Some(success), Some("same")),
            op(1, tool, Some(success), Some("same")),
            op(2, tool, None, Some("unknown")),
            op(3, tool, Some(success), Some("same")),
        ]);
        assert!(!report.complete);
        assert!(report.findings.is_empty());
    }
    let mut rows: Vec<_> = (0..4)
        .map(|i| {
            op(
                i,
                "read",
                Some(true),
                Some(if i % 2 == 0 { "A" } else { "B" }),
            )
        })
        .collect();
    rows.push(op(4, "bash", None, Some("unknown")));
    rows.push(op(5, "read", Some(true), Some("A")));
    rows.push(op(6, "read", Some(true), Some("B")));
    let report = parse(&rows);
    assert!(!report.complete);
    assert!(report.findings.is_empty());

    let bytes = op(0, "bash", None, Some("unknown")).to_string() + "\n";
    let out = cli(
        &["diagnose", "--stdin", "--json", "--require-complete"],
        Some(bytes.as_bytes()),
    );
    assert_eq!(out.status.code(), Some(3));
    let value = data(&out);
    assert_eq!(value["data"]["complete"], false);
    assert!(
        value["data"]["coverage"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["status"] == "skipped")
    );

    let mut rows: Vec<_> = (0..3)
        .map(|i| op(i, "bash", Some(false), Some("same failure")))
        .collect();
    rows.push(op(3, "bash", None, Some("unknown")));
    let bytes = rows
        .iter()
        .map(|r| r.to_string() + "\n")
        .collect::<String>();
    let out = cli(
        &["diagnose", "--stdin", "--json", "--require-complete"],
        Some(bytes.as_bytes()),
    );
    assert_eq!(out.status.code(), Some(3));
    let value = data(&out);
    assert_eq!(value["data"]["complete"], false);
    assert!(
        value["data"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["rule_id"] == "RF001")
    );
}
