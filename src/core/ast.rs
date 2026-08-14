//! AST Skeletonization engine for extracting high-level code structure and signatures.
//!
//! Replaces function and method implementations with lightweight placeholders `{ /* ... */ }`
//! while preserving imports, type definitions, structs, interfaces, enums, and signatures.

#![allow(dead_code)]

/// Skeletonizes source code according to the file extension.
/// Supports `.rs`, `.ts`, `.tsx`, `.js`, `.jsx`, `.py`, and `.go`.
pub fn skeletonize(code: &str, file_ext: &str) -> String {
    let ext = file_ext.trim_start_matches('.').to_lowercase();
    match ext.as_str() {
        "rs" => skeletonize_rust(code),
        "ts" | "tsx" | "js" | "jsx" => skeletonize_js_ts(code),
        "py" => skeletonize_python(code),
        "go" => skeletonize_go(code),
        _ => code.to_string(),
    }
}

/// Skeletonizes Rust code by replacing function bodies with `{ /* ... */ }`.
fn skeletonize_rust(code: &str) -> String {
    let mut out = Vec::new();
    let mut in_fn_body = false;
    let mut brace_depth = 0;

    for line in code.lines() {
        let trimmed = line.trim();

        if in_fn_body {
            let open_count = trimmed.chars().filter(|&c| c == '{').count();
            let close_count = trimmed.chars().filter(|&c| c == '}').count();

            if open_count > close_count {
                brace_depth += open_count - close_count;
            } else if close_count > open_count {
                let diff = close_count - open_count;
                if diff >= brace_depth {
                    brace_depth = 0;
                    in_fn_body = false;
                    out.push("    /* ... */".to_string());
                    out.push("}".to_string());
                    continue;
                } else {
                    brace_depth -= diff;
                }
            } else if open_count == close_count && open_count > 0 {
                // Stay in body
            }

            if brace_depth == 0 {
                in_fn_body = false;
                out.push("    /* ... */".to_string());
                out.push("}".to_string());
            }
            continue;
        }

        // Detect function start
        if (trimmed.starts_with("pub fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("pub async fn ")
            || trimmed.starts_with("async fn ")
            || trimmed.starts_with("pub(crate) fn "))
            && !trimmed.ends_with(';')
        {
            if trimmed.contains('{') {
                let open_count = trimmed.chars().filter(|&c| c == '{').count();
                let close_count = trimmed.chars().filter(|&c| c == '}').count();
                if open_count > close_count {
                    brace_depth = open_count - close_count;
                    in_fn_body = true;
                    if let Some(idx) = line.find('{') {
                        out.push(format!("{} {{", &line[..idx].trim_end()));
                    } else {
                        out.push(line.to_string());
                    }
                    continue;
                }
            }
        }

        out.push(line.to_string());
    }

    if in_fn_body {
        out.push("    /* ... */".to_string());
        out.push("}".to_string());
    }

    out.join("\n")
}

/// Skeletonizes TypeScript/JavaScript code by replacing function/method bodies with `{ /* ... */ }`.
fn skeletonize_js_ts(code: &str) -> String {
    let mut out = Vec::new();
    let mut in_fn_body = false;
    let mut brace_depth = 0;

    for line in code.lines() {
        let trimmed = line.trim();

        if in_fn_body {
            let open_count = trimmed.chars().filter(|&c| c == '{').count();
            let close_count = trimmed.chars().filter(|&c| c == '}').count();

            if open_count > close_count {
                brace_depth += open_count - close_count;
            } else if close_count > open_count {
                let diff = close_count - open_count;
                if diff >= brace_depth {
                    brace_depth = 0;
                    in_fn_body = false;
                    out.push("  /* ... */".to_string());
                    out.push("}".to_string());
                    continue;
                } else {
                    brace_depth -= diff;
                }
            }

            if brace_depth == 0 {
                in_fn_body = false;
                out.push("  /* ... */".to_string());
                out.push("}".to_string());
            }
            continue;
        }

        if (trimmed.starts_with("function ")
            || trimmed.starts_with("export function ")
            || trimmed.starts_with("export default function ")
            || trimmed.starts_with("async function ")
            || trimmed.starts_with("export async function "))
            && trimmed.contains('{')
        {
            let open_count = trimmed.chars().filter(|&c| c == '{').count();
            let close_count = trimmed.chars().filter(|&c| c == '}').count();
            if open_count > close_count {
                brace_depth = open_count - close_count;
                in_fn_body = true;
                if let Some(idx) = line.find('{') {
                    out.push(format!("{} {{", &line[..idx].trim_end()));
                } else {
                    out.push(line.to_string());
                }
                continue;
            }
        }

        out.push(line.to_string());
    }

    out.join("\n")
}

/// Skeletonizes Python code by replacing function bodies with `    # ...`.
fn skeletonize_python(code: &str) -> String {
    let mut out = Vec::new();
    let mut in_def = false;
    let mut def_indent = 0;

    for line in code.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            out.push(line.to_string());
            continue;
        }

        let current_indent = line.len() - line.trim_start().len();

        if in_def {
            if current_indent <= def_indent && !trimmed.starts_with('#') {
                in_def = false;
            } else {
                continue;
            }
        }

        if (trimmed.starts_with("def ") || trimmed.starts_with("async def "))
            && trimmed.ends_with(':')
        {
            in_def = true;
            def_indent = current_indent;
            out.push(line.to_string());
            let indent_str = " ".repeat(def_indent + 4);
            out.push(format!("{}# ...", indent_str));
            continue;
        }

        out.push(line.to_string());
    }

    out.join("\n")
}

/// Skeletonizes Go code by replacing function bodies with `{ /* ... */ }`.
fn skeletonize_go(code: &str) -> String {
    let mut out = Vec::new();
    let mut in_fn_body = false;
    let mut brace_depth = 0;

    for line in code.lines() {
        let trimmed = line.trim();

        if in_fn_body {
            let open_count = trimmed.chars().filter(|&c| c == '{').count();
            let close_count = trimmed.chars().filter(|&c| c == '}').count();

            if open_count > close_count {
                brace_depth += open_count - close_count;
            } else if close_count > open_count {
                let diff = close_count - open_count;
                if diff >= brace_depth {
                    brace_depth = 0;
                    in_fn_body = false;
                    out.push("\t/* ... */".to_string());
                    out.push("}".to_string());
                    continue;
                } else {
                    brace_depth -= diff;
                }
            }

            if brace_depth == 0 {
                in_fn_body = false;
                out.push("\t/* ... */".to_string());
                out.push("}".to_string());
            }
            continue;
        }

        if trimmed.starts_with("func ") && trimmed.contains('{') {
            let open_count = trimmed.chars().filter(|&c| c == '{').count();
            let close_count = trimmed.chars().filter(|&c| c == '}').count();
            if open_count > close_count {
                brace_depth = open_count - close_count;
                in_fn_body = true;
                if let Some(idx) = line.find('{') {
                    out.push(format!("{} {{", &line[..idx].trim_end()));
                } else {
                    out.push(line.to_string());
                }
                continue;
            }
        }

        out.push(line.to_string());
    }

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skeletonize_rust() {
        let input = r#"pub struct User {
    pub name: String,
}

pub fn greet(name: &str) -> String {
    let msg = format!("Hello, {}!", name);
    println!("{}", msg);
    msg
}"#;
        let skeleton = skeletonize(input, "rs");
        assert!(skeleton.contains("pub struct User"));
        assert!(skeleton.contains("pub fn greet(name: &str) -> String {"));
        assert!(skeleton.contains("/* ... */"));
        assert!(!skeleton.contains("println!"));
    }

    #[test]
    fn test_skeletonize_ts() {
        let input = r#"export interface Config {
  port: number;
}

export function startServer(config: Config): void {
  const app = express();
  app.listen(config.port);
}"#;
        let skeleton = skeletonize(input, "ts");
        assert!(skeleton.contains("export interface Config"));
        assert!(skeleton.contains("export function startServer(config: Config): void {"));
        assert!(skeleton.contains("/* ... */"));
        assert!(!skeleton.contains("express()"));
    }

    #[test]
    fn test_skeletonize_python() {
        let input = r#"class App:
    def run(self):
        print("Starting")
        self.connect()
        return True"#;
        let skeleton = skeletonize(input, "py");
        assert!(skeleton.contains("class App:"));
        assert!(skeleton.contains("def run(self):"));
        assert!(skeleton.contains("# ..."));
        assert!(!skeleton.contains("print(\"Starting\")"));
    }

    #[test]
    fn test_skeletonize_go() {
        let input = r#"package main

func CalculateSum(a int, b int) int {
	res := a + b
	return res
}"#;
        let skeleton = skeletonize(input, "go");
        assert!(skeleton.contains("package main"));
        assert!(skeleton.contains("func CalculateSum(a int, b int) int {"));
        assert!(skeleton.contains("/* ... */"));
        assert!(!skeleton.contains("res := a + b"));
    }
}
