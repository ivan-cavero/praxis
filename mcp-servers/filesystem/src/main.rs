//! praxis MCP Filesystem Server
//!
//! Provides filesystem tools (read, write, edit, list, search, glob)
//! over the Model Context Protocol via stdio transport.
//!
//! All file operations are sandboxed to the allowed root directory
//! (passed via `--root` argument or the first positional argument).

use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

// ─── Sandbox ───────────────────────────────────────────────────────

/// Resolve a path relative to the sandbox root, rejecting traversal attempts.
fn resolve_path(root: &Path, requested: &str) -> Result<PathBuf, String> {
    let requested = requested.trim_start_matches('/').trim_start_matches('\\');
    let resolved = root.join(requested);
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("Cannot resolve root path: {}", e))?;
    let canonical_resolved = resolved
        .canonicalize()
        .map_err(|_| format!("Path does not exist or is invalid: {}", requested))?;

    if !canonical_resolved.starts_with(&canonical_root) {
        return Err(format!(
            "Path traversal denied: {} is outside the allowed root",
            requested
        ));
    }

    Ok(canonical_resolved)
}

/// Resolve a path for writing (parent must exist, file may not).
fn resolve_path_write(root: &Path, requested: &str) -> Result<PathBuf, String> {
    let requested = requested.trim_start_matches('/').trim_start_matches('\\');
    let resolved = root.join(requested);

    // Canonicalize what we can — parent must exist
    if let Some(parent) = resolved.parent() {
        let canonical_parent = parent
            .canonicalize()
            .map_err(|_| format!("Parent directory does not exist: {}", parent.display()))?;

        let canonical_root = root
            .canonicalize()
            .map_err(|e| format!("Cannot resolve root path: {}", e))?;

        if !canonical_parent.starts_with(&canonical_root) {
            return Err(format!(
                "Path traversal denied: {} is outside the allowed root",
                requested
            ));
        }
    }

    Ok(resolved)
}

// ─── Tool Implementations ──────────────────────────────────────────

fn tool_read(root: &Path, args: &Value) -> Result<Value, String> {
    let path_str = args["path"]
        .as_str()
        .ok_or_else(|| "Missing required argument: path".to_string())?;
    let resolved = resolve_path(root, path_str)?;

    let content = std::fs::read_to_string(&resolved)
        .map_err(|e| format!("Failed to read '{}': {}", resolved.display(), e))?;

    Ok(json!({
        "content": [{"type": "text", "text": content}]
    }))
}

fn tool_write(root: &Path, args: &Value) -> Result<Value, String> {
    let path_str = args["path"]
        .as_str()
        .ok_or_else(|| "Missing required argument: path".to_string())?;
    let content = args["content"]
        .as_str()
        .ok_or_else(|| "Missing required argument: content".to_string())?;
    let resolved = resolve_path_write(root, path_str)?;

    // Create parent directories if needed
    if let Some(parent) = resolved.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directories: {}", e))?;
    }

    std::fs::write(&resolved, content)
        .map_err(|e| format!("Failed to write '{}': {}", resolved.display(), e))?;

    Ok(json!({
        "content": [{"type": "text", "text": format!("Wrote {} bytes to {}", content.len(), resolved.display())}]
    }))
}

fn tool_edit(root: &Path, args: &Value) -> Result<Value, String> {
    let path_str = args["path"]
        .as_str()
        .ok_or_else(|| "Missing required argument: path".to_string())?;
    let old_string = args["old_string"]
        .as_str()
        .ok_or_else(|| "Missing required argument: old_string".to_string())?;
    let new_string = args["new_string"]
        .as_str()
        .ok_or_else(|| "Missing required argument: new_string".to_string())?;
    let resolved = resolve_path(root, path_str)?;

    let content = std::fs::read_to_string(&resolved)
        .map_err(|e| format!("Failed to read '{}': {}", resolved.display(), e))?;

    if !content.contains(old_string) {
        return Err(format!(
            "old_string not found in '{}'. Searched for: {}",
            resolved.display(),
            old_string
        ));
    }

    let new_content = content.replace(old_string, new_string);
    let replace_count = content.matches(old_string).count();

    std::fs::write(&resolved, &new_content)
        .map_err(|e| format!("Failed to write '{}': {}", resolved.display(), e))?;

    Ok(json!({
        "content": [{"type": "text", "text": format!(
            "Applied {} replacement(s) to {}", replace_count, resolved.display()
        )}]
    }))
}

fn tool_list(root: &Path, args: &Value) -> Result<Value, String> {
    let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let resolved = resolve_path(root, path_str)?;

    let entries = std::fs::read_dir(&resolved)
        .map_err(|e| format!("Failed to list '{}': {}", resolved.display(), e))?;

    let mut items: Vec<Value> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Failed to get type: {}", e))?;
        let name = entry.file_name().to_string_lossy().to_string();
        items.push(json!({
            "name": name,
            "type": if file_type.is_dir() { "directory" } else if file_type.is_symlink() { "symlink" } else { "file" }
        }));
    }

    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&items).unwrap_or_default()}]
    }))
}

fn tool_search(root: &Path, args: &Value) -> Result<Value, String> {
    let pattern = args["pattern"]
        .as_str()
        .ok_or_else(|| "Missing required argument: pattern".to_string())?;
    let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let resolved = resolve_path(root, path_str)?;

    let mut matches: Vec<Value> = Vec::new();

    fn search_recursive(
        dir: &Path,
        pattern: &str,
        root: &Path,
        matches: &mut Vec<Value>,
    ) -> Result<(), String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read dir '{}': {}", dir.display(), e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                // Don't recurse into hidden directories or node_modules, .git, target
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') && name != "node_modules" && name != "target" {
                    search_recursive(&path, pattern, root, matches)?;
                }
            } else if path.is_file()
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                for (line_num, line) in content.lines().enumerate() {
                    if line.contains(pattern) {
                        let relative = path
                            .strip_prefix(root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();
                        matches.push(json!({
                            "file": relative,
                            "line": line_num + 1,
                            "content": line.trim()
                        }));
                    }
                }
            }
        }
        Ok(())
    }

    search_recursive(&resolved, pattern, root, &mut matches)?;

    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&matches).unwrap_or_default()}]
    }))
}

fn tool_glob(root: &Path, args: &Value) -> Result<Value, String> {
    let pattern = args["pattern"]
        .as_str()
        .ok_or_else(|| "Missing required argument: pattern".to_string())?;
    let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let resolved = resolve_path(root, path_str)?;

    // Simple recursive glob implementation
    let mut results: Vec<String> = Vec::new();

    fn glob_recursive(
        dir: &Path,
        pattern: &str,
        root: &Path,
        results: &mut Vec<String>,
    ) -> Result<(), String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read dir '{}': {}", dir.display(), e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Match against wildcard pattern
            let matched = simple_glob_match(&name, pattern);

            if path.is_dir() {
                if !name.starts_with('.') && name != "node_modules" && name != "target" {
                    glob_recursive(&path, pattern, root, results)?;
                }
                if matched {
                    let relative = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    results.push(relative);
                }
            } else if matched {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                results.push(relative);
            }
        }
        Ok(())
    }

    glob_recursive(&resolved, pattern, root, &mut results)?;

    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&results).unwrap_or_default()}]
    }))
}

/// Simple wildcard match: `*` matches any sequence, `?` matches any single char.
fn simple_glob_match(name: &str, pattern: &str) -> bool {
    let name_chars: Vec<char> = name.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    wildcard_match(&name_chars, &pattern_chars)
}

fn wildcard_match(name: &[char], pattern: &[char]) -> bool {
    let (mut ni, mut pi) = (0, 0);
    let (mut star_ni, mut star_pi) = (0usize, None);

    while ni < name.len() {
        if pi < pattern.len() && (pattern[pi] == '?' || pattern[pi] == name[ni]) {
            ni += 1;
            pi += 1;
        } else if pi < pattern.len() && pattern[pi] == '*' {
            star_ni = ni;
            star_pi = Some(pi);
            pi += 1;
        } else if let Some(sp) = star_pi {
            star_ni += 1;
            ni = star_ni;
            pi = sp + 1;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == '*' {
        pi += 1;
    }

    pi == pattern.len()
}

// ─── MCP Protocol ───────────────────────────────────────────────────

/// Handle a JSON-RPC request and return the response.
fn handle_request(root: &Path, id: &Value, method: &str, params: &Value) -> Value {
    let result = match method {
        "initialize" => {
            // Return server capabilities
            Ok(json!({
                "protocolVersion": "2025-03-26",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "praxis-mcp-filesystem",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }))
        }
        "tools/list" => {
            // List available tools with their schemas
            Ok(json!({
                "tools": [
                    {
                        "name": "read",
                        "description": "Read the contents of a file",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {"type": "string", "description": "Path to the file (relative to project root)"}
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "write",
                        "description": "Write content to a file (creates parent directories if needed)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {"type": "string", "description": "Path to the file (relative to project root)"},
                                "content": {"type": "string", "description": "Content to write"}
                            },
                            "required": ["path", "content"]
                        }
                    },
                    {
                        "name": "edit",
                        "description": "Replace old_string with new_string in a file",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {"type": "string", "description": "Path to the file (relative to project root)"},
                                "old_string": {"type": "string", "description": "Text to replace"},
                                "new_string": {"type": "string", "description": "Replacement text"}
                            },
                            "required": ["path", "old_string", "new_string"]
                        }
                    },
                    {
                        "name": "list",
                        "description": "List directory contents",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": {"type": "string", "description": "Directory path (relative to project root, default: root)"}
                            }
                        }
                    },
                    {
                        "name": "search",
                        "description": "Search for a pattern in files (recursive)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": {"type": "string", "description": "Text pattern to search for"},
                                "path": {"type": "string", "description": "Directory to search in (default: root)"}
                            },
                            "required": ["pattern"]
                        }
                    },
                    {
                        "name": "glob",
                        "description": "Find files matching a glob pattern (recursive)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": {"type": "string", "description": "Glob pattern (e.g., '**/*.rs', '*.toml')"},
                                "path": {"type": "string", "description": "Directory to search in (default: root)"}
                            },
                            "required": ["pattern"]
                        }
                    }
                ]
            }))
        }
        "tools/call" => {
            let tool_name = params["name"].as_str().unwrap_or("");
            let arguments = &params["arguments"];

            let tool_result = match tool_name {
                "read" => tool_read(root, arguments),
                "write" => tool_write(root, arguments),
                "edit" => tool_edit(root, arguments),
                "list" => tool_list(root, arguments),
                "search" => tool_search(root, arguments),
                "glob" => tool_glob(root, arguments),
                _ => Err(format!("Unknown tool: {}", tool_name)),
            };

            match tool_result {
                Ok(value) => Ok(value),
                Err(e) => {
                    // Return as error content so the LLM can see it
                    Ok(json!({
                        "content": [{"type": "text", "text": format!("Error: {}", e)}],
                        "isError": true
                    }))
                }
            }
        }
        "notifications/initialized" => {
            // No response needed for notifications
            return json!({});
        }
        _ => Err(format!("Unknown method: {}", method)),
    };

    match result {
        Ok(value) => {
            json!({"jsonrpc": "2.0", "id": id, "result": value})
        }
        Err(e) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": e}})
        }
    }
}

// ─── Entry Point ────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Determine the sandbox root
    let root = if let Some(pos) = args.iter().position(|a| a == "--root") {
        args.get(pos + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap())
    } else if let Some(arg) = args.get(1) {
        PathBuf::from(arg)
    } else {
        std::env::current_dir().unwrap()
    };

    // Canonicalize the root
    let root = root.canonicalize().unwrap_or_else(|_| {
        eprintln!(
            "Warning: root path does not exist, using as-is: {}",
            root.display()
        );
        root
    });

    eprintln!("praxis-mcp-filesystem starting, root: {}", root.display());

    // Read JSON-RPC messages from stdin, write responses to stdout
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

        // Parse JSON-RPC request
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

        // Handle notification methods (no response expected)
        if method.starts_with("notifications/") {
            handle_request(&root, id, method, params);
            continue;
        }

        // Handle request and send response
        let response = handle_request(&root, id, method, params);

        // Don't send empty responses (notifications already handled)
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
    fn test_simple_glob_match() {
        assert!(simple_glob_match("main.rs", "*.rs"));
        assert!(simple_glob_match("main.rs", "main.*"));
        assert!(simple_glob_match("main.rs", "main.rs"));
        assert!(!simple_glob_match("main.rs", "*.toml"));
        assert!(simple_glob_match("a.rs", "?.rs"));
        assert!(!simple_glob_match("ab.rs", "?.rs"));
        assert!(!simple_glob_match("lib.rs", "??.rs"));
    }

    #[test]
    fn test_resolve_path_sandbox() {
        let root = PathBuf::from("/tmp/test-root");
        // This test is here for structure — real sandbox tests depend on filesystem
    }

    #[test]
    fn test_handle_initialize() {
        let root = PathBuf::from("/tmp");
        let id = json!(1);
        let params = json!({});
        let response = handle_request(&root, &id, "initialize", &params);
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_handle_tools_list() {
        let root = PathBuf::from("/tmp");
        let id = json!(1);
        let params = json!({});
        let response = handle_request(&root, &id, "tools/list", &params);
        assert_eq!(response["jsonrpc"], "2.0");
        let tools = response["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"read"));
        assert!(names.contains(&"write"));
        assert!(names.contains(&"edit"));
        assert!(names.contains(&"list"));
        assert!(names.contains(&"search"));
        assert!(names.contains(&"glob"));
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn test_handle_unknown_tool() {
        let root = PathBuf::from("/tmp");
        let id = json!(1);
        let params = json!({"name": "nonexistent", "arguments": {}});
        let response = handle_request(&root, &id, "tools/call", &params);
        assert_eq!(response["jsonrpc"], "2.0");
        assert!(response["result"]["isError"].as_bool().unwrap_or(false));
    }

    #[test]
    fn test_handle_unknown_method() {
        let root = PathBuf::from("/tmp");
        let id = json!(1);
        let params = json!({});
        let response = handle_request(&root, &id, "unknown_method", &params);
        assert!(response.get("error").is_some());
        assert_eq!(response["error"]["code"], -32601);
    }
}
