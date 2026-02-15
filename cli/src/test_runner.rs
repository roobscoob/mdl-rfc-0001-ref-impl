use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use interpreter::{DiagnosticError, RuntimeValue};

#[derive(Debug, Deserialize)]
pub struct ExpectedWarning {
    /// Substring that must appear in the warning message.
    pub contains: String,

    /// If set, the warning's span must start on this 1-based source line.
    #[serde(default)]
    pub line: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct TestConfig {
    /// Human-readable test description.
    #[serde(default)]
    pub description: Option<String>,

    /// Entry block name (case-insensitive). Defaults to "main".
    #[serde(default = "default_entry")]
    pub entry: String,

    /// Arguments to pass to the entry block.
    #[serde(default)]
    pub args: Vec<toml::Value>,

    /// Expected exact stdout output (trimmed comparison).
    #[serde(default)]
    pub expect_output: Option<String>,

    /// Expected runtime error — the error's Display string must contain this substring.
    #[serde(default)]
    pub expect_error: Option<String>,

    /// If true, the test expects parsing to fail.
    #[serde(default)]
    pub expect_parse_error: bool,

    /// Expected warnings. If present (even empty), warning count and content are checked.
    /// Each entry checks message substring and optionally the source line.
    #[serde(default)]
    pub expect_warnings: Option<Vec<ExpectedWarning>>,
}

fn default_entry() -> String {
    "main".to_string()
}

fn toml_arg_to_runtime(val: &toml::Value) -> RuntimeValue {
    match val {
        toml::Value::Integer(n) => RuntimeValue::Number(*n as f64),
        toml::Value::Float(f) => RuntimeValue::Number(*f),
        toml::Value::Boolean(b) => RuntimeValue::Boolean(*b),
        toml::Value::String(s) => RuntimeValue::String(s.clone()),
        other => RuntimeValue::String(other.to_string()),
    }
}

/// Parse a `.test.md` file into its TOML config and mdl source.
fn parse_test_file(content: &str) -> Result<(TestConfig, &str), String> {
    let content = content.trim_start_matches('\u{feff}'); // strip BOM

    if !content.starts_with("---") {
        return Err("missing opening --- frontmatter delimiter".into());
    }

    let after_open = &content[3..];
    let after_open = after_open
        .strip_prefix('\n')
        .or_else(|| after_open.strip_prefix("\r\n"))
        .unwrap_or(after_open);

    let close_pos = after_open
        .find("\n---")
        .ok_or("missing closing --- frontmatter delimiter")?;

    let toml_str = after_open[..close_pos].trim_end_matches('\r');
    let rest_start = close_pos + 4; // skip \n---
    let source = after_open[rest_start..]
        .strip_prefix("\r\n")
        .or_else(|| after_open[rest_start..].strip_prefix('\n'))
        .unwrap_or(&after_open[rest_start..]);

    let config: TestConfig =
        toml::from_str(toml_str).map_err(|e| format!("TOML parse error: {}", e))?;

    Ok((config, source))
}

pub enum TestOutcome {
    Pass,
    Fail(String),
}

pub struct TestResult {
    pub path: PathBuf,
    pub description: Option<String>,
    pub outcome: TestOutcome,
}

fn run_single_test(path: &Path) -> TestResult {
    // 1. Read file
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return TestResult {
                path: path.to_path_buf(),
                description: None,
                outcome: TestOutcome::Fail(format!("cannot read file: {}", e)),
            };
        }
    };

    // 2. Parse frontmatter
    let (config, source) = match parse_test_file(&content) {
        Ok(pair) => pair,
        Err(e) => {
            return TestResult {
                path: path.to_path_buf(),
                description: None,
                outcome: TestOutcome::Fail(format!("frontmatter error: {}", e)),
            };
        }
    };

    let description = config.description.clone();

    // 3. Parse mdl source
    let parser = mdl::parser::Parser::new(source.to_string(), 0);
    let parse_result = parser.parse();

    // 4. Handle expect_parse_error
    if config.expect_parse_error {
        return TestResult {
            path: path.to_path_buf(),
            description,
            outcome: match parse_result {
                Err(_) => TestOutcome::Pass,
                Ok(_) => TestOutcome::Fail("expected parse error, but parsing succeeded".into()),
            },
        };
    }

    let program = match parse_result {
        Ok(p) => p,
        Err(errs) => {
            let msgs: Vec<String> = errs.iter().map(|e| e.message.clone()).collect();
            return TestResult {
                path: path.to_path_buf(),
                description,
                outcome: TestOutcome::Fail(format!(
                    "unexpected parse error: {}",
                    msgs.join("; ")
                )),
            };
        }
    };

    // 5. Execute
    let arguments: Vec<RuntimeValue> = config.args.iter().map(toml_arg_to_runtime).collect();

    let base_dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut output_buf = Vec::new();
    let exec_result = interpreter::execute_program_entry(
        &program,
        &mut output_buf,
        base_dir,
        &config.entry,
        arguments,
    );

    // 6. Split result into value/error and diagnostics
    let (exec_result, diagnostics) = match exec_result {
        Ok((val, diags)) => (Ok(val), diags),
        Err(err) => (Err(err), Vec::new()),
    };

    // 7. Check error/output expectations
    let outcome = match (&config.expect_error, &config.expect_output, exec_result) {
        (Some(expected_err), _, Err(runtime_err)) => {
            let err_str = runtime_err.to_string();
            if err_str.contains(expected_err.as_str()) {
                None
            } else {
                Some(format!(
                    "expected error containing \"{}\", got: {}",
                    expected_err, err_str
                ))
            }
        }
        (Some(expected_err), _, Ok(_)) => Some(format!(
            "expected error containing \"{}\", but execution succeeded",
            expected_err
        )),
        (None, Some(_), Err(runtime_err)) => {
            Some(format!("unexpected runtime error: {}", runtime_err))
        }
        (None, Some(expected_output), Ok(_)) => {
            let actual = String::from_utf8_lossy(&output_buf);
            let actual_trimmed = actual.trim();
            let expected_trimmed = expected_output.trim();
            if actual_trimmed == expected_trimmed {
                None
            } else {
                Some(format!(
                    "output mismatch\n  expected: {}\n  actual:   {}",
                    expected_trimmed, actual_trimmed
                ))
            }
        }
        (None, None, Err(runtime_err)) => {
            Some(format!("unexpected runtime error: {}", runtime_err))
        }
        (None, None, Ok(_)) => None,
    };

    // Short-circuit if error/output check already failed
    if let Some(reason) = outcome {
        return TestResult {
            path: path.to_path_buf(),
            description,
            outcome: TestOutcome::Fail(reason),
        };
    }

    // 8. Check warning expectations
    if let Some(expected_warnings) = &config.expect_warnings {
        if let Some(reason) = check_warnings(source, &diagnostics, expected_warnings) {
            return TestResult {
                path: path.to_path_buf(),
                description,
                outcome: TestOutcome::Fail(reason),
            };
        }
    }

    TestResult {
        path: path.to_path_buf(),
        description,
        outcome: TestOutcome::Pass,
    }
}

/// Convert a byte offset in `source` to a 1-based line number.
fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

/// Check that actual warnings match expectations. Returns `Some(reason)` on mismatch.
fn check_warnings(
    source: &str,
    diagnostics: &[DiagnosticError],
    expected: &[ExpectedWarning],
) -> Option<String> {
    let actual_warnings: Vec<&DiagnosticError> =
        diagnostics.iter().filter(|d| d.is_warning).collect();

    if actual_warnings.len() != expected.len() {
        let actual_msgs: Vec<String> = actual_warnings
            .iter()
            .map(|w| format!("  - {}", w))
            .collect();
        return Some(format!(
            "expected {} warning(s), got {}\n  actual warnings:\n{}",
            expected.len(),
            actual_warnings.len(),
            if actual_msgs.is_empty() {
                "    (none)".to_string()
            } else {
                actual_msgs.join("\n")
            }
        ));
    }

    for (i, (actual, expected)) in actual_warnings.iter().zip(expected.iter()).enumerate() {
        let msg = actual.to_string();

        if !msg.contains(&expected.contains) {
            return Some(format!(
                "warning[{}]: expected message containing \"{}\", got: {}",
                i, expected.contains, msg
            ));
        }

        if let Some(expected_line) = expected.line {
            if let Some(span) = &actual.span {
                let actual_line = byte_offset_to_line(source, span.start);
                if actual_line != expected_line {
                    return Some(format!(
                        "warning[{}]: expected on line {}, but span is on line {}",
                        i, expected_line, actual_line
                    ));
                }
            } else {
                return Some(format!(
                    "warning[{}]: expected on line {}, but warning has no span",
                    i, expected_line
                ));
            }
        }
    }

    None
}

/// Discover `.test.md` files grouped by category (subfolder relative to root).
/// Files directly in `root` get category "" (uncategorized).
/// Returns a BTreeMap so categories are sorted alphabetically.
fn discover_categorized(root: &Path) -> BTreeMap<String, Vec<PathBuf>> {
    let mut categories: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    collect_tests(root, root, &mut categories);
    // Sort files within each category
    for files in categories.values_mut() {
        files.sort();
    }
    categories
}

fn collect_tests(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<PathBuf>>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_tests(&path, root, out);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".test.md") {
                let category = path
                    .parent()
                    .and_then(|p| p.strip_prefix(root).ok())
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                out.entry(category).or_default().push(path);
            }
        }
    }
}

/// List available categories for the given test path.
pub fn list_categories(path: &Path) {
    if path.is_file() {
        eprintln!("(single file, no categories)");
        return;
    }

    let categories = discover_categorized(path);
    if categories.is_empty() {
        eprintln!("no .test.md files found in {}", path.display());
        return;
    }

    eprintln!("available categories:");
    for (cat, files) in &categories {
        let label = if cat.is_empty() { "(root)" } else { cat.as_str() };
        eprintln!("  {} ({} tests)", label, files.len());
    }
}

fn pass_label(no_color: bool) -> &'static str {
    if no_color { "PASS" } else { "\x1b[32mPASS\x1b[0m" }
}

fn fail_label(no_color: bool) -> &'static str {
    if no_color { "FAIL" } else { "\x1b[31mFAIL\x1b[0m" }
}

fn bold(s: &str, no_color: bool) -> String {
    if no_color {
        s.to_string()
    } else {
        format!("\x1b[1m{}\x1b[0m", s)
    }
}

/// Run all `.test.md` files under `path` (or a single file).
/// If `categories` is non-empty, only run tests in those categories.
/// Returns exit code: 0 = all pass, 1 = any failure.
pub fn run_tests(path: &Path, no_color: bool, categories: &[String]) -> i32 {
    // Single file mode — ignore categories
    if path.is_file() {
        let result = run_single_test(path);
        let label = result
            .description
            .as_deref()
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?")
            });
        return match &result.outcome {
            TestOutcome::Pass => {
                eprintln!("  {}  {}", pass_label(no_color), label);
                eprintln!();
                eprintln!("test result: {}. 1 passed, 0 failed", if no_color { "ok" } else { "\x1b[32mok\x1b[0m" });
                0
            }
            TestOutcome::Fail(reason) => {
                eprintln!("  {}  {}", fail_label(no_color), label);
                eprintln!();
                eprintln!("failures:");
                eprintln!();
                eprintln!("  --- {} ---", path.display());
                for line in reason.lines() {
                    eprintln!("  {}", line);
                }
                eprintln!();
                eprintln!("test result: {}. 0 passed, 1 failed (of 1)",
                    if no_color { "FAILED" } else { "\x1b[31mFAILED\x1b[0m" });
                1
            }
        };
    }

    let all_categories = discover_categorized(path);

    if all_categories.is_empty() {
        eprintln!("no .test.md files found in {}", path.display());
        return 1;
    }

    // Filter categories if specified
    let run_categories: BTreeMap<&str, &Vec<PathBuf>> = if categories.is_empty() {
        all_categories.iter().map(|(k, v)| (k.as_str(), v)).collect()
    } else {
        let mut filtered = BTreeMap::new();
        for requested in categories {
            let req = requested.trim_matches('/');
            let mut found = false;
            for (cat, files) in &all_categories {
                if cat == req || cat.starts_with(&format!("{}/", req)) {
                    filtered.insert(cat.as_str(), files);
                    found = true;
                }
            }
            if !found {
                eprintln!(
                    "warning: category '{}' not found (available: {})",
                    req,
                    all_categories
                        .keys()
                        .map(|k| if k.is_empty() { "(root)" } else { k.as_str() })
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        filtered
    };

    if run_categories.is_empty() {
        eprintln!("no matching categories found");
        return 1;
    }

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<TestResult> = Vec::new();

    for (cat, files) in &run_categories {
        // Print category header
        let header = if cat.is_empty() {
            "(root)".to_string()
        } else {
            cat.to_string()
        };
        eprintln!();
        eprintln!("{}", bold(&header, no_color));

        for file in *files {
            let result = run_single_test(file);
            let label = result
                .description
                .as_deref()
                .unwrap_or_else(|| {
                    file.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?")
                });

            match &result.outcome {
                TestOutcome::Pass => {
                    passed += 1;
                    eprintln!("  {}  {}", pass_label(no_color), label);
                }
                TestOutcome::Fail(_) => {
                    failed += 1;
                    eprintln!("  {}  {}", fail_label(no_color), label);
                    failures.push(result);
                }
            }
        }
    }

    // Print failure details
    if !failures.is_empty() {
        eprintln!();
        eprintln!("failures:");
        for f in &failures {
            eprintln!();
            eprintln!("  --- {} ---", f.path.display());
            if let TestOutcome::Fail(reason) = &f.outcome {
                for line in reason.lines() {
                    eprintln!("  {}", line);
                }
            }
        }
    }

    // Summary
    eprintln!();
    if failed == 0 {
        if no_color {
            eprintln!("test result: ok. {} passed, 0 failed", passed);
        } else {
            eprintln!("test result: \x1b[32mok\x1b[0m. {} passed, 0 failed", passed);
        }
        0
    } else {
        let total = passed + failed;
        if no_color {
            eprintln!(
                "test result: FAILED. {} passed, {} failed (of {})",
                passed, failed, total
            );
        } else {
            eprintln!(
                "test result: \x1b[31mFAILED\x1b[0m. {} passed, {} failed (of {})",
                passed, failed, total
            );
        }
        1
    }
}
