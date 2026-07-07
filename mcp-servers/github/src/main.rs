//! praxis MCP GitHub Server
//!
//! Provides GitHub API operations (PRs, issues, branches, code search)
//! over the Model Context Protocol via stdio transport.
//!
//! Authentication: reads `GITHUB_TOKEN` from environment, or uses `gh` CLI.
//! Repository scope: passed via `--repo <owner/repo>` argument.

use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::process::Command;

// ─── GitHub API Helpers ───────────────────────────────────────────────

/// Execute a `gh` CLI command and return stdout.
fn gh_run(args: &[&str]) -> Result<String, String> {
    let output = Command::new("gh")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute gh CLI: {}. Is GitHub CLI installed?", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout.trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("gh error: {}", stderr.trim()))
    }
}

/// Execute a curl-based HTTP request to the GitHub API (fallback when `gh` is unavailable).
fn api_get(endpoint: &str, token: &str) -> Result<Value, String> {
    let url = format!(
        "https://api.github.com/{}",
        endpoint.trim_start_matches('/')
    );
    let output = Command::new("curl")
        .args([
            "-s",
            "-H",
            &format!("Authorization: Bearer {}", token),
            "-H",
            "Accept: application/vnd.github.v3+json",
            "-H",
            "User-Agent: praxis-mcp-github/0.1.0",
            &url,
        ])
        .output()
        .map_err(|e| format!("Failed to execute curl: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        serde_json::from_str(&stdout)
            .map_err(|e| format!("Failed to parse GitHub API response: {}", e))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("curl error: {}", stderr.trim()))
    }
}

fn api_post(endpoint: &str, token: &str, body: &Value) -> Result<Value, String> {
    let url = format!(
        "https://api.github.com/{}",
        endpoint.trim_start_matches('/')
    );
    let body_str =
        serde_json::to_string(body).map_err(|e| format!("Serialization error: {}", e))?;

    let output = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            "-H",
            &format!("Authorization: Bearer {}", token),
            "-H",
            "Accept: application/vnd.github.v3+json",
            "-H",
            "Content-Type: application/json",
            "-H",
            "User-Agent: praxis-mcp-github/0.1.0",
            "-d",
            &body_str,
            &url,
        ])
        .output()
        .map_err(|e| format!("Failed to execute curl: {}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        serde_json::from_str(&stdout)
            .map_err(|e| format!("Failed to parse GitHub API response: {}", e))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if let Ok(val) = serde_json::from_str::<Value>(&stderr) {
            let msg = val["message"].as_str().unwrap_or(stderr.trim());
            Err(format!("GitHub API error: {}", msg))
        } else {
            Err(format!("curl error: {}", stderr.trim()))
        }
    }
}

/// Get API token from environment.
fn get_token() -> Result<String, String> {
    std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .map_err(|_| "GITHUB_TOKEN or GH_TOKEN not set".to_string())
}

// ─── Tool Implementations ─────────────────────────────────────────────

fn tool_create_pr(repo: &str, _args: &Value, use_gh: bool, token: &str) -> Result<Value, String> {
    let title = _args["title"]
        .as_str()
        .ok_or_else(|| "Missing required argument: title".to_string())?;
    let body = _args.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let head = _args["head"]
        .as_str()
        .ok_or_else(|| "Missing required argument: head".to_string())?;
    let base = _args.get("base").and_then(|v| v.as_str()).unwrap_or("main");

    if use_gh {
        let output = gh_run(&[
            "pr", "create", "--repo", repo, "--title", title, "--body", body, "--head", head,
            "--base", base,
        ])?;
        Ok(json!({
            "content": [{"type": "text", "text": output}]
        }))
    } else {
        let body_val = json!({
            "title": title,
            "body": body,
            "head": head,
            "base": base,
        });
        let result = api_post(&format!("repos/{}/pulls", repo), token, &body_val)?;
        let html_url = result["html_url"].as_str().unwrap_or("");
        let number = result["number"].as_u64().unwrap_or(0);
        Ok(json!({
            "content": [{"type": "text", "text": format!("Created PR #{}: {}", number, html_url)}]
        }))
    }
}

fn tool_list_issues(repo: &str, args: &Value, use_gh: bool, token: &str) -> Result<Value, String> {
    let state = args.get("state").and_then(|v| v.as_str()).unwrap_or("open");
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);

    if use_gh {
        let output = gh_run(&[
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            state,
            "--limit",
            &limit.to_string(),
            "--json",
            "number,title,state,labels,updatedAt",
        ])?;
        Ok(json!({
            "content": [{"type": "text", "text": output}]
        }))
    } else {
        let result = api_get(
            &format!(
                "repos/{}/issues?state={}&per_page={}&sort=updated",
                repo, state, limit
            ),
            token,
        )?;
        let items: Vec<Value> = result
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|issue| {
                json!({
                    "number": issue["number"],
                    "title": issue["title"],
                    "state": issue["state"],
                    "updated_at": issue["updated_at"]
                })
            })
            .collect();
        Ok(json!({
            "content": [{"type": "text", "text": serde_json::to_string_pretty(&items).unwrap_or_default()}]
        }))
    }
}

fn tool_create_issue(repo: &str, args: &Value, use_gh: bool, token: &str) -> Result<Value, String> {
    let title = args["title"]
        .as_str()
        .ok_or_else(|| "Missing required argument: title".to_string())?;
    let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");

    if use_gh {
        let output = gh_run(&[
            "issue", "create", "--repo", repo, "--title", title, "--body", body,
        ])?;
        Ok(json!({
            "content": [{"type": "text", "text": output}]
        }))
    } else {
        let body_val = json!({ "title": title, "body": body });
        let result = api_post(&format!("repos/{}/issues", repo), token, &body_val)?;
        let html_url = result["html_url"].as_str().unwrap_or("");
        let number = result["number"].as_u64().unwrap_or(0);
        Ok(json!({
            "content": [{"type": "text", "text": format!("Created issue #{}: {}", number, html_url)}]
        }))
    }
}

fn tool_list_prs(repo: &str, args: &Value, use_gh: bool, token: &str) -> Result<Value, String> {
    let state = args.get("state").and_then(|v| v.as_str()).unwrap_or("open");
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);

    if use_gh {
        let output = gh_run(&[
            "pr",
            "list",
            "--repo",
            repo,
            "--state",
            state,
            "--limit",
            &limit.to_string(),
            "--json",
            "number,title,state,headRefName,baseRefName,updatedAt",
        ])?;
        Ok(json!({
            "content": [{"type": "text", "text": output}]
        }))
    } else {
        let result = api_get(
            &format!(
                "repos/{}/pulls?state={}&per_page={}&sort=updated",
                repo, state, limit
            ),
            token,
        )?;
        let items: Vec<Value> = result
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|pr| {
                json!({
                    "number": pr["number"],
                    "title": pr["title"],
                    "state": pr["state"],
                    "head": pr["head"]["ref"],
                    "base": pr["base"]["ref"],
                    "updated_at": pr["updated_at"]
                })
            })
            .collect();
        Ok(json!({
            "content": [{"type": "text", "text": serde_json::to_string_pretty(&items).unwrap_or_default()}]
        }))
    }
}

fn tool_list_branches(
    repo: &str,
    _args: &Value,
    use_gh: bool,
    token: &str,
) -> Result<Value, String> {
    if use_gh {
        let output = gh_run(&[
            "repo",
            "view",
            "--repo",
            repo,
            "--json",
            "refs",
            "--jq",
            ".refs.nodes[] | select(.prefix == \"refs/heads/\") | {name: .name}",
        ])?;
        Ok(json!({
            "content": [{"type": "text", "text": output}]
        }))
    } else {
        let result = api_get(&format!("repos/{}/branches?per_page=100", repo), token)?;
        let items: Vec<Value> = result
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|b| json!({ "name": b["name"] }))
            .collect();
        Ok(json!({
            "content": [{"type": "text", "text": serde_json::to_string_pretty(&items).unwrap_or_default()}]
        }))
    }
}

fn tool_search_code(_repo: &str, args: &Value, use_gh: bool, token: &str) -> Result<Value, String> {
    let query = args["query"]
        .as_str()
        .ok_or_else(|| "Missing required argument: query".to_string())?;

    if use_gh {
        let output = gh_run(&["search", "code", query, "--limit", "20"])?;
        Ok(json!({
            "content": [{"type": "text", "text": output}]
        }))
    } else {
        let result = api_get(
            &format!("search/code?q={}&per_page=20", urlencode(query)),
            token,
        )?;
        let items: Vec<Value> = result["items"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|item| {
                json!({
                    "name": item["name"],
                    "path": item["path"],
                    "repository": item["repository"]["full_name"],
                    "url": item["html_url"]
                })
            })
            .collect();
        Ok(json!({
            "content": [{"type": "text", "text": serde_json::to_string_pretty(&items).unwrap_or_default()}]
        }))
    }
}

/// Simple URL encoding for query strings.
fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

// ─── MCP Protocol ─────────────────────────────────────────────────────

fn handle_request(
    repo: &str,
    use_gh: bool,
    token: &str,
    id: &Value,
    method: &str,
    params: &Value,
) -> Value {
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-03-26",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "praxis-mcp-github",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "tools/list" => Ok(json!({
            "tools": [
                {
                    "name": "create_pr",
                    "description": "Create a pull request on GitHub",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {"type": "string", "description": "PR title"},
                            "body": {"type": "string", "description": "PR body/description"},
                            "head": {"type": "string", "description": "Head branch (source)"},
                            "base": {"type": "string", "description": "Base branch (target, default: main)"}
                        },
                        "required": ["title", "head"]
                    }
                },
                {
                    "name": "list_issues",
                    "description": "List issues in the repository",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "state": {"type": "string", "description": "open, closed, or all (default: open)"},
                            "limit": {"type": "number", "description": "Max results (default: 20)"}
                        }
                    }
                },
                {
                    "name": "create_issue",
                    "description": "Create a new issue on GitHub",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {"type": "string", "description": "Issue title"},
                            "body": {"type": "string", "description": "Issue body"}
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "list_prs",
                    "description": "List pull requests in the repository",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "state": {"type": "string", "description": "open, closed, or all (default: open)"},
                            "limit": {"type": "number", "description": "Max results (default: 20)"}
                        }
                    }
                },
                {
                    "name": "list_branches",
                    "description": "List branches in the repository",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "search_code",
                    "description": "Search code across GitHub repositories",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {"type": "string", "description": "Search query"}
                        },
                        "required": ["query"]
                    }
                }
            ]
        })),
        "tools/call" => {
            let tool_name = params["name"].as_str().unwrap_or("");
            let arguments = &params["arguments"];

            let tool_result = match tool_name {
                "create_pr" => tool_create_pr(repo, arguments, use_gh, token),
                "list_issues" => tool_list_issues(repo, arguments, use_gh, token),
                "create_issue" => tool_create_issue(repo, arguments, use_gh, token),
                "list_prs" => tool_list_prs(repo, arguments, use_gh, token),
                "list_branches" => tool_list_branches(repo, arguments, use_gh, token),
                "search_code" => tool_search_code(repo, arguments, use_gh, token),
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

    let repo = if let Some(pos) = args.iter().position(|a| a == "--repo") {
        args.get(pos + 1).cloned().unwrap_or_default()
    } else {
        String::new()
    };

    // Detect if `gh` CLI is available
    let use_gh = Command::new("gh")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let token = if !use_gh {
        get_token().unwrap_or_default()
    } else {
        String::new()
    };

    if repo.is_empty() {
        eprintln!("Warning: no --repo specified. Use --repo <owner/repo>");
    }

    eprintln!(
        "praxis-mcp-github starting, repo: {}, auth: {}",
        if repo.is_empty() { "none" } else { &repo },
        if use_gh {
            "gh CLI"
        } else if !token.is_empty() {
            "GITHUB_TOKEN"
        } else {
            "none"
        }
    );

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
            handle_request(&repo, use_gh, &token, id, method, params);
            continue;
        }

        let response = handle_request(&repo, use_gh, &token, id, method, params);

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

    #[test]
    fn test_urlencode() {
        assert_eq!(urlencode("hello world"), "hello+world");
        assert_eq!(urlencode("a/b"), "a%2Fb");
        assert_eq!(urlencode("simple"), "simple");
    }

    #[test]
    fn test_handle_initialize() {
        let id = json!(1);
        let params = json!({});
        let response = handle_request("owner/repo", false, "", &id, "initialize", &params);
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_handle_tools_list() {
        let id = json!(1);
        let params = json!({});
        let response = handle_request("owner/repo", false, "", &id, "tools/list", &params);
        let tools = response["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"create_pr"));
        assert!(names.contains(&"list_issues"));
        assert!(names.contains(&"search_code"));
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn test_handle_unknown_method() {
        let id = json!(1);
        let params = json!({});
        let response = handle_request("owner/repo", false, "", &id, "unknown_method", &params);
        assert!(response.get("error").is_some());
        assert_eq!(response["error"]["code"], -32601);
    }
}
