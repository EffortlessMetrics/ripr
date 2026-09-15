use std::path::Path;

use crate::{
    TestOracleClass, TestOracleTest, collect_test_oracle_tests_from_roots, json_escape,
    markdown_cell, normalize_path, reports_dir, test_oracle_counts, test_oracle_source_roots,
    write_report_in,
};

pub(crate) use crate::test_efficiency_report_impl as test_efficiency_report;

pub(crate) fn test_oracle_report() -> Result<(), String> {
    let roots = test_oracle_source_roots();
    test_oracle_report_impl_for_roots(&roots, &reports_dir())
}

pub(crate) fn test_oracle_report_impl_for_roots(
    roots: &[&Path],
    report_directory: &Path,
) -> Result<(), String> {
    let tests = collect_test_oracle_tests_from_roots(roots)?;
    write_report_in(
        report_directory,
        "test-oracles.md",
        &test_oracle_report_markdown(&tests),
    )?;
    write_report_in(
        report_directory,
        "test-oracles.json",
        &test_oracle_report_json(&tests),
    )
}

fn test_oracle_report_status(tests: &[TestOracleTest]) -> &'static str {
    if tests.is_empty() {
        return "not_run";
    }
    if tests
        .iter()
        .any(|test| matches!(test.class, TestOracleClass::Weak | TestOracleClass::Smoke))
    {
        "warn"
    } else {
        "pass"
    }
}

fn test_oracle_report_explanation(tests: &[TestOracleTest]) -> Option<&'static str> {
    tests
        .is_empty()
        .then_some("No tests were selected; oracle evidence was not established for this report.")
}

pub(crate) fn test_oracle_report_markdown(tests: &[TestOracleTest]) -> String {
    let counts = test_oracle_counts(tests);
    let bdd_named = tests
        .iter()
        .filter(|test| crate::is_bdd_test_name(&test.name))
        .count();
    let mut body = format!(
        "# ripr test oracle report\n\nStatus: {}\n\nMode: advisory\n\nThis report measures the apparent discriminator strength of `ripr`'s own Rust tests. It does not fail existing debt yet.\n\n## Summary\n\n- Strong: {}\n- Medium: {}\n- Weak: {}\n- Smoke: {}\n- BDD-shaped names: {} / {}\n\n",
        test_oracle_report_status(tests),
        counts.get("strong").copied().unwrap_or(0),
        counts.get("medium").copied().unwrap_or(0),
        counts.get("weak").copied().unwrap_or(0),
        counts.get("smoke").copied().unwrap_or(0),
        bdd_named,
        tests.len(),
    );
    if let Some(explanation) = test_oracle_report_explanation(tests) {
        body.push_str(&format!("Explanation: {explanation}\n\n"));
    }

    body.push_str("## Weak Or Smoke Tests\n\n");
    let weak_or_smoke = tests
        .iter()
        .filter(|test| matches!(test.class, TestOracleClass::Weak | TestOracleClass::Smoke))
        .collect::<Vec<_>>();
    if weak_or_smoke.is_empty() {
        body.push_str("None detected.\n\n");
    } else {
        for test in weak_or_smoke {
            body.push_str(&format!(
                "- `{}`:{} `{}` classified `{}`\n",
                normalize_path(&test.path),
                test.line,
                test.name,
                test.class.as_str()
            ));
            for observation in &test.observations {
                body.push_str(&format!(
                    "  - line {}: `{}` - {}\n",
                    observation.line, observation.pattern, observation.detail
                ));
            }
        }
        body.push('\n');
    }

    body.push_str("## All Tests\n\n| Test | Class | Evidence |\n| --- | --- | --- |\n");
    for test in tests {
        let evidence = test
            .observations
            .iter()
            .map(|observation| format!("{}: {}", observation.line, observation.pattern))
            .collect::<Vec<_>>()
            .join("<br>");
        body.push_str(&format!(
            "| `{}`:{} `{}` | `{}` | {} |\n",
            normalize_path(&test.path),
            test.line,
            markdown_cell(&test.name),
            test.class.as_str(),
            markdown_cell(&evidence)
        ));
    }
    body
}

pub(crate) fn test_oracle_report_json(tests: &[TestOracleTest]) -> String {
    let counts = test_oracle_counts(tests);
    let explanation = test_oracle_report_explanation(tests)
        .map(|value| format!("  \"explanation\": \"{}\",\n", json_escape(value)))
        .unwrap_or_default();
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{}\",\n{}  \"advisory\": true,\n  \"counts\": {{\n    \"strong\": {},\n    \"medium\": {},\n    \"weak\": {},\n    \"smoke\": {}\n  }},\n  \"tests\": [\n",
        test_oracle_report_status(tests),
        explanation,
        counts.get("strong").copied().unwrap_or(0),
        counts.get("medium").copied().unwrap_or(0),
        counts.get("weak").copied().unwrap_or(0),
        counts.get("smoke").copied().unwrap_or(0)
    );
    for (test_index, test) in tests.iter().enumerate() {
        if test_index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!(
            "      \"path\": \"{}\",\n",
            json_escape(&normalize_path(&test.path))
        ));
        body.push_str(&format!(
            "      \"name\": \"{}\",\n",
            json_escape(&test.name)
        ));
        body.push_str(&format!("      \"line\": {},\n", test.line));
        body.push_str(&format!("      \"class\": \"{}\",\n", test.class.as_str()));
        body.push_str("      \"observations\": [\n");
        for (observation_index, observation) in test.observations.iter().enumerate() {
            if observation_index > 0 {
                body.push_str(",\n");
            }
            body.push_str(&format!(
                "        {{\n          \"line\": {},\n          \"class\": \"{}\",\n          \"pattern\": \"{}\",\n          \"detail\": \"{}\"\n        }}",
                observation.line, observation.class.as_str(), json_escape(&observation.pattern),
                json_escape(&observation.detail)
            ));
        }
        body.push_str("\n      ]\n    }");
    }
    body.push_str("\n  ]\n}\n");
    body
}
