use crate::agent::loop_commands::{anchored_redirect_target, bound_root, shell_arg};
use crate::cli::unknown_argument;
use std::path::PathBuf;

use super::{
    DEFAULT_AGENT_PACKET, DEFAULT_BASE, DEFAULT_FIRST_ACTION, DEFAULT_GAP_LEDGER,
    DEFAULT_GATE_DECISION, DEFAULT_HEAD, DEFAULT_OUT_DIR, DEFAULT_RECEIPTS_DIR,
    DEFAULT_REVIEW_COMMENTS, DEFAULT_ROOT,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FirstPrOptions {
    pub(super) root: String,
    pub(super) base: String,
    /// `false` when `--base` was omitted: `base` then holds a placeholder
    /// until the CLI entry resolves the repository's default branch.
    pub(super) base_explicit: bool,
    pub(super) head: String,
    pub(super) check_output: Option<String>,
    pub(super) gap_ledger: String,
    pub(super) gap_ledger_explicit: bool,
    pub(super) first_action: String,
    pub(super) review_comments: String,
    pub(super) agent_packet: String,
    pub(super) gate_decision: String,
    pub(super) receipts_dir: String,
    pub(super) out_dir: String,
    pub(super) check: bool,
    pub(super) preflight: bool,
    /// Test-only ceiling for `GIT_CEILING_DIRECTORIES`; `None` in production.
    pub(crate) git_ceiling: Option<PathBuf>,
}

impl FirstPrOptions {
    /// The selected root bound once for product-generated commands (#3999):
    /// a relative `--root` resolves against this process's working directory,
    /// the same directory `repo_root` resolved it against, so a pasted command
    /// analyzes and writes the selected repository from any directory.
    pub(super) fn command_root(&self) -> String {
        bound_root(&self.root)
    }

    /// A first-pr artifact path rendered as a generated command argument,
    /// quoted for the shell. first-pr resolves its artifact paths against the
    /// selected root, while `first-action`, `review-comments`, `agent packet`,
    /// `gate evaluate`, `reports gap-ledger` and shell redirects resolve them
    /// against the invocation working directory. Anchoring at the bound root
    /// keeps `--root` and every path naming the same repository when a command
    /// is pasted elsewhere (#3948, #4287); an absolute path passes through.
    pub(super) fn anchored_arg(&self, path: &str) -> String {
        shell_arg(&anchored_redirect_target(&self.root, path))
    }
}

impl Default for FirstPrOptions {
    fn default() -> Self {
        Self {
            root: DEFAULT_ROOT.to_string(),
            base: DEFAULT_BASE.to_string(),
            base_explicit: false,
            head: DEFAULT_HEAD.to_string(),
            check_output: None,
            gap_ledger: DEFAULT_GAP_LEDGER.to_string(),
            gap_ledger_explicit: false,
            first_action: DEFAULT_FIRST_ACTION.to_string(),
            review_comments: DEFAULT_REVIEW_COMMENTS.to_string(),
            agent_packet: DEFAULT_AGENT_PACKET.to_string(),
            gate_decision: DEFAULT_GATE_DECISION.to_string(),
            receipts_dir: DEFAULT_RECEIPTS_DIR.to_string(),
            out_dir: DEFAULT_OUT_DIR.to_string(),
            check: false,
            preflight: false,
            git_ceiling: None,
        }
    }
}

pub(super) fn parse_options(args: &[String]) -> Result<FirstPrOptions, String> {
    let mut options = FirstPrOptions {
        preflight: true,
        ..FirstPrOptions::default()
    };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                options.root = non_empty_arg(args, i, "--root")?.to_string();
            }
            "--base" => {
                i += 1;
                options.base = non_empty_arg(args, i, "--base")?.to_string();
                options.base_explicit = true;
            }
            "--head" => {
                i += 1;
                options.head = non_empty_arg(args, i, "--head")?.to_string();
            }
            "--check-output" => {
                i += 1;
                options.check_output = Some(non_empty_arg(args, i, "--check-output")?.to_string());
            }
            "--gap-ledger" => {
                i += 1;
                options.gap_ledger = non_empty_arg(args, i, "--gap-ledger")?.to_string();
                options.gap_ledger_explicit = true;
            }
            "--first-action" => {
                i += 1;
                options.first_action = non_empty_arg(args, i, "--first-action")?.to_string();
            }
            "--review-comments" => {
                i += 1;
                options.review_comments = non_empty_arg(args, i, "--review-comments")?.to_string();
            }
            "--agent-packet" => {
                i += 1;
                options.agent_packet = non_empty_arg(args, i, "--agent-packet")?.to_string();
            }
            "--gate-decision" => {
                i += 1;
                options.gate_decision = non_empty_arg(args, i, "--gate-decision")?.to_string();
            }
            "--receipts-dir" => {
                i += 1;
                options.receipts_dir = non_empty_arg(args, i, "--receipts-dir")?.to_string();
            }
            "--out-dir" => {
                i += 1;
                options.out_dir = non_empty_arg(args, i, "--out-dir")?.to_string();
            }
            "--check" => options.check = true,
            other => return Err(unknown_argument("first-pr", other)),
        }
        i += 1;
    }
    Ok(options)
}

fn non_empty_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("missing value for {flag}"));
    };
    if value.trim().is_empty() {
        return Err(format!("first-pr {flag} requires a non-empty value"));
    }
    Ok(value)
}

pub(super) fn print_help() {
    println!("{FIRST_PR_HELP}");
}

/// Help body for `ripr first-pr` / `ripr start-here`. Also the flag source for
/// unknown-argument suggestions, so accepted flags have to appear as
/// option-list lines, not only inside the usage brackets.
pub(crate) const FIRST_PR_HELP: &str = "\
Create the start-here packet for one PR from existing RIPR artifacts.

usage: ripr first-pr|start-here [--root <path>] [--base <rev>] [--head <rev>] [--check-output <path>] [--gap-ledger <path>] [--first-action <path>] [--review-comments <path>] [--agent-packet <path>] [--gate-decision <path>] [--receipts-dir <path>] [--out-dir <path>] [--check]

Options:
  --root <path>              Workspace root. Defaults to .
  --base <rev>               PR base revision. When omitted, resolved like
                             `ripr check`: origin/HEAD, then origin/main,
                             origin/master, main, and master.
  --head <rev>               PR head revision. Defaults to HEAD.
  --check-output <path>      Optional check JSON to consume instead of running analysis.
  --gap-ledger <path>        Gap-decision ledger JSON. Defaults to target/ripr/reports/gap-decision-ledger.json.
  --first-action <path>      First-useful-action JSON. Defaults to target/ripr/reports/first-useful-action.json.
  --review-comments <path>   Review-comments JSON. Defaults to target/ripr/review/comments.json.
  --agent-packet <path>      Agent packet JSON. Defaults to target/ripr/workflow/agent-packet.json.
  --gate-decision <path>     Gate-decision JSON. Defaults to target/ripr/reports/gate-decision.json.
  --receipts-dir <path>      Receipt directory. Defaults to target/ripr/receipts.
  --out-dir <path>           Output directory. Defaults to target/ripr/reports.
  --check                    Verify the existing start-here packet is up to date.

Start-here language:
  - start here: open target/ripr/reports/start-here.md first when it exists
  - safe next action: repair one named gap, regenerate missing evidence, or stop on no-action
  - missing artifact / stale evidence / wrong root / malformed artifact: fail closed before repair work
  - no actionable gap: advisory no-action, not runtime adequacy or mutation proof
  - preview-limited evidence: syntax-first and advisory, with static limits before repair language
  - receipt lifecycle: receipt_missing, receipt_found, receipt_stale, receipt_gap_mismatch, receipt_movement_improved, receipt_movement_unchanged, receipt_not_applicable
  - verify command / receipt command / receipt path: static movement proof rail
";
