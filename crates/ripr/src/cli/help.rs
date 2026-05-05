pub(super) fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> &'static str {
    r#"ripr — static RIPR mutation exposure analysis for Rust

Usage:
  ripr check [--base origin/main] [--diff PATH] [--mode draft] [--format human|json|github]
  ripr explain [--base REV|--diff PATH] <finding-id|file:line>
  ripr context [--base REV|--diff PATH] --at <finding-id|file:line>
  ripr lsp [--stdio]
  ripr doctor

What it does:
  Reads changed Rust code, creates mutation-like probes, and estimates whether
  tests appear to reach, infect, propagate, and reveal the changed behavior
  through meaningful oracles. It does not run mutants.

Quick start:
  ripr doctor
  ripr check --diff crates/ripr/examples/sample/example.diff
  ripr check --diff crates/ripr/examples/sample/example.diff --json
  ripr explain --diff crates/ripr/examples/sample/example.diff <finding-id>
"#
}

pub(super) fn print_check_help() {
    println!("{}", check_help_text());
}

fn check_help_text() -> &'static str {
    r#"Usage: ripr check [OPTIONS]

Options:
  --root PATH              Workspace root. Defaults to current directory.
  --base REV               Base revision for git diff. Defaults to origin/main.
  --diff PATH              Read a unified diff file instead of running git diff.
  --mode MODE              instant, draft, fast, deep, or ready. Defaults to draft.
  --format FORMAT          human, json, github, badge-json, badge-shields,
                           badge-plus-json, badge-plus-shields, repo-badge-json,
                           repo-badge-shields, repo-badge-plus-json, or
                           repo-badge-plus-shields. Defaults to human.
                           badge-plus-* and repo-badge-plus-* formats require
                           target/ripr/reports/test-efficiency.json (run
                           `cargo xtask test-efficiency-report` first).
                           repo-badge-* formats render against the full
                           repo baseline and emit `scope: "repo"`; the
                           non-repo badge-* formats remain diff-scoped.
  --json                   Shortcut for --format json.
  --no-unchanged-tests     Limit the index to changed Rust files.

Examples:
  ripr check
  ripr check --base HEAD~1
  ripr check --diff crates/ripr/examples/sample/example.diff --format github
  ripr check --mode ready --json
"#
}

pub(super) fn print_explain_help() {
    println!("{}", explain_help_text());
}

pub(super) fn print_context_help() {
    println!("{}", context_help_text());
}

pub(super) fn print_doctor_help() {
    println!("{}", doctor_help_text());
}

pub(super) fn print_lsp_help() {
    println!("{}", lsp_help_text());
}

fn explain_help_text() -> &'static str {
    "Usage: ripr explain [--root PATH] [--base REV|--diff PATH] <finding-id|file:line>"
}

fn context_help_text() -> &'static str {
    "Usage: ripr context [--root PATH] [--base REV|--diff PATH] --at <finding-id|file:line> [--max-related-tests N] [--json]"
}

fn doctor_help_text() -> &'static str {
    "Usage: ripr doctor [--root PATH]"
}

fn lsp_help_text() -> &'static str {
    r#"Usage: ripr lsp [--stdio] [--version]

Options:
  --stdio       Run the language server over stdio LSP framing. This is the default.
  --version     Print the language server version.
"#
}

#[cfg(test)]
mod tests {
    use super::{
        check_help_text, context_help_text, doctor_help_text, explain_help_text, help_text,
        lsp_help_text,
    };

    #[test]
    fn top_level_help_mentions_core_commands() {
        let help = help_text();
        assert!(help.contains("ripr check"));
        assert!(help.contains("ripr explain"));
        assert!(help.contains("ripr context"));
        assert!(help.contains("ripr doctor"));
    }

    #[test]
    fn check_help_mentions_all_badge_formats() {
        let help = check_help_text();
        assert!(help.contains("badge-plus-json"));
        assert!(help.contains("repo-badge-plus-shields"));
        assert!(help.contains("test-efficiency-report"));
    }

    #[test]
    fn command_specific_usage_lines_are_stable() {
        assert!(explain_help_text().starts_with("Usage: ripr explain"));
        assert!(context_help_text().contains("--max-related-tests N"));
        assert_eq!(doctor_help_text(), "Usage: ripr doctor [--root PATH]");
        assert!(lsp_help_text().contains("--stdio"));
    }
}
