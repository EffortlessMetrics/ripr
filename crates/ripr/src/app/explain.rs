use super::CheckInput;
use super::check_workspace_with_config;
use super::selector::select_finding;
use crate::config::RiprConfig;
use crate::output;
use std::path::Path;

/// Computes findings and renders a single selected finding in human format.
///
/// The selector can be either a finding identifier (for example
/// `probe:path_to_file.rs:42:family`) or a `file:line` location.
pub fn explain_finding(root: &Path, selector: &str) -> Result<String, String> {
    explain_finding_with_input(
        CheckInput {
            root: root.to_path_buf(),
            ..CheckInput::default()
        },
        selector,
    )
}

/// Like [`explain_finding`] but allows overriding the full check input.
pub fn explain_finding_with_input(input: CheckInput, selector: &str) -> Result<String, String> {
    explain_finding_with_config(input, selector, &RiprConfig::default())
}

pub(crate) fn explain_finding_with_config(
    input: CheckInput,
    selector: &str,
    config: &RiprConfig,
) -> Result<String, String> {
    let output = check_workspace_with_config(input, config)?;
    match select_finding(&output.findings, selector) {
        Some(finding) => Ok(output::human::render_finding_with_config(finding, config)),
        None => Err(format!("no finding matched {selector:?}")),
    }
}

/// Like [`explain_finding_with_config`] but loads the finding set from a
/// previously written check artifact (`--from`, RIPR-SPEC-0140) instead of
/// re-running the pipeline. The artifact identity gate is fail-closed; on a
/// verified hit, selection and rendering are identical to the fresh path, so
/// the rendered output is byte-identical to a recomputed run given the same
/// render options. `asserted_base` is an explicitly passed `--base`, verified
/// against the recording rather than used as an override.
pub(crate) fn explain_finding_from_artifact(
    input: CheckInput,
    selector: &str,
    config: &RiprConfig,
    artifact_path: &Path,
    asserted_base: Option<&str>,
) -> Result<String, String> {
    let findings = super::check_artifact::load_findings_for_reuse(
        artifact_path,
        &input,
        config,
        asserted_base,
    )?;
    match select_finding(&findings, selector) {
        Some(finding) => Ok(output::human::render_finding_with_config(finding, config)),
        None => Err(format!("no finding matched {selector:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Mode;
    use std::path::PathBuf;

    fn sample_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sample")
    }

    fn sample_diff_input() -> CheckInput {
        let root = sample_root();
        CheckInput {
            root: root.clone(),
            diff_file: Some(root.join("example.diff")),
            mode: Mode::Draft,
            ..CheckInput::default()
        }
    }

    fn fixture_diff_input(name: &str) -> CheckInput {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures")
            .join(name);
        CheckInput {
            root: fixture.join("input"),
            diff_file: Some(fixture.join("diff.patch")),
            mode: Mode::Draft,
            ..CheckInput::default()
        }
    }

    fn require_contains(rendered: &str, needle: &str) -> Result<(), String> {
        if rendered.contains(needle) {
            Ok(())
        } else {
            Err(format!(
                "expected rendered explain output to contain {needle:?}"
            ))
        }
    }

    fn require_not_contains(rendered: &str, needle: &str) -> Result<(), String> {
        if rendered.contains(needle) {
            Err(format!(
                "expected rendered explain output to omit {needle:?}"
            ))
        } else {
            Ok(())
        }
    }

    fn explain_fixture(name: &str, selector: &str) -> Result<String, String> {
        let mut input = fixture_diff_input(name);
        let config = crate::config::load_for_root(&input.root)?;
        crate::config::apply_to_check_input(
            &mut input,
            &config,
            crate::config::CheckInputExplicit::default(),
        );
        explain_finding_with_config(input, selector, &config)
    }

    #[test]
    fn explain_finding_with_input_renders_selected_finding() -> Result<(), String> {
        let rendered = explain_finding_with_input(
            sample_diff_input(),
            "probe:crates_ripr_examples_sample_src_lib.rs:error_path:a776c683",
        )?;

        assert!(rendered.contains("Static exposure"));
        assert!(rendered.contains("no_static_path"));
        assert!(rendered.contains("InvoiceError::InvalidCurrency"));
        Ok(())
    }

    #[test]
    fn explain_finding_renders_typescript_actionable_packet_field_note() -> Result<(), String> {
        let rendered = explain_fixture(
            "ts_repair_packet_complete",
            "probe:src_discount.ts:typescript_preview:2396aec1",
        )?;

        require_contains(&rendered, "TypeScript repair packet (advisory)")?;
        require_contains(&rendered, "repair packet ready: true")?;
        require_contains(
            &rendered,
            "canonical gap: gap:typescript:typescript_preview:2396aec1",
        )?;
        require_contains(&rendered, "edit surface: tests/discount.test.ts")?;
        require_contains(&rendered, "verify: jest tests/discount.test.ts")?;
        require_contains(&rendered, "receipt: ripr outcome ")?;
        require_contains(&rendered, "authority: preview_advisory_only")?;
        require_not_contains(&rendered, "status: not actionable")?;
        Ok(())
    }

    #[test]
    fn explain_finding_renders_typescript_blocked_packet_limitation() -> Result<(), String> {
        let rendered = explain_fixture(
            "ts_static_limit",
            "probe:src_cache.ts:typescript_preview:bac50022",
        )?;

        require_contains(&rendered, "TypeScript repair packet (advisory)")?;
        require_contains(&rendered, "repair packet ready: false")?;
        require_contains(&rendered, "status: not actionable")?;
        require_contains(
            &rendered,
            "limitation: static limit `dynamic_dispatch` prevents bounded TypeScript repair guidance",
        )?;
        require_contains(
            &rendered,
            "next capability needed: resolve the named static limit and re-run TypeScript preview evidence extraction",
        )?;
        require_not_contains(
            &rendered,
            "canonical gap: gap:typescript:typescript_preview:bac50022",
        )?;
        Ok(())
    }

    #[test]
    fn explain_finding_public_wrapper_reports_invalid_root() {
        let result = explain_finding(Path::new("missing-ripr-root-for-explain"), "probe:missing");

        assert!(
            result
                .err()
                .is_some_and(|err| err.contains("failed to run git diff"))
        );
    }
}
