//! Guard the deliberately simple, one-producer-per-required-step CI surface.
//! The repository workflow gate remains responsible for general YAML policy.

const WORKFLOW: &str = include_str!("../../.github/workflows/rust-gates.yml");
const REQUIRED_GATES: &[(&str, &str)] = &[
    ("formatting", "cargo fmt --check"),
    ("precommit", "cargo xtask precommit"),
    (
        "promotion_honesty",
        "cargo xtask check-evidence-promotion-honesty",
    ),
    ("agent_skills", "cargo xtask check-agent-skills"),
    ("dependencies", "cargo xtask check-dependencies"),
    ("process_policy", "cargo xtask check-process-policy"),
    ("network_policy", "cargo xtask check-network-policy"),
    ("workspace_check", "cargo check --workspace --all-targets"),
    (
        "clippy",
        "cargo clippy --workspace --all-targets -- -D warnings",
    ),
    ("rust-tests", "cargo nextest run --workspace --profile ci"),
    ("goldens", "cargo xtask goldens check"),
    ("fixtures", "cargo xtask fixtures"),
];

fn step_id(block: &str) -> Option<&str> {
    block
        .lines()
        .find_map(|line| line.strip_prefix("        id: "))
}

fn contract_violations(workflow: &str) -> Vec<String> {
    let blocks: Vec<_> = workflow.split("\n      - ").skip(1).collect();
    let mut findings = Vec::new();
    let mut previous = None;
    for &(id, command) in REQUIRED_GATES {
        let matches: Vec<_> = blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| step_id(block) == Some(id))
            .collect();
        if matches.len() != 1 {
            findings.push(format!("{id}: expected one step"));
            continue;
        }
        let Some(&(index, block)) = matches.first() else {
            continue;
        };
        if !block.lines().any(|line| {
            let line = line.trim();
            line.strip_prefix("run: ").unwrap_or(line) == command
        }) {
            findings.push(format!("{id}: exact command missing"));
        }
        if block.lines().any(|line| {
            line.starts_with("        if:") || line.starts_with("        continue-on-error:")
        }) {
            findings.push(format!("{id}: required step must remain unconditional"));
        }
        let command_count = workflow
            .lines()
            .map(str::trim)
            .map(|line| line.strip_prefix("run: ").unwrap_or(line))
            .filter(|line| *line == command)
            .count();
        if command_count != 1 {
            findings.push(format!("{id}: expected one command invocation"));
        }
        if previous.is_some_and(|prior| prior >= index) {
            findings.push(format!("{id}: gate order changed"));
        }
        previous = Some(index);
    }
    let summaries: Vec<_> = blocks
        .iter()
        .filter(|block| step_id(block) == Some("gate_summary"))
        .collect();
    if summaries.len() != 1 {
        findings.push("summary: expected one step".to_owned());
        return findings;
    }
    let Some(summary) = summaries.first() else {
        return findings;
    };
    if !summary.lines().any(|line| line == "        if: always()") {
        findings.push("summary: must run after non-success".to_owned());
    }
    for id in REQUIRED_GATES
        .iter()
        .map(|(id, _)| *id)
        .chain(std::iter::once("pr_evidence"))
    {
        let variable = format!("{}_RESULT", id.replace('-', "_").to_ascii_uppercase());
        let binding = format!("          {variable}: ${{{{ steps.{id}.outcome }}}}");
        let row =
            format!("            printf '| {id} | %s |\\n' \"${{{variable}:-not_reported}}\"");
        if !summary.lines().any(|line| line == binding) || !summary.lines().any(|line| line == row)
        {
            findings.push(format!("summary: {id} must retain its actual outcome"));
        }
    }
    findings
}

#[test]
fn rust_gate_workflow_preserves_complete_ordered_command_inventory() {
    let findings = contract_violations(WORKFLOW);
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn rust_gate_workflow_rejects_missing_required_command() {
    let changed = WORKFLOW.replace(
        "        run: cargo xtask check-network-policy",
        "        run: echo policy omitted",
    );
    let findings = contract_violations(&changed);
    assert!(findings.contains(&"network_policy: exact command missing".to_owned()));
}

#[test]
fn rust_gate_workflow_rejects_duplicate_required_command() {
    let changed = format!(
        "{WORKFLOW}\n      - name: Duplicate tests\n        run: cargo nextest run --workspace --profile ci\n"
    );
    let findings = contract_violations(&changed);
    assert!(findings.contains(&"rust-tests: expected one command invocation".to_owned()));
}

#[test]
fn rust_gate_workflow_rejects_suppressed_or_optional_required_command() {
    for replacement in [
        "        id: rust-tests\n        continue-on-error: true",
        "        id: rust-tests\n        if: false",
    ] {
        let changed = WORKFLOW.replace("        id: rust-tests", replacement);
        let findings = contract_violations(&changed);
        assert!(
            findings.contains(&"rust-tests: required step must remain unconditional".to_owned())
        );
    }
    let changed = WORKFLOW.replace(
        "          cargo nextest run --workspace --profile ci",
        "          cargo nextest run --workspace --profile ci || true",
    );
    let findings = contract_violations(&changed);
    assert!(findings.contains(&"rust-tests: exact command missing".to_owned()));
}

#[test]
fn rust_gate_workflow_rejects_policy_after_workspace_tests() {
    let changed = WORKFLOW
        .replace("        id: precommit", "        id: swap")
        .replace("        id: rust-tests", "        id: precommit")
        .replace("        id: swap", "        id: rust-tests");
    let findings = contract_violations(&changed);
    assert!(
        findings
            .iter()
            .any(|finding| finding.ends_with("gate order changed"))
    );
}

#[test]
fn rust_gate_workflow_rejects_failure_only_summary() {
    let changed = WORKFLOW.replace(
        "        id: gate_summary\n        if: always()",
        "        id: gate_summary\n        if: failure()",
    );
    let findings = contract_violations(&changed);
    assert!(findings.contains(&"summary: must run after non-success".to_owned()));
}

#[test]
fn rust_gate_workflow_rejects_fabricated_success_or_missing_outcome() {
    for changed in [
        WORKFLOW.replace(
            "RUST_TESTS_RESULT: ${{ steps.rust-tests.outcome }}",
            "RUST_TESTS_RESULT: success",
        ),
        WORKFLOW.replace("${RUST_TESTS_RESULT:-not_reported}", "success"),
        WORKFLOW.replace(
            "${RUST_TESTS_RESULT:-not_reported}",
            "${RUST_TESTS_RESULT:-success}",
        ),
    ] {
        let findings = contract_violations(&changed);
        assert!(
            findings.contains(&"summary: rust-tests must retain its actual outcome".to_owned())
        );
    }
}
