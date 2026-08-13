use std::fs;
use std::path::PathBuf;

fn workflow_text() -> Result<String, String> {
    let xtask = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = xtask
        .parent()
        .ok_or_else(|| "xtask manifest directory has no repository parent".to_string())?;
    fs::read_to_string(root.join(".github/workflows/source-promotion-contract.yml"))
        .map_err(|error| format!("read source-promotion contract workflow: {error}"))
}

#[test]
fn changed_workflows_require_exact_reviewed_non_source_dispositions() -> Result<(), String> {
    let workflow = workflow_text()?;
    for needle in [
        "changed_workflows=$(git diff --name-only \"$BASE_SHA...$PR_HEAD\" -- .github/workflows)",
        "[.dispositions[]? | select(.key == $path and (.disposition == \"swarm_blob\" or .disposition == \"integrated\"))] | length",
        "test \"$reviewed_count\" -ne 1",
        "promotion PR changes workflows without exactly one reviewed non-source disposition",
    ] {
        assert!(workflow.contains(needle), "workflow missing contract fragment: {needle}");
    }
    assert!(
        !workflow.contains("/^\\.github\\/workflows\\/source-promotion-contract\\.yml$/d"),
        "workflow must not substitute a hardcoded workflow exception for reviewed resolution authority"
    );
    Ok(())
}

#[test]
fn workflow_rejection_reason_is_single_line_after_multiple_unreviewed_paths() -> Result<(), String> {
    let workflow = workflow_text()?;
    assert!(workflow.contains("unreviewed_workflows=\"$unreviewed_workflows,$workflow\""));
    assert!(workflow.contains("unreviewed_workflows=\"$workflow\""));
    assert!(
        !workflow.contains("fail \"promotion PR changes non-contract workflows: $unexpected_workflows\""),
        "multi-line git diff output must not flow directly into a single-line GITHUB_OUTPUT value"
    );
    Ok(())
}
