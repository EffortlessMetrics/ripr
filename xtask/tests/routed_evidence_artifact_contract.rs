//! The selected docs/Rust producer must transport the same canonical receipt.
use std::fs;
use std::path::Path;

const ARTIFACT: &str = "ripr-routed-rust-evidence-${{ github.run_id }}-${{ github.run_attempt }}";
const RECEIPTS: &[&str] = &[
    "target/ripr/reports/routed-rust-plan.json",
    "target/ripr/reports/routed-rust-plan.md",
    "target/ripr/reports/routed-rust-execution.json",
    "target/ripr/reports/routed-rust-command-*.log",
];

fn workflow() -> Result<String, String> {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows/routed-rust.yml"),
    )
    .map_err(|err| err.to_string())
}

fn job<'a>(workflow: &'a str, name: &str) -> Result<&'a str, String> {
    workflow
        .split(&format!("\n  {name}:\n"))
        .nth(1)
        .map(|body| {
            let end = body
                .match_indices("\n  ")
                .find(|(index, _)| body.as_bytes().get(index + 3) != Some(&b' '))
                .map_or(body.len(), |(index, _)| index);
            &body[..end]
        })
        .ok_or_else(|| format!("missing job {name}"))
}

fn step<'a>(job: &'a str, name: &str) -> Result<&'a str, String> {
    job.split(&format!("      - name: {name}\n"))
        .nth(1)
        .map(|body| body.split("\n      - ").next().unwrap_or(body))
        .ok_or_else(|| format!("missing step {name}"))
}

fn transport_contract(workflow: &str) -> Result<(), String> {
    for producer in ["rust-github", "docs-gate"] {
        let body = job(workflow, producer)?;
        let upload = step(body, "Upload child execution evidence")
            .map_err(|err| format!("{producer}: {err}"))?;
        if !upload
            .lines()
            .any(|line| line == format!("          name: {ARTIFACT}"))
        {
            return Err(format!("{producer}: canonical run/attempt artifact absent"));
        }
        if !upload.contains("        if: always()\n")
            || !upload
                .lines()
                .any(|line| line.starts_with("        uses: actions/upload-artifact@"))
        {
            return Err(format!("{producer}: terminal receipt upload absent"));
        }
        for path in RECEIPTS {
            if !upload
                .lines()
                .any(|line| line == format!("            {path}"))
            {
                return Err(format!("{producer}: canonical receipt path absent: {path}"));
            }
        }
    }
    let result = job(workflow, "result")?;
    let download = step(result, "Download the selected child evidence")?;
    if !download
        .lines()
        .any(|line| line == format!("          name: {ARTIFACT}"))
        || !download
            .lines()
            .any(|line| line == "          path: target/ripr/reports/")
        || download.contains("pattern:")
        || download.contains("merge-multiple:")
    {
        return Err("result: selected exact artifact contract changed".to_string());
    }
    let clear = step(result, "Clear any non-artifact routed evidence")?;
    let quarantine = step(
        result,
        "Quarantine evidence after an artifact integrity failure",
    )?;
    if !quarantine.contains("steps.evidence_download.outcome != 'success'") {
        return Err("result: missing artifact must quarantine evidence".to_string());
    }
    for path in &RECEIPTS[..3] {
        if !clear.contains(path) || !quarantine.contains(path) {
            return Err(format!("result: stale receipt survives: {path}"));
        }
    }
    if !job(workflow, "rust-github")?.contains("needs.detect-docs-only.outputs.docs_only != 'true'")
        || !job(workflow, "docs-gate")?
            .contains("needs.detect-docs-only.outputs.docs_only == 'true'")
    {
        return Err("receipt producers must be mutually exclusive".to_string());
    }
    Ok(())
}

#[test]
fn both_selected_producers_transport_the_exact_result_receipt() -> Result<(), String> {
    transport_contract(&workflow()?)
}

#[test]
fn stale_missing_or_merged_artifacts_cannot_satisfy_the_transport() -> Result<(), String> {
    let workflow = workflow()?;
    transport_contract(&workflow)?;
    for selected_job in ["rust-github", "docs-gate", "result"] {
        let body = job(&workflow, selected_job)?;
        for replacement in [
            "ripr-docs-gate",
            "ripr-routed-rust-evidence-${{ github.run_id }}",
            "ripr-routed-rust-evidence-previous-attempt",
        ] {
            let changed_body = body.replacen(ARTIFACT, replacement, 1);
            assert_ne!(changed_body, body);
            let changed = workflow.replace(body, &changed_body);
            assert_ne!(changed, workflow);
            assert!(transport_contract(&changed).is_err());
        }
    }
    let changed = workflow.replace(
        "            target/ripr/reports/routed-rust-execution.json\n",
        "",
    );
    assert_ne!(changed, workflow);
    assert!(transport_contract(&changed).is_err());
    let result = job(&workflow, "result")?;
    let download = step(result, "Download the selected child evidence")?;
    for option in [
        "          pattern: ripr-routed-rust-*",
        "          merge-multiple: true",
    ] {
        let changed = workflow.replace(download, &format!("{download}\n{option}"));
        assert_ne!(changed, workflow);
        assert!(transport_contract(&changed).is_err());
    }
    let changed = workflow.replace("steps.evidence_download.outcome != 'success'", "false");
    assert_ne!(changed, workflow);
    assert!(transport_contract(&changed).is_err());
    Ok(())
}
