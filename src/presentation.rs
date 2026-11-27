use std::{fmt::Write, io::IsTerminal};
use traceveil::{VERSION, report::Report};

pub fn safe(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        let n = c as u32;
        if (n < 32 && n != 10 && n != 9)
            || (127..=159).contains(&n)
            || n == 0x061c
            || (0x200e..=0x200f).contains(&n)
            || (0x202a..=0x202e).contains(&n)
            || (0x2066..=0x2069).contains(&n)
        {
            let _ = write!(out, "\\u{n:04x}");
        } else {
            out.push(c);
        }
    }
    out
}
pub fn color(mode: &str) -> bool {
    mode != "never"
        && std::env::var_os("NO_COLOR").is_none()
        && (mode == "always" || std::io::stdout().is_terminal())
}
fn accent(text: &str, code: &str, on: bool) -> String {
    if on {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.into()
    }
}
pub fn width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| console::Term::stdout().size().1 as usize)
        .clamp(40, 160)
}
fn wrapped(text: &str, indent: usize) -> String {
    let limit = width().saturating_sub(indent);
    let mut out = String::new();
    let mut n = 0;
    for word in text.split_whitespace() {
        let w = console::measure_text_width(word);
        if n > 0 && n + 1 + w > limit {
            out.push('\n');
            n = 0;
        }
        if n == 0 {
            out.push_str(&" ".repeat(indent));
        } else {
            out.push(' ');
            n += 1;
        }
        out.push_str(word);
        n += w;
    }
    out.push('\n');
    out
}
pub fn welcome() -> String {
    format!(
        "TRACEVEIL {VERSION}\nDebug your coding agents.\n\nStart here\n  traceveil demo                 Try a synthetic session, offline\n  traceveil analyze --last       Analyze this project’s latest Pi session\n  traceveil sessions             Browse this project’s Pi sessions\n  traceveil init --session-dir DIR\n                                 Use a custom session directory\n\nRun traceveil --help for commands. No session was analyzed.\n"
    )
}
pub fn report(r: &Report, compact: bool, colors: bool, demo: bool) -> String {
    let mut out = format!(
        "{}\nDebug your coding agents.\n\n",
        accent("TRACEVEIL — SESSION REPORT", "1;36", colors)
    );
    if demo {
        out.push_str("SYNTHETIC DEMO — generated through the production diagnostic engine.\n\n");
    }
    let coverage = if !r.complete {
        "Partial — some evidence is unavailable"
    } else if r
        .coverage
        .rules
        .iter()
        .any(|r| r.status == "skipped" && !r.suppressed)
    {
        "Limited — some enabled rules have no eligible evidence"
    } else {
        "Complete for supported observations"
    };
    let _ = write!(
        out,
        "Agent       {}\nSession     {}\nRecords     {} selected / {} recorded\nTool calls  {} ({} results)\nCoverage    {}\n\n",
        if r.adapter == "pi" {
            "Pi v3"
        } else {
            "Canonical JSONL"
        },
        r.report_id,
        r.coverage.selected_records,
        r.coverage.records,
        r.coverage.tool_calls,
        r.coverage.tool_results,
        coverage
    );
    out.push_str("FINDINGS\n");
    if r.findings.is_empty() {
        out.push_str("  No matching patterns in the available evidence.\n  This is not proof that the session was healthy or the task succeeded.\n");
    }
    for f in &r.findings {
        let sev = f.severity.to_uppercase();
        let code = match f.severity {
            "low" => "36",
            "medium" => "33",
            _ => "1;31",
        };
        let _ = writeln!(out, "\n  {}  {}", accent(&sev, code, colors), f.description);
        let _ = writeln!(
            out,
            "          {} matching observations · {}",
            f.occurrences, f.rule_id
        );
        let _ = writeln!(out, "          Evidence: {}", f.reference);
        if !compact {
            out.push_str(&wrapped(f.recommendation, 10));
        }
    }
    out.push_str("\nCOVERAGE\n");
    for rule in &r.coverage.rules {
        let _ = writeln!(
            out,
            "  {:7}  {:9}  {} eligible · threshold {}{}",
            rule.rule_id,
            rule.status,
            rule.eligible_observations,
            rule.threshold,
            if rule.suppressed {
                " (suppressed by policy)"
            } else {
                ""
            }
        );
    }
    if !r.warnings.is_empty() {
        out.push_str("\nUnavailable or uncertain evidence\n");
        for w in r.warnings.iter().take(if compact { 3 } else { 12 }) {
            let explanation = match w.code {
                "NESTED_TOOL_CALLS_NOT_ANALYZED" => {
                    "Nested Code Mode calls are not analyzed; only recorded outer calls are visible."
                }
                "TOOL_STATUS_UNAVAILABLE" => {
                    "Some operations have no recorded success/failure status."
                }
                "TOOL_OUTPUT_UNAVAILABLE" => {
                    "Some operations lack the recorded output required by a rule."
                }
                "READ_COMPLETENESS_UNAVAILABLE" => {
                    "Some reads do not state whether the requested range was captured completely."
                }
                "READ_RANGE_INCOMPLETE" => {
                    "Some requested read ranges were not captured completely."
                }
                "READ_SCOPE_UNAVAILABLE" => "Some reads lack an observed path or range.",
                "TOOL_RESULTS_UNAVAILABLE" => "Some tool calls have no matching result.",
                _ => w.code,
            };
            out.push_str(&wrapped(explanation, 2));
        }
        if r.warnings_omitted > 0 {
            let _ = writeln!(out, "  {} additional warnings omitted", r.warnings_omitted);
        }
    }
    out.push_str("\nNEXT ACTIONS\n");
    if let Some(f) = r.findings.first() {
        out.push_str(&wrapped(f.recommendation, 2));
        out.push_str("  Verify intent and original evidence before changing the workflow.\n");
        let mut options = String::new();
        if r.max_gap_seconds != 600 {
            let _ = write!(options, " --max-gap-seconds {}", r.max_gap_seconds);
        }
        for rule in &r.coverage.rules {
            if rule.threshold != 3 {
                let _ = write!(
                    options,
                    " --rule-threshold {}={}",
                    rule.rule_id, rule.threshold
                );
            }
            if rule.suppressed {
                let _ = write!(options, " --suppress {}", rule.rule_id);
            }
        }
        out.push_str("\n  Inspection template (join wrapped lines):\n");
        if demo {
            let _ = writeln!(out, "  traceveil inspect f001 --demo{options}");
        } else {
            let _ = writeln!(
                out,
                "  traceveil inspect '{}' SESSION --agent {}{options}",
                f.reference, r.adapter
            );
            out.push_str("  Replace SESSION with the exact file, or --stdin with the same bytes.\n  Repeat --leaf ID if one was selected. Policy above is pinned.\n");
        }
    } else {
        out.push_str("  Check coverage and skipped rules before drawing conclusions.\n");
    }
    out.push_str(
        "\nFindings are candidates, not proven root causes. Source content stays local.\n",
    );
    if width() < 80 {
        out.lines()
            .map(|line| {
                wrapped(
                    line,
                    line.len().saturating_sub(line.trim_start().len()).min(2),
                )
            })
            .collect()
    } else {
        out
    }
}

pub fn label(text: &str) -> String {
    safe(text).replace('\n', "\\n").replace('\t', "\\t")
}
