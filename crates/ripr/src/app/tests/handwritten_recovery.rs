//! Cross-layer handwritten-source recovery uses real analysis and output producers.
use crate::app::{CheckInput, check_workspace_with_config};
use crate::config::{RiprConfig, tests_only_parse};
use crate::domain::ExposureClass;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);
impl Fixture {
    fn new(label: &str) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-handwritten-{label}-{stamp}"));
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self(root))
    }
    fn write(&self, relative: &str, text: &str) -> Result<(), String> {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, text).map_err(|e| e.to_string())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn handwritten_files_diff_recovery_indexes_effective_test_without_renaming() -> Result<(), String> {
    let fixture = Fixture::new("diff")?;
    fixture.write(
        "Cargo.toml",
        "[package]\nname='generated_policy_control'\nversion='0.1.0'\nedition='2024'\n",
    )?;
    fixture.write(
        "src/lib.rs",
        "pub fn weight(input: i32) -> i32 {\n    input * 3\n}\n",
    )?;
    let test_path = "tests/generated_weight_tests.rs";
    let effective = "use generated_policy_control::weight;\n\n#[test]\nfn checks_weight() {\n    assert_eq!(weight(4), 12);\n}\n";
    let no_assert = "use generated_policy_control::weight;\n\n#[test]\nfn checks_weight() {\n    let _ = weight(4);\n}\n";
    let config = tests_only_parse(
        "[languages.rust]\nhandwritten_files = ['tests/generated_weight_tests.rs']\n",
    )?;
    let nonmatching =
        tests_only_parse("[languages.rust]\nhandwritten_files = ['tests/generated_other.rs']\n")?;
    let default = RiprConfig::default();
    for (test_text, active_config, expected, has_related) in [
        (effective, &default, ExposureClass::NoStaticPath, false),
        (effective, &nonmatching, ExposureClass::NoStaticPath, false),
        (effective, &config, ExposureClass::Exposed, true),
        (no_assert, &config, ExposureClass::ReachableUnrevealed, true),
        (effective, &default, ExposureClass::NoStaticPath, false),
    ] {
        fixture.write(test_path, test_text)?;
        let patch = format!(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n pub fn weight(input: i32) -> i32 {{\n-    input * 2\n+    input * 3\n }}\ndiff --git a/{test_path} b/{test_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{test_path}\n@@ -0,0 +1,6 @@\n{}",
            test_text
                .lines()
                .map(|line| format!("+{line}\n"))
                .collect::<String>()
        );
        fixture.write("diff.patch", &patch)?;
        let output = check_workspace_with_config(
            CheckInput {
                root: fixture.0.clone(),
                diff_file: Some(fixture.0.join("diff.patch")),
                ..CheckInput::default()
            },
            active_config,
        )?;
        assert_eq!(output.findings.len(), 1, "one changed return-value subject");
        let finding = &output.findings[0];
        assert_eq!(finding.class, expected);
        assert_eq!(!finding.related_tests.is_empty(), has_related);
        let outcome = output
            .analysis_outcome
            .as_ref()
            .ok_or("diff outcome required")?;
        assert_eq!(outcome.kind.is_complete(), has_related);
        if !has_related {
            assert!(
                outcome
                    .limitations
                    .iter()
                    .any(|limitation| { limitation.recovery.detail.contains("handwritten_files") })
            );
        }
    }
    fixture.write(".cargo-checksum.json", "{}")?;
    let output = check_workspace_with_config(
        CheckInput {
            root: fixture.0.clone(),
            diff_file: Some(fixture.0.join("diff.patch")),
            ..CheckInput::default()
        },
        &config,
    )?;
    assert!(output.findings.is_empty());
    let rendered = crate::output::json::render(&output);
    assert!(rendered.contains("handwritten_files cannot override"));
    assert!(!rendered.contains("declare exact repository-relative paths"));
    let envelope: serde_json::Value =
        serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
    assert_eq!(
        envelope
            .pointer("/analysis_outcome/analysis_complete")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    Ok(())
}

#[test]
fn handwritten_files_recovery_serializes_bounded_ascii_and_unicode_paths() -> Result<(), String> {
    for (label, directory, name) in [
        ("ascii", "a".repeat(60), "b".repeat(60)),
        ("unicode", "é".repeat(45), "界".repeat(45)),
    ] {
        let fixture = Fixture::new(label)?;
        fixture.write(
            "Cargo.toml",
            "[package]\nname='bounded_recovery'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        fixture.write("src/lib.rs", "pub fn ordinary() -> i32 { 1 }\n")?;
        let mut patch = String::new();
        for index in 0..3 {
            let path = format!("src/{directory}/generated_{index}_{name}.rs");
            fixture.write(&path, "pub fn generated_value() -> i32 { 3 }\n")?;
            patch.push_str(&format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+pub fn generated_value() -> i32 {{ 3 }}\n"));
        }
        fixture.write("diff.patch", &patch)?;
        let output = check_workspace_with_config(
            CheckInput {
                root: fixture.0.clone(),
                diff_file: Some(fixture.0.join("diff.patch")),
                ..CheckInput::default()
            },
            &RiprConfig::default(),
        )?;
        let outcome = output
            .analysis_outcome
            .as_ref()
            .ok_or("generated-source outcome required")?;
        assert!(!outcome.kind.is_complete());
        assert_eq!(outcome.counts.changed_file_count, 3);
        let recovery = outcome
            .limitations
            .iter()
            .find(|item| item.recovery.detail.contains("handwritten_files"))
            .ok_or("scoped inclusion recovery required")?;
        assert_eq!(recovery.affected_items, Some(3));
        assert!(recovery.recovery.detail.chars().count() <= 512);
        assert!(
            recovery
                .recovery
                .detail
                .contains("declare exact repository-relative paths")
        );
        assert!(
            recovery
                .recovery
                .detail
                .contains("vendor markers remain excluded")
        );
        let rendered = crate::output::json::render(&output);
        let envelope: serde_json::Value =
            serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
        assert_eq!(
            envelope
                .pointer("/analysis_outcome/analysis_complete")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert!(crate::output::human::render(&output).contains("handwritten_files"));
    }
    Ok(())
}
