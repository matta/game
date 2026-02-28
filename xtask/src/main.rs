//! `xtask` — workspace automation for the project.
//!
//! Provides CI-style checks: clippy suppression scanning and
//! Rust file token-count enforcement.
use anyhow::Result as AnyhowResult;
use clap::{Parser, Subcommand};
use color_eyre::Result;
use color_eyre::eyre::{bail, eyre};
use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Default maximum number of non-comment, non-whitespace tokens allowed per Rust file.
const DEFAULT_TOKEN_LIMIT: usize = 2547;
/// Exact clippy lints that repository policy forbids suppressing with `allow`/`expect`.
///
/// We intentionally do **not** enforce this via `cargo clippy ... -F clippy::<lint>` in
/// `justfile`, and we also tried crate-level `#![forbid(clippy::...)]`. Both approaches
/// conflicted with clap derive internals that emit `#[allow(clippy::...)]` and produced
/// hard errors (`E0453`) or future-incompat diagnostics (`forbidden_lint_groups`).
///
/// This regex scanner is less principled than an approach integrated directly with
/// clippy/rustc lint plumbing, but it provides a reliable project-level guard today.
const SUPPRESSION_DENYLIST_CLIPPY_LINTS: &[&str] = &[
    "clippy::cognitive_complexity",
    "clippy::type_complexity",
    "clippy::too_many_arguments",
    "clippy::too_many_lines",
    "clippy::large_enum_variant",
    "clippy::struct_excessive_bools",
];
/// Clippy lint groups that are broad enough to suppress denylisted lints transitively.
const SUPPRESSION_DENYLIST_CLIPPY_GROUPS: &[&str] = &["complexity", "perf", "pedantic"];

/// Regex pattern used to match Rust lint attributes of the form `#[allow(...)]` / `#[expect(...)]`.
///
/// Known limitation: this pattern uses dotall `(?s)` plus a lazy `(.*?)` capture for
/// attribute arguments. In unusual nested-parenthesis cases, capture can terminate at an
/// inner `)` instead of the outer attribute boundary, which can cause false negatives
/// (missed matches) but should not create denylist false positives.
const LINT_ATTRIBUTE_PATTERN: &str = r"(?s)#\s*!?\s*\[\s*(allow|expect)\s*\((.*?)\)\s*]";
/// Regex pattern used to extract `clippy::...` lint tokens from lint-attribute argument lists.
const CLIPPY_LINT_TOKEN_PATTERN: &str = r"clippy::[a-z_]+";

/// Top-level CLI entry point for the xtask binary.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all checks
    Check {
        /// Scan all tracked files instead of just edited ones
        #[arg(long)]
        all: bool,
    },
    /// Check for suppressions of clippy lints denied by workspace policy.
    CheckClippySuppressions {
        /// Scan all tracked files instead of just edited ones
        #[arg(long)]
        all: bool,
    },
    /// Check for Rust files that are too large (token count)
    CheckRustTokenCount {
        /// Scan all tracked files instead of just edited ones
        #[arg(long)]
        all: bool,

        /// Set the maximum number of non-comment, non-whitespace tokens allowed
        #[arg(long, default_value_t = DEFAULT_TOKEN_LIMIT)]
        limit: usize,

        /// Just print the token counts for all files and exit
        #[arg(long)]
        print_counts: bool,
    },
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    match cli.command {
        Commands::Check { all } => {
            check_clippy_suppressions(all)?;
            check_rust_token_count(all, DEFAULT_TOKEN_LIMIT, false)?;
            Ok(())
        }
        Commands::CheckClippySuppressions { all } => check_clippy_suppressions(all),
        Commands::CheckRustTokenCount { all, limit, print_counts } => {
            check_rust_token_count(all, limit, print_counts)
        }
    }
}

/// Returns the set of file paths to check, relative to `root`.
///
/// When `all` is `false`, returns only files that are changed (staged, unstaged,
/// or untracked) relative to `HEAD`. Falls back to all tracked files when no
/// changes are detected, or when `all` is `true`.
fn get_files_to_check(root: &Path, all: bool) -> Result<HashSet<String>> {
    let mut files: HashSet<String> = get_git_files(root, &["ls-files"])?.into_iter().collect();
    let untracked = get_git_files(root, &["ls-files", "--others", "--exclude-standard"])?;
    files.extend(untracked.clone());

    if !all {
        // Get both staged and unstaged changes
        let mut changed: HashSet<String> =
            get_git_files(root, &["diff", "--name-only", "HEAD"])?.into_iter().collect();
        // Also get untracked files
        changed.extend(untracked);

        if !changed.is_empty() {
            return Ok(changed);
        }
    }

    Ok(files)
}

/// TOML schema for `.rust-line-count-exceptions.toml`.
///
/// Lists regex patterns for files that are exempt from the token-count limit.
#[derive(Deserialize, Default)]
struct ExceptionsConfig {
    /// Regex patterns matched against relative file paths to exempt from the limit.
    #[serde(default)]
    exceptions: Vec<String>,
}

/// Checks that every Rust file in the workspace stays under `limit` tokens.
///
/// Files matching patterns in `.rust-line-count-exceptions.toml` are skipped.
/// When `print_counts` is set, prints per-file counts and exits without failing.
/// Returns an error listing all violating files when any exceed `limit`.
fn check_rust_token_count(all: bool, limit: usize, print_counts: bool) -> Result<()> {
    let root = env::current_dir()?;
    let config_path = root.join(".rust-line-count-exceptions.toml");

    let exceptions = if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: ExceptionsConfig = toml::from_str(&content).map_err(|e| {
            color_eyre::eyre::eyre!("Failed to parse .rust-line-count-exceptions.toml: {}", e)
        })?;
        config
            .exceptions
            .into_iter()
            .map(|pattern| {
                Regex::new(&pattern)
                    .map_err(|e| color_eyre::eyre::eyre!("Invalid regex {}: {}", pattern, e))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };

    let files = get_files_to_check(&root, all)?;
    let mut violations = Vec::new();
    let mut max_count = 0;
    let mut max_file = String::new();

    for file_path in files {
        let path = root.join(&file_path);
        if !path.exists() || path.is_dir() {
            continue;
        }

        // Only check Rust files
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }

        if exceptions.iter().any(|re| re.is_match(&file_path)) {
            continue;
        }

        let count = count_tokens(&path).map_err(|e| {
            color_eyre::eyre::eyre!("Failed to count tokens in {}: {}", file_path, e)
        })?;
        if count > max_count {
            max_count = count;
            max_file = file_path.clone();
        }

        if print_counts {
            println!("{}: {}", file_path, count);
        }

        if count > limit {
            violations.push((file_path, count));
        }
    }

    if print_counts {
        println!("Max token count: {} (in {})", max_count, max_file);
        return Ok(());
    }

    if !violations.is_empty() {
        violations.sort_by(|a, b| b.1.cmp(&a.1));
        println!("SOURCE TOKEN LIMIT EXCEEDED");
        println!(
            "This gate enforces Rust SOURCE CODE token count (limit: {}), NOT line count.",
            limit
        );
        println!("Comments and whitespace are excluded from this token count.");
        println!("\nThe following Rust files exceed the source token limit:");
        for (file, count) in violations {
            println!("{}: {} tokens", file, count);
        }
        println!("\nSuggestions for corrective action:");
        println!(
            "- Split tests out into separate files (e.g., tests/ directory or separate module)."
        );
        println!("- Improve modularization by extracting large components into new modules.");
        println!("- Refactor long functions into smaller, more manageable pieces.");
        bail!("Source token limit exceeded (not line count; comments and whitespace excluded).");
    }

    println!(
        "All Rust files are within the source token limit ({} tokens; comments/whitespace excluded).",
        limit
    );
    Ok(())
}

/// One source-level suppression hit for a policy-denied clippy lint.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClippySuppressionHit {
    /// Attribute kind (`allow` or `expect`) that performed the suppression.
    kind: String,
    /// Fully qualified clippy lint path (for example, `clippy::too_many_lines`).
    lint: String,
    /// 1-based line where the lint token appears in source.
    line: usize,
}

/// Checks Rust files for `#[allow(...)]` or `#[expect(...)]` suppressions that
/// target clippy lints denied by workspace policy.
fn check_clippy_suppressions(all: bool) -> Result<()> {
    let root = env::current_dir()?;
    let files = get_files_to_check(&root, all)?;
    let mut violations: Vec<(String, ClippySuppressionHit)> = Vec::new();

    let lint_attr_re = Regex::new(LINT_ATTRIBUTE_PATTERN)?;
    let clippy_lint_re = Regex::new(CLIPPY_LINT_TOKEN_PATTERN)?;

    for file_path in files {
        let path = root.join(&file_path);
        if !path.exists() || path.is_dir() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path)?;
        for hit in find_denied_clippy_suppressions(&source, &lint_attr_re, &clippy_lint_re)? {
            violations.push((file_path.clone(), hit));
        }
    }

    if !violations.is_empty() {
        violations.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.line.cmp(&right.1.line))
                .then_with(|| left.1.lint.cmp(&right.1.lint))
        });

        println!("DISALLOWED CLIPPY SUPPRESSIONS FOUND");
        println!(
            "Policy-denied clippy lints must not be suppressed with #[allow(...)] or #[expect(...)]."
        );
        for (file, hit) in violations {
            println!("{}:{} -> {}({})", file, hit.line, hit.kind, hit.lint);
        }
        bail!("Found suppressions of clippy lints denied by workspace policy.");
    }

    println!("No suppressions found for clippy lints denied by workspace policy.");
    Ok(())
}

/// Finds denylisted clippy suppressions in Rust source text.
///
/// This uses regex matching over attribute syntax for pragmatic portability in `xtask`.
/// It is intentionally conservative and may miss exotic macro-generated forms that only a
/// clippy-integrated pass could model perfectly.
fn find_denied_clippy_suppressions(
    source: &str,
    lint_attr_re: &Regex,
    clippy_lint_re: &Regex,
) -> Result<Vec<ClippySuppressionHit>> {
    let mut hits = Vec::new();
    for captures in lint_attr_re.captures_iter(source) {
        let kind = captures
            .get(1)
            .ok_or_else(|| eyre!("capture group 1 missing in lint attribute match"))?
            .as_str();
        let args = captures
            .get(2)
            .ok_or_else(|| eyre!("capture group 2 missing in lint attribute match"))?;

        for lint_match in clippy_lint_re.find_iter(args.as_str()) {
            let lint = lint_match.as_str();
            if !is_denied_clippy_suppression(lint) {
                continue;
            }

            let byte_index = args.start() + lint_match.start();
            let line = source[..byte_index].bytes().filter(|byte| *byte == b'\n').count() + 1;
            hits.push(ClippySuppressionHit {
                kind: kind.to_string(),
                lint: lint.to_string(),
                line,
            });
        }
    }

    Ok(hits)
}

/// Returns `true` when `lint` is denied from source-level suppression by policy.
fn is_denied_clippy_suppression(lint: &str) -> bool {
    if SUPPRESSION_DENYLIST_CLIPPY_LINTS.contains(&lint) {
        return true;
    }

    lint.strip_prefix("clippy::")
        .is_some_and(|group| SUPPRESSION_DENYLIST_CLIPPY_GROUPS.contains(&group))
}

/// Counts non-comment, non-whitespace tokens in a Rust source file.
///
/// Uses `ra_ap_rustc_lexer` to tokenize the file, filtering out
/// `LineComment`, `BlockComment`, and `Whitespace` tokens.
fn count_tokens(path: &Path) -> AnyhowResult<usize> {
    let content = fs::read_to_string(path)?;
    let count = ra_ap_rustc_lexer::tokenize(&content, FrontmatterAllowed::Yes)
        .filter(|token| {
            !matches!(
                token.kind,
                TokenKind::LineComment { .. }
                    | TokenKind::BlockComment { .. }
                    | TokenKind::Whitespace
            )
        })
        .count();
    Ok(count)
}

/// Runs a `git` subcommand with `args` under `root` and returns the lines of stdout.
///
/// Fails if the git invocation returns a non-zero exit code.
fn get_git_files(root: &Path, args: &[&str]) -> Result<Vec<String>> {
    let output = Command::new("git").current_dir(root).args(args).output()?;

    if !output.status.success() {
        bail!("Git command failed: git {}", args.join(" "));
    }

    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "TODO: remove all calls to expect")]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens() {
        let temp = tempfile::tempdir().expect("Failed to create temp dir");
        let path = temp.path().join("test.rs");

        let code = r##"
            fn main() {
                // This is a comment
                let x = 1; /* This is also a comment */
                let s = "This is a string // with a comment inside";
            }
        "##;
        fs::write(&path, code).expect("Failed to write test file");

        let count = count_tokens(&path).expect("Should count tokens");
        // Tokens:
        // 1: fn
        // 2: main
        // 3: (
        // 4: )
        // 5: {
        // 6: let
        // 7: x
        // 8: =
        // 9: 1
        // 10: ;
        // 11: let
        // 12: s
        // 13: =
        // 14: "This is a string // with a comment inside"
        // 15: ;
        // 16: }
        assert_eq!(count, 16);
    }

    #[test]
    fn test_find_denied_clippy_suppressions_detects_direct_and_group_lints() -> Result<()> {
        let lint_attr_re = Regex::new(LINT_ATTRIBUTE_PATTERN)?;
        let clippy_lint_re = Regex::new(CLIPPY_LINT_TOKEN_PATTERN)?;

        let source = format!(
            r#"
            #[allow(clippy::{})]
            fn heavy() {{}}

            #[expect(clippy::{}, clippy::all)]
            fn complex() {{}}
        "#,
            "cognitive_complexity", "too_many_lines"
        );
        let hits = find_denied_clippy_suppressions(&source, &lint_attr_re, &clippy_lint_re)?;
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].lint, "clippy::cognitive_complexity");
        assert_eq!(hits[1].lint, "clippy::too_many_lines");
        Ok(())
    }

    #[test]
    fn test_find_denied_clippy_suppressions_ignores_unrelated_lints() -> Result<()> {
        let lint_attr_re = Regex::new(LINT_ATTRIBUTE_PATTERN)?;
        let clippy_lint_re = Regex::new(CLIPPY_LINT_TOKEN_PATTERN)?;

        let source = r#"
            #[allow(dead_code)]
            #[expect(unused_variables)]
            fn quiet() {}
        "#;
        let hits = find_denied_clippy_suppressions(source, &lint_attr_re, &clippy_lint_re)?;
        assert!(hits.is_empty());
        Ok(())
    }
}
