use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "traceveil-product-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p.canonicalize().unwrap())
    }
    fn run(&self, args: &[&str]) -> Output {
        self.input(args, None)
    }
    fn input(&self, args: &[&str], input: Option<&[u8]>) -> Output {
        let mut p = Command::new(env!("CARGO_BIN_EXE_traceveil"))
            .args(args)
            .current_dir(&self.0)
            .env("HOME", &self.0)
            .env("USERPROFILE", &self.0)
            .env("TRACEVEIL_CONFIG", self.0.join("config/settings.json"))
            .env_remove("PI_CODING_AGENT_SESSION_DIR")
            .env_remove("PI_CODING_AGENT_DIR")
            .env_remove("NO_COLOR")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(b) = input {
            p.stdin.take().unwrap().write_all(b).unwrap();
        }
        p.wait_with_output().unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn value(out: &Output) -> Value {
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
    serde_json::from_slice(&out.stdout).unwrap()
}
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmarks/fixtures/demo.pi.jsonl")
}
fn session(path: &Path, project: &Path) {
    let s = fs::read_to_string(fixture()).unwrap();
    let mut rows: Vec<Value> = s
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    rows[0]["cwd"] = json!(project);
    fs::write(
        path,
        rows.iter()
            .map(|r| r.to_string() + "\n")
            .collect::<String>(),
    )
    .unwrap();
}

#[test]
fn installed_style_demo_and_context_bound_inspection_need_no_checkout() {
    let t = Temp::new();
    let out = t.run(&["demo", "--json"]);
    assert!(out.status.success());
    let report = value(&out);
    assert_eq!(report["data"]["synthetic"], true);
    assert_eq!(report["data"]["findings"].as_array().unwrap().len(), 3);
    let reference = report["data"]["findings"][0]["reference"].as_str().unwrap();
    for args in [
        vec!["inspect", reference, "--json"],
        vec!["inspect", "f001", "--demo", "--json"],
    ] {
        let out = t.run(&args);
        assert!(out.status.success());
        assert_eq!(value(&out)["data"]["finding"]["rule_id"], "RF001");
    }
    let out = t.run(&["inspect", "f001", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(value(&out)["error"]["code"], "EVIDENCE_CONTEXT_REQUIRED");
    assert!(!t.0.join("config").exists());
}
#[test]
fn non_tty_welcome_json_quiet_and_completions_are_predictable() {
    let t = Temp::new();
    let out = t.run(&[]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("traceveil demo") && text.contains("No session was analyzed"));
    let out = t.run(&["--json"]);
    assert_eq!(value(&out)["data"]["session_analyzed"], false);
    let out = t.run(&["demo", "--quiet"]);
    assert!(out.status.success() && out.stdout.is_empty() && out.stderr.is_empty());
    let out = t.run(&["demo", "--json", "--color", "always"]);
    assert!(out.status.success());
    assert!(!out.stdout.contains(&27));
    assert_eq!(value(&out)["ok"], true);
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let out = t.run(&["completions", shell]);
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("traceveil"));
    }
    let out = t.run(&["analyze", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        value(&out)["error"]["code"],
        "DISCOVERY_AUTHORIZATION_REQUIRED"
    );
}
#[test]
fn analyze_preserves_diagnose_data_and_policy_changes_references() {
    let t = Temp::new();
    let file = t.0.join("session.jsonl");
    session(&file, &t.0);
    let p = file.to_str().unwrap();
    let a = value(&t.run(&["analyze", p, "--json"]));
    let d = value(&t.run(&["diagnose", p, "--json"]));
    assert_eq!(a["data"], d["data"]);
    let out = t.run(&[
        "analyze",
        p,
        "--suppress",
        "RF001",
        "--rule-threshold",
        "READ001=4",
        "--json",
    ]);
    let changed = value(&out);
    assert!(out.status.success());
    assert_ne!(a["data"]["report_id"], changed["data"]["report_id"]);
    assert!(
        changed["data"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["rule_id"] != "RF001" && f["rule_id"] != "READ001")
    );
    assert_eq!(changed["data"]["coverage"]["rules"][0]["suppressed"], true);
    assert_eq!(changed["data"]["coverage"]["rules"][2]["threshold"], 4);
    let reference = a["data"]["findings"][0]["reference"].as_str().unwrap();
    let out = t.run(&["inspect", reference, p, "--suppress", "RF001", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(value(&out)["error"]["code"], "STALE_REFERENCE");
}
#[test]
fn project_discovery_selects_supported_matching_headers_and_handles_bad_sources() {
    let t = Temp::new();
    let sessions = t.0.join("sessions");
    fs::create_dir(&sessions).unwrap();
    session(&sessions.join("match.jsonl"), &t.0);
    session(&sessions.join("other.jsonl"), &t.0.join("other"));
    fs::write(sessions.join("bad.jsonl"), "not-json\n").unwrap();
    let dir = sessions.to_str().unwrap();
    let out = t.run(&["sessions", "--session-dir", dir, "--metadata", "--json"]);
    assert!(out.status.success());
    let data = value(&out);
    assert!(
        data["data"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["format"] == "malformed-header")
    );
    let out = t.run(&["analyze", "--last", "--session-dir", dir, "--json"]);
    assert!(out.status.success());
    assert_eq!(value(&out)["data"]["findings"].as_array().unwrap().len(), 3);
    let before = fs::read(sessions.join("match.jsonl")).unwrap();
    let out = t.run(&["init", "--session-dir", dir, "--json"]);
    assert!(out.status.success());
    let out = t.run(&["analyze", "--last", "--json"]);
    assert!(out.status.success());
    assert_eq!(before, fs::read(sessions.join("match.jsonl")).unwrap());
}
#[test]
fn user_configuration_validates_policy_and_does_not_create_agent_config() {
    let t = Temp::new();
    for args in [
        vec!["config", "set", "threshold.RF001", "4", "--json"],
        vec!["config", "set", "suppress.READ001", "true", "--json"],
    ] {
        assert!(t.run(&args).status.success());
    }
    let out = t.run(&["config", "show", "--json"]);
    assert_eq!(value(&out)["data"]["rule_thresholds"]["RF001"], 4);
    for args in [
        vec!["config", "set", "threshold.RF001", "2", "--json"],
        vec!["config", "set", "suppress.NOPE", "true", "--json"],
    ] {
        assert_eq!(t.run(&args).status.code(), Some(2));
    }
    assert!(!t.0.join(".pi").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(t.0.join("config/settings.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[test]
fn optional_skill_lifecycle_preserves_modified_and_unrelated_files() {
    let t = Temp::new();
    fs::create_dir(t.0.join(".pi")).unwrap();
    fs::write(t.0.join(".pi/README.md"), "owned by user").unwrap();
    assert!(
        t.run(&["skills", "install", "--agent", "pi", "--json"])
            .status
            .success()
    );
    let skill = t.0.join(".pi/skills/traceveil/SKILL.md");
    assert!(fs::read_to_string(&skill).unwrap().starts_with("---\n"));
    fs::write(&skill, "user modification").unwrap();
    let out = t.run(&["skills", "uninstall", "--agent", "pi", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(value(&out)["error"]["code"], "SKILL_MODIFIED");
    assert!(
        t.run(&["skills", "install", "--agent", "pi", "--force", "--json"])
            .status
            .success()
    );
    fs::write(skill.parent().unwrap().join("notes.txt"), "keep").unwrap();
    assert!(
        t.run(&["skills", "uninstall", "--agent", "pi", "--json"])
            .status
            .success()
    );
    assert!(!skill.exists());
    assert!(skill.parent().unwrap().join("notes.txt").exists());
    assert_eq!(
        fs::read_to_string(t.0.join(".pi/README.md")).unwrap(),
        "owned by user"
    );
}

#[test]
fn project_redirects_need_explicit_authorization_and_jsonl_needs_a_source() {
    let t = Temp::new();
    fs::create_dir(t.0.join(".pi")).unwrap();
    fs::write(
        t.0.join(".pi/settings.json"),
        json!({"sessionDir":t.0}).to_string(),
    )
    .unwrap();
    for args in [
        vec!["analyze", "--last", "--json"],
        vec!["sessions", "--metadata", "--json"],
    ] {
        let out = t.run(&args);
        assert_eq!(out.status.code(), Some(2));
        assert_eq!(
            value(&out)["error"]["code"],
            "DISCOVERY_AUTHORIZATION_REQUIRED"
        );
    }
    let out = t.run(&["analyze", "--agent", "jsonl", "--json"]);
    assert_eq!(value(&out)["error"]["code"], "INVALID_ARGUMENTS");
}
#[test]
fn bounded_config_and_receipt_readers_preserve_existing_files() {
    let t = Temp::new();
    fs::create_dir(t.0.join("config")).unwrap();
    let bytes = vec![b'x'; 65537];
    fs::write(t.0.join("config/settings.json"), &bytes).unwrap();
    let out = t.run(&["config", "set", "threshold.RF001", "4", "--json"]);
    assert_eq!(value(&out)["error"]["code"], "INPUT_TOO_LARGE");
    assert_eq!(fs::read(t.0.join("config/settings.json")).unwrap(), bytes);
    let path = t.0.join(".pi/skills/traceveil");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("SKILL.md"), "user owned").unwrap();
    let out = t.run(&["skills", "install", "--force", "--json"]);
    assert_eq!(value(&out)["error"]["code"], "SKILL_NOT_OWNED");
    fs::File::create(path.join(".traceveil-install.json"))
        .unwrap()
        .set_len(16777216)
        .unwrap();
    let out = t.run(&["skills", "install", "--json"]);
    assert_eq!(value(&out)["error"]["code"], "INPUT_TOO_LARGE");
    assert_eq!(
        fs::read_to_string(path.join("SKILL.md")).unwrap(),
        "user owned"
    );
}
#[test]
fn suppression_is_visible_and_does_not_mean_missing_telemetry() {
    let t = Temp::new();
    let out = t.run(&[
        "analyze",
        "--demo",
        "--suppress",
        "READ001",
        "--require-complete",
        "--json",
    ]);
    assert!(out.status.success());
    let r = value(&out);
    assert_eq!(r["data"]["coverage"]["rules"][2]["status"], "skipped");
    assert_eq!(r["data"]["coverage"]["rules"][2]["suppressed"], true);
}
#[cfg(unix)]
#[test]
fn private_writes_and_uninstall_refuse_symlinked_ancestors_and_receipts() {
    use std::os::unix::fs::symlink;
    let t = Temp::new();
    let outside = t.0.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, t.0.join("config")).unwrap();
    let out = t.run(&["config", "set", "threshold.RF001", "4", "--json"]);
    assert_eq!(value(&out)["error"]["code"], "DESTINATION_UNSAFE");
    assert!(!outside.join("settings.json").exists());
    fs::remove_file(t.0.join("config")).unwrap();
    fs::create_dir(t.0.join(".pi")).unwrap();
    symlink(&outside, t.0.join(".pi/skills")).unwrap();
    let out = t.run(&["skills", "install", "--json"]);
    assert_eq!(value(&out)["error"]["code"], "DESTINATION_UNSAFE");
    assert!(!outside.join("traceveil").exists());
    fs::remove_file(t.0.join(".pi/skills")).unwrap();
    assert!(t.run(&["skills", "install", "--json"]).status.success());
    let path = t.0.join(".pi/skills/traceveil");
    fs::remove_file(path.join(".traceveil-install.json")).unwrap();
    fs::write(outside.join("receipt"), "preserve").unwrap();
    symlink(
        outside.join("receipt"),
        path.join(".traceveil-install.json"),
    )
    .unwrap();
    let out = t.run(&["skills", "uninstall", "--json"]);
    assert_eq!(value(&out)["error"]["code"], "DESTINATION_UNSAFE");
    assert_eq!(
        fs::read_to_string(outside.join("receipt")).unwrap(),
        "preserve"
    );
    assert!(path.join("SKILL.md").exists());
}

#[test]
fn narrow_monochrome_reports_wrap_without_ansi_color() {
    let out = Command::new(env!("CARGO_BIN_EXE_traceveil"))
        .args(["demo", "--color", "always"])
        .env("NO_COLOR", "1")
        .env("COLUMNS", "40")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains('\x1b'));
    for line in text.lines() {
        assert!(line.chars().count() <= 40, "overwide: {line}");
    }
}

#[test]
fn project_receipts_identify_distributed_content_but_do_not_grant_replacement_authority() {
    for command in [
        vec!["skills", "uninstall", "--json"],
        vec!["skills", "install", "--force", "--json"],
    ] {
        let t = Temp::new();
        let dir = t.0.join(".pi/skills/traceveil");
        fs::create_dir_all(&dir).unwrap();
        let text = "user notes, not a distributed skill";
        fs::write(dir.join("SKILL.md"), text).unwrap();
        fs::write(
            dir.join(".traceveil-install.json"),
            json!({"version":traceveil::VERSION,"sha256":traceveil::digest(text)}).to_string(),
        )
        .unwrap();
        let out = t.run(&command);
        assert_eq!(out.status.code(), Some(2));
        assert_eq!(value(&out)["error"]["code"], "SKILL_NOT_OWNED");
        assert_eq!(fs::read_to_string(dir.join("SKILL.md")).unwrap(), text);
    }
    for command in [
        vec!["skills", "uninstall", "--json"],
        vec!["skills", "install", "--json"],
    ] {
        let t = Temp::new();
        assert!(t.run(&["skills", "install", "--json"]).status.success());
        let path = t.0.join(".pi/skills/traceveil/SKILL.md");
        fs::write(&path, "user notes").unwrap();
        let out = t.run(&command);
        assert_eq!(out.status.code(), Some(2));
        assert_eq!(value(&out)["error"]["code"], "SKILL_MODIFIED");
        assert_eq!(fs::read_to_string(path).unwrap(), "user notes");
    }
}

#[test]
fn inspection_replays_pinned_policy_for_files_and_stdin_after_configuration_changes() {
    let t = Temp::new();
    let file = t.0.join("source with spaces.jsonl");
    session(&file, &t.0);
    let path = file.to_str().unwrap();
    assert!(
        t.run(&["config", "set", "threshold.RF001", "4", "--json"])
            .status
            .success()
    );
    assert!(
        t.run(&["config", "set", "suppress.READ001", "true", "--json"])
            .status
            .success()
    );
    let legacy = value(&t.run(&["diagnose", path, "--json"]));
    let reference = legacy["data"]["findings"][0]["reference"].as_str().unwrap();
    let old = value(&t.run(&["inspect", reference, path, "--json"]));
    assert_eq!(old["data"]["finding"], legacy["data"]["findings"][0]);
    for stdin in [false, true] {
        let mut args = vec!["analyze"];
        if stdin {
            args.push("--stdin");
        } else {
            args.push(path);
        }
        args.extend([
            "--repeat-threshold",
            "4",
            "--rule-threshold",
            "LOOP001=3",
            "--max-gap-seconds",
            "321",
        ]);
        let input = stdin.then_some(include_bytes!("../benchmarks/fixtures/demo.jsonl").as_slice());
        let human = t.input(&args, input);
        assert!(human.status.success());
        let text = String::from_utf8(human.stdout).unwrap();
        let start = text.find("traceveil inspect").unwrap();
        let text = &text[start..];
        let end = text.find("Replace SESSION").unwrap();
        let mut replay: Vec<String> = text[..end]
            .split_whitespace()
            .skip(1)
            .map(|s| s.trim_matches('\'').to_owned())
            .collect();
        let position = replay.iter().position(|s| s == "SESSION").unwrap();
        replay[position] = if stdin { "--stdin".into() } else { path.into() };
        replay.push("--json".into());
        fs::write(
            t.0.join("config/settings.json"),
            "invalid unrelated configuration",
        )
        .unwrap();
        let refs: Vec<_> = replay.iter().map(String::as_str).collect();
        let result = t.input(&refs, input);
        assert!(
            result.status.success(),
            "{:?}",
            String::from_utf8_lossy(&result.stdout)
        );
        let inspection = value(&result);
        assert_eq!(inspection["data"]["max_gap_seconds"], 321);
        assert_eq!(inspection["data"]["finding"]["rule_id"], "LOOP001");
        fs::remove_file(t.0.join("config/settings.json")).unwrap();
        assert!(
            t.run(&["config", "set", "threshold.RF001", "4", "--json"])
                .status
                .success()
        );
        assert!(
            t.run(&["config", "set", "suppress.READ001", "true", "--json"])
                .status
                .success()
        );
    }
}
#[test]
fn equivalent_effective_policies_share_identity_and_each_rule_override_is_applied() {
    let t = Temp::new();
    let plain = value(&t.run(&["demo", "--json"]));
    let explicit = value(&t.run(&["analyze", "--demo", "--rule-threshold", "RF001=3", "--json"]));
    assert_eq!(plain["data"]["report_id"], explicit["data"]["report_id"]);
    let global = value(&t.run(&["analyze", "--demo", "--repeat-threshold", "4", "--json"]));
    let individual = value(&t.run(&[
        "analyze",
        "--demo",
        "--rule-threshold",
        "RF001=4",
        "--rule-threshold",
        "LOOP001=4",
        "--rule-threshold",
        "READ001=4",
        "--json",
    ]));
    assert_eq!(global["data"]["report_id"], individual["data"]["report_id"]);
    for id in ["RF001", "LOOP001", "READ001"] {
        let option = format!("{id}=4");
        let report = value(&t.run(&["analyze", "--demo", "--rule-threshold", &option, "--json"]));
        let findings = report["data"]["findings"].as_array().unwrap();
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().all(|f| f["rule_id"] != id));
        let report = value(&t.run(&["analyze", "--demo", "--suppress", id, "--json"]));
        assert!(
            report["data"]["findings"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["rule_id"] != id)
        );
    }
}

#[test]
fn explicit_relative_configuration_uses_the_selected_working_directory() {
    let t = Temp::new();
    let out = Command::new(env!("CARGO_BIN_EXE_traceveil"))
        .args(["config", "set", "threshold.RF001", "4", "--json"])
        .current_dir(&t.0)
        .env("HOME", &t.0)
        .env("TRACEVEIL_CONFIG", "relative.json")
        .output()
        .unwrap();
    assert!(out.status.success());
    let config: Value =
        serde_json::from_slice(&fs::read(t.0.join("relative.json")).unwrap()).unwrap();
    assert_eq!(config["rule_thresholds"]["RF001"], 4);
}

#[cfg(unix)]
#[test]
fn non_utf8_agent_configuration_is_rejected_before_lossy_path_resolution() {
    use std::os::unix::ffi::OsStringExt;
    let t = Temp::new();
    let out = Command::new(env!("CARGO_BIN_EXE_traceveil"))
        .args(["agents", "--json"])
        .current_dir(&t.0)
        .env("HOME", &t.0)
        .env(
            "PI_CODING_AGENT_DIR",
            std::ffi::OsString::from_vec(vec![b'~', b'/', 255]),
        )
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(value(&out)["error"]["code"], "PATH_ENCODING_UNSUPPORTED");
}
