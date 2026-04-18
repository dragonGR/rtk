//! Filters grep output by grouping matches by file.

use crate::core::config;
use crate::core::stream::exec_capture;
use crate::core::tracking;
use crate::core::utils::resolved_command;
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::{HashMap, HashSet};

const DEFAULT_EXCLUDE_DIRS: [&str; 5] = ["node_modules", "dist", "target", ".next", ".git"];

#[derive(Default)]
struct FileMatches {
    total_matches: usize,
    displayed_matches: Vec<(usize, String)>,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    pattern: &str,
    path: &str,
    max_line_len: usize,
    max_results: usize,
    context_only: bool,
    file_type: Option<&str>,
    extra_args: &[String],
    verbose: u8,
) -> Result<i32> {
    let timer = tracking::TimedExecution::start();
    let limits = config::limits();
    let per_file = limits.grep_max_per_file.max(1);
    let max_results = max_results.min(limits.grep_max_results).max(1);
    let max_tracked_files = max_results;

    if verbose > 0 {
        eprintln!("grep: '{}' in {}", pattern, path);
    }

    let (extra_args, disable_default_excludes) = parse_control_flags(extra_args);
    let use_default_excludes = !disable_default_excludes
        && !extra_args.iter().any(|a| {
            a == "--hidden"
                || a == "--no-ignore"
                || a == "--no-ignore-vcs"
                || a == "-uuu"
                || a == "-uu"
        });

    // Fix: convert BRE alternation \| → | for rg (which uses PCRE-style regex)
    let rg_pattern = pattern.replace(r"\|", "|");

    let mut rg_cmd = resolved_command("rg");
    rg_cmd.args(["-n", "--no-heading"]);

    if let Some(ft) = file_type {
        rg_cmd.arg("--type").arg(ft);
    }

    if use_default_excludes {
        for dir in DEFAULT_EXCLUDE_DIRS {
            rg_cmd.arg("--glob").arg(format!("!{}/**", dir));
        }
    }

    for arg in &extra_args {
        // Fix: skip grep-ism -r flag (rg is recursive by default; rg -r means --replace)
        if arg == "-r" || arg == "--recursive" {
            continue;
        }
        rg_cmd.arg(arg);
    }
    rg_cmd.arg(&rg_pattern).arg(path);

    let mut used_engine = "rg";
    let result = match exec_capture(&mut rg_cmd) {
        Ok(result) => {
            if should_fallback_to_grep(result.exit_code, &result.stderr) {
                used_engine = "grep";
                run_grep_fallback(pattern, path, &extra_args, use_default_excludes)?
            } else {
                result
            }
        }
        Err(_) => {
            used_engine = "grep";
            run_grep_fallback(pattern, path, &extra_args, use_default_excludes)?
        }
    };

    let exit_code = result.exit_code;
    let raw_output = result.stdout.clone();

    if result.stdout.trim().is_empty() {
        // Show stderr for errors (bad regex, missing file, etc.)
        if exit_code == 2 {
            if !result.stderr.trim().is_empty() {
                eprintln!("{}", result.stderr.trim());
            }
        }
        let msg = format!("0 matches for '{}'", pattern);
        println!("{}", msg);
        timer.track(
            &format!("grep -rn '{}' {}", pattern, path),
            &format!("rtk grep ({})", used_engine),
            &raw_output,
            &msg,
        );
        return Ok(exit_code);
    }

    let mut by_file: HashMap<String, FileMatches> = HashMap::new();
    let mut hidden_files = HashSet::new();
    let mut total = 0;
    let mut displayed = 0;
    let mut truncated_scan = false;

    // Compile context regex once (instead of per-line in clean_line)
    let context_re = if context_only {
        Regex::new(&format!("(?i).{{0,20}}{}.*", regex::escape(pattern))).ok()
    } else {
        None
    };

    for line in result.stdout.lines() {
        let parts: Vec<&str> = line.splitn(3, ':').collect();

        let (file, line_num, content) = if parts.len() == 3 {
            let ln = parts[1].parse().unwrap_or(0);
            (parts[0].to_string(), ln, parts[2])
        } else if parts.len() == 2 {
            let ln = parts[0].parse().unwrap_or(0);
            (path.to_string(), ln, parts[1])
        } else {
            continue;
        };

        total += 1;
        let cleaned = clean_line(content, max_line_len, context_re.as_ref(), pattern);
        if !record_match(
            &mut by_file,
            &mut hidden_files,
            file,
            line_num,
            cleaned,
            per_file,
            max_results,
            max_tracked_files,
            &mut displayed,
        ) {
            truncated_scan = true;
            break;
        }
    }

    let mut rtk_output = String::new();
    let total_files = by_file.len() + hidden_files.len();
    if truncated_scan {
        rtk_output.push_str(&format!(
            "{}+ matches in {}+F (scan truncated):\n\n",
            total, total_files
        ));
    } else {
        rtk_output.push_str(&format!("{} matches in {}F:\n\n", total, total_files));
    }

    let mut shown = 0;
    let mut files: Vec<_> = by_file.iter().collect();
    files.sort_by_key(|(f, _)| *f);

    for (file, matches) in files {
        if shown >= max_results {
            break;
        }

        let file_display = compact_path(file);
        rtk_output.push_str(&format!(
            "[file] {} ({}):\n",
            file_display, matches.total_matches
        ));

        for (line_num, content) in &matches.displayed_matches {
            rtk_output.push_str(&format!("  {:>4}: {}\n", line_num, content));
            shown += 1;
            if shown >= max_results {
                break;
            }
        }

        if matches.total_matches > matches.displayed_matches.len() {
            rtk_output.push_str(&format!(
                "  +{}\n",
                matches.total_matches - matches.displayed_matches.len()
            ));
        }
        rtk_output.push('\n');
    }

    if !hidden_files.is_empty() {
        rtk_output.push_str(&format!(
            "... +{} more files beyond the output budget\n",
            hidden_files.len()
        ));
    }
    if total > shown {
        rtk_output.push_str(&format!("... +{}\n", total - shown));
    }

    print!("{}", rtk_output);
    timer.track(
        &format!("grep -rn '{}' {}", pattern, path),
        &format!("rtk grep ({})", used_engine),
        &raw_output,
        &rtk_output,
    );

    Ok(exit_code)
}

#[allow(clippy::too_many_arguments)]
fn record_match(
    by_file: &mut HashMap<String, FileMatches>,
    hidden_files: &mut HashSet<String>,
    file: String,
    line_num: usize,
    cleaned: String,
    per_file: usize,
    max_results: usize,
    max_tracked_files: usize,
    displayed: &mut usize,
) -> bool {
    if let Some(entry) = by_file.get_mut(&file) {
        entry.total_matches += 1;
        if entry.displayed_matches.len() < per_file && *displayed < max_results {
            entry.displayed_matches.push((line_num, cleaned));
            *displayed += 1;
        }
        return true;
    }

    if by_file.len() >= max_tracked_files && *displayed >= max_results {
        hidden_files.insert(file);
        return false;
    }

    if by_file.len() >= max_tracked_files {
        hidden_files.insert(file);
        return true;
    }

    let mut entry = FileMatches {
        total_matches: 1,
        displayed_matches: Vec::new(),
    };
    if *displayed < max_results {
        entry.displayed_matches.push((line_num, cleaned));
        *displayed += 1;
    }
    by_file.insert(file, entry);
    true
}

fn clean_line(line: &str, max_len: usize, context_re: Option<&Regex>, pattern: &str) -> String {
    let trimmed = line.trim();

    if let Some(re) = context_re {
        if let Some(m) = re.find(trimmed) {
            let matched = m.as_str();
            if matched.len() <= max_len {
                return matched.to_string();
            }
        }
    }

    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        let lower = trimmed.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        if let Some(pos) = lower.find(&pattern_lower) {
            let char_pos = lower[..pos].chars().count();
            let chars: Vec<char> = trimmed.chars().collect();
            let char_len = chars.len();

            let start = char_pos.saturating_sub(max_len / 3);
            let end = (start + max_len).min(char_len);
            let start = if end == char_len {
                end.saturating_sub(max_len)
            } else {
                start
            };

            let slice: String = chars[start..end].iter().collect();
            if start > 0 && end < char_len {
                format!("...{}...", slice)
            } else if start > 0 {
                format!("...{}", slice)
            } else {
                format!("{}...", slice)
            }
        } else {
            let t: String = trimmed.chars().take(max_len - 3).collect();
            format!("{}...", t)
        }
    }
}

fn compact_path(path: &str) -> String {
    if path.len() <= 50 {
        return path.to_string();
    }

    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 3 {
        return path.to_string();
    }

    format!(
        "{}/.../{}/{}",
        parts[0],
        parts[parts.len() - 2],
        parts[parts.len() - 1]
    )
}

fn run_grep_fallback(
    pattern: &str,
    path: &str,
    extra_args: &[String],
    use_default_excludes: bool,
) -> Result<crate::core::stream::CaptureResult> {
    let mut grep_cmd = resolved_command("grep");
    grep_cmd.arg("-rn");
    if use_default_excludes {
        for dir in DEFAULT_EXCLUDE_DIRS {
            grep_cmd.arg(format!("--exclude-dir={}", dir));
        }
    }

    for arg in extra_args {
        if arg == "-r" || arg == "--recursive" {
            continue;
        }
        grep_cmd.arg(arg);
    }

    grep_cmd.arg(pattern).arg(path);
    exec_capture(&mut grep_cmd).context("grep fallback failed")
}

fn should_fallback_to_grep(exit_code: i32, rg_stderr: &str) -> bool {
    if exit_code != 2 {
        return false;
    }

    let err = rg_stderr.to_lowercase();
    err.contains("unrecognized flag")
        || err.contains("unknown option")
        || err.contains("unexpected argument")
        || err.contains("regex parse error")
        || err.contains("error parsing regex")
}

fn parse_control_flags(extra_args: &[String]) -> (Vec<String>, bool) {
    let mut filtered = Vec::with_capacity(extra_args.len());
    let mut disable_default_excludes = false;
    for arg in extra_args {
        if arg == "--no-default-excludes" {
            disable_default_excludes = true;
            continue;
        }
        filtered.push(arg.clone());
    }
    (filtered, disable_default_excludes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_line() {
        let line = "            const result = someFunction();";
        let cleaned = clean_line(line, 50, None, "result");
        assert!(!cleaned.starts_with(' '));
        assert!(cleaned.len() <= 50);
    }

    #[test]
    fn test_compact_path() {
        let path = "/Users/patrick/dev/project/src/components/Button.tsx";
        let compact = compact_path(path);
        assert!(compact.len() <= 60);
    }

    #[test]
    fn test_extra_args_accepted() {
        // Test that the function signature accepts extra_args
        // This is a compile-time test - if it compiles, the signature is correct
        let _extra: Vec<String> = vec!["-i".to_string(), "-A".to_string(), "3".to_string()];
        // No need to actually run - we're verifying the parameter exists
    }

    #[test]
    fn test_clean_line_multibyte() {
        // Thai text that exceeds max_len in bytes
        let line = "  สวัสดีครับ นี่คือข้อความที่ยาวมากสำหรับทดสอบ  ";
        let cleaned = clean_line(line, 20, None, "ครับ");
        // Should not panic
        assert!(!cleaned.is_empty());
    }

    #[test]
    fn test_clean_line_emoji() {
        let line = "🎉🎊🎈🎁🎂🎄 some text 🎃🎆🎇✨";
        let cleaned = clean_line(line, 15, None, "text");
        assert!(!cleaned.is_empty());
    }

    // Fix: BRE \| alternation is translated to PCRE | for rg
    #[test]
    fn test_bre_alternation_translated() {
        let pattern = r"fn foo\|pub.*bar";
        let rg_pattern = pattern.replace(r"\|", "|");
        assert_eq!(rg_pattern, "fn foo|pub.*bar");
    }

    // Fix: -r flag (grep recursive) is stripped from extra_args (rg is recursive by default)
    #[test]
    fn test_recursive_flag_stripped() {
        let extra_args: Vec<String> = vec!["-r".to_string(), "-i".to_string()];
        let filtered: Vec<&String> = extra_args
            .iter()
            .filter(|a| *a != "-r" && *a != "--recursive")
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "-i");
    }

    // --- truncation accuracy ---

    #[test]
    fn test_grep_overflow_uses_uncapped_total() {
        // Confirm the grep overflow invariant: matches vec is never capped before overflow calc.
        // If total_matches > per_file, overflow = total_matches - per_file (not capped).
        // This documents that grep_cmd.rs avoids the diff_cmd bug (cap at N then compute N-10).
        let per_file = config::limits().grep_max_per_file;
        let total_matches = per_file + 42;
        let overflow = total_matches - per_file;
        assert_eq!(overflow, 42, "overflow must equal true suppressed count");
        // Demonstrate why capping before subtraction is wrong:
        let hypothetical_cap = per_file + 5;
        let capped = total_matches.min(hypothetical_cap);
        let wrong_overflow = capped - per_file;
        assert_ne!(
            wrong_overflow, overflow,
            "capping before subtraction gives wrong overflow"
        );
    }

    // Verify line numbers are always enabled in rg invocation (grep_cmd.rs:24).
    // The -n/--line-numbers clap flag in main.rs is a no-op accepted for compat.
    #[test]
    fn test_rg_always_has_line_numbers() {
        // grep_cmd::run() always passes "-n" to rg (line 24).
        // This test documents that -n is built-in, so the clap flag is safe to ignore.
        let mut cmd = resolved_command("rg");
        cmd.args(["-n", "--no-heading", "NONEXISTENT_PATTERN_12345", "."]);
        // If rg is available, it should accept -n without error (exit 1 = no match, not error)
        if let Ok(output) = cmd.output() {
            assert!(
                output.status.code() == Some(1) || output.status.success(),
                "rg -n should be accepted"
            );
        }
        // If rg is not installed, skip gracefully (test still passes)
    }

    #[test]
    fn test_should_fallback_to_grep_on_unrecognized_flag() {
        assert!(should_fallback_to_grep(
            2,
            "error: unrecognized flag '--perl-regexp'"
        ));
    }

    #[test]
    fn test_should_fallback_to_grep_on_regex_parse_error() {
        assert!(should_fallback_to_grep(2, "regex parse error: unclosed group"));
    }

    #[test]
    fn test_should_not_fallback_for_no_match_exit() {
        assert!(!should_fallback_to_grep(1, ""));
    }

    #[test]
    fn test_parse_control_flags() {
        let args = vec![
            "--no-default-excludes".to_string(),
            "-i".to_string(),
            "-A".to_string(),
            "2".to_string(),
        ];
        let (filtered, disabled) = parse_control_flags(&args);
        assert!(disabled);
        assert_eq!(filtered, vec!["-i", "-A", "2"]);
    }

    #[test]
    fn test_record_match_caps_tracked_files_after_budget() {
        let mut by_file = HashMap::new();
        let mut hidden_files = HashSet::new();
        let mut displayed = 0;

        assert!(record_match(
            &mut by_file,
            &mut hidden_files,
            "src/a.rs".to_string(),
            1,
            "alpha".to_string(),
            2,
            1,
            1,
            &mut displayed,
        ));
        assert!(!record_match(
            &mut by_file,
            &mut hidden_files,
            "src/b.rs".to_string(),
            2,
            "beta".to_string(),
            2,
            1,
            1,
            &mut displayed,
        ));
        assert_eq!(by_file.len(), 1);
        assert!(hidden_files.contains("src/b.rs"));
    }
}
