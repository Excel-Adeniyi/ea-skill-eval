//! End-to-end tests that run the built binary against fixture files.
//!
//! These use the heuristic judge only, so the suite stays offline, fast and
//! deterministic. The LLM path is covered by unit tests on parsing and request
//! shape; exercising a real model here would make CI depend on a 40-second
//! local inference call.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn binary() -> PathBuf {
    // Cargo builds integration tests into target/<profile>/deps, so the binary
    // under test sits two levels up.
    let mut path = std::env::current_exe().expect("test binary should have a path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join(env!("CARGO_PKG_NAME"))
}

fn write_fixture(directory: &Path, name: &str, body: &str) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, body).expect("fixture should be writable");
    path
}

fn run(arguments: &[&str]) -> Output {
    Command::new(binary())
        .args(arguments)
        .output()
        .expect("binary should run")
}

const TWO_PLATFORMS: &str = r#"[
  {
    "id": "good",
    "instructions": "Mention ownership and borrowing.",
    "task": "Explain Rust memory.",
    "output": "Rust memory uses ownership and borrowing to mention safety.",
    "platform": "claude_code",
    "model": "claude-opus-5",
    "prompting": "explicit",
    "skill_triggered": true
  },
  {
    "id": "bad",
    "instructions": "Mention ownership and borrowing.",
    "task": "Explain Rust memory.",
    "output": "The weather is lovely today.",
    "platform": "codex_cli",
    "model": "gpt-5-codex",
    "prompting": "implicit",
    "skill_triggered": false
  }
]"#;

#[test]
fn scores_traces_and_prints_a_table() {
    let temporary = std::env::temp_dir().join("skill_eval_table_test");
    fs::create_dir_all(&temporary).expect("temp dir");
    let fixture = write_fixture(&temporary, "traces.json", TWO_PLATFORMS);

    let output = run(&[fixture.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("good"));
    assert!(stdout.contains("bad"));
    assert!(stdout.contains("Claude Code"));
    assert!(stdout.contains("Codex CLI"));
    assert!(stdout.contains("Overall mean"));
}

#[test]
fn emits_machine_readable_json() {
    let temporary = std::env::temp_dir().join("skill_eval_json_test");
    fs::create_dir_all(&temporary).expect("temp dir");
    let fixture = write_fixture(&temporary, "traces.json", TWO_PLATFORMS);

    let output = run(&[fixture.to_str().unwrap(), "--format", "json"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");

    assert!(output.status.success());
    assert_eq!(parsed["trace_count"], 2);
    assert_eq!(parsed["judge_requested"], "heuristic");
    assert_eq!(parsed["platforms"].as_array().unwrap().len(), 2);
    // Both prompting modes were recorded, so both appear.
    assert_eq!(parsed["triggering"].as_array().unwrap().len(), 2);
}

#[test]
fn exits_two_when_a_trace_falls_below_the_threshold() {
    let temporary = std::env::temp_dir().join("skill_eval_threshold_test");
    fs::create_dir_all(&temporary).expect("temp dir");
    let fixture = write_fixture(&temporary, "traces.json", TWO_PLATFORMS);

    let output = run(&[fixture.to_str().unwrap(), "--threshold", "0.5"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("bad"));
}

#[test]
fn exits_zero_when_every_trace_clears_the_threshold() {
    let temporary = std::env::temp_dir().join("skill_eval_pass_test");
    fs::create_dir_all(&temporary).expect("temp dir");
    let fixture = write_fixture(&temporary, "traces.json", TWO_PLATFORMS);

    let output = run(&[fixture.to_str().unwrap(), "--threshold", "0.0"]);

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn exits_one_when_the_input_is_missing() {
    let output = run(&["definitely-not-here.json"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("could not load traces"));
}

#[test]
fn still_loads_traces_captured_before_the_platform_fields_existed() {
    let temporary = std::env::temp_dir().join("skill_eval_legacy_test");
    fs::create_dir_all(&temporary).expect("temp dir");
    let fixture = write_fixture(
        &temporary,
        "traces.json",
        r#"[{"id":"old","instructions":"Be brief.","task":"Explain enums.","output":"An enum is one of several variants."}]"#,
    );

    let output = run(&[fixture.to_str().unwrap(), "--format", "json"]);
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("valid JSON");

    assert!(output.status.success());
    assert_eq!(parsed["evaluations"][0]["platform"], "unknown");
}

#[test]
fn metric_filter_limits_the_scored_metrics() {
    let temporary = std::env::temp_dir().join("skill_eval_filter_test");
    fs::create_dir_all(&temporary).expect("temp dir");
    let fixture = write_fixture(&temporary, "traces.json", TWO_PLATFORMS);

    let output = run(&[
        fixture.to_str().unwrap(),
        "--format",
        "json",
        "--metric",
        "relevancy",
    ]);
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("valid JSON");

    let scores = parsed["evaluations"][0]["scores"].as_array().unwrap();
    assert_eq!(scores.len(), 1);
    assert_eq!(scores[0]["metric"], "TaskRelevancy");
}
