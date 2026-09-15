use std::path::Path;

pub(crate) fn strip_yaml_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let chars: Vec<char> = line.chars().collect();
    for idx in 0..chars.len() {
        match chars[idx] {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => {
                let backslash_run = chars[..idx]
                    .iter()
                    .rev()
                    .take_while(|&&c| c == '\\')
                    .count();
                if backslash_run % 2 == 0 {
                    in_double = !in_double;
                }
            }
            '#' if !in_single && !in_double => return &line[..chars[idx].len_utf8() * idx],
            _ => {}
        }
    }
    line
}

pub(crate) fn active_yaml_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(strip_yaml_comment)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn has_active_line(lines: &[String], pattern: &str) -> bool {
    lines.iter().any(|line| line.contains(pattern))
}

pub(crate) fn forbids_active_line(lines: &[String], pattern: &str) -> bool {
    lines.iter().any(|line| line.contains(pattern))
}

const DROID_SAFE_ACTION: &str = "EffortlessMetrics/droid-action-safe";
const DROID_SAFE_ACTION_SHA: &str = "7c1377ccbacddc95560d1570547a5baa51de01ec";
const DROID_UNSAFE_UPSTREAM_ACTION: &str = "Factory-AI/droid-action";
const DROID_GH_CLI_ARCHIVE: &str = "gh_2.82.1_linux_amd64.tar.gz";
const DROID_GH_CLI_SHA256: &str =
    "afada88676dfccea384e6cc28ae990b3e31bbc55f9d75c4697f902c757fa462b";

pub(crate) fn check_droid_action_refs(violations: &mut Vec<String>, path_label: &str, text: &str) {
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let after_comment = strip_yaml_comment(trimmed).trim();
        let after_uses = after_comment
            .strip_prefix("- uses: ")
            .or_else(|| after_comment.strip_prefix("uses: "));
        if let Some(after_uses) = after_uses
            && let Some(at_pos) = after_uses.find('@')
        {
            let action = &after_uses[..at_pos];
            let ref_part = after_uses[at_pos + 1..]
                .split_whitespace()
                .next()
                .unwrap_or("");
            if !(ref_part.len() == 40 && ref_part.chars().all(|c| c.is_ascii_hexdigit())) {
                violations.push(format!(
                    "{path_label}:{} action ref must use immutable commit SHA: {action}@{ref_part}",
                    line_number + 1
                ));
            }
            if action == DROID_UNSAFE_UPSTREAM_ACTION {
                violations.push(format!(
                    "{path_label}:{} must not use unsafe upstream Droid action for BYOK workflows: {action}@{ref_part}",
                    line_number + 1
                ));
            }
            if action.contains("droid-action")
                && !(action == DROID_SAFE_ACTION && ref_part == DROID_SAFE_ACTION_SHA)
            {
                violations.push(format!(
                    "{path_label}:{} Droid action must use approved safe action ref {DROID_SAFE_ACTION}@{DROID_SAFE_ACTION_SHA}",
                    line_number + 1
                ));
            }
        }
    }
}

pub(crate) fn check_droid_common(
    violations: &mut Vec<String>,
    path_label: &str,
    text: &str,
    require_same_repo_guard: bool,
    require_review_model: bool,
) {
    let lines = active_yaml_lines(text);

    if require_same_repo_guard
        && !has_active_line(&lines, "head.repo.full_name == github.repository")
    {
        violations.push(format!(
            "{path_label}: same-repo guard (head.repo.full_name == github.repository) is required"
        ));
    }

    if require_review_model && !has_active_line(&lines, "review_model: \"custom:MiniMax-M3-0\"") {
        violations.push(format!(
            "{path_label}: review_model must be custom:MiniMax-M3-0"
        ));
    }

    if !has_active_line(&lines, "security_model: \"custom:MiniMax-M3-0\"") {
        violations.push(format!(
            "{path_label}: security_model must be custom:MiniMax-M3-0"
        ));
    }

    if !has_active_line(&lines, "$HOME/.factory/settings.json") {
        violations.push(format!(
            "{path_label}: must write $HOME/.factory/settings.json"
        ));
    }

    if !has_active_line(&lines, "${MINIMAX_API_KEY}") {
        violations.push(format!(
            "{path_label}: must keep ${{MINIMAX_API_KEY}} literal in settings.json"
        ));
    }

    if forbids_active_line(&lines, "settings:") {
        violations.push(format!(
            "{path_label}: must not use the Droid Action settings: input for BYOK"
        ));
    }

    for anthropic_global in ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"] {
        let empty_double_quoted = format!("{anthropic_global}: \"\"");
        let empty_single_quoted = format!("{anthropic_global}: ''");
        let has_empty_override = has_active_line(&lines, &empty_double_quoted)
            || has_active_line(&lines, &empty_single_quoted);
        let has_non_empty_override = lines.iter().any(|line| {
            line.starts_with(&format!("{anthropic_global}:"))
                && line != &empty_double_quoted
                && line != &empty_single_quoted
        });

        if !has_empty_override || has_non_empty_override {
            violations.push(format!(
                "{path_label}: must clear {anthropic_global} with an empty step env override"
            ));
        }
    }

    let lower_lines: Vec<String> = lines.iter().map(|l| l.to_ascii_lowercase()).collect();
    if has_active_line(&lower_lines, "show_full_output: true") {
        violations.push(format!("{path_label}: must not enable show_full_output"));
    } else if !has_active_line(&lower_lines, "show_full_output: false") {
        violations.push(format!(
            "{path_label}: must explicitly set show_full_output: false"
        ));
    }

    if has_active_line(&lower_lines, "upload_debug_artifacts: true") {
        violations.push(format!(
            "{path_label}: must not enable Droid debug artifact upload"
        ));
    } else if !has_active_line(&lower_lines, "upload_debug_artifacts: false") {
        violations.push(format!(
            "{path_label}: must explicitly set upload_debug_artifacts: false"
        ));
    }

    if !has_active_line(&lines, "command -v gh") {
        violations.push(format!(
            "{path_label}: must check for GitHub CLI before running Droid"
        ));
    }
    if !has_active_line(&lines, DROID_GH_CLI_ARCHIVE) {
        violations.push(format!(
            "{path_label}: must install pinned GitHub CLI archive {DROID_GH_CLI_ARCHIVE}"
        ));
    }
    if !has_active_line(&lines, DROID_GH_CLI_SHA256) {
        violations.push(format!(
            "{path_label}: must verify pinned GitHub CLI SHA256 {DROID_GH_CLI_SHA256}"
        ));
    }
    if !has_active_line(&lines, "$GITHUB_PATH") {
        violations.push(format!(
            "{path_label}: must add the pinned GitHub CLI bin directory to $GITHUB_PATH"
        ));
    }

    check_droid_action_refs(violations, path_label, text);
}

pub(crate) fn check_droid_security_scan_config(
    violations: &mut Vec<String>,
    path_label: &str,
    text: &str,
) {
    let lines = active_yaml_lines(text);

    if !has_active_line(&lines, "workflow_dispatch:") {
        violations.push(format!(
            "{path_label}: workflow_dispatch trigger is required"
        ));
    }

    if !has_active_line(&lines, "cron: \"0 8 * * 1\"") {
        violations.push(format!(
            "{path_label}: weekly Monday 08:00 UTC schedule is required"
        ));
    }

    if !has_active_line(&lines, "droid-security-scan-${{ github.repository }}") {
        violations.push(format!(
            "{path_label}: concurrency group must be repository-scoped"
        ));
    }

    if !has_active_line(&lines, "cancel-in-progress: false") {
        violations.push(format!(
            "{path_label}: concurrency cancel-in-progress must be false"
        ));
    }

    if !has_active_line(&lines, "MINIMAX_API_KEY: ${{ secrets.MINIMAX_API_KEY }}") {
        violations.push(format!(
            "{path_label}: MINIMAX_API_KEY must be job-level env"
        ));
    }

    if !has_active_line(&lines, "security_scan_schedule: true") {
        violations.push(format!("{path_label}: security_scan_schedule must be true"));
    }

    if !has_active_line(&lines, "security_scan_days: 7") {
        violations.push(format!("{path_label}: security_scan_days must be 7"));
    }

    if !has_active_line(&lines, "security_severity_threshold: medium") {
        violations.push(format!(
            "{path_label}: security_severity_threshold must be medium"
        ));
    }

    if !has_active_line(&lines, "security_block_on_critical: true") {
        violations.push(format!(
            "{path_label}: security_block_on_critical must be true"
        ));
    }

    if !has_active_line(&lines, "security_block_on_high: false") {
        violations.push(format!(
            "{path_label}: security_block_on_high must be false"
        ));
    }

    check_droid_common(violations, path_label, text, false, false);
}

pub(crate) fn check_droid_review_config() -> Result<(), String> {
    let mut violations = Vec::new();

    let droid_review_path = ".github/workflows/droid-review.yml";
    let droid_path = ".github/workflows/droid.yml";
    let droid_security_scan_path = ".github/workflows/droid-security-scan.yml";

    if let Ok(text) = crate::read_text_lossy(Path::new(droid_review_path)) {
        let lines = active_yaml_lines(&text);

        if !has_active_line(&lines, "opened")
            || !has_active_line(&lines, "synchronize")
            || !has_active_line(&lines, "ready_for_review")
            || !has_active_line(&lines, "reopened")
        {
            violations.push(format!(
                "{droid_review_path}: pull_request types must include opened, synchronize, ready_for_review, reopened"
            ));
        }

        if lines
            .iter()
            .any(|line| line.to_ascii_lowercase().contains("draft"))
            && lines
                .iter()
                .any(|line| line.contains("if:") && line.to_ascii_lowercase().contains("draft"))
        {
            violations.push(format!(
                "{droid_review_path}: must not filter out draft PRs"
            ));
        }

        if !has_active_line(&lines, "cancel-in-progress: false") {
            violations.push(format!(
                "{droid_review_path}: concurrency cancel-in-progress must be false"
            ));
        }

        if !has_active_line(
            &lines,
            "droid-review-${{ github.repository }}-${{ github.event.pull_request.number }}",
        ) {
            violations.push(format!(
                "{droid_review_path}: concurrency group must be per repository and PR number"
            ));
        }

        if !has_active_line(&lines, "automatic_review: true") {
            violations.push(format!(
                "{droid_review_path}: automatic_review must be true"
            ));
        }

        if !has_active_line(&lines, "automatic_security_review: true") {
            violations.push(format!(
                "{droid_review_path}: automatic_security_review must be true"
            ));
        }

        if !has_active_line(&lines, "review_depth: shallow") {
            violations.push(format!(
                "{droid_review_path}: review_depth must be shallow unless intentionally changed"
            ));
        }

        if !has_active_line(&lines, "MINIMAX_API_KEY: ${{ secrets.MINIMAX_API_KEY }}") {
            violations.push(format!(
                "{droid_review_path}: MINIMAX_API_KEY must be job-level env"
            ));
        }

        check_droid_common(&mut violations, droid_review_path, &text, true, true);
    } else {
        violations.push(format!("{droid_review_path}: file not found or unreadable"));
    }

    if let Ok(text) = crate::read_text_lossy(Path::new(droid_path)) {
        let lines = active_yaml_lines(&text);

        if !has_active_line(&lines, "OWNER")
            || !has_active_line(&lines, "MEMBER")
            || !has_active_line(&lines, "COLLABORATOR")
        {
            violations.push(format!(
                "{droid_path}: trusted actor guard (OWNER, MEMBER, COLLABORATOR) is required"
            ));
        }

        check_droid_common(&mut violations, droid_path, &text, true, true);
    } else {
        violations.push(format!("{droid_path}: file not found or unreadable"));
    }

    if let Ok(text) = crate::read_text_lossy(Path::new(droid_security_scan_path)) {
        check_droid_security_scan_config(&mut violations, droid_security_scan_path, &text);
    } else {
        violations.push(format!(
            "{droid_security_scan_path}: file not found or unreadable"
        ));
    }

    crate::finish_policy_report(
        crate::PolicyReportSpec {
            report_file: "droid-review-config.md",
            check: "check-droid-review-config",
            why_it_matters: "Droid workflows handle repository secrets and automated review or security output; invariant drift can expose secrets, break BYOK model selection, or degrade review quality.",
            fix_kind: crate::FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Restore the required invariant in the workflow YAML.",
                "If the invariant is intentionally changed, update docs/agent-context/review-invariants.md and add an xtask exception only after repo review.",
            ],
            rerun_command: "cargo xtask check-droid-review-config",
            exception_template: None,
        },
        &violations,
    )
}
