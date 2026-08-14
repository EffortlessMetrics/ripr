use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

fn workflow_text() -> Result<String, String> {
    let xtask = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = xtask
        .parent()
        .ok_or_else(|| "xtask manifest directory has no repository parent".to_string())?;
    fs::read_to_string(root.join(".github/workflows/source-promotion-contract.yml"))
        .map_err(|error| format!("read source-promotion contract workflow: {error}"))
}

fn workflow_disposition_is_authorized(manifest: &Value, path: &str) -> bool {
    let Some(dispositions) = manifest.get("dispositions").and_then(Value::as_array) else {
        return false;
    };
    let matching = dispositions
        .iter()
        .filter(|row| row.get("key").and_then(Value::as_str) == Some(path))
        .collect::<Vec<_>>();
    !matching.is_empty()
        && matching.iter().all(|row| {
            matches!(
                row.get("disposition").and_then(Value::as_str),
                Some("swarm_blob" | "integrated")
            )
        })
}

#[test]
fn changed_workflows_require_unanimous_reviewed_non_source_dispositions() -> Result<(), String> {
    let workflow = workflow_text()?;
    for needle in [
        "changed_workflows=$(git diff --name-only \"$BASE_SHA...$PR_HEAD\" -- .github/workflows)",
        "[.dispositions[]? | select(.key == $path)] | length",
        "[.dispositions[]? | select(.key == $path and (.disposition == \"swarm_blob\" or .disposition == \"integrated\"))] | length",
        "test \"$resolution_count\" -eq 0 || test \"$reviewed_count\" -ne \"$resolution_count\"",
        "promotion PR changes workflows without unanimous reviewed non-source dispositions",
    ] {
        assert!(
            workflow.contains(needle),
            "workflow missing contract fragment: {needle}"
        );
    }
    assert!(
        !workflow.contains("/^\\.github\\/workflows\\/source-promotion-contract\\.yml$/d"),
        "workflow must not substitute a hardcoded workflow exception for reviewed resolution authority"
    );
    Ok(())
}

#[test]
fn workflow_disposition_authority_rejects_missing_mixed_or_unreviewed_rows() {
    let path = ".github/workflows/routed-rust.yml";
    let rejected = [
        json!({"dispositions": []}),
        json!({"dispositions": [{"key": path, "disposition": "source_blob"}]}),
        json!({"dispositions": [
            {"key": path, "disposition": "swarm_blob"},
            {"key": path, "disposition": "source_blob"}
        ]}),
        json!({"dispositions": [
            {"key": path, "disposition": "integrated"},
            {"key": path, "disposition": "source_blob"}
        ]}),
    ];

    for manifest in rejected {
        assert!(
            !workflow_disposition_is_authorized(&manifest, path),
            "missing, mixed, or unreviewed workflow dispositions must fail closed: {manifest}"
        );
    }
}

#[test]
fn workflow_disposition_authority_accepts_one_or_more_allowed_category_rows() {
    let path = ".github/workflows/routed-rust.yml";
    for manifest in [
        json!({"dispositions": [{"key": path, "disposition": "swarm_blob"}]}),
        json!({"dispositions": [{"key": path, "disposition": "integrated"}]}),
        json!({"dispositions": [
            {"kind": "conflict", "key": path, "disposition": "integrated"},
            {"kind": "source_survivor", "key": path, "disposition": "swarm_blob"}
        ]}),
    ] {
        assert!(
            workflow_disposition_is_authorized(&manifest, path),
            "all category rows authorize non-source workflow movement: {manifest}"
        );
    }
}

#[test]
fn workflow_rejection_reason_is_single_line_after_multiple_unreviewed_paths() -> Result<(), String>
{
    let workflow = workflow_text()?;
    assert!(workflow.contains("unreviewed_workflows=\"$unreviewed_workflows,$workflow\""));
    assert!(workflow.contains("unreviewed_workflows=\"$workflow\""));
    assert!(
        !workflow.contains(
            "fail \"promotion PR changes non-contract workflows: $unexpected_workflows\""
        ),
        "multi-line git diff output must not flow directly into a single-line GITHUB_OUTPUT value"
    );
    Ok(())
}
