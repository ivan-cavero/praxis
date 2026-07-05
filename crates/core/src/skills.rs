//! Predefined skills — built-in skill templates that can be optionally loaded.
//!
//! Skills are injected into every agent's task context as additional system
//! guidance. They are optional: a project can enable zero, one, or many.
//!
//! Built-in skills ship with praxis and cover common patterns:
//! - `rust-best-practices` — idiomatic Rust patterns
//! - `clean-code` — general clean code principles
//! - `testing` — test-writing guidance
//! - `security` — security audit checklist
//! - `git-workflow` — commit and branch conventions
//! - `code-review` — review checklist
//!
//! Custom skills live in the project's `skills/` directory as `.md` files.

use std::collections::HashMap;

/// A predefined skill with a name, description, and content.
#[derive(Debug, Clone)]
pub struct PredefinedSkill {
    /// Unique identifier (kebab-case).
    pub id: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// Short description of what the skill covers.
    pub description: &'static str,
    /// The full skill content (markdown) injected into agent context.
    pub content: &'static str,
}

/// All built-in skills that ship with praxis.
pub fn builtin_skills() -> &'static [PredefinedSkill] {
    &BUILTIN_SKILLS
}

/// Get a specific built-in skill by ID.
pub fn get_builtin_skill(id: &str) -> Option<&'static PredefinedSkill> {
    BUILTIN_SKILLS.iter().find(|s| s.id == id)
}

/// Load skills by IDs. Returns the combined content for injection.
///
/// Unknown IDs are silently skipped (logged by caller).
pub fn load_skills_by_ids(ids: &[&str]) -> String {
    let mut content = String::new();
    for id in ids {
        if let Some(skill) = get_builtin_skill(id) {
            content.push_str(&format!("# Skill: {}\n\n", skill.name));
            content.push_str(skill.content);
            content.push_str("\n\n");
        }
    }
    content
}

/// Load skills from a config map of `skill_id -> enabled`.
/// Only skills with `enabled: true` are loaded.
pub fn load_skills_from_config(config: &HashMap<String, bool>) -> String {
    let ids: Vec<&str> = config
        .iter()
        .filter(|(_, enabled)| **enabled)
        .map(|(id, _)| id.as_str())
        .collect();
    load_skills_by_ids(&ids)
}

/// List all available skill IDs (built-in only; custom skills are discovered
/// from the filesystem at runtime).
pub fn list_available_skill_ids() -> Vec<&'static str> {
    BUILTIN_SKILLS.iter().map(|s| s.id).collect()
}

// ─── Built-in Skills ──────────────────────────────────────────

static BUILTIN_SKILLS: [PredefinedSkill; 6] = [
    PredefinedSkill {
        id: "rust-best-practices",
        name: "Rust Best Practices",
        description: "Idiomatic Rust patterns: error handling, ownership, async, traits",
        content: r#"## Rust Best Practices

### Error Handling
- Use `?` operator for error propagation. Never `unwrap()`/`expect()` outside tests.
- `thiserror` for library error enums, `anyhow` for application-level context.
- Return `Result<T, E>` from functions that can fail.

### Ownership & Borrowing
- `&str` / `&[T]` for function params. Return `String`/`Vec` only when you must allocate.
- `impl Iterator` return types over collecting into `Vec` when the caller only iterates.
- Avoid `clone()` where a borrow works.

### Async
- `tokio` for async runtime. Never block in async context — use `spawn_blocking`.
- Scope `MutexGuard` before `.await` — never hold a guard across an await point.

### Traits
- `Send + Sync` bounds on all trait objects that cross thread/actor boundaries.
- `#[async_trait]` for async methods in traits.
- Prefer trait objects (`dyn Trait`) only when you need dynamic dispatch.

### Style
- `tracing` macros (`info!`, `warn!`, `error!`, `#[instrument]`) — never `println!` in libraries.
- `#[expect(clippy::..., reason = "...")]` over `#[allow(...)]`.
- File-based modules (`foo.rs`, not `foo/mod.rs`).
"#,
    },
    PredefinedSkill {
        id: "clean-code",
        name: "Clean Code",
        description: "General clean code principles: naming, functions, comments, simplicity",
        content: r#"## Clean Code Principles

### Naming
- Descriptive names — never `x`, `a`, `i`, `res`, `e`, `fn`, `cb`, `tmp`, `val`, `obj`.
- Functions: verb-first (`getUserById`, `formatDate`, `validateInput`).
- Constants: SCREAMING_SNAKE_CASE (`MAX_ITERATIONS`, `DEFAULT_TIMEOUT`).
- Booleans: `is_`/`has_`/`can_`/`should_` prefix (`is_healthy`, `has_permission`).

### Functions
- Small and focused — one responsibility.
- Fewer parameters (≤3 ideal). If more, group into a struct.
- No side effects hidden in "get" or "check" functions.

### Comments
- If you need a comment, rewrite the code. Comments signal unclear logic.
- Comments explain WHY, not WHAT. The code already shows what.

### Simplicity
- Keep it simple or don't do it. Prefer the boring, readable solution.
- Delete useless code without fear. Dead code, unused imports — remove them.
- If you can't explain it fast, it's wrong. Unsummarizable complexity is a design flaw.
- Make it work first, optimize later. Correctness before performance.
"#,
    },
    PredefinedSkill {
        id: "testing",
        name: "Testing",
        description: "Test-writing guidance: unit tests, integration tests, edge cases",
        content: r#"## Testing Guidance

### Unit Tests
- Test one thing per test function. Name describes the scenario.
- Arrange-Act-Assert pattern.
- Test edge cases: empty input, None, zero, max values, boundary conditions.
- Test error paths, not just happy paths.

### Integration Tests
- Test the full workflow end-to-end.
- Use mock providers for external dependencies.
- Verify state transitions and side effects.

### What to Test
- Public API behavior, not implementation details.
- Error handling: does it fail gracefully?
- Concurrency: race conditions, deadlocks.
- Performance: regression detection for hot paths.

### Test Code Quality
- Tests are code too — apply the same quality standards.
- No magic numbers — use named constants.
- Setup and teardown should be explicit and minimal.
"#,
    },
    PredefinedSkill {
        id: "security",
        name: "Security Audit",
        description: "Security checklist: injection, XSS, secrets, auth, input validation",
        content: r#"## Security Audit Checklist

### Input Validation
- Validate ALL external input at boundaries (routes, requests, external APIs).
- Never trust user input — sanitize before use.
- Use parameterized queries, never string concatenation for SQL.

### Secrets
- Never hardcode API keys, tokens, or passwords in source code.
- Credentials go through the vault (keyring/env) — never raw `env::var` in library code.
- Never log secrets, API keys, or `.env` values.

### Injection
- SQL Injection: parameterized queries only.
- Command Injection: sanitize input to `exec()`, `spawn()`, `child_process`.
- Path Traversal: canonicalize and validate file paths.
- XSS: escape user input before rendering, never `innerHTML` with raw input.

### Authentication
- JWT: verify signature + expiry on every request. Never accept `alg: none`.
- Session tokens: use secure, httpOnly cookies. Rotate on privilege change.
- Passwords: hash with bcrypt/argon2, never store plaintext.

### Deserialization
- Never `eval()` untrusted input.
- Validate JSON structure before `JSON.parse` on external data.
- Beware prototype pollution in object merge/assign.
"#,
    },
    PredefinedSkill {
        id: "git-workflow",
        name: "Git Workflow",
        description: "Commit and branch conventions: conventional commits, atomic changes",
        content: r#"## Git Workflow

### Commits
- Conventional commits: `type(scope): short description`
- Types: `feat`, `fix`, `refactor`, `chore`, `docs`, `test`, `perf`
- Scope = crate or area: `feat(core): ...`, `fix(vault): ...`
- One logical change per commit. Never bundle unrelated changes.
- Write a concise commit message that matches the repo style.

### Branches
- Small, focused branches. One feature or fix per branch.
- Rebase before merging to keep history clean.
- Never force-push without explicit approval.

### Code Review
- Review all commits included in the PR, not just the latest.
- Check: correctness, style, security, test coverage.
- Small commits, or you're hiding something. Atomic, logical, reviewable units only.
"#,
    },
    PredefinedSkill {
        id: "code-review",
        name: "Code Review",
        description: "Review checklist: correctness, style, security, performance, tests",
        content: r#"## Code Review Checklist

### Correctness
- Does the code do what it claims?
- Are edge cases handled? (empty, None, zero, max, boundary)
- Are error paths covered? Does it fail gracefully?
- Are there off-by-one errors? Race conditions?

### Style
- Does it follow repo conventions? (naming, patterns, idioms)
- Are names descriptive? No single letters or cryptic abbreviations.
- Is the code readable? Can you understand it in one pass?

### Security
- Is all external input validated?
- Are there any hardcoded secrets?
- Are queries parameterized? Is user input sanitized?
- Are auth checks in place?

### Performance
- Are there unnecessary allocations? Clones that could be borrows?
- Are there O(n²) loops that could be O(n)?
- Is async used correctly? No blocking in async context?

### Tests
- Are there tests for the new code?
- Do the tests cover error paths, not just happy paths?
- Are the tests meaningful, not just coverage padding?
"#,
    },
];

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_skills_count() {
        assert!(builtin_skills().len() >= 6, "should have at least 6 built-in skills");
    }

    #[test]
    fn test_get_builtin_skill_by_id() {
        let skill = get_builtin_skill("rust-best-practices");
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().name, "Rust Best Practices");
    }

    #[test]
    fn test_get_builtin_skill_unknown() {
        assert!(get_builtin_skill("nonexistent").is_none());
    }

    #[test]
    fn test_load_skills_by_ids() {
        let content = load_skills_by_ids(&["rust-best-practices", "testing"]);
        assert!(content.contains("Rust Best Practices"));
        assert!(content.contains("Testing Guidance"));
        assert!(!content.contains("Security Audit"));
    }

    #[test]
    fn test_load_skills_skips_unknown() {
        let content = load_skills_by_ids(&["rust-best-practices", "nonexistent"]);
        assert!(content.contains("Rust Best Practices"));
        assert!(!content.contains("nonexistent"));
    }

    #[test]
    fn test_load_skills_from_config() {
        let mut config = HashMap::new();
        config.insert("rust-best-practices".to_string(), true);
        config.insert("testing".to_string(), false);
        config.insert("security".to_string(), true);

        let content = load_skills_from_config(&config);
        assert!(content.contains("Rust Best Practices"));
        assert!(!content.contains("Testing Guidance"));
        assert!(content.contains("Security Audit"));
    }

    #[test]
    fn test_list_available_skill_ids() {
        let ids = list_available_skill_ids();
        assert!(ids.contains(&"rust-best-practices"));
        assert!(ids.contains(&"clean-code"));
        assert!(ids.contains(&"security"));
    }

    #[test]
    fn test_all_skills_have_content() {
        for skill in builtin_skills() {
            assert!(!skill.content.is_empty(), "skill '{}' has empty content", skill.id);
            assert!(!skill.name.is_empty(), "skill '{}' has empty name", skill.id);
            assert!(!skill.description.is_empty(), "skill '{}' has empty description", skill.id);
        }
    }
}
