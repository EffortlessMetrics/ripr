use crate::app::{CHECK_OUTPUT_SCHEMA_VERSION, CheckInput};
use crate::output::json::render_pretty_with_newline;
use serde_json::json;

const DIFF_SCOPE_OVERSIZED_PREFIX: &str = "diff_scope_oversized:";
const DIFF_SCOPE_REPAIR_ROUTE: &str = "analysis/diff-scope-budget";

pub(crate) fn render_diff_scope_limited_check_json(
    input: &CheckInput,
    error: &str,
) -> Result<Option<String>, String> {
    if !is_diff_scope_oversized(error) {
        return Ok(None);
    }

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
            "static_unknown": 0
        },
        "findings": [],
        "analysis_scope": {
            "scope": "diff",
            "run_status": "diff_scope_oversized",
            "basis": "rust_diff_scope_budget",
            "downstream_consumable": false,
            "limitation": "diff_scope_oversized",
            "repair_route": DIFF_SCOPE_REPAIR_ROUTE
        },
        "run_limitations": [
            {
                "category": "diff_scope_oversized",
                "run_status": "diff_scope_oversized",
                "basis": "rust_diff_scope_budget",
                "downstream_consumable": false,
                "message": error,
                "repair_route": DIFF_SCOPE_REPAIR_ROUTE
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
