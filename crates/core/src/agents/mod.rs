//! Agent definitions — data-driven agent roles loaded from Markdown+YAML files.
//!
//! Replaces the hardcoded `AgentFactory` match statement. Each agent is a
//! `.md` file with YAML frontmatter (config) and a Markdown body (system prompt).
//!
//! ## Scopes (resolution: project > global > built-in)
//!
//! | Scope | Path | Editable |
//! |-------|------|----------|
//! | Built-in | compile-time (`include_str!`) | No |
//! | Global | `~/.config/praxis/agents/*.md` | Yes (CLI `--global`) |
//! | Project | `<project_dir>/agents/*.md` | Yes (CLI default, frontend) |
//!
//! Same name = closer scope wins. A project can override the built-in `coder`
//! without touching the global or built-in definition.

pub mod definition;
pub mod registry;

pub use definition::{AgentDefinition, AgentFrontmatter, parse_agent_md, ParseError};
pub use registry::{AgentRegistry, AgentScope};
