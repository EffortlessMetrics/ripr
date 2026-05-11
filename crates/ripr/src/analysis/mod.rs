pub(crate) mod canonical_gap;
mod classifier;
mod classify;
mod diff;
mod extract;
mod facts;
mod language;
mod pipeline;
mod probes;
mod rust_index;
mod seam_cache;
mod seam_classification;
mod seam_inventory;
pub(crate) mod seams;
mod sort;
mod summary;
mod syntax;
pub(crate) mod test_grip_evidence;
mod value_resolution;
mod workspace;

pub(crate) use diff::{load_diff, parse_unified_diff};
pub(crate) use seam_classification::{ClassifiedSeam, SeamGripClassCounts};
pub(crate) use seam_inventory::{
    inventory_classified_seams_at_with_config, inventory_seam_grip_class_counts_at_with_config,
    inventory_seams_at,
};
pub(crate) use seams::{RepoSeam, RequiredDiscriminator};

use crate::config::OraclePolicy;
use crate::domain::{Finding, Summary};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisMode {
    Instant,
    Draft,
    Fast,
    Deep,
    Ready,
}

#[derive(Clone, Debug)]
pub struct AnalysisOptions {
    pub root: PathBuf,
    pub base: Option<String>,
    pub diff_file: Option<PathBuf>,
    pub mode: AnalysisMode,
    pub include_unchanged_tests: bool,
}

#[derive(Clone, Debug)]
pub struct AnalysisResult {
    pub summary: Summary,
    pub findings: Vec<Finding>,
}

pub fn run_analysis(options: &AnalysisOptions) -> Result<AnalysisResult, String> {
    run_analysis_with_oracle_policy(options, &OraclePolicy::default())
}

pub(crate) fn run_analysis_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
) -> Result<AnalysisResult, String> {
    pipeline::run_diff_pipeline_with_oracle_policy(options, oracle_policy)
}

pub fn run_repo_analysis(options: &AnalysisOptions) -> Result<AnalysisResult, String> {
    run_repo_analysis_with_oracle_policy(options, &OraclePolicy::default())
}

pub(crate) fn run_repo_analysis_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
) -> Result<AnalysisResult, String> {
    pipeline::run_repo_pipeline_with_oracle_policy(options, oracle_policy)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChangedLineOwner {
    pub(crate) file: PathBuf,
    pub(crate) line: usize,
    pub(crate) owner: String,
}

pub(crate) fn owner_symbols_for_lines(
    root: &Path,
    lines: &[(PathBuf, usize)],
) -> Result<Vec<ChangedLineOwner>, String> {
    let files = lines
        .iter()
        .map(|(file, _)| file.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let index = rust_index::build_index(root, &files)?;
    let mut owners = lines
        .iter()
        .filter_map(|(file, line)| {
            rust_index::find_owner_function(&index, file, *line).map(|function| ChangedLineOwner {
                file: file.clone(),
                line: *line,
                owner: function.id.to_string(),
            })
        })
        .collect::<Vec<_>>();
    owners.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.owner.cmp(&right.owner))
    });
    owners.dedup();
    Ok(owners)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "Test fixture builders use unwrap on fs operations against fresh temp dirs; receipted via policy/no-panic-allowlist.toml entries for crates/ripr/src/analysis/mod.rs."
)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn analyzes_simple_predicate_gap() {
        let root = temp_dir("simple");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("tests/pricing.rs"),
            r#"
#[test]
fn premium_customer_gets_discount() {
    let total = x::price(10000, 100);
    assert!(total > 0);
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("diff.patch"),
            r#"diff --git a/src/lib.rs b/src/lib.rs
index 0000000..1111111 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,3 @@
 pub fn price(amount: i32, threshold: i32) -> i32 {
+    if amount >= threshold { amount - 10 } else { amount }
 }
"#,
        )
        .unwrap();
        let out = run_analysis(&AnalysisOptions {
            root: root.clone(),
            base: None,
            diff_file: Some(root.join("diff.patch")),
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
        })
        .unwrap();
        assert!(!out.findings.is_empty());
        assert!(
            out.findings
                .iter()
                .any(|f| f.class == crate::domain::ExposureClass::WeaklyExposed
                    || f.class == crate::domain::ExposureClass::InfectionUnknown)
        );

        let instant = run_analysis(&AnalysisOptions {
            root: root.clone(),
            base: None,
            diff_file: Some(root.join("diff.patch")),
            mode: AnalysisMode::Instant,
            include_unchanged_tests: true,
        })
        .unwrap();
        assert!(instant.findings.iter().any(|finding| {
            finding.class == crate::domain::ExposureClass::NoStaticPath
                && finding.related_tests.is_empty()
        }));
    }

    #[test]
    fn repo_analysis_finds_predicate_in_production_file() -> Result<(), String> {
        let root = temp_dir("repo_pred");
        fs::create_dir_all(root.join("src"))
            .map_err(|e| format!("failed to create src dir: {e}"))?;
        fs::create_dir_all(root.join("tests"))
            .map_err(|e| format!("failed to create tests dir: {e}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2024'\n",
        )
        .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#,
        )
        .map_err(|e| format!("failed to write src/lib.rs: {e}"))?;
        fs::write(
            root.join("tests/pricing.rs"),
            r#"
#[test]
fn premium_customer_gets_discount() {
    let total = x::price(10000, 100);
    assert!(total > 0);
}
"#,
        )
        .map_err(|e| format!("failed to write tests/pricing.rs: {e}"))?;

        let out = run_repo_analysis(&AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
        })?;

        if out.findings.is_empty() {
            return Err("expected at least one finding from repo analysis".to_string());
        }
        if !out
            .findings
            .iter()
            .any(|f| f.probe.family == crate::domain::ProbeFamily::Predicate)
        {
            return Err("expected at least one Predicate family finding".to_string());
        }
        Ok(())
    }

    #[test]
    fn owner_symbols_for_lines_names_containing_function() -> Result<(), String> {
        let root = temp_dir("owner_lines");
        fs::create_dir_all(root.join("src"))
            .map_err(|e| format!("failed to create src dir: {e}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    let discount = 10;
    if amount >= threshold {
        amount - discount
    } else {
        amount
    }
}

pub fn unrelated() -> i32 {
    0
}
"#,
        )
        .map_err(|e| format!("failed to write src/lib.rs: {e}"))?;

        let owners = owner_symbols_for_lines(
            &root,
            &[
                (PathBuf::from("src/lib.rs"), 3),
                (PathBuf::from("src/lib.rs"), 11),
            ],
        )?;

        if !owners
            .iter()
            .any(|owner| owner.line == 3 && owner.owner.ends_with("discounted_total"))
        {
            return Err(format!("expected discounted_total owner, got {owners:?}"));
        }
        if !owners
            .iter()
            .any(|owner| owner.line == 11 && owner.owner.ends_with("unrelated"))
        {
            return Err(format!("expected unrelated owner, got {owners:?}"));
        }
        Ok(())
    }

    #[test]
    fn repo_analysis_excludes_test_files_from_probe_seed() -> Result<(), String> {
        let root = temp_dir("repo_exclude_tests");
        fs::create_dir_all(root.join("src"))
            .map_err(|e| format!("failed to create src dir: {e}"))?;
        fs::create_dir_all(root.join("tests"))
            .map_err(|e| format!("failed to create tests dir: {e}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2024'\n",
        )
        .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn dummy() {
}
"#,
        )
        .map_err(|e| format!("failed to write src/lib.rs: {e}"))?;
        fs::write(
            root.join("tests/test_file.rs"),
            r#"
#[test]
fn test_with_predicate() {
    let x = 5;
    if x > 3 {
        assert!(true);
    }
}
"#,
        )
        .map_err(|e| format!("failed to write tests/test_file.rs: {e}"))?;

        let out = run_repo_analysis(&AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
        })?;

        for finding in &out.findings {
            let file_str = finding.probe.location.file.to_string_lossy().to_lowercase();
            if file_str.contains("test") || file_str.contains("tests") {
                return Err(format!(
                    "expected no findings from test files, but found one at {}",
                    file_str
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn empty_diff_yields_zero_diff_findings_but_repo_has_findings() -> Result<(), String> {
        let root = temp_dir("repo_vs_diff");
        fs::create_dir_all(root.join("src"))
            .map_err(|e| format!("failed to create src dir: {e}"))?;
        fs::create_dir_all(root.join("tests"))
            .map_err(|e| format!("failed to create tests dir: {e}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2024'\n",
        )
        .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#,
        )
        .map_err(|e| format!("failed to write src/lib.rs: {e}"))?;
        fs::write(
            root.join("tests/pricing.rs"),
            r#"
#[test]
fn premium_customer_gets_discount() {
    let total = x::price(10000, 100);
    assert!(total > 0);
}
"#,
        )
        .map_err(|e| format!("failed to write tests/pricing.rs: {e}"))?;
        fs::write(
            root.join("empty.patch"),
            r#"diff --git a/src/lib.rs b/src/lib.rs
index 0000000..1111111 100644
--- a/src/lib.rs
+++ b/src/lib.rs
"#,
        )
        .map_err(|e| format!("failed to write empty.patch: {e}"))?;

        let diff_out = run_analysis(&AnalysisOptions {
            root: root.clone(),
            base: None,
            diff_file: Some(root.join("empty.patch")),
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
        })?;

        if !diff_out.findings.is_empty() {
            return Err("expected zero findings from empty diff".to_string());
        }

        let repo_out = run_repo_analysis(&AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
        })?;

        if repo_out.findings.is_empty() {
            return Err("expected at least one finding from repo analysis".to_string());
        }
        Ok(())
    }
}
