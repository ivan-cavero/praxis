//! praxis MCP Web Search Server
//!
//! Provides web search capabilities using DuckDuckGo Lite HTML search
//! (no API key required). For advanced use, supports Brave Search API
//! via `BRAVE_API_KEY` environment variable.
//!
//! Protocol: Model Context Protocol over stdio transport.

use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::process::Command;

// ─── Search Engine Implementations ────────────────────────────────────

/// Parse DuckDuckGo Lite HTML search results.
fn parse_ddg_html(html: &str) -> Vec<Value> {
    let mut results: Vec<Value> = Vec::new();

    // Simple HTML parser for DDG Lite results
    for line in html.lines() {
        let trimmed = line.trim();

        // DDG Lite uses <a rel="nofollow" href="...">title</a>
        if let Some(href_start) = trimmed.find("href=\"") {
            let href_start = href_start + 6;
            if let Some(href_end) = trimmed[href_start..].find('\"') {
                let url = &trimmed[href_start..href_start + href_end];

                // Extract title from between > and </a>
                let after_href = &trimmed[href_start + href_end..];
                if let Some(title_start) = after_href.find('>') {
                    let title_start = title_start + 1;
                    if let Some(title_end) = after_href[title_start..].find("</a>") {
                        let title = &after_href[title_start..title_start + title_end];

                        // Skip empty/irrelevant results
                        if !url.starts_with("http") || title.is_empty() {
                            continue;
                        }

                        // Check for DDG wrapper/redirect URLs
                        let clean_url = if url.contains("//duckduckgo.com/l/?") {
                            // Extract original URL from redirect
                            if let Some(uddg_start) = url.find("uddg=") {
                                let uddg_start = uddg_start + 5;
                                let uddg_end = url[uddg_start..]
                                    .find('&')
                                    .map(|i| uddg_start + i)
                                    .unwrap_or(url.len());
                                let encoded = &url[uddg_start..uddg_end];
                                // Simple URL decode
                                let decoded = url_decode(encoded);
                                decoded
                            } else {
                                url.to_string()
                            }
                        } else {
                            url.to_string()
                        };

                        results.push(json!({
                            "title": html_unescape(title),
                            "url": clean_url,
                        }));
                    }
                }
            }
        }

        // DDG Lite also shows snippets in a div with class "result-snippet"
        if trimmed.contains("result-snippet") {
            if let Some(start) = trimmed.find("result-snippet\">") {
                let start = start + 16;
                if let Some(end) = trimmed[start..].find("</") {
                    let snippet = &trimmed[start..start + end];
                    if let Some(last) = results.last_mut() {
                        last["snippet"] = json!(html_unescape(snippet.trim()));
                    }
                }
            }
        }
    }

    results
}

/// Minimal HTML unescape for entities used in DDG results.
fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

/// Minimal URL percent-decode.
fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

/// Search using DuckDuckGo Lite (no API key required).
fn search_ddg(query: &str, max_results: usize) -> Result<Value, String> {
    let url = format!("https://lite.duckduckgo.com/lite/?q={}", urlencode(query));

    let output = Command::new("curl")
        .args(&["-s", "-L", "-A", "Mozilla/5.0", &url])
        .output()
        .map_err(|e| format!("Failed to execute curl: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl error: {}", stderr.trim()));
    }

    let html = String::from_utf8_lossy(&output.stdout).to_string();
    let mut results = parse_ddg_html(&html);

    // Limit results
    results.truncate(max_results);

    Ok(json!({
        "results": results,
        "total": results.len()
    }))
}

/// Search using Brave Search API (requires BRAVE_API_KEY).
fn search_brave(query: &str, max_results: usize, api_key: &str) -> Result<Value, String> {
    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
        urlencode(query),
        max_results
    );

    let output = Command::new("curl")
        .args(&[
            "-s",
            "-H",
            &format!("Accept: application/json"),
            "-H",
            &format!("Accept-Encoding: gzip"),
            "-H",
            &format!("X-Subscription-Token: {}", api_key),
            &url,
        ])
        .output()
        .map_err(|e| format!("Failed to execute curl: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl error: {}", stderr.trim()));
    }

    let body = String::from_utf8_lossy(&output.stdout).to_string();
    let parsed: Value = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse Brave response: {}", e))?;

    let web_results = parsed["web"]["results"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            json!({
                "title": r["title"],
                "url": r["url"],
                "snippet": r["description"]
            })
        })
        .collect::<Vec<Value>>();

    Ok(json!({
        "results": web_results,
        "total": web_results.len()
    }))
}

/// URL encode a query string.
fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

// ─── Tool Implementations ─────────────────────────────────────────────

fn tool_search(args: &Value, brave_api_key: &Option<String>) -> Result<Value, String> {
    let query = args["query"]
        .as_str()
        .ok_or_else(|| "Missing required argument: query".to_string())?;

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;
    let engine = args
        .get("engine")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");

    match engine {
        "brave" => {
            let key = brave_api_key.as_ref().ok_or_else(|| {
                "Brave Search API key not set. Set BRAVE_API_KEY environment variable.".to_string()
            })?;
            search_brave(query, max_results, key)
        }
        "ddg" | "auto" => search_ddg(query, max_results),
        _ => Err(format!(
            "Unknown search engine: {}. Use 'ddg', 'brave', or 'auto'.",
            engine
        )),
    }
}

fn tool_extract(args: &Value) -> Result<Value, String> {
    let url = args["url"]
        .as_str()
        .ok_or_else(|| "Missing required argument: url".to_string())?;

    let output = Command::new("curl")
        .args(&[
            "-s",
            "-L",
            "-A",
            "Mozilla/5.0 (compatible; praxis-mcp-web-search/0.1.0)",
            url,
        ])
        .output()
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl error: {}", stderr.trim()));
    }

    let html = String::from_utf8_lossy(&output.stdout).to_string();

    // Simple text extraction: remove script/style tags and all HTML tags
    let text = strip_html_tags(&html);
    // Limit to reasonable size
    let text = if text.len() > 10000 {
        format!(
            "{}... [truncated from {} chars]",
            &text[..10000],
            text.len()
        )
    } else {
        text
    };

    Ok(json!({
        "content": [{"type": "text", "text": text}]
    }))
}

/// Strip HTML tags, script/style blocks, and normalize whitespace.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut tag_name = String::new();
    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            in_tag = true;
            tag_name.clear();
            // Read tag name
            for ch in chars.by_ref() {
                if ch == '>' || ch == ' ' || ch == '/' {
                    if ch == '>' {
                        in_tag = false;
                    }
                    break;
                }
                tag_name.push(ch.to_ascii_lowercase());
            }

            let tag = tag_name.trim_start_matches('/');
            in_script = tag == "script";
            in_style = tag == "style";

            if tag == "br" || tag == "p" || tag == "/p" || tag == "/div" || tag == "/tr" {
                result.push('\n');
            }
            continue;
        }

        if in_tag {
            if c == '>' {
                in_tag = false;
            }
            continue;
        }

        if in_script || in_style {
            continue;
        }

        // Normalize whitespace
        if c.is_whitespace() {
            if !result.ends_with(' ') && !result.is_empty() {
                result.push(' ');
            }
        } else {
            result.push(c);
        }
    }

    result.trim().to_string()
}

// ─── MCP Protocol ─────────────────────────────────────────────────────

fn handle_request(
    id: &Value,
    method: &str,
    params: &Value,
    brave_api_key: &Option<String>,
) -> Value {
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-03-26",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "praxis-mcp-web-search",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "tools/list" => Ok(json!({
            "tools": [
                {
                    "name": "search",
                    "description": "Search the web using DuckDuckGo (default) or Brave Search API. Returns title, URL, and snippet for each result.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {"type": "string", "description": "Search query"},
                            "max_results": {"type": "number", "description": "Maximum results (default: 10)"},
                            "engine": {"type": "string", "description": "Search engine: 'auto' (DDG), 'ddg', or 'brave' (requires BRAVE_API_KEY)"}
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "extract",
                    "description": "Fetch and extract readable text content from a URL",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "url": {"type": "string", "description": "URL to fetch and extract"}
                        },
                        "required": ["url"]
                    }
                }
            ]
        })),
        "tools/call" => {
            let tool_name = params["name"].as_str().unwrap_or("");
            let arguments = &params["arguments"];

            let tool_result = match tool_name {
                "search" => tool_search(arguments, brave_api_key),
                "extract" => tool_extract(arguments),
                _ => Err(format!("Unknown tool: {}", tool_name)),
            };

            match tool_result {
                Ok(value) => {
                    // Wrap search results into MCP content format
                    if tool_name == "search" {
                        let text = serde_json::to_string_pretty(&value).unwrap_or_default();
                        Ok(json!({
                            "content": [{"type": "text", "text": text}]
                        }))
                    } else {
                        Ok(value)
                    }
                }
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
    let brave_api_key = std::env::var("BRAVE_API_KEY").ok();

    eprintln!(
        "praxis-mcp-web-search starting, engine: {}",
        if brave_api_key.is_some() {
            "DuckDuckGo + Brave"
        } else {
            "DuckDuckGo"
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
            handle_request(id, method, params, &brave_api_key);
            continue;
        }

        let response = handle_request(id, method, params, &brave_api_key);

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
        assert_eq!(urlencode("rust lang"), "rust+lang");
        assert_eq!(urlencode("a/b?c=d"), "a%2Fb%3Fc%3Dd");
    }

    #[test]
    fn test_html_unescape() {
        assert_eq!(html_unescape("&amp;"), "&");
        assert_eq!(html_unescape("&lt;code&gt;"), "<code>");
        assert_eq!(html_unescape("hello &amp; goodbye"), "hello & goodbye");
    }

    #[test]
    fn test_url_decode() {
        assert_eq!(url_decode("hello+world"), "hello world");
        assert_eq!(url_decode("a%20b"), "a b");
        assert_eq!(
            url_decode("https%3A%2F%2Fexample.com"),
            "https://example.com"
        );
    }

    #[test]
    fn test_strip_html_tags() {
        let html = "<html><body><p>Hello <b>world</b></p><script>alert(1)</script></body></html>";
        let text = strip_html_tags(html);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn test_strip_html_tags_with_style() {
        let html = "<style>body { color: red; }</style><p>Content</p>";
        let text = strip_html_tags(html);
        assert_eq!(text, "Content");
    }

    #[test]
    fn test_handle_initialize() {
        let id = json!(1);
        let params = json!({});
        let response = handle_request(&id, "initialize", &params, &None);
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_handle_tools_list() {
        let id = json!(1);
        let params = json!({});
        let response = handle_request(&id, "tools/list", &params, &None);
        let tools = response["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"search"));
        assert!(names.contains(&"extract"));
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_handle_unknown_method() {
        let id = json!(1);
        let params = json!({});
        let response = handle_request(&id, "unknown_method", &params, &None);
        assert!(response.get("error").is_some());
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn test_parse_ddg_empty() {
        let results = parse_ddg_html("<html></html>");
        assert!(results.is_empty());
    }
}
