mod classifier;
mod diff;
mod probes;
mod rust_index;
mod workspace;

use crate::domain::{Finding, Summary};
use std::path::PathBuf;

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
    let diff_text = diff::load_diff(
        &options.root,
        options.base.as_deref(),
        options.diff_file.as_ref(),
    )?;
    let changed_files = diff::parse_unified_diff(&diff_text);
    let changed_rust_paths = changed_files
        .iter()
        .filter(|file| file.path.extension().and_then(|e| e.to_str()) == Some("rs"))
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let rust_files = workspace::discover_rust_files(&options.root)?;
    let index_files = workspace::select_rust_files_for_mode(
        &rust_files,
        &changed_rust_paths,
        options.mode,
        options.include_unchanged_tests,
    );
    let index = rust_index::build_index(&options.root, &index_files)?;

    let mut findings = Vec::new();
    let mut changed_rust_files = 0usize;

    for changed in changed_files
        .iter()
        .filter(|file| file.path.extension().and_then(|e| e.to_str()) == Some("rs"))
    {
        changed_rust_files += 1;
        let probes = probes::probes_for_file(&options.root, changed, &index);
        for probe in probes {
            findings.push(classifier::classify_probe(&probe, &index));
        }
    }

    findings.sort_by(|a, b| {
        a.probe
            .location
            .file
            .cmp(&b.probe.location.file)
            .then(a.probe.location.line.cmp(&b.probe.location.line))
            .then(a.probe.family.as_str().cmp(b.probe.family.as_str()))
    });

    let mut summary = Summary {
        changed_rust_files,
        probes: findings.len(),
        findings: findings.len(),
        ..Summary::default()
    };
    for finding in &findings {
        match finding.class {
            crate::domain::ExposureClass::Exposed => summary.exposed += 1,
            crate::domain::ExposureClass::WeaklyExposed => summary.weakly_exposed += 1,
            crate::domain::ExposureClass::ReachableUnrevealed => summary.reachable_unrevealed += 1,
            crate::domain::ExposureClass::NoStaticPath => summary.no_static_path += 1,
            crate::domain::ExposureClass::InfectionUnknown => summary.infection_unknown += 1,
            crate::domain::ExposureClass::PropagationUnknown => summary.propagation_unknown += 1,
            crate::domain::ExposureClass::StaticUnknown => summary.static_unknown += 1,
        }
    }

    Ok(AnalysisResult { summary, findings })
}

#[cfg(test)]
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
}
