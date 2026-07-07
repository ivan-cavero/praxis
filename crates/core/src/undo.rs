//! Undo/redo — step-by-step file-change history for sessions.
//!
//! While [`crate::rollback`] captures a single baseline at session start,
//! this module records a **stack** of change snapshots after each phase
//! iteration. Users can then step backward ([`undo_change`]) or forward
//! ([`redo_change`]) through the history, restoring the working tree to any
//! recorded point.
//!
//! All git operations shell out to the system `git` binary and degrade
//! gracefully in non-git directories (records are saved with empty commit
//! hashes and diffs; undo/redo become no-ops).

use praxis_persistence::{ChangeRecord, SqliteEventStore};
use std::path::Path;
use std::process::Command;

/// Capture the current git state as a new change record for `session_id`.
///
/// Records `git rev-parse HEAD` and `git diff HEAD` at the current working
/// tree state. If the working directory is not a git repo, both fields are
/// empty strings (the record is still saved — undo/redo will no-op).
///
/// Returns the saved record, or an error only if persistence fails.
pub fn capture_change(
    store: &SqliteEventStore,
    session_id: uuid::Uuid,
    working_dir: &Path,
    description: &str,
) -> anyhow::Result<ChangeRecord> {
    let commit_hash = git_head(working_dir).unwrap_or_default();
    let diff = git_diff_head(working_dir).unwrap_or_default();

    let seq = store
        .next_change_seq(&session_id.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to get next change seq: {}", e))?;

    let record = ChangeRecord {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        seq,
        commit_hash,
        diff,
        description: description.to_string(),
        undone: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    store
        .save_change_record(&record)
        .map_err(|e| anyhow::anyhow!("Failed to save change record: {}", e))?;

    tracing::info!(
        "Captured change #{} for session {}: {}",
        seq,
        session_id,
        description
    );

    Ok(record)
}

/// Undo the latest active change for `session_id`.
///
/// Marks the latest non-undone record as `undone` and restores the working
/// tree to the state of the previous active record (or the session baseline
/// if there are no remaining active records).
///
/// Returns a description of what was undone, or an error if no active changes
/// exist or git operations fail.
pub fn undo_change(
    store: &SqliteEventStore,
    session_id: uuid::Uuid,
    working_dir: &Path,
) -> anyhow::Result<String> {
    let latest = store
        .get_latest_active_change(&session_id.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to query latest active change: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No active changes to undo for session {}", session_id))?;

    // Mark it as undone.
    store
        .set_change_undone(&latest.id, true)
        .map_err(|e| anyhow::anyhow!("Failed to mark change as undone: {}", e))?;

    // Find the previous active record to restore to.
    let records = store
        .list_change_records(&session_id.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to list change records: {}", e))?;

    let target = records
        .iter()
        .filter(|r| r.seq < latest.seq && !r.undone)
        .max_by_key(|r| r.seq);

    match target {
        Some(prev) => {
            restore_to_record(working_dir, prev)?;
            Ok(format!(
                "Undone: {} (restored to change #{}: {})",
                latest.description, prev.seq, prev.description
            ))
        }
        None => {
            // No previous active record — restore to the session baseline.
            restore_to_baseline(store, session_id, working_dir)?;
            Ok(format!(
                "Undone: {} (restored to session baseline)",
                latest.description
            ))
        }
    }
}

/// Redo the latest undone change for `session_id`.
///
/// Marks the latest undone record as active and restores the working tree to
/// its state.
///
/// Returns a description of what was redone, or an error if no undone changes
/// exist or git operations fail.
pub fn redo_change(
    store: &SqliteEventStore,
    session_id: uuid::Uuid,
    working_dir: &Path,
) -> anyhow::Result<String> {
    let latest = store
        .get_latest_undone_change(&session_id.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to query latest undone change: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No undone changes to redo for session {}", session_id))?;

    // Mark it as active.
    store
        .set_change_undone(&latest.id, false)
        .map_err(|e| anyhow::anyhow!("Failed to mark change as active: {}", e))?;

    restore_to_record(working_dir, &latest)?;

    Ok(format!(
        "Redone: change #{}: {}",
        latest.seq, latest.description
    ))
}

/// List all change records for a session, ordered by sequence.
pub fn list_changes(
    store: &SqliteEventStore,
    session_id: uuid::Uuid,
) -> anyhow::Result<Vec<ChangeRecord>> {
    store
        .list_change_records(&session_id.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to list change records: {}", e))
}

// ─── Internal helpers ──────────────────────────────────────────

/// Restore the working tree to the state captured in `record`.
///
/// Steps (each skipped if the field is empty):
/// 1. `git checkout -- .` + `git clean -fd` — discard current uncommitted changes.
/// 2. `git reset --hard <commit_hash>` — move HEAD to the record's commit.
/// 3. Re-apply the record's diff via `git apply`.
fn restore_to_record(working_dir: &Path, record: &ChangeRecord) -> anyhow::Result<()> {
    if record.commit_hash.is_empty() && record.diff.is_empty() {
        // Non-git directory — nothing to restore.
        return Ok(());
    }

    // 1. Discard all uncommitted changes.
    let _ = Command::new("git")
        .args(["checkout", "--", "."])
        .current_dir(working_dir)
        .output();
    let _ = Command::new("git")
        .args(["clean", "-fd"])
        .current_dir(working_dir)
        .output();

    // 2. Reset HEAD to the record's commit.
    if !record.commit_hash.is_empty() {
        let reset = Command::new("git")
            .args(["reset", "--hard", &record.commit_hash])
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

    // 3. Re-apply the record's diff (if any).
    if !record.diff.is_empty() {
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
                .write_all(record.diff.as_bytes())
                .map_err(|e| anyhow::anyhow!("git apply write failed: {}", e))?;
        }

        let output = apply
            .wait_with_output()
            .map_err(|e| anyhow::anyhow!("git apply wait failed: {}", e))?;
        if !output.status.success() {
            tracing::warn!(
                "git apply failed to restore diff: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    Ok(())
}

/// Restore the working tree to the session baseline (used when undoing the
/// first change in the stack).
fn restore_to_baseline(
    store: &SqliteEventStore,
    session_id: uuid::Uuid,
    working_dir: &Path,
) -> anyhow::Result<()> {
    let baseline = store
        .get_session_baseline(&session_id.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to load session baseline: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No baseline found for session {}", session_id))?;

    if baseline.baseline_commit.is_empty() && baseline.uncommitted_diff.is_empty() {
        return Ok(()); // Non-git directory.
    }

    // Discard all uncommitted changes.
    let _ = Command::new("git")
        .args(["checkout", "--", "."])
        .current_dir(working_dir)
        .output();
    let _ = Command::new("git")
        .args(["clean", "-fd"])
        .current_dir(working_dir)
        .output();

    // Reset HEAD to baseline commit.
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

    // Re-apply the original uncommitted diff.
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
        }

        let output = apply
            .wait_with_output()
            .map_err(|e| anyhow::anyhow!("git apply wait failed: {}", e))?;
        if !output.status.success() {
            tracing::warn!(
                "git apply failed to restore baseline diff: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    Ok(())
}

// ─── Git helpers ───────────────────────────────────────────────

/// Get the current HEAD commit hash, or `None` if not in a git repo.
fn git_head(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
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
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_change_non_git_directory() {
        let store = SqliteEventStore::in_memory().unwrap();
        let sid = uuid::Uuid::new_v4();
        let tmp = std::env::temp_dir();

        let record = capture_change(&store, sid, &tmp, "test change").unwrap();

        assert_eq!(record.session_id, sid.to_string());
        assert_eq!(record.seq, 0);
        assert_eq!(record.description, "test change");
        assert!(!record.undone);
        // In a non-git dir (temp_dir might be in a repo), commit_hash and diff
        // may or may not be empty. Just verify the record was saved.
        let records = store.list_change_records(&sid.to_string()).unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_capture_multiple_changes_increments_seq() {
        let store = SqliteEventStore::in_memory().unwrap();
        let sid = uuid::Uuid::new_v4();
        let tmp = std::env::temp_dir();

        let r0 = capture_change(&store, sid, &tmp, "change 0").unwrap();
        let r1 = capture_change(&store, sid, &tmp, "change 1").unwrap();
        let r2 = capture_change(&store, sid, &tmp, "change 2").unwrap();

        assert_eq!(r0.seq, 0);
        assert_eq!(r1.seq, 1);
        assert_eq!(r2.seq, 2);

        let records = store.list_change_records(&sid.to_string()).unwrap();
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn test_undo_no_active_changes_returns_error() {
        let store = SqliteEventStore::in_memory().unwrap();
        let sid = uuid::Uuid::new_v4();
        let tmp = std::env::temp_dir();

        let result = undo_change(&store, sid, &tmp);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No active changes to undo")
        );
    }

    #[test]
    fn test_redo_no_undone_changes_returns_error() {
        let store = SqliteEventStore::in_memory().unwrap();
        let sid = uuid::Uuid::new_v4();
        let tmp = std::env::temp_dir();

        let result = redo_change(&store, sid, &tmp);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No undone changes to redo")
        );
    }

    #[test]
    fn test_undo_marks_record_as_undone() {
        let store = SqliteEventStore::in_memory().unwrap();
        let sid = uuid::Uuid::new_v4();
        let tmp = std::env::temp_dir();

        // Capture a change.
        capture_change(&store, sid, &tmp, "change 0").unwrap();

        // Undo it — should mark as undone. Since there's no baseline, this
        // will error on restore_to_baseline, but the record should still be
        // marked as undone. Let's verify the DB state directly.
        // (In a non-git dir with no baseline, undo will error, but the undone
        // flag is set before the restore attempt.)
        let _ = undo_change(&store, sid, &tmp);

        let records = store.list_change_records(&sid.to_string()).unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].undone);
    }

    #[test]
    fn test_list_changes_returns_ordered_records() {
        let store = SqliteEventStore::in_memory().unwrap();
        let sid = uuid::Uuid::new_v4();
        let tmp = std::env::temp_dir();

        capture_change(&store, sid, &tmp, "first").unwrap();
        capture_change(&store, sid, &tmp, "second").unwrap();
        capture_change(&store, sid, &tmp, "third").unwrap();

        let records = list_changes(&store, sid).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].description, "first");
        assert_eq!(records[1].description, "second");
        assert_eq!(records[2].description, "third");
    }
}
