//! praxis MCP Git Local Server
//!
//! Provides Git operations (init, add, commit, log, diff, status, branch,
//! checkout, push, pull) over the Model Context Protocol via stdio transport.
//!
//! All operations run relative to the repository root (passed via `--root`
//! argument or the first positional argument). The server discovers the
//! git repo root from that path.

use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// ─── Git Helpers ──────────────────────────────────────────────────────

/// Execute a git command in the repo root, returning stdout on success.
fn git_run(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("Failed to execute git: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout.trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("Git error: {}", stderr.trim()))
    }
}

/// Resolve the repo root from a given working directory.
fn resolve_repo(working_dir: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(working_dir);
    let canonical = path
        .canonicalize()
        .map_err(|_| format!("Path does not exist: {}", working_dir))?;

    // Verify it's a git repository (or can be initialized as one)
    Ok(canonical)
}

// ─── Tool Implementations ─────────────────────────────────────────────

fn tool_status(repo: &Path, _args: &Value) -> Result<Value, String> {
    let output = git_run(repo, &["status", "--porcelain"])?;

    let entries: Vec<Value> = if output.is_empty() {
        Vec::new()
    } else {
        output
            .lines()
            .map(|line| {
                let (status, path) = line.split_at(2);
                json!({
                    "status": status.trim(),
                    "path": path.trim()
                })
            })
            .collect()
    };

    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&entries).unwrap_or_default()}]
    }))
}

fn tool_add(repo: &Path, args: &Value) -> Result<Value, String> {
    let path = args["path"].as_str().unwrap_or(".");
    git_run(repo, &["add", path])?;
    Ok(json!({
        "content": [{"type": "text", "text": format!("Staged: {}", path)}]
    }))
}

fn tool_commit(repo: &Path, args: &Value) -> Result<Value, String> {
    let message = args["message"]
        .as_str()
        .ok_or_else(|| "Missing required argument: message".to_string())?;

    let mut cmd_args = vec!["commit", "-m", message];
    let allow_empty = args
        .get("allow_empty")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if allow_empty {
        cmd_args.push("--allow-empty");
    }

    let output = git_run(repo, &cmd_args)?;
    Ok(json!({
        "content": [{"type": "text", "text": output}]
    }))
}

fn tool_log(repo: &Path, args: &Value) -> Result<Value, String> {
    let max_count = args.get("max_count").and_then(|v| v.as_u64()).unwrap_or(20);
    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("%h %s (%an, %ar)");

    let output = git_run(
        repo,
        &[
            "log",
            &format!("--max-count={}", max_count),
            &format!("--format={}", format),
        ],
    )?;

    let commits: Vec<&str> = output.lines().collect();
    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&commits).unwrap_or_default()}]
    }))
}

fn tool_diff(repo: &Path, args: &Value) -> Result<Value, String> {
    let staged = args
        .get("staged")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let path = args.get("path").and_then(|v| v.as_str());

    let mut cmd_args = vec!["diff"];
    if staged {
        cmd_args.push("--cached");
    }
    if let Some(p) = path {
        cmd_args.push("--");
        cmd_args.push(p);
    }

    let output = git_run(repo, &cmd_args)?;
    Ok(json!({
        "content": [{"type": "text", "text": output}]
    }))
}

fn tool_branch(repo: &Path, args: &Value) -> Result<Value, String> {
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("list");

    match action {
        "list" => {
            let output = git_run(repo, &["branch", "--list"])?;
            let branches: Vec<&str> = output.lines().collect();
            Ok(json!({
                "content": [{"type": "text", "text": serde_json::to_string_pretty(&branches).unwrap_or_default()}]
            }))
        }
        "create" => {
            let name = args["name"]
                .as_str()
                .ok_or_else(|| "Missing required argument: name".to_string())?;
            git_run(repo, &["branch", name])?;
            Ok(json!({
                "content": [{"type": "text", "text": format!("Created branch: {}", name)}]
            }))
        }
        "delete" => {
            let name = args["name"]
                .as_str()
                .ok_or_else(|| "Missing required argument: name".to_string())?;
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            if force {
                git_run(repo, &["branch", "-D", name])?;
            } else {
                git_run(repo, &["branch", "-d", name])?;
            }
            Ok(json!({
                "content": [{"type": "text", "text": format!("Deleted branch: {}", name)}]
            }))
        }
        _ => Err(format!(
            "Unknown branch action: {}. Use list, create, or delete.",
            action
        )),
    }
}

fn tool_checkout(repo: &Path, args: &Value) -> Result<Value, String> {
    let branch = args["branch"]
        .as_str()
        .ok_or_else(|| "Missing required argument: branch".to_string())?;

    let create = args
        .get("create")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if create {
        git_run(repo, &["checkout", "-b", branch])?;
        Ok(json!({
            "content": [{"type": "text", "text": format!("Created and switched to branch: {}", branch)}]
        }))
    } else {
        git_run(repo, &["checkout", branch])?;
        Ok(json!({
            "content": [{"type": "text", "text": format!("Switched to branch: {}", branch)}]
        }))
    }
}

fn tool_push(repo: &Path, args: &Value) -> Result<Value, String> {
    let remote = args
        .get("remote")
        .and_then(|v| v.as_str())
        .unwrap_or("origin");
    let branch = args
        .get("branch")
        .and_then(|v| v.as_str())
        .unwrap_or("HEAD");

    let output = git_run(repo, &["push", remote, branch])?;
    Ok(json!({
        "content": [{"type": "text", "text": output}]
    }))
}

fn tool_pull(repo: &Path, args: &Value) -> Result<Value, String> {
    let remote = args
        .get("remote")
        .and_then(|v| v.as_str())
        .unwrap_or("origin");
    let branch = args
        .get("branch")
        .and_then(|v| v.as_str())
        .unwrap_or("HEAD");

    let output = git_run(repo, &["pull", remote, branch])?;
    Ok(json!({
        "content": [{"type": "text", "text": output}]
    }))
}

// ─── MCP Protocol ─────────────────────────────────────────────────────

fn handle_request(repo: &Path, id: &Value, method: &str, params: &Value) -> Value {
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-03-26",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "praxis-mcp-git-local",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "tools/list" => Ok(json!({
            "tools": [
                {
                    "name": "git_status",
                    "description": "Show the working tree status (porcelain format)",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "git_add",
                    "description": "Stage file contents to the index",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "description": "Path to stage (default: '.' for all)"}
                        }
                    }
                },
                {
                    "name": "git_commit",
                    "description": "Record changes to the repository",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "message": {"type": "string", "description": "Commit message"},
                            "allow_empty": {"type": "boolean", "description": "Allow empty commit"}
                        },
                        "required": ["message"]
                    }
                },
                {
                    "name": "git_log",
                    "description": "Show commit logs",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "max_count": {"type": "number", "description": "Maximum commits to show (default: 20)"},
                            "format": {"type": "string", "description": "Log format (default: '%h %s (%an, %ar)')"}
                        }
                    }
                },
                {
                    "name": "git_diff",
                    "description": "Show changes between commits, working tree, etc.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "staged": {"type": "boolean", "description": "Show staged (cached) diff"},
                            "path": {"type": "string", "description": "Filter diff to this path"}
                        }
                    }
                },
                {
                    "name": "git_branch",
                    "description": "List, create, or delete branches",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "action": {"type": "string", "description": "list, create, or delete", "enum": ["list", "create", "delete"]},
                            "name": {"type": "string", "description": "Branch name (required for create/delete)"},
                            "force": {"type": "boolean", "description": "Force delete"}
                        }
                    }
                },
                {
                    "name": "git_checkout",
                    "description": "Switch branches or restore working tree files",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "branch": {"type": "string", "description": "Branch name"},
                            "create": {"type": "boolean", "description": "Create and switch to new branch"}
                        },
                        "required": ["branch"]
                    }
                },
                {
                    "name": "git_push",
                    "description": "Push changes to remote repository",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "remote": {"type": "string", "description": "Remote name (default: origin)"},
                            "branch": {"type": "string", "description": "Branch name (default: HEAD)"}
                        }
                    }
                },
                {
                    "name": "git_pull",
                    "description": "Fetch from and integrate with remote repository",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "remote": {"type": "string", "description": "Remote name (default: origin)"},
                            "branch": {"type": "string", "description": "Branch name (default: HEAD)"}
                        }
                    }
                }
            ]
        })),
        "tools/call" => {
            let tool_name = params["name"].as_str().unwrap_or("");
            let arguments = &params["arguments"];

            let tool_result = match tool_name {
                "git_status" => tool_status(repo, arguments),
                "git_add" => tool_add(repo, arguments),
                "git_commit" => tool_commit(repo, arguments),
                "git_log" => tool_log(repo, arguments),
                "git_diff" => tool_diff(repo, arguments),
                "git_branch" => tool_branch(repo, arguments),
                "git_checkout" => tool_checkout(repo, arguments),
                "git_push" => tool_push(repo, arguments),
                "git_pull" => tool_pull(repo, arguments),
                _ => Err(format!("Unknown tool: {}", tool_name)),
            };

            match tool_result {
                Ok(value) => Ok(value),
                Err(e) => Ok(json!({
                    "content": [{"type": "text", "text": format!("Error: {}", e)}],
                    "isError": true
                })),
            }
        }
        "notifications/initialized" => return json!({}),
        _ => Err(format!("Unknown method: {}", method)),
    };

    match result {
        Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}),
        Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": e}}),
    }
}

// ─── Entry Point ──────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let repo_root = if let Some(pos) = args.iter().position(|a| a == "--root") {
        args.get(pos + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap())
    } else if let Some(arg) = args.get(1) {
        PathBuf::from(arg)
    } else {
        std::env::current_dir().unwrap()
    };

    let repo = resolve_repo(repo_root.to_str().unwrap_or(".")).unwrap_or_else(|e| {
        eprintln!("Warning: {}", e);
        repo_root
    });

    eprintln!("praxis-mcp-git-local starting, root: {}", repo.display());

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error reading stdin: {}", e);
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let error_resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": format!("Parse error: {}", e)}
                });
                let _ = writeln!(
                    stdout_lock,
                    "{}",
                    serde_json::to_string(&error_resp).unwrap()
                );
                let _ = stdout_lock.flush();
                continue;
            }
        };

        let id = &request["id"];
        let method = request["method"].as_str().unwrap_or("");
        let params = &request["params"];

        if method.starts_with("notifications/") {
            handle_request(&repo, id, method, params);
            continue;
        }

        let response = handle_request(&repo, id, method, params);

        if response.is_object() && response.get("jsonrpc").is_some() {
            let response_str = serde_json::to_string(&response).unwrap();
            let _ = writeln!(stdout_lock, "{}", response_str);
            let _ = stdout_lock.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_repo(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("praxis-git-test-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let output = Command::new("git")
            .args(&["init"])
            .current_dir(&dir)
            .output()
            .expect("Failed to init git repo");
        assert!(output.status.success());

        // Configure user for commits
        Command::new("git")
            .args(&["config", "user.email", "test@test.com"])
            .current_dir(&dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["config", "user.name", "Test"])
            .current_dir(&dir)
            .output()
            .unwrap();

        // Create an initial file and commit
        fs::write(dir.join("README.md"), "# Test").unwrap();
        Command::new("git")
            .args(&["add", "."])
            .current_dir(&dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&dir)
            .output()
            .unwrap();

        dir
    }

    #[test]
    fn test_git_status() {
        let repo = setup_test_repo("status");
        let result = tool_status(&repo, &json!({}));
        assert!(result.is_ok(), "tool_status failed: {:?}", result.err());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn test_git_log() {
        let repo = setup_test_repo("log");
        let result = tool_log(&repo, &json!({"max_count": 5}));
        assert!(result.is_ok(), "tool_log failed: {:?}", result.err());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn test_git_branch_list() {
        let repo = setup_test_repo("branch-list");
        let result = tool_branch(&repo, &json!({"action": "list"}));
        assert!(
            result.is_ok(),
            "tool_branch(list) failed: {:?}",
            result.err()
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn test_git_branch_create_delete() {
        let repo = setup_test_repo("branch-cd");
        let result = tool_branch(&repo, &json!({"action": "create", "name": "test-branch"}));
        assert!(
            result.is_ok(),
            "tool_branch(create) failed: {:?}",
            result.err()
        );
        let result = tool_branch(&repo, &json!({"action": "delete", "name": "test-branch"}));
        assert!(
            result.is_ok(),
            "tool_branch(delete) failed: {:?}",
            result.err()
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn test_handle_initialize() {
        let repo = PathBuf::from("/tmp");
        let id = json!(1);
        let params = json!({});
        let response = handle_request(&repo, &id, "initialize", &params);
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_handle_tools_list() {
        let repo = PathBuf::from("/tmp");
        let id = json!(1);
        let params = json!({});
        let response = handle_request(&repo, &id, "tools/list", &params);
        let tools = response["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"git_status"));
        assert!(names.contains(&"git_commit"));
        assert!(names.contains(&"git_branch"));
        assert!(names.contains(&"git_push"));
        assert_eq!(tools.len(), 9);
    }
}
