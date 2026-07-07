//! Session rollback — capture and restore git baselines.
//!
//! When a goal run starts, [`capture_baseline`] records the current git HEAD
//! commit and any uncommitted working-tree diff into the event store. If the
//! session's changes need to be reverted, [`restore_baseline`] resets the
//! working tree back to that exact pre-session state. [`diff_from_baseline`]
//! shows what the session changed.
//!
//! All git operations shell out to the system `git` binary and are no-ops
//! (returning empty values) when the working directory is not a git repository,
//! so rollback degrades gracefully in non-git projects.

use praxis_persistence::{SessionBaseline, SqliteEventStore};
use std::path::Path;
use std::process::Command;

/// Capture a git baseline for `session_id` in `working_dir` and persist it.
///
/// Records:
/// - `git rev-parse HEAD` (the baseline commit)
/// - `git diff HEAD` (uncommitted working-tree changes)
///
/// If `working_dir` is not a git repo, both fields are empty strings and the
/// baseline is still saved (rollback will then be a no-op). Returns the
/// captured baseline, or an error only if persistence fails.
pub fn capture_baseline(
    store: &SqliteEventStore,
    session_id: uuid::Uuid,
    working_dir: &Path,
) -> anyhow::Result<SessionBaseline> {
    let baseline_commit = git_head(working_dir).unwrap_or_default();
    let uncommitted_diff = git_diff_head(working_dir).unwrap_or_default();

    let baseline = SessionBaseline {
        session_id: session_id.to_string(),
        baseline_commit,
        uncommitted_diff,
        project_path: working_dir
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| working_dir.to_string_lossy().to_string()),
        captured_at: chrono::Utc::now().to_rfc3339(),
    };

    store
        .save_session_baseline(&baseline)
        .map_err(|e| anyhow::anyhow!("Failed to save session baseline: {}", e))?;

    tracing::info!(
        "Captured rollback baseline for session {}: commit={}",
        session_id,
        if baseline.baseline_commit.is_empty() {
            "(non-git)"
        } else {
            &baseline.baseline_commit
        }
    );

    Ok(baseline)
}

/// Restore the working tree to the baseline captured for `session_id`.
///
/// Steps (each skipped if the baseline field is empty):
/// 1. `git checkout -- .` + `git clean -fd` — discard uncommitted changes.
/// 2. `git reset --hard <baseline_commit>` — move HEAD back to the baseline.
/// 3. Re-apply the original uncommitted diff via `git apply`.
///
/// Returns a description of what was restored, or an error if git fails.
pub fn restore_baseline(
    store: &SqliteEventStore,
    session_id: uuid::Uuid,
    working_dir: &Path,
) -> anyhow::Result<String> {
    let baseline = store
        .get_session_baseline(&session_id.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to load session baseline: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No rollback baseline found for session {}", session_id))?;

    if baseline.baseline_commit.is_empty() && baseline.uncommitted_diff.is_empty() {
        return Ok("Baseline was captured in a non-git directory; nothing to restore.".to_string());
    }

    // 1. Discard all uncommitted changes (staged + unstaged + untracked).
    let _ = Command::new("git")
        .args(["checkout", "--", "."])
        .current_dir(working_dir)
        .output();
    let _ = Command::new("git")
        .args(["clean", "-fd"])
        .current_dir(working_dir)
        .output();

    // 2. Reset HEAD to the baseline commit.
    if !baseline.baseline_commit.is_empty() {
        let reset = Command::new("git")
            .args(["reset", "--hard", &baseline.baseline_commit])
            .current_dir(working_dir)
            .output()
            .map_err(|e| anyhow::anyhow!("git reset --hard failed: {}", e))?;
        if !reset.status.success() {
            return Err(anyhow::anyhow!(
                "git reset --hard failed: {}",
                String::from_utf8_lossy(&reset.stderr)
            ));
        }
    }

    // 3. Re-apply the original uncommitted diff (if any).
    if !baseline.uncommitted_diff.is_empty() {
        let mut apply = Command::new("git")
            .args(["apply"])
            .current_dir(working_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("git apply spawn failed: {}", e))?;

        use std::io::Write;
        if let Some(mut child_stdin) = apply.stdin.take() {
            child_stdin
                .write_all(baseline.uncommitted_diff.as_bytes())
                .map_err(|e| anyhow::anyhow!("git apply write failed: {}", e))?;
            // Drop stdin to signal EOF.
        }

        let output = apply
            .wait_with_output()
            .map_err(|e| anyhow::anyhow!("git apply wait failed: {}", e))?;
        if !output.status.success() {
            tracing::warn!(
                "git apply failed to restore uncommitted diff: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    tracing::info!("Restored rollback baseline for session {}", session_id);
    Ok(format!(
        "Restored to commit {} (uncommitted diff re-applied: {})",
        if baseline.baseline_commit.is_empty() {
            "(none)"
        } else {
            &baseline.baseline_commit[..8.min(baseline.baseline_commit.len())]
        },
        if baseline.uncommitted_diff.is_empty() {
            "no"
        } else {
            "yes"
        }
    ))
}

/// Compute the diff between the baseline commit and the current working tree.
///
/// Returns the unified diff text. If no baseline exists or the directory is
/// not a git repo, returns an empty string.
pub fn diff_from_baseline(
    store: &SqliteEventStore,
    session_id: uuid::Uuid,
    working_dir: &Path,
) -> anyhow::Result<String> {
    let Some(baseline) = store
        .get_session_baseline(&session_id.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to load session baseline: {}", e))?
    else {
        return Ok(String::new());
    };

    if baseline.baseline_commit.is_empty() {
        return Ok(String::new());
    }

    let output = Command::new("git")
        .args(["diff", &baseline.baseline_commit, "HEAD"])
        .current_dir(working_dir)
        .output()
        .map_err(|e| anyhow::anyhow!("git diff failed: {}", e))?;

    if !output.status.success() {
        return Ok(String::new());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ─── Git helpers ───────────────────────────────────────────────

/// Get the current HEAD commit hash, or `None` if not in a git repo.
fn git_head(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if hash.is_empty() {
        None
    } else {
        Some(hash)
    }
}

/// Get the uncommitted working-tree diff (`git diff HEAD`), or `None` if not
/// in a git repo.
fn git_diff_head(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo(dir: &Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .expect("git init");
        Command::new("git")
            .args(["config", "user.email", "test@praxis.dev"])
            .current_dir(dir)
            .output()
            .expect("git config email");
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .expect("git config name");
    }

    fn commit_all(dir: &Path, msg: &str) {
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir)
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", msg])
            .current_dir(dir)
            .output()
            .expect("git commit");
    }

    #[test]
    fn test_capture_and_restore_baseline() {
        let tmp = std::env::temp_dir().join(format!("praxis-rollback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).expect("mkdir");
        init_repo(&tmp);

        // Initial commit with a file.
        std::fs::write(tmp.join("a.txt"), "original\n").expect("write");
        commit_all(&tmp, "initial");

        let store = SqliteEventStore::in_memory().expect("store");
        let sid = uuid::Uuid::new_v4();

        // Capture baseline.
        let baseline = capture_baseline(&store, sid, &tmp).expect("capture");
        assert!(!baseline.baseline_commit.is_empty());
        assert!(baseline.uncommitted_diff.is_empty());

        // Simulate session changes: modify the file + add a new one.
        std::fs::write(tmp.join("a.txt"), "modified by session\n").expect("write");
        std::fs::write(tmp.join("b.txt"), "new file from session\n").expect("write");

        // Restore.
        let msg = restore_baseline(&store, sid, &tmp).expect("restore");
        assert!(msg.contains("Restored"));

        // Verify files are back to baseline state.
        assert_eq!(
            std::fs::read_to_string(tmp.join("a.txt")).unwrap().trim_end(),
            "original"
        );
        assert!(!tmp.join("b.txt").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_capture_with_uncommitted_diff() {
        let tmp = std::env::temp_dir().join(format!("praxis-rollback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).expect("mkdir");
        init_repo(&tmp);

        std::fs::write(tmp.join("a.txt"), "v1\n").expect("write");
        commit_all(&tmp, "initial");

        // Make an uncommitted change before capturing.
        std::fs::write(tmp.join("a.txt"), "v2-uncommitted\n").expect("write");

        let store = SqliteEventStore::in_memory().expect("store");
        let sid = uuid::Uuid::new_v4();

        let baseline = capture_baseline(&store, sid, &tmp).expect("capture");
        assert!(!baseline.baseline_commit.is_empty());
        assert!(!baseline.uncommitted_diff.is_empty());

        // Session modifies further.
        std::fs::write(tmp.join("a.txt"), "v3-session\n").expect("write");

        // Restore should bring back v2-uncommitted.
        restore_baseline(&store, sid, &tmp).expect("restore");
        assert_eq!(
            std::fs::read_to_string(tmp.join("a.txt")).unwrap().trim_end(),
            "v2-uncommitted"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_diff_from_baseline() {
        let tmp = std::env::temp_dir().join(format!("praxis-rollback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).expect("mkdir");
        init_repo(&tmp);

        std::fs::write(tmp.join("a.txt"), "original\n").expect("write");
        commit_all(&tmp, "initial");

        let store = SqliteEventStore::in_memory().expect("store");
        let sid = uuid::Uuid::new_v4();
        capture_baseline(&store, sid, &tmp).expect("capture");

        // No changes yet → empty diff.
        let diff = diff_from_baseline(&store, sid, &tmp).expect("diff");
        assert!(diff.is_empty());

        // Make a change and commit it.
        std::fs::write(tmp.join("a.txt"), "changed\n").expect("write");
        commit_all(&tmp, "session change");

        let diff = diff_from_baseline(&store, sid, &tmp).expect("diff");
        assert!(diff.contains("changed"));
        assert!(diff.contains("-original"));
        assert!(diff.contains("+changed"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_non_git_directory() {
        let tmp = std::env::temp_dir().join(format!("praxis-rollback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).expect("mkdir");
        // No git init.

        let store = SqliteEventStore::in_memory().expect("store");
        let sid = uuid::Uuid::new_v4();

        let baseline = capture_baseline(&store, sid, &tmp).expect("capture");
        assert!(baseline.baseline_commit.is_empty());
        assert!(baseline.uncommitted_diff.is_empty());

        let msg = restore_baseline(&store, sid, &tmp).expect("restore");
        assert!(msg.contains("non-git"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
