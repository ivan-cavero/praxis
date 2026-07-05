//! Watch command — real-time session monitoring from the CLI.
//!
//! Polls the API server every N seconds and displays:
//! - Session status, phase, iteration
//! - Tokens, cost
//! - Recent events (agent starts, completions, tool calls, delegations)

use colored::Colorize;
use serde::{Deserialize, Serialize};

const CLEAR: &str = "\x1b[2J\x1b[H";

#[derive(Debug, Serialize, Deserialize)]
struct SessionState {
    session_id: String,
    phase: String,
    iteration: u32,
    tokens_used: u64,
    cost_usd: f64,
    status: String,
    state_file: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionEntry {
    id: String,
    project: String,
    goal: String,
    phase: String,
    iteration: u32,
    status: String,
    started_at: String,
    completed_at: Option<String>,
    tokens_used: Option<u64>,
    cost_usd: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EventEntry {
    id: String,
    event_type: String,
    payload: serde_json::Value,
    version: u32,
    created_at: String,
}

pub async fn run(session_id: &str, api_url: &str, interval_secs: u64) {
    let client = reqwest::Client::new();
    let interval = std::time::Duration::from_secs(interval_secs);
    let mut last_event_count: usize = 0;

    println!("{} Watching session {} (refresh: {}s, API: {})", "→".cyan(), session_id.yellow(), interval_secs, api_url.dimmed());
    println!("{} Press Ctrl+C to stop.\n", "→".cyan());

    loop {
        // Fetch session state
        let state_url = format!("{}/api/sessions/{}/state", api_url, session_id);
        let state = match client.get(&state_url).send().await {
            Ok(resp) => resp.json::<SessionState>().await.ok(),
            Err(_) => None,
        };

        // Fetch session info
        let session_url = format!("{}/api/sessions/{}", api_url, session_id);
        let session = match client.get(&session_url).send().await {
            Ok(resp) => resp.json::<SessionEntry>().await.ok(),
            Err(_) => None,
        };

        // Fetch events
        let events_url = format!("{}/api/sessions/{}/events", api_url, session_id);
        let events = match client.get(&events_url).send().await {
            Ok(resp) => resp.json::<Vec<EventEntry>>().await.unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        // Clear screen and render
        print!("{CLEAR}");
        render(session_id, &state, &session, &events, &mut last_event_count);

        // Stop if session is no longer running
        if let Some(s) = &state {
            if s.status != "running" {
                println!("\n{} Session ended with status: {}", "→".cyan(), s.status.yellow());
                return;
            }
        }

        tokio::time::sleep(interval).await;
    }
}

fn render(
    session_id: &str,
    state: &Option<SessionState>,
    session: &Option<SessionEntry>,
    events: &[EventEntry],
    last_event_count: &mut usize,
) {
    // Header
    println!("{} {}", "●".bright_cyan(), "PRAXIS WATCH".bold());
    println!("Session: {} | {}", session_id.yellow(), chrono::Local::now().format("%H:%M:%S"));
    println!("{}", "─".repeat(80));

    match (state, session) {
        (Some(state), Some(session)) => {
            // Status bar
            let status_color = match state.status.as_str() {
                "running" => state.status.green(),
                "completed" => state.status.bright_green(),
                "failed" => state.status.red(),
                _ => state.status.yellow(),
            };
            println!(
                "Status: {} | Phase: {} | Iteration: {}",
                status_color,
                state.phase.cyan(),
                state.iteration,
            );
            println!(
                "Tokens: {} | Cost: ${:.4}",
                format_tokens(state.tokens_used).bright_blue(),
                state.cost_usd,
            );
            println!(
                "Goal: {}",
                session.goal.chars().take(80).collect::<String>(),
            );
            println!("{}", "─".repeat(80));

            // Events
            let new_events = if events.len() > *last_event_count {
                &events[*last_event_count..]
            } else {
                &[]
            };

            if events.is_empty() {
                println!("No events yet...");
            } else {
                println!("Events ({} total, {} new):", events.len(), new_events.len());
                println!("{}", "─".repeat(80));

                // Show last 15 events
                let display_events: Vec<&EventEntry> = events.iter().rev().take(15).collect();
                for event in display_events.iter().rev() {
                    render_event(event);
                }
            }

            *last_event_count = events.len();

            // STATE.md preview
            if let Some(state_file) = &state.state_file {
                if !state_file.is_empty() {
                    println!("{}", "─".repeat(80));
                    println!("{}", "STATE.md".bold());
                    println!("{}", "─".repeat(80));
                    // Show last 20 lines of STATE.md
                    let lines: Vec<&str> = state_file.lines().collect();
                    let start = lines.len().saturating_sub(20);
                    for line in &lines[start..] {
                        println!("{}", line);
                    }
                }
            }
        }
        _ => {
            println!("{}: cannot reach API server or session not found.", "error".red());
            println!("Make sure the API server is running: {}", "praxis server".cyan());
        }
    }

    println!("\n{} Refreshing every few seconds... (Ctrl+C to stop)", "→".dimmed());
}

fn render_event(event: &EventEntry) {
    let time = event.created_at.chars().skip(11).take(8).collect::<String>();
    let event_type = &event.event_type;

    let icon = match event_type.as_str() {
        "AgentStarted" => "▶".bright_cyan(),
        "AgentCompleted" => "✓".green(),
        "AgentOutput" => "·".dimmed(),
        "ToolCalled" => "🔧".cyan(),
        "PhaseChanged" => "→".bright_magenta(),
        "TokenUsed" => "⚡".bright_yellow(),
        "DelegationStarted" => "↳".bright_blue(),
        "DelegationCompleted" => "↳✓".bright_green(),
        _ => "•".dimmed(),
    };

    // Extract key info from payload
    let summary = extract_event_summary(event_type, &event.payload);

    println!(
        "  {} {} {:<20} {}",
        icon,
        time.dimmed(),
        event_type.bright_black(),
        summary,
    );
}

fn extract_event_summary(event_type: &str, payload: &serde_json::Value) -> String {
    match event_type {
        "AgentStarted" => {
            let agent = payload.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
            let phase = payload.get("phase").and_then(|v| v.as_str()).unwrap_or("?");
            format!("agent={} phase={}", agent, phase)
        }
        "AgentCompleted" => {
            let agent = payload.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
            let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("?");
            let duration = payload.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            format!("agent={} status={} ({}ms)", agent, status, duration)
        }
        "AgentOutput" => {
            let agent = payload.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
            let delta = payload.get("delta").and_then(|v| v.as_str()).unwrap_or("");
            let preview: String = delta.chars().take(60).collect();
            format!("agent={} delta=\"{}\"", agent, preview)
        }
        "ToolCalled" => {
            let tool = payload.get("tool").and_then(|v| v.as_str()).unwrap_or("?");
            let success = payload.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            let duration = payload.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            format!("tool={} success={} ({}ms)", tool, success, duration)
        }
        "PhaseChanged" => {
            let to = payload.get("to").and_then(|v| v.as_str()).unwrap_or("?");
            format!("→ {}", to)
        }
        "TokenUsed" => {
            let model = payload.get("model").and_then(|v| v.as_str()).unwrap_or("?");
            let input = payload.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
            let output = payload.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
            format!("model={} in={} out={}", model, input, output)
        }
        "DelegationStarted" => {
            let parent = payload.get("parent").and_then(|v| v.as_str()).unwrap_or("?");
            let child = payload.get("child").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} → {}", parent, child)
        }
        "DelegationCompleted" => {
            let parent = payload.get("parent").and_then(|v| v.as_str()).unwrap_or("?");
            let child = payload.get("child").and_then(|v| v.as_str()).unwrap_or("?");
            let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} → {} ({})", parent, child, status)
        }
        _ => payload.to_string(),
    }
}

fn format_tokens(n: u64) -> String {
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    }
}
