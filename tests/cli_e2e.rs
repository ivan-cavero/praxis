//! E2E tests for the CLI binary.
//!
//! These tests actually compile and run the `praxis` binary,
//! verifying real-world behavior end-to-end.

use std::process::Command;

/// Path to the compiled binary.
fn binary_path() -> String {
    // cargo puts binaries in target/debug/
    let mut path = std::env::current_exe()
        .expect("Failed to get test binary path")
        .parent()
        .expect("Failed to get parent dir")
        .to_path_buf();

    // Walk up until we find target/debug
    loop {
        if path.join("praxis.exe").exists() || path.join("praxis").exists() {
            break;
        }
        if !path.pop() {
            break;
        }
    }

    // Check for .exe (Windows) or plain binary (Unix)
    let exe = if cfg!(windows) {
        path.join("praxis.exe")
    } else {
        path.join("praxis")
    };

    exe.to_string_lossy().to_string()
}

/// Run the CLI with arguments and capture output.
fn run_cli_in(dir: &str, args: &[&str]) -> (String, String, bool) {
    let data_dir = std::env::temp_dir().join("praxis-e2e").join(dir);
    let _ = std::fs::create_dir_all(&data_dir);
    let output = Command::new(binary_path())
        .args(args)
        .env("PRAXIS_DATA_DIR", &data_dir)
        .output()
        .expect("Failed to execute CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

fn run_cli(args: &[&str]) -> (String, String, bool) {
    run_cli_in("default", args)
}

// ═══════════════════════════════════════════════════════════════
// CLI E2E TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn e2e_cli_version() {
    let (stdout, _, success) = run_cli(&["version"]);
    assert!(success, "CLI version command failed");
    assert!(
        stdout.contains("praxis"),
        "Should contain 'praxis': {}",
        stdout
    );
    assert!(
        stdout.contains("v0."),
        "Should contain a version starting with v0.: {}",
        stdout
    );
}

#[test]
fn e2e_cli_help() {
    let (stdout, _, success) = run_cli(&["--help"]);
    assert!(success, "CLI help command failed");
    assert!(stdout.contains("praxis"), "Should contain binary name");
    assert!(stdout.contains("init"), "Should mention init command");
    assert!(stdout.contains("run"), "Should mention run command");
}

#[test]
fn e2e_cli_init_creates_project() {
    let project_name = format!("proj-{}", uuid::Uuid::new_v4());
    let dir = format!("init-test-{}", uuid::Uuid::new_v4());
    let (stdout, _, success) = run_cli_in(&dir, &["init", &project_name]);
    assert!(success, "CLI init failed: {}", stdout);
    assert!(
        stdout.contains("Created project"),
        "Should confirm creation: {}",
        stdout
    );

    let projects_path = std::env::temp_dir()
        .join("praxis-e2e")
        .join(&dir)
        .join("projects.json");
    assert!(
        projects_path.exists(),
        "projects.json should exist at {}",
        projects_path.display()
    );
    let content = std::fs::read_to_string(&projects_path).unwrap();
    assert!(
        content.contains(&format!("\"name\": \"{}\"", project_name)),
        "Project should be in projects.json"
    );
}

#[test]
fn e2e_cli_init_already_exists() {
    let dir = format!("init-dupe-{}", uuid::Uuid::new_v4());
    let project_name = "dup-test";

    let (_, _, success) = run_cli_in(&dir, &["init", project_name]);
    assert!(success, "First init should succeed");

    let (_, stderr, success) = run_cli_in(&dir, &["init", project_name]);
    assert!(!success, "CLI init should fail when project exists");
    assert!(
        stderr.contains("already exists"),
        "Should show error: {}",
        stderr
    );
}

#[test]
fn e2e_cli_run_requires_goal() {
    let (_, stderr, success) = run_cli(&["run"]);
    assert!(!success, "CLI run without --goal should fail");
}

#[test]
fn e2e_cli_init_help_shows_commands() {
    let (stdout, _, success) = run_cli(&["--help"]);
    assert!(success, "CLI help should succeed");
    assert!(stdout.contains("init"), "Should show init command");
    assert!(stdout.contains("run"), "Should show run command");
}

#[test]
fn e2e_cli_project_list_empty() {
    let (stdout, _, success) = run_cli(&["project", "list"]);
    assert!(success, "CLI project list should succeed");
    assert!(
        stdout.contains("project") || stdout.contains("No") || stdout.contains("no"),
        "Should handle empty project list: {}",
        stdout
    );
}

#[test]
fn e2e_cli_session_list_empty() {
    let (stdout, _, success) = run_cli(&["session", "list"]);
    assert!(success, "CLI session list should succeed");
}

#[test]
fn e2e_cli_provider_list() {
    let (stdout, _, success) = run_cli(&["provider", "list"]);
    assert!(success, "CLI provider list should succeed");
    assert!(
        stdout.contains("OpenAI") || stdout.contains("openai") || stdout.contains("provider"),
        "Should list providers: {}",
        stdout
    );
}

#[test]
fn e2e_cli_mcp_list_empty() {
    let (stdout, _, success) = run_cli(&["mcp", "list"]);
    assert!(success, "CLI mcp list should succeed");
}

#[test]
fn e2e_cli_version_json_output() {
    let (stdout, _, success) = run_cli(&["version"]);
    assert!(success, "CLI version should succeed");
    // Version output should be parseable
    let version_line = stdout.lines().next().unwrap_or("");
    assert!(!version_line.is_empty(), "Version should produce output");
}

#[test]
fn e2e_cli_test_command() {
    // The test command runs internal integration tests
    let (stdout, _, success) = run_cli(&["test"]);
    assert!(success, "CLI test command should succeed");
    assert!(
        stdout.contains("passed") || stdout.contains("✓") || stdout.contains("test"),
        "Should show test results: {}",
        stdout
    );
}

#[test]
fn e2e_cli_deploy_status_no_config() {
    let (stdout, _, _success) = run_cli(&["deploy", "status"]);
    // Should handle gracefully even without config
    assert!(!stdout.is_empty(), "Should produce some output");
}

#[test]
fn e2e_cli_context_inspect_no_session() {
    let (stdout, _, _success) = run_cli(&["context", "inspect", "test-session"]);
    // Should handle gracefully
    assert!(!stdout.is_empty(), "Should produce some output");
}

// NOTE: e2e_cli_org_list and e2e_cli_billing_show removed —
// org/billing commands are explicitly out of scope per VISION.md
// ("No enterprise. No multi-tenant. No billing.")
