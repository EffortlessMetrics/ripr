use std::fs;
use std::path::{Path, PathBuf};

use super::gate::evaluate;
use super::model::{
    ExpectedIdentity, GateInput, GateVerdict, PackageQualificationGate, PackageQualificationReceipt,
};
use super::parse::parse_receipt;
use super::render::{render_gate_json, render_gate_markdown, render_receipt_json};
use crate::policy::distribution::load_distribution_contract;

const DEFAULT_OUT: &str = "target/ripr/reports";
const GATE_JSON: &str = "package-qualification-gate.json";
const GATE_MD: &str = "package-qualification-gate.md";
const RECEIPT_JSON: &str = "package-qualification-receipt.json";

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let options = parse_args(args)?;
    let (receipt, report) = evaluate_path(&options)?;
    write_reports(&options.out, &receipt, &report)?;
    if report.verdict != GateVerdict::Passed {
        return Err(format!(
            "package-qualification-gate: {}",
            report.failures.join("; ")
        ));
    }
    Ok(())
}

pub(crate) fn evaluate_path(
    options: &CommandOptions,
) -> Result<(PackageQualificationReceipt, PackageQualificationGate), String> {
    let text = fs::read_to_string(&options.receipt).map_err(|error| {
        format!(
            "failed to read package qualification receipt {}: {error}",
            options.receipt.display()
        )
    })?;
    let receipt = parse_receipt(&text)?;
    let full_matrix_targets = match options.full_matrix_targets.clone() {
        Some(targets) => targets,
        None if receipt.selection_scope == super::model::SelectionScope::DeclaredFullMatrix => {
            load_full_matrix_targets()?
        }
        None => Vec::new(),
    };
    let gate = evaluate(&GateInput {
        receipt: &receipt,
        expected: &options.expected,
        full_matrix_targets: &full_matrix_targets,
    });
    Ok((receipt, gate))
}

pub(crate) struct CommandOptions {
    pub(crate) receipt: PathBuf,
    pub(crate) expected: ExpectedIdentity,
    pub(crate) out: PathBuf,
    pub(crate) full_matrix_targets: Option<Vec<String>>,
}

fn parse_args(args: &[String]) -> Result<CommandOptions, String> {
    let mut receipt = None;
    let mut expected = ExpectedIdentity::default();
    let mut out = PathBuf::from(DEFAULT_OUT);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--receipt" => {
                receipt = Some(PathBuf::from(require_value(args, index, "--receipt")?));
                index += 2;
            }
            "--expected-commit" => {
                expected.commit = Some(require_value(args, index, "--expected-commit")?);
                index += 2;
            }
            "--expected-tree" => {
                expected.tree = Some(require_value(args, index, "--expected-tree")?);
                index += 2;
            }
            "--expected-package-hash" => {
                expected.package_hash =
                    Some(require_value(args, index, "--expected-package-hash")?);
                index += 2;
            }
            "--expected-payload-hash" => {
                expected.payload_hash =
                    Some(require_value(args, index, "--expected-payload-hash")?);
                index += 2;
            }
            "--out" => {
                out = PathBuf::from(require_value(args, index, "--out")?);
                index += 2;
            }
            other => {
                return Err(format!(
                    "unknown package-qualification-gate argument `{other}`; {}",
                    usage()
                ));
            }
        }
    }
    let Some(receipt) = receipt else {
        return Err(usage().to_string());
    };
    Ok(CommandOptions {
        receipt,
        expected,
        out,
        full_matrix_targets: None,
    })
}

fn require_value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value; {}", usage()))
}

fn usage() -> &'static str {
    "usage: cargo xtask package-qualification-gate --receipt <path> [--expected-commit <sha>] [--expected-tree <sha>] [--expected-package-hash <digest>] [--expected-payload-hash <digest>] [--out <dir>]"
}

fn load_full_matrix_targets() -> Result<Vec<String>, String> {
    let contract = load_distribution_contract()?;
    Ok(contract
        .target
        .iter()
        .map(|target| target.rust_target.clone())
        .collect())
}

fn write_reports(
    out: &Path,
    receipt: &PackageQualificationReceipt,
    report: &PackageQualificationGate,
) -> Result<(), String> {
    fs::create_dir_all(out)
        .map_err(|error| format!("failed to create {}: {error}", out.display()))?;
    let receipt_path = out.join(RECEIPT_JSON);
    let json_path = out.join(GATE_JSON);
    let md_path = out.join(GATE_MD);
    fs::write(&receipt_path, render_receipt_json(receipt)?)
        .map_err(|error| format!("failed to write {}: {error}", receipt_path.display()))?;
    fs::write(&json_path, render_gate_json(report)?)
        .map_err(|error| format!("failed to write {}: {error}", json_path.display()))?;
    fs::write(&md_path, render_gate_markdown(report))
        .map_err(|error| format!("failed to write {}: {error}", md_path.display()))?;
    Ok(())
}
