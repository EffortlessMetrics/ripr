use crate::analysis::{
    canonical_gap::canonical_gap_identity, inventory_changed_test_classified_seams_at_with_config,
};
use crate::cli::commands_context::{ensure_command_root, load_root_input_and_config};
use crate::cli::help;
use crate::cli::parse::expect_value;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
struct RerunOptions {
    root: PathBuf,
    changed_test: PathBuf,
    json: bool,
    out: Option<PathBuf>,
}

#[derive(Serialize)]
struct TargetedRerunReport {
    schema_version: &'static str,
    state: &'static str,
    selector: TargetedRerunSelector,
    cache: TargetedRerunCache,
    seams: Vec<TargetedRerunSeam>,
    authority_boundary: &'static str,
}

#[derive(Serialize)]
struct TargetedRerunSelector {
    changed_test: String,
    selected_test_count: usize,
    direct_call_names: Vec<String>,
}

#[derive(Serialize)]
struct TargetedRerunCache {
    file_fact_status: String,
    hits: usize,
    misses: usize,
    corrupt_ignored: usize,
    stores: usize,
    store_errors: usize,
}

#[derive(Serialize)]
struct TargetedRerunSeam {
    canonical_gap_id: Option<String>,
    seam_id: String,
    file: String,
    line: usize,
    owner: String,
    static_class: String,
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        help::print_rerun_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    ensure_command_root(&options.root, "rerun")?;
    let (input, config) = load_root_input_and_config(&options.root)?;
    let changed_test = normalize_changed_test(&input.root, &options.changed_test)?;
    let inventory = inventory_changed_test_classified_seams_at_with_config(
        &input.root,
        &config,
        &changed_test,
    )?;
    let report = TargetedRerunReport {
        schema_version: "ripr-targeted-rerun-v1",
        state: "current_state_only",
        selector: TargetedRerunSelector {
            changed_test: display_path(&changed_test),
            selected_test_count: inventory.selected_test_count,
            direct_call_names: inventory.direct_call_names,
        },
        cache: TargetedRerunCache {
            file_fact_status: inventory.file_fact_cache.status_label(),
            hits: inventory.file_fact_cache.hits,
            misses: inventory.file_fact_cache.misses,
            corrupt_ignored: inventory.file_fact_cache.corrupt_ignored,
            stores: inventory.file_fact_cache.stores,
            store_errors: inventory.file_fact_cache.store_errors,
        },
        seams: inventory
            .classified
            .iter()
            .map(|entry| TargetedRerunSeam {
                canonical_gap_id: canonical_gap_identity(entry).map(|identity| identity.id),
                seam_id: entry.seam.id().as_str().to_string(),
                file: display_path(entry.seam.file()),
                line: entry.seam.display_line(),
                owner: entry.seam.owner().to_string(),
                static_class: entry.class.as_str().to_string(),
            })
            .collect(),
        authority_boundary: "static evidence only; no before snapshot was supplied, so gap movement is not inferred",
    };
    let rendered = if options.json {
        serde_json::to_string_pretty(&report)
            .map_err(|err| format!("serialize targeted rerun report failed: {err}"))?
    } else {
        render_human(&report)
    };
    if let Some(out) = options.out {
        let out = resolve_output_path(&input.root, &out);
        write_text_file(&out, &rendered)?;
        println!("Wrote {}", out.display());
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn parse_options(args: &[String]) -> Result<RerunOptions, String> {
    let mut root = PathBuf::from(".");
    let mut changed_test = None;
    let mut json = false;
    let mut out = None;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = PathBuf::from(expect_value(args, index, "--root")?);
            }
            "--changed-test" => {
                index += 1;
                if changed_test.is_some() {
                    return Err("rerun accepts exactly one selector".to_string());
                }
                changed_test = Some(PathBuf::from(expect_value(args, index, "--changed-test")?));
            }
            "--json" => json = true,
            "--out" => {
                index += 1;
                out = Some(PathBuf::from(expect_value(args, index, "--out")?));
            }
            "--gap" => {
                return Err(
                    "rerun --gap is not available in this changed-test implementation; use --changed-test"
                        .to_string(),
                );
            }
            other => return Err(format!("unknown rerun argument {other:?}")),
        }
        index += 1;
    }
    let changed_test =
        changed_test.ok_or_else(|| "rerun requires --changed-test <path>".to_string())?;
    Ok(RerunOptions {
        root,
        changed_test,
        json,
        out,
    })
}

fn normalize_changed_test(root: &Path, changed_test: &Path) -> Result<PathBuf, String> {
    if changed_test.is_absolute() {
        return changed_test
            .strip_prefix(root)
            .map(PathBuf::from)
            .map_err(|err| {
                format!(
                    "rerun changed test {} is outside root {}: {err}",
                    changed_test.display(),
                    root.display()
                )
            });
    }
    Ok(changed_test.to_path_buf())
}

fn resolve_output_path(root: &Path, out: &Path) -> PathBuf {
    if out.is_absolute() {
        out.to_path_buf()
    } else {
        root.join(out)
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn write_text_file(path: &Path, rendered: &str) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    std::fs::write(path, rendered).map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn render_human(report: &TargetedRerunReport) -> String {
    let mut lines = vec![
        "State: current_state_only".to_string(),
        format!("Changed test: {}", report.selector.changed_test),
        format!("Selected tests: {}", report.selector.selected_test_count),
        format!(
            "Direct call names: {}",
            report.selector.direct_call_names.join(", ")
        ),
        format!(
            "File-fact cache: {} (hits {}, misses {})",
            report.cache.file_fact_status, report.cache.hits, report.cache.misses
        ),
        "Recomputed seams:".to_string(),
    ];
    lines.extend(report.seams.iter().map(|seam| {
        format!(
            "  - [{}] {}:{} {} ({})",
            seam.static_class, seam.file, seam.line, seam.owner, seam.seam_id
        )
    }));
    lines.push(format!("Boundary: {}", report.authority_boundary));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        TargetedRerunCache, TargetedRerunReport, TargetedRerunSeam, TargetedRerunSelector,
        parse_options, render_human,
    };
    use std::path::{Path, PathBuf};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn rerun_requires_changed_test_selector() {
        assert_eq!(
            parse_options(&[]),
            Err("rerun requires --changed-test <path>".to_string())
        );
    }

    #[test]
    fn rerun_rejects_gap_selector_until_its_ledger_adapter_exists() {
        assert_eq!(
            parse_options(&args(&["--gap", "gap:example"])),
            Err(
                "rerun --gap is not available in this changed-test implementation; use --changed-test"
                    .to_string()
            )
        );
    }

    #[test]
    fn rerun_rejects_multiple_changed_test_selectors() {
        assert_eq!(
            parse_options(&args(&[
                "--changed-test",
                "tests/first.rs",
                "--changed-test",
                "tests/second.rs",
            ])),
            Err("rerun accepts exactly one selector".to_string())
        );
    }

    #[test]
    fn rerun_parses_changed_test_json_output() -> Result<(), String> {
        let options = parse_options(&args(&[
            "--root",
            "workspace",
            "--changed-test",
            "tests/pricing.rs",
            "--json",
            "--out",
            "target/rerun.json",
        ]))?;
        if options.root.as_path() != Path::new("workspace")
            || options.changed_test.as_path() != Path::new("tests/pricing.rs")
            || !options.json
            || options.out != Some(PathBuf::from("target/rerun.json"))
        {
            return Err(format!("unexpected rerun options: {options:?}"));
        }
        Ok(())
    }

    #[test]
    fn human_rerun_report_names_current_state_only_boundary() -> Result<(), String> {
        let report = TargetedRerunReport {
            schema_version: "ripr-targeted-rerun-v1",
            state: "current_state_only",
            selector: TargetedRerunSelector {
                changed_test: "tests/pricing.rs".to_string(),
                selected_test_count: 1,
                direct_call_names: vec!["discounted_total".to_string()],
            },
            cache: TargetedRerunCache {
                file_fact_status: "warm".to_string(),
                hits: 2,
                misses: 0,
                corrupt_ignored: 0,
                stores: 0,
                store_errors: 0,
            },
            seams: vec![TargetedRerunSeam {
                canonical_gap_id: Some("gap:example".to_string()),
                seam_id: "seam:example".to_string(),
                file: "src/lib.rs".to_string(),
                line: 8,
                owner: "pricing::discounted_total".to_string(),
                static_class: "weakly_gripped".to_string(),
            }],
            authority_boundary: "static evidence only; no before snapshot was supplied, so gap movement is not inferred",
        };
        let rendered = render_human(&report);
        for expected in [
            "State: current_state_only",
            "File-fact cache: warm",
            "static evidence only",
        ] {
            if !rendered.contains(expected) {
                return Err(format!("missing {expected:?} from {rendered:?}"));
            }
        }
        Ok(())
    }
}
