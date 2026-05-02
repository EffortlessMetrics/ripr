pub(super) fn print_help() {
    println!(
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
"#
    );
}

pub(super) fn print_check_help() {
    println!(
        r#"Usage: ripr check [OPTIONS]

Options:
  --root PATH              Workspace root. Defaults to current directory.
  --base REV               Base revision for git diff. Defaults to origin/main.
  --diff PATH              Read a unified diff file instead of running git diff.
  --mode MODE              instant, draft, fast, deep, or ready. Defaults to draft.
  --format FORMAT          human, json, or github. Defaults to human.
  --json                   Shortcut for --format json.
  --no-unchanged-tests     Limit the index to changed Rust files.
"#
    );
}
