//! Agent definition — the parsed form of an agent `.md` file.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Built-in agent templates (compile-time embedded) ──────────

pub const ARCHITECT_MD: &str = include_str!("../../agents/architect.md");
pub const CODER_MD: &str = include_str!("../../agents/coder.md");
pub const REVIEWER_MD: &str = include_str!("../../agents/reviewer.md");
pub const SECURITY_MD: &str = include_str!("../../agents/security.md");
pub const TESTER_MD: &str = include_str!("../../agents/tester.md");
pub const GIT_MD: &str = include_str!("../../agents/git.md");
pub const RESEARCHER_MD: &str = include_str!("../../agents/researcher.md");
pub const EXPLORER_MD: &str = include_str!("../../agents/explorer.md");

/// All built-in agent templates, in canonical order.
pub const BUILTIN_AGENTS: &[(&str, &str)] = &[
    ("architect", ARCHITECT_MD),
    ("coder", CODER_MD),
    ("reviewer", REVIEWER_MD),
    ("security", SECURITY_MD),
    ("tester", TESTER_MD),
    ("git", GIT_MD),
    ("researcher", RESEARCHER_MD),
    ("explorer", EXPLORER_MD),
];

// ─── Frontmatter (YAML) ────────────────────────────────────────

/// YAML frontmatter of an agent `.md` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
    #[serde(default = "default_max_depth")]
    pub max_depth: u8,
    #[serde(default)]
    pub can_spawn: Vec<String>,
    /// Maximum number of sub-agents this agent can spawn per turn (0 = no
    /// limit). When the parent emits more `DELEGATE:` lines than this in a
    /// single output, only the first `max_sub_agents` are executed; the rest
    /// are logged and skipped. Defaults to 3.
    #[serde(default = "default_max_sub_agents")]
    pub max_sub_agents: u32,
}

fn default_max_sub_agents() -> u32 {
    3
}

fn default_model() -> String {
    "gpt-5".to_string()
}
fn default_temperature() -> f32 {
    0.3
}
fn default_max_tokens() -> u32 {
    4096
}
fn default_max_turns() -> u32 {
    25
}
fn default_max_depth() -> u8 {
    0
}

impl Default for AgentFrontmatter {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            model: default_model(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            tools: Vec::new(),
            max_turns: default_max_turns(),
            max_depth: default_max_depth(),
            can_spawn: Vec::new(),
            max_sub_agents: default_max_sub_agents(),
        }
    }
}

// ─── Full definition (frontmatter + body) ─────────────────────

/// A complete agent definition: frontmatter config + Markdown body (system prompt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    #[serde(flatten)]
    pub frontmatter: AgentFrontmatter,
    /// The Markdown body — becomes the agent's system prompt.
    pub system_prompt: String,
}

impl AgentDefinition {
    pub fn name(&self) -> &str {
        &self.frontmatter.name
    }
    pub fn model(&self) -> &str {
        &self.frontmatter.model
    }
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }
    pub fn tools(&self) -> &[String] {
        &self.frontmatter.tools
    }
    pub fn can_spawn(&self) -> &[String] {
        &self.frontmatter.can_spawn
    }
    pub fn max_depth(&self) -> u8 {
        self.frontmatter.max_depth
    }
    pub fn max_turns(&self) -> u32 {
        self.frontmatter.max_turns
    }
    /// Maximum sub-agents this agent can spawn per turn (0 = no limit).
    pub fn max_sub_agents(&self) -> u32 {
        self.frontmatter.max_sub_agents
    }

    /// Can this agent delegate to subagents?
    pub fn can_delegate(&self) -> bool {
        self.frontmatter.max_depth > 0 && !self.frontmatter.can_spawn.is_empty()
    }

    /// Can this agent spawn the given agent type?
    pub fn can_spawn_type(&self, agent_type: &str) -> bool {
        self.frontmatter.can_spawn.iter().any(|s| s == agent_type)
    }

    /// Convert to a `ResolvedRole` for use with the existing agent factory.
    pub fn to_resolved_role(&self) -> crate::orchestrator::roles::ResolvedRole {
        crate::orchestrator::roles::ResolvedRole {
            role_name: self.frontmatter.name.clone(),
            model: self.frontmatter.model.clone(),
            temperature: self.frontmatter.temperature,
            max_tokens: self.frontmatter.max_tokens,
            system_prompt: self.system_prompt.clone(),
            tools: self.frontmatter.tools.clone(),
            context_profile: "balanced".to_string(),
        }
    }
}

// ─── Parser ───────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("missing frontmatter delimiters (---)")]
    MissingDelimiters,
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml_neo::Error),
    #[error("empty system prompt (body is empty)")]
    EmptyBody,
}

/// Parse a Markdown+YAML agent file.
///
/// Format:
/// ```text
/// ---
/// name: coder
/// model: gpt-5
/// ...
/// ---
///
/// You are an expert Rust engineer...
/// ```
pub fn parse_agent_md(content: &str) -> Result<AgentDefinition, ParseError> {
    let content = content.trim_start_matches('\u{feff}');

    // Must start with `---`
    if !content.starts_with("---") {
        return Err(ParseError::MissingDelimiters);
    }

    // Find the closing `---`
    let after_first = &content[3..];
    let close = after_first
        .find("\n---")
        .ok_or(ParseError::MissingDelimiters)?;

    let yaml_block = &after_first[..close];
    // +4 to skip `\n---` and move past it; +1 if there's a newline after
    let body_start = 3 + close + 4;
    let body = if body_start < content.len() {
        content[body_start..].trim()
    } else {
        ""
    };

    if body.is_empty() {
        return Err(ParseError::EmptyBody);
    }

    let frontmatter: AgentFrontmatter = serde_yaml_neo::from_str(yaml_block)?;

    Ok(AgentDefinition {
        frontmatter,
        system_prompt: body.to_string(),
    })
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_agent() {
        let md = "---\nname: test\n---\nYou are a test agent.";
        let def = parse_agent_md(md).unwrap();
        assert_eq!(def.name(), "test");
        assert_eq!(def.system_prompt(), "You are a test agent.");
        assert_eq!(def.model(), "gpt-5"); // default
        assert_eq!(def.max_depth(), 0); // default
        assert!(!def.can_delegate());
    }

    #[test]
    fn test_parse_full_agent() {
        let md = "---\nname: researcher\nmodel: gpt-5\ntemperature: 0.3\nmax_tokens: 8192\ntools:\n  - web_search\n  - filesystem\nmax_turns: 25\nmax_depth: 1\ncan_spawn:\n  - explorer\n---\nYou are a research agent.";
        let def = parse_agent_md(md).unwrap();
        assert_eq!(def.name(), "researcher");
        assert_eq!(def.model(), "gpt-5");
        assert_eq!(def.frontmatter.temperature, 0.3);
        assert_eq!(def.frontmatter.max_tokens, 8192);
        assert_eq!(def.tools(), &["web_search", "filesystem"]);
        assert_eq!(def.max_turns(), 25);
        assert_eq!(def.max_depth(), 1);
        assert!(def.can_delegate());
        assert!(def.can_spawn_type("explorer"));
        assert!(!def.can_spawn_type("coder"));
    }

    #[test]
    fn test_parse_missing_delimiters() {
        let md = "name: test\nYou are a test.";
        assert!(matches!(
            parse_agent_md(md),
            Err(ParseError::MissingDelimiters)
        ));
    }

    #[test]
    fn test_parse_empty_body() {
        let md = "---\nname: test\n---\n";
        assert!(matches!(parse_agent_md(md), Err(ParseError::EmptyBody)));
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let md = "---\nname: [invalid yaml\n---\nBody.";
        assert!(parse_agent_md(md).is_err());
    }

    #[test]
    fn test_all_builtin_agents_parse() {
        for (name, content) in BUILTIN_AGENTS {
            let def = parse_agent_md(content)
                .unwrap_or_else(|e| panic!("built-in agent '{name}' failed to parse: {e}"));
            assert_eq!(def.name(), *name, "built-in agent name mismatch");
            assert!(
                !def.system_prompt().is_empty(),
                "built-in agent '{name}' has empty system prompt"
            );
        }
    }

    #[test]
    fn test_builtin_count() {
        assert_eq!(BUILTIN_AGENTS.len(), 8, "expected 8 built-in agents");
    }

    #[test]
    fn test_coder_is_leaf() {
        let def = parse_agent_md(CODER_MD).unwrap();
        assert_eq!(def.max_depth(), 0);
        assert!(!def.can_delegate());
    }

    #[test]
    fn test_architect_can_delegate() {
        let def = parse_agent_md(ARCHITECT_MD).unwrap();
        assert_eq!(def.max_depth(), 2);
        assert!(def.can_delegate());
        assert!(def.can_spawn_type("researcher"));
        assert!(def.can_spawn_type("coder"));
        assert!(!def.can_spawn_type("explorer"));
    }

    #[test]
    fn test_researcher_can_spawn_explorer() {
        let def = parse_agent_md(RESEARCHER_MD).unwrap();
        assert_eq!(def.max_depth(), 1);
        assert!(def.can_spawn_type("explorer"));
        assert!(!def.can_spawn_type("coder"));
    }

    #[test]
    fn test_serialize_roundtrip() {
        let md = "---\nname: test\nmodel: gpt-5\n---\nBody.";
        let def = parse_agent_md(md).unwrap();
        let json = serde_json::to_string(&def).unwrap();
        let back: AgentDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name(), "test");
        assert_eq!(back.system_prompt(), "Body.");
    }
}
