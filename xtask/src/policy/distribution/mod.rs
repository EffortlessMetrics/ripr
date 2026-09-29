mod contract;
mod crate_targets;
mod targets;
mod version;
mod workflow;

use std::fs;

use crate::{FixKind, PolicyReportSpec, finish_policy_report};

pub(crate) use contract::DistributionContract;
pub(crate) use version::pep440_version;

pub(crate) const CONTRACT_PATH: &str = "policy/distribution.toml";
const WORKSPACE_MANIFEST_PATH: &str = "Cargo.toml";
const CRATE_MANIFEST_PATH: &str = "crates/ripr/Cargo.toml";
const SERVER_ARCHIVE_WORKFLOW_PATH: &str = ".github/workflows/server-archive-qualification.yml";
const SERVER_ARCHIVE_WORKFLOW_TEXT: &str =
    include_str!("../../../../.github/workflows/server-archive-qualification.yml");

pub(crate) fn load_distribution_contract() -> Result<DistributionContract, String> {
    let text = fs::read_to_string(CONTRACT_PATH)
        .map_err(|err| format!("failed to read {CONTRACT_PATH}: {err}"))?;
    parse_distribution_contract(CONTRACT_PATH, &text)
}

pub(crate) fn parse_distribution_contract(
    path: &str,
    text: &str,
) -> Result<DistributionContract, String> {
    contract::parse_distribution_contract(path, text)
}

pub(crate) fn check_distribution_contract() -> Result<(), String> {
    let report_spec = PolicyReportSpec {
        report_file: "distribution-contract.md",
        check: "check-release-targets (distribution contract)",
        why_it_matters: "PyPI, npm, native archives, and installed-product qualification must refer to one product identity, version source, feature set, and target map. Drift here can publish the right name with the wrong binary or silently pair one launcher version with another payload.",
        fix_kind: FixKind::AuthorDecisionRequired,
        recommended_fixes: &[
            "Update policy/distribution.toml as the package identity and target authority; do not hand-edit divergent adapter copies.",
            "Keep the Cargo workspace version as the sole product version source and use the tested SemVer-to-PEP-440 mapping for Python metadata.",
            "Add or remove a target only with its exact Rust target, executable, archive, wheel family, npm package, os/cpu/libc metadata, and qualification owner.",
            "Leave compatibility_state = \"unqualified\" until #4489 records the measured native compatibility floor; a tag family is not compatibility proof.",
        ],
        rerun_command: "cargo xtask check-release-targets",
        exception_template: None,
    };

    let mut violations = Vec::new();
    let contract = match load_distribution_contract() {
        Ok(contract) => Some(contract),
        Err(err) => {
            violations.push(err);
            None
        }
    };
    let workspace_text = match fs::read_to_string(WORKSPACE_MANIFEST_PATH) {
        Ok(text) => Some(text),
        Err(err) => {
            violations.push(format!("failed to read {WORKSPACE_MANIFEST_PATH}: {err}"));
            None
        }
    };
    let crate_text = match fs::read_to_string(CRATE_MANIFEST_PATH) {
        Ok(text) => Some(text),
        Err(err) => {
            violations.push(format!("failed to read {CRATE_MANIFEST_PATH}: {err}"));
            None
        }
    };

    if let (Some(contract), Some(workspace_text), Some(crate_text)) = (
        contract.as_ref(),
        workspace_text.as_deref(),
        crate_text.as_deref(),
    ) {
        violations.extend(evaluate_contract(
            CONTRACT_PATH,
            contract,
            WORKSPACE_MANIFEST_PATH,
            workspace_text,
            CRATE_MANIFEST_PATH,
            crate_text,
        ));
    }

    finish_policy_report(report_spec, &violations)
}

fn evaluate_contract(
    contract_path: &str,
    contract: &DistributionContract,
    workspace_path: &str,
    workspace_text: &str,
    crate_path: &str,
    crate_text: &str,
) -> Vec<String> {
    let mut violations = Vec::new();
    contract::validate_contract_identity(contract_path, contract, &mut violations);
    targets::validate_targets(contract_path, &contract.target, &mut violations);
    workflow::validate_archive_workflow(
        SERVER_ARCHIVE_WORKFLOW_PATH,
        SERVER_ARCHIVE_WORKFLOW_TEXT,
        &contract.target,
        &mut violations,
    );

    match contract::parse_workspace_version(workspace_path, workspace_text) {
        Ok(version) => {
            if let Err(err) = pep440_version(&version) {
                violations.push(format!(
                    "{workspace_path}: workspace version `{version}` cannot map to PyPI: {err}"
                ));
            }
        }
        Err(err) => violations.push(err),
    }

    contract::validate_crate_manifest(crate_path, crate_text, &contract.product, &mut violations);
    crate_targets::validate_single_binary(
        crate_path,
        crate_text,
        &contract.product,
        &mut violations,
    );
    violations
}

#[cfg(test)]
mod tests;
