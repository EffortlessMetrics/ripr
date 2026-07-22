use crate::app::{CHECK_OUTPUT_SCHEMA_VERSION, CheckInput};
use crate::output::json::render_pretty_with_newline;
use serde_json::json;

const DIFF_SCOPE_OVERSIZED_PREFIX: &str = "diff_scope_oversized:";
const DIFF_SCOPE_REPAIR_ROUTE: &str = "analysis/diff-scope-budget";
const REPO_SCOPE_OVERSIZED_PREFIX: &str = "repo_scope_oversized:";
const REPO_SCOPE_REPAIR_ROUTE: &str = "analysis/repo-scope-budget";

pub(crate) fn render_diff_scope_limited_check_json(
    input: &CheckInput,
    error: &str,
) -> Result<Option<String>, String> {
    if !is_diff_scope_oversized(error) && !is_repo_scope_oversized(error) {
        return Ok(None);
    }

    // Same non-consumable envelope for both scope guards (#2109 review):
    // only the scope identity differs.
    let (scope, run_status, basis, repair_route) = if is_repo_scope_oversized(error) {
        (
            "repo",
            "repo_scope_oversized",
            "rust_repo_scope_budget",
            REPO_SCOPE_REPAIR_ROUTE,
        )
    } else {
        (
            "diff",
            "diff_scope_oversized",
            "rust_diff_scope_budget",
            DIFF_SCOPE_REPAIR_ROUTE,
        )
    };

    let mut value = json!({
        "schema_version": CHECK_OUTPUT_SCHEMA_VERSION,
        "tool": "ripr",
        "mode": input.mode.as_str(),
        "root": input.root.display().to_string(),
        "summary": {
            "changed_rust_files": 0,
            "probes": 0,
            "findings": 0,
            "exposed": 0,
            "weakly_exposed": 0,
            "reachable_unrevealed": 0,
            "no_static_path": 0,
            "infection_unknown": 0,
            "propagation_unknown": 0,
            "static_unknown": 0,
            "changed_files_by_language": []
        },
        "findings": [],
        "analysis_scope": {
            "scope": scope,
            "run_status": run_status,
            "basis": basis,
            "downstream_consumable": false,
            "limitation": run_status,
            "repair_route": repair_route
        },
        "run_limitations": [
            {
                "category": run_status,
                "run_status": run_status,
                "basis": basis,
                "downstream_consumable": false,
                "message": error,
                "repair_route": repair_route
            }
        ]
    });

    if let Some(base) = &input.base {
        value["base"] = json!(base);
    }

    render_pretty_with_newline(&value, "limited check").map(Some)
}

fn is_diff_scope_oversized(error: &str) -> bool {
    error.trim_start().starts_with(DIFF_SCOPE_OVERSIZED_PREFIX)
}

fn is_repo_scope_oversized(error: &str) -> bool {
    error.trim_start().starts_with(REPO_SCOPE_OVERSIZED_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Mode, OutputFormat};
    use serde_json::Value;
    use std::path::PathBuf;

    fn input() -> CheckInput {
        CheckInput {
            root: PathBuf::from("."),
            base: Some("origin/main".to_string()),
            diff_file: Some(PathBuf::from("example.diff")),
            mode: Mode::Draft,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            perl_facts_path: None,
            suppression_policy: None,
        }
    }

    #[test]
    fn repo_scope_oversized_renders_the_same_non_consumable_envelope() -> Result<(), String> {
        // #2109 review: JSON consumers must get the named error and repair
        // route for the repo guard too, in the same envelope shape.
        let rendered = render_diff_scope_limited_check_json(
            &input(),
            "repo_scope_oversized: 900 indexed Rust files exceed the RIPR_MAX_REPO_INDEX_FILES limit (800); analysis was not run",
        )?;
        let Some(rendered) = rendered else {
            return Err("expected a limited artifact for the repo guard".to_string());
        };
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse limited artifact: {err}"))?;
        for (pointer, expected) in [
            ("/analysis_scope/scope", "repo"),
            ("/analysis_scope/run_status", "repo_scope_oversized"),
            ("/analysis_scope/basis", "rust_repo_scope_budget"),
            ("/analysis_scope/repair_route", "analysis/repo-scope-budget"),
            ("/run_limitations/0/category", "repo_scope_oversized"),
        ] {
            let actual = value.pointer(pointer).and_then(Value::as_str);
            if actual != Some(expected) {
                return Err(format!("{pointer}: expected {expected}, got {actual:?}"));
            }
        }
        if value.pointer("/analysis_scope/downstream_consumable") != Some(&Value::Bool(false)) {
            return Err("downstream_consumable must be false".to_string());
        }
        Ok(())
    }

    #[test]
    fn limited_artifact_summary_carries_empty_language_breakdown() -> Result<(), String> {
        // #2103 review: the limited artifact must keep the same summary
        // shape as every normal check output at the same schema version.
        let rendered = render_diff_scope_limited_check_json(
            &input(),
            "diff_scope_oversized: 900 indexed Rust files exceed the limit (800)",
        )?;
        let Some(rendered) = rendered else {
            return Err("expected a limited artifact for the oversized error".to_string());
        };
        let value: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("parse limited artifact: {err}"))?;
        let breakdown = value
            .pointer("/summary/changed_files_by_language")
            .and_then(Value::as_array);
        match breakdown {
            Some(entries) if entries.is_empty() => Ok(()),
            other => Err(format!(
                "expected summary.changed_files_by_language to be an empty array, got {other:?}"
            )),
        }
    }

    #[test]
    fn non_budget_error_does_not_render_limited_artifact() -> Result<(), String> {
        let rendered = render_diff_scope_limited_check_json(&input(), "git diff failed")?;

        assert!(
            rendered.is_none(),
            "non-budget error should not render limited JSON"
        );
        Ok(())
    }

    #[test]
    fn budget_error_renders_non_consumable_limited_artifact() -> Result<(), String> {
        let rendered = render_diff_scope_limited_check_json(
            &input(),
            "diff_scope_oversized: 3 changed Rust lines exceed the limit",
        )?
        .ok_or("expected limited artifact")?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse JSON: {err}"))?;

        let cases = [
            (
                &value["schema_version"],
                Value::String(CHECK_OUTPUT_SCHEMA_VERSION.to_string()),
                "schema_version",
            ),
            (
                &value["analysis_scope"]["run_status"],
                Value::String("diff_scope_oversized".to_string()),
                "analysis_scope.run_status",
            ),
            (
                &value["analysis_scope"]["downstream_consumable"],
                Value::Bool(false),
                "analysis_scope.downstream_consumable",
            ),
            (
                &value["run_limitations"][0]["category"],
                Value::String("diff_scope_oversized".to_string()),
                "run_limitations[0].category",
            ),
            (
                &value["run_limitations"][0]["downstream_consumable"],
                Value::Bool(false),
                "run_limitations[0].downstream_consumable",
            ),
        ];
        for (actual, expected, label) in cases {
            assert_eq!(actual, &expected, "unexpected {label}");
        }
        assert_eq!(value["findings"].as_array().map(Vec::len), Some(0));
        Ok(())
    }

    #[test]
    fn budget_error_without_base_omits_base_and_keeps_limitation() -> Result<(), String> {
        let mut check_input = input();
        check_input.base = None;
        let message = "\n  diff_scope_oversized: 4 changed Rust lines exceed the limit";
        let rendered = render_diff_scope_limited_check_json(&check_input, message)?
            .ok_or("expected limited artifact with leading whitespace")?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse JSON: {err}"))?;

        assert!(
            value.get("base").is_none(),
            "base should be omitted when absent: {value}"
        );
        let cases = [
            (
                &value["analysis_scope"]["repair_route"],
                Value::String(DIFF_SCOPE_REPAIR_ROUTE.to_string()),
                "analysis_scope.repair_route",
            ),
            (
                &value["run_limitations"][0]["message"],
                Value::String(message.to_string()),
                "run_limitations[0].message",
            ),
        ];
        for (actual, expected, label) in cases {
            assert_eq!(actual, &expected, "unexpected {label}");
        }
        Ok(())
    }
}
