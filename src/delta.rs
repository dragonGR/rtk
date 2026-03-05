use std::collections::HashSet;
use std::path::PathBuf;

const MAX_ADDED_LINES: usize = 24;
const MAX_REMOVED_LINES: usize = 12;

fn enabled() -> bool {
    // Default ON. Set RTK_DELTA=0 to disable.
    std::env::var("RTK_DELTA").ok().as_deref() != Some("0")
}

fn sanitize_key(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    for ch in key.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.len() > 80 {
        out.truncate(80);
    }
    out
}

fn snapshot_path(command_key: &str) -> Option<PathBuf> {
    let base = dirs::data_local_dir()?;
    let key = sanitize_key(command_key);
    Some(base.join("rtk").join("delta").join(format!("{}.txt", key)))
}

fn load_snapshot(command_key: &str) -> Option<String> {
    let path = snapshot_path(command_key)?;
    std::fs::read_to_string(path).ok()
}

fn save_snapshot(command_key: &str, content: &str) {
    if let Some(path) = snapshot_path(command_key) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, content);
    }
}

/// Return delta-focused output for repeated command runs.
///
/// First run (no snapshot): returns `current` as-is.
/// Subsequent runs:
/// - unchanged: compact no-change summary
/// - changed: added/removed line summaries with caps
pub fn apply(command_key: &str, current: &str) -> String {
    if !enabled() {
        return current.to_string();
    }

    let previous = load_snapshot(command_key);
    save_snapshot(command_key, current);

    let Some(previous) = previous else {
        return current.to_string();
    };

    if previous == current {
        let line_count = current.lines().count();
        return format!(
            "Δ {}: no changes since previous run ({} lines unchanged)",
            command_key, line_count
        );
    }

    let prev_set: HashSet<&str> = previous.lines().collect();
    let curr_set: HashSet<&str> = current.lines().collect();

    let mut added: Vec<&str> = current
        .lines()
        .filter(|line| !line.trim().is_empty() && !prev_set.contains(line))
        .collect();
    added.dedup();

    let mut removed: Vec<&str> = previous
        .lines()
        .filter(|line| !line.trim().is_empty() && !curr_set.contains(line))
        .collect();
    removed.dedup();

    let mut out = String::new();
    out.push_str(&format!(
        "Δ {}: +{} / -{} changed lines\n",
        command_key,
        added.len(),
        removed.len()
    ));

    if !added.is_empty() {
        out.push_str("Added:\n");
        for line in added.iter().take(MAX_ADDED_LINES) {
            out.push_str(&format!("+ {}\n", line));
        }
        if added.len() > MAX_ADDED_LINES {
            out.push_str(&format!("... +{} more added lines\n", added.len() - MAX_ADDED_LINES));
        }
    }

    if !removed.is_empty() {
        out.push_str("Removed:\n");
        for line in removed.iter().take(MAX_REMOVED_LINES) {
            out.push_str(&format!("- {}\n", line));
        }
        if removed.len() > MAX_REMOVED_LINES {
            out.push_str(&format!(
                "... +{} more removed lines\n",
                removed.len() - MAX_REMOVED_LINES
            ));
        }
    }

    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_key(prefix: &str) -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{}_{}_{}", prefix, std::process::id(), n)
    }

    #[test]
    fn test_apply_first_run_returns_current() {
        let key = unique_key("delta_first");
        let current = "line1\nline2";
        let result = apply(&key, current);
        assert_eq!(result, current);
    }

    #[test]
    fn test_apply_no_changes_returns_compact_summary() {
        let key = unique_key("delta_same");
        let current = "a\nb\nc";
        let _ = apply(&key, current);
        let result = apply(&key, current);
        assert!(result.contains("no changes since previous run"));
    }

    #[test]
    fn test_apply_changed_returns_added_removed() {
        let key = unique_key("delta_changed");
        let _ = apply(&key, "a\nb\nc");
        let result = apply(&key, "a\nc\nd");
        assert!(result.contains("Added:"));
        assert!(result.contains("+ d"));
        assert!(result.contains("Removed:"));
        assert!(result.contains("- b"));
    }
}
