//! Universal build error extraction and file-grouped aggregation for JS/TS and build commands.

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildErrorItem {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub code: String,
    pub message: String,
}

static TS_ERROR_1: LazyLock<Regex> = LazyLock::new(|| {
    // path/to/file.ts(12,5): error TS2322: Message
    Regex::new(r"^(.+?)\((\d+),(\d+)\):\s+(?:error|warning)\s+(TS\d+)?:\s*(.+)$").unwrap()
});

static TS_ERROR_2: LazyLock<Regex> = LazyLock::new(|| {
    // path/to/file.ts:12:5 - error TS2322: Message
    Regex::new(r"^(.+?):(\d+):(\d+)\s+-\s+(?:error|warning)\s+(TS\d+)?:\s*(.+)$").unwrap()
});

static GENERIC_ERROR_1: LazyLock<Regex> = LazyLock::new(|| {
    // path/to/file.ts:12:5: error: Message
    Regex::new(r"^(.+?):(\d+):(\d+):\s*(?:error|Error|warning|Warning):\s*(.+)$").unwrap()
});

static GENERIC_ERROR_2: LazyLock<Regex> = LazyLock::new(|| {
    // path/to/file.ts:12:5: Error message (e.g. ESLint or Biome)
    Regex::new(r"^([a-zA-Z0-9_\-./\\]+\.[a-zA-Z0-9]+):(\d+):(\d+):\s+(.+)$").unwrap()
});

static NEXT_FILE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    // ./src/app/page.tsx:24:10 or ./src/app/page.tsx
    Regex::new(r"^\.?/([a-zA-Z0-9_\-./\\]+\.[a-zA-Z0-9]+)(?::(\d+)(?::(\d+))?)?$").unwrap()
});

/// Extracts structured build errors from raw command output.
pub fn extract_build_errors(output: &str) -> Vec<BuildErrorItem> {
    let mut errors = Vec::new();
    let lines: Vec<&str> = output.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.is_empty() {
            i += 1;
            continue;
        }

        if let Some(caps) = TS_ERROR_1.captures(line) {
            errors.push(BuildErrorItem {
                file: caps[1].trim().to_string(),
                line: caps[2].parse().unwrap_or(0),
                col: caps[3].parse().unwrap_or(0),
                code: caps.get(4).map(|m| m.as_str().to_string()).unwrap_or_default(),
                message: caps[5].trim().to_string(),
            });
            i += 1;
            continue;
        }

        if let Some(caps) = TS_ERROR_2.captures(line) {
            errors.push(BuildErrorItem {
                file: caps[1].trim().to_string(),
                line: caps[2].parse().unwrap_or(0),
                col: caps[3].parse().unwrap_or(0),
                code: caps.get(4).map(|m| m.as_str().to_string()).unwrap_or_default(),
                message: caps[5].trim().to_string(),
            });
            i += 1;
            continue;
        }

        if let Some(caps) = GENERIC_ERROR_1.captures(line) {
            let file = caps[1].trim();
            if !file.contains("node_modules/") {
                errors.push(BuildErrorItem {
                    file: file.to_string(),
                    line: caps[2].parse().unwrap_or(0),
                    col: caps[3].parse().unwrap_or(0),
                    code: String::new(),
                    message: caps[4].trim().to_string(),
                });
                i += 1;
                continue;
            }
        }

        if let Some(caps) = GENERIC_ERROR_2.captures(line) {
            let file = caps[1].trim();
            if !file.contains("node_modules/") && !file.starts_with("http:") && !file.starts_with("https:") {
                errors.push(BuildErrorItem {
                    file: file.to_string(),
                    line: caps[2].parse().unwrap_or(0),
                    col: caps[3].parse().unwrap_or(0),
                    code: String::new(),
                    message: caps[4].trim().to_string(),
                });
                i += 1;
                continue;
            }
        }

        // Multi-line Next.js / Webpack format:
        // ./src/app/page.tsx:24:10
        // Type error: Cannot find module...
        if let Some(caps) = NEXT_FILE_LINE.captures(line) {
            let file = caps[1].to_string();
            let line_num: usize = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let col_num: usize = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);

            let mut msg_found = false;
            let mut j = i + 1;
            while j < lines.len() && j < i + 4 {
                let next_line = lines[j].trim();
                if next_line.starts_with("Type error:")
                    || next_line.starts_with("Error:")
                    || next_line.starts_with("SyntaxError:")
                {
                    let clean_msg = next_line
                        .trim_start_matches("Type error:")
                        .trim_start_matches("Error:")
                        .trim();
                    errors.push(BuildErrorItem {
                        file: file.clone(),
                        line: line_num,
                        col: col_num,
                        code: String::new(),
                        message: clean_msg.to_string(),
                    });
                    msg_found = true;
                    i = j + 1;
                    break;
                }
                j += 1;
            }
            if msg_found {
                continue;
            }
        }

        i += 1;
    }

    errors
}

/// Formats extracted build errors into a compact, file-grouped summary.
pub fn format_build_errors(errors: &[BuildErrorItem]) -> String {
    if errors.is_empty() {
        return String::new();
    }

    let mut file_order = Vec::new();
    let mut by_file: HashMap<String, Vec<&BuildErrorItem>> = HashMap::new();

    for err in errors {
        if !by_file.contains_key(&err.file) {
            file_order.push(err.file.clone());
        }
        by_file.entry(err.file.clone()).or_default().push(err);
    }

    let mut out = String::new();
    out.push_str(&format!(
        "Build Failed: {} errors across {} files\n\n",
        errors.len(),
        file_order.len()
    ));

    for file in &file_order {
        if let Some(file_errs) = by_file.get(file) {
            out.push_str(&format!("{} ({} errors):\n", file, file_errs.len()));
            for err in file_errs {
                let code_prefix = if !err.code.is_empty() {
                    format!("{} ", err.code)
                } else {
                    String::new()
                };
                let line_prefix = if err.line > 0 {
                    format!("L{}: ", err.line)
                } else {
                    String::new()
                };
                let msg_short = crate::core::utils::truncate(&err.message, 120);
                out.push_str(&format!("  • {}{}{}\n", line_prefix, code_prefix, msg_short));
            }
            out.push('\n');
        }
    }

    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ts_errors() {
        let output = r#"
> project@1.0.0 build
> tsc

src/server/api/auth.ts(12,5): error TS2322: Type 'string' is not assignable to type 'number'.
src/server/api/auth.ts(18,10): error TS2339: Property 'id' does not exist on type 'User'.
src/utils/helpers.ts(8,1): error TS2304: Cannot find name 'formatDate'.
"#;
        let errors = extract_build_errors(output);
        assert_eq!(errors.len(), 3);
        assert_eq!(errors[0].file, "src/server/api/auth.ts");
        assert_eq!(errors[0].line, 12);
        assert_eq!(errors[0].code, "TS2322");
        assert_eq!(errors[2].file, "src/utils/helpers.ts");

        let formatted = format_build_errors(&errors);
        assert!(formatted.contains("Build Failed: 3 errors across 2 files"));
        assert!(formatted.contains("src/server/api/auth.ts (2 errors):"));
        assert!(formatted.contains("L12: TS2322 Type 'string' is not assignable"));
        assert!(formatted.contains("src/utils/helpers.ts (1 errors):"));
    }

    #[test]
    fn test_extract_next_js_errors() {
        let output = r#"
Failed to compile.

./src/app/page.tsx:24:10
Type error: Cannot find module '@/components/Button' or its corresponding type declarations.
"#;
        let errors = extract_build_errors(output);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, "src/app/page.tsx");
        assert_eq!(errors[0].line, 24);

        let formatted = format_build_errors(&errors);
        assert!(formatted.contains("Build Failed: 1 errors across 1 files"));
        assert!(formatted.contains("src/app/page.tsx (1 errors):"));
        assert!(formatted.contains("L24: Cannot find module"));
    }
}
