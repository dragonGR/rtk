use crate::tracking;
use crate::utils::{command_exists_cached, run_command_streaming, strip_ansi, truncate};
use anyhow::{Context, Result};
use std::borrow::Cow;
use std::process::Command;

pub fn run(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Try next directly first, fallback to npx if not found
    let next_exists = command_exists_cached("next");

    let mut cmd = if next_exists {
        Command::new("next")
    } else {
        let mut c = Command::new("npx");
        c.arg("next");
        c
    };

    cmd.arg("build");

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        let tool = if next_exists { "next" } else { "npx next" };
        eprintln!("Running: {} build", tool);
    }

    let output = run_command_streaming(&mut cmd)
        .context("Failed to run next build (try: npm install -g next)")?;

    let mut raw = String::with_capacity(output.stdout.len() + output.stderr.len() + 1);
    raw.push_str(&String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        raw.push('\n');
        raw.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    let filtered = filter_next_build(&raw);

    println!("{}", filtered);

    timer.track("next build", "rtk next build", &raw, &filtered);

    // Preserve exit code for CI/CD
    if !output.status.success() {
        return Err(crate::utils::status_code_error(output.status, "command failed"));
    }

    Ok(())
}

/// Filter Next.js build output - extract routes, bundles, warnings
fn filter_next_build(output: &str) -> String {
    let mut routes_static = 0;
    let mut routes_dynamic = 0;
    let mut routes_total = 0;
    let mut bundles: Vec<(String, f64, Option<f64>)> = Vec::new();
    let mut warnings = 0;
    let mut errors = 0;
    let mut build_time = String::new();

    let mut saw_already_optimized = false;
    let mut saw_cache = false;
    let mut saw_ready = false;

    for raw_line in output.lines() {
        // Strip ANSI per-line to avoid keeping a second full-sized output string in memory.
        let clean_line: Cow<'_, str> = if raw_line.contains('\u{1b}') {
            Cow::Owned(strip_ansi(raw_line))
        } else {
            Cow::Borrowed(raw_line)
        };
        let line: &str = clean_line.as_ref();

        if line.contains("already optimized") {
            saw_already_optimized = true;
        }
        if line.contains("Cache") {
            saw_cache = true;
        }
        if line.contains("Ready") {
            saw_ready = true;
        }

        // Count route types by symbol
        if line.starts_with("○") {
            routes_static += 1;
            routes_total += 1;
        } else if line.starts_with("●") || line.starts_with("◐") {
            routes_dynamic += 1;
            routes_total += 1;
        } else if line.starts_with("λ") {
            routes_total += 1;
        }

        // Extract bundle information (route + size + total size)
        if let Some((route, size, total)) = parse_bundle_line(line) {

            // Calculate percentage increase if both sizes present
            let pct_change = if total > 0.0 {
                Some(((total - size) / size) * 100.0)
            } else {
                None
            };

            bundles.push((route, total, pct_change));
        }

        // Count warnings and errors
        if line.to_lowercase().contains("warning") {
            warnings += 1;
        }
        if line.to_lowercase().contains("error") && !line.contains("0 error") {
            errors += 1;
        }

        // Extract build time
        if line.contains("Compiled") || line.contains("in") {
            if let Some(time_match) = extract_time(line) {
                build_time = time_match;
            }
        }
    }

    // Detect if build was skipped (already built)
    let already_built = saw_already_optimized || saw_cache || (routes_total == 0 && saw_ready);

    // Build filtered output
    let mut result = String::new();
    result.push_str("⚡ Next.js Build\n");
    result.push_str("═══════════════════════════════════════\n");

    if already_built && routes_total == 0 {
        result.push_str("✓ Already built (using cache)\n\n");
    } else if routes_total > 0 {
        result.push_str(&format!(
            "✓ {} routes ({} static, {} dynamic)\n\n",
            routes_total, routes_static, routes_dynamic
        ));
    }

    if !bundles.is_empty() {
        result.push_str("Bundles:\n");

        // Sort by size (descending) and show top 10
        bundles.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (route, size, pct_change) in bundles.iter().take(10) {
            let warning_marker = if let Some(pct) = pct_change {
                if *pct > 10.0 {
                    format!(" ⚠️ (+{:.0}%)", pct)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            result.push_str(&format!(
                "  {:<30} {:>6.0} kB{}\n",
                truncate(route, 30),
                size,
                warning_marker
            ));
        }

        if bundles.len() > 10 {
            result.push_str(&format!("\n  ... +{} more routes\n", bundles.len() - 10));
        }

        result.push('\n');
    }

    // Show build time and status
    if !build_time.is_empty() {
        result.push_str(&format!("Time: {} | ", build_time));
    }

    result.push_str(&format!("Errors: {} | Warnings: {}\n", errors, warnings));

    result.trim().to_string()
}

/// Extract time from build output (e.g., "Compiled in 34.2s")
fn extract_time(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let number = &line[start..i];

            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }

            if i + 1 < bytes.len() && bytes[i] == b'm' && bytes[i + 1] == b's' {
                return Some(format!("{}ms", number));
            }
            if i < bytes.len() && bytes[i] == b's' {
                return Some(format!("{}s", number));
            }
        } else {
            i += 1;
        }
    }
    None
}

fn parse_bundle_line(line: &str) -> Option<(String, f64, f64)> {
    let trimmed = line.trim_start_matches(|c: char| {
        c.is_whitespace() || matches!(c, '│' | '├' | '└' | '┌' | '─')
    });
    let mut parts = trimmed.split_whitespace();
    let marker = parts.next()?;
    if !matches!(marker, "○" | "●" | "◐" | "λ" | "✓") {
        return None;
    }
    let route = parts.next()?.to_string();
    let size: f64 = parts.next()?.parse().ok()?;
    let _size_unit = parts.next()?;
    let total: f64 = parts.next()?.parse().ok()?;
    let _total_unit = parts.next()?;
    Some((route, size, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_next_build() {
        let output = r#"
   ▲ Next.js 15.2.0

   Creating an optimized production build ...
✓ Compiled successfully
✓ Linting and checking validity of types
✓ Collecting page data
○ /                            1.2 kB        132 kB
● /dashboard                   2.5 kB        156 kB
○ /api/auth                    0.5 kB         89 kB

Route (app)                    Size     First Load JS
┌ ○ /                          1.2 kB        132 kB
├ ● /dashboard                 2.5 kB        156 kB
└ ○ /api/auth                  0.5 kB         89 kB

○  (Static)  prerendered as static content
●  (SSG)     prerendered as static HTML
λ  (Server)  server-side renders at runtime

✓ Built in 34.2s
"#;
        let result = filter_next_build(output);
        assert!(result.contains("⚡ Next.js Build"));
        assert!(result.contains("routes"));
        assert!(!result.contains("Creating an optimized")); // Should filter verbose logs
    }

    #[test]
    fn test_extract_time() {
        assert_eq!(extract_time("Built in 34.2s"), Some("34.2s".to_string()));
        assert_eq!(
            extract_time("Compiled in 1250ms"),
            Some("1250ms".to_string())
        );
        assert_eq!(extract_time("No time here"), None);
    }
}
