use super::{
    DEFAULT_AGENT_PACKET, DEFAULT_BASE, DEFAULT_FIRST_ACTION, DEFAULT_GAP_LEDGER,
    DEFAULT_GATE_DECISION, DEFAULT_HEAD, DEFAULT_OUT_DIR, DEFAULT_RECEIPTS_DIR,
    DEFAULT_REVIEW_COMMENTS, DEFAULT_ROOT,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FirstPrOptions {
    pub(super) root: String,
    pub(super) base: String,
    pub(super) head: String,
    pub(super) check_output: Option<String>,
    pub(super) gap_ledger: String,
    pub(super) first_action: String,
    pub(super) review_comments: String,
    pub(super) agent_packet: String,
    pub(super) gate_decision: String,
    pub(super) receipts_dir: String,
    pub(super) out_dir: String,
    pub(super) check: bool,
    pub(super) preflight: bool,
}

impl Default for FirstPrOptions {
    fn default() -> Self {
        Self {
            root: DEFAULT_ROOT.to_string(),
            base: DEFAULT_BASE.to_string(),
            head: DEFAULT_HEAD.to_string(),
            check_output: None,
            gap_ledger: DEFAULT_GAP_LEDGER.to_string(),
            first_action: DEFAULT_FIRST_ACTION.to_string(),
            review_comments: DEFAULT_REVIEW_COMMENTS.to_string(),
            agent_packet: DEFAULT_AGENT_PACKET.to_string(),
            gate_decision: DEFAULT_GATE_DECISION.to_string(),
            receipts_dir: DEFAULT_RECEIPTS_DIR.to_string(),
            out_dir: DEFAULT_OUT_DIR.to_string(),
            check: false,
            preflight: false,
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
            other => return Err(format!("unknown first-pr argument {other:?}")),
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
    println!("{}", first_pr_help_text());
}

pub(super) fn first_pr_help_text() -> &'static str {
    "Create the start-here packet for one PR from existing RIPR artifacts.\n\nusage: ripr first-pr|start-here [--root <path>] [--base <rev>] [--head <rev>] [--check-output <path>] [--gap-ledger <path>] [--first-action <path>] [--review-comments <path>] [--agent-packet <path>] [--gate-decision <path>] [--receipts-dir <path>] [--out-dir <path>] [--check]\n\nStart-here language:\n  - start here: open target/ripr/reports/start-here.md first when it exists\n  - safe next action: repair one named gap, regenerate missing evidence, or stop on no-action\n  - missing artifact / stale evidence / wrong root / malformed artifact: fail closed before repair work\n  - no actionable gap: advisory no-action, not runtime adequacy or mutation proof\n  - preview-limited evidence: syntax-first and advisory, with static limits before repair language\n  - receipt lifecycle: receipt_missing, receipt_found, receipt_stale, receipt_gap_mismatch, receipt_movement_improved, receipt_movement_unchanged, receipt_not_applicable\n  - verify command / receipt command / receipt path: static movement proof rail"
}
