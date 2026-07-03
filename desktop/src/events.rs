//! Tauri event types — events emitted from Rust to frontend.
//!
//! These types are defined for future emission to the Vue frontend.
//! They document the shape of events that the frontend should expect.

use serde::Serialize;

/// Event emitted when an agent changes phase.
#[derive(Serialize)]
#[expect(dead_code, reason = "Planned for streaming events to frontend")]
pub struct PhaseChangedEvent {
    pub agent_id: String,
    pub from: String,
    pub to: String,
    pub timestamp: String,
}

/// Event emitted for token usage.
#[derive(Serialize)]
#[expect(dead_code, reason = "Planned for streaming events to frontend")]
pub struct TokenUsedEvent {
    pub provider: String,
    pub model: String,
    pub input: u32,
    pub output: u32,
    pub timestamp: String,
}

/// Event emitted for context pressure changes.
#[derive(Serialize)]
#[expect(dead_code, reason = "Planned for streaming events to frontend")]
pub struct ContextPressureEvent {
    pub pressure: f32,
    pub agent_id: String,
    pub action: String,
    pub timestamp: String,
}

/// Event emitted for drift alerts.
#[derive(Serialize)]
#[expect(dead_code, reason = "Planned for streaming events to frontend")]
pub struct DriftAlertEvent {
    pub asi_score: f32,
    pub severity: String,
    pub details: String,
    pub timestamp: String,
}
