//! Discriminator narration must follow the final classification, not oracle rank.

use super::super::owners_tests::{extract_owners, extract_tests};
use super::classify_change_with_old;
use crate::domain::{ExposureClass, Finding, StageState};
use std::path::Path;

/// Exercise the existing source-fact producer and final classifier together.
fn classify_case(
    source: &str,
    tests: &str,
    line: usize,
    before: &str,
    after: &str,
) -> Result<Finding, String> {
    let file = Path::new("src/subject.py");
    let owners = extract_owners(file, source);
    let tests = extract_tests(Path::new("tests/test_subject.py"), tests);
    classify_change_with_old(file, line, after, Some(before), &owners, &tests)
        .ok_or_else(|| "behavioral fixture must produce a finding".to_string())
}

/// Keep the exact non-credit wording separate from the positive control below.
fn assert_strong_oracle_is_not_discrimination(finding: &Finding) {
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
    assert_eq!(
        finding.ripr.reveal.discriminate.summary,
        "Related Python test uses a `exact_value` oracle, but static evidence does not establish discrimination of the changed behavior."
    );
}

/// Explicit positional and keyword arguments both bypass the changed default.
#[test]
fn overridden_default_does_not_get_a_positive_discriminator_summary() -> Result<(), String> {
    let source =
        "def render(name, verbose=True):\n    return f\"[debug] {name}\" if verbose else name\n";
    for invocation in ["render(\"Sam\", verbose=False)", "render(\"Sam\", False)"] {
        let tests = format!(
            "from src.subject import render\n\ndef test_render():\n    assert {invocation} == \"Sam\"\n"
        );
        let finding = classify_case(
            source,
            &tests,
            1,
            "def render(name, verbose=False):",
            "def render(name, verbose=True):",
        )?;
        assert_strong_oracle_is_not_discrimination(&finding);
        assert!(
            finding
                .activation
                .missing_discriminators
                .iter()
                .any(|missing| missing.value == "call `render` without `verbose`"),
            "the existing omission repair must survive the narration fix"
        );
    }
    Ok(())
}

/// Omitting the parameter reaches the changed default under an exact observer.
#[test]
fn exercised_default_retains_its_positive_discriminator_summary() -> Result<(), String> {
    let finding = classify_case(
        "def render(name, verbose=True):\n    return f\"[debug] {name}\" if verbose else name\n",
        "from src.subject import render\n\ndef test_render():\n    assert render(\"Sam\") == \"[debug] Sam\"\n",
        1,
        "def render(name, verbose=False):",
        "def render(name, verbose=True):",
    )?;
    assert_eq!(finding.class, ExposureClass::Exposed);
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Yes);
    assert_eq!(
        finding.ripr.reveal.discriminate.summary,
        "Related Python test uses a `exact_value` oracle; static evidence suggests the changed behavior is discriminated."
    );
    Ok(())
}

/// Observing the unchanged input is not observing the changed return value.
#[test]
fn orthogonal_strong_oracle_does_not_get_a_positive_discriminator_summary() -> Result<(), String> {
    let finding = classify_case(
        "def next_value(count):\n    return count - 1\n",
        "from src.subject import next_value\n\ndef test_input():\n    count = 5\n    result = next_value(count)\n    assert count == 5\n    assert result > 0\n",
        2,
        "    return count + 1",
        "    return count - 1",
    )?;
    assert_strong_oracle_is_not_discrimination(&finding);
    Ok(())
}

/// An exact normal-path result does not discriminate a changed raised exception.
#[test]
fn normal_path_oracle_does_not_credit_the_changed_error_path_summary() -> Result<(), String> {
    let finding = classify_case(
        "def parse(text):\n    if not text:\n        raise KeyError(\"empty\")\n    return int(text)\n",
        "from src.subject import parse\n\ndef test_parse():\n    assert parse(\"42\") == 42\n",
        3,
        "        raise ValueError(\"empty\")",
        "        raise KeyError(\"empty\")",
    )?;
    assert_strong_oracle_is_not_discrimination(&finding);
    Ok(())
}

/// A named static limitation remains the first authority even with a strong test.
#[test]
fn static_limit_keeps_its_named_discriminator_summary() -> Result<(), String> {
    let finding = classify_case(
        "def call_named(client, name):\n    return getattr(client, name)()\n",
        "from src.subject import call_named\n\ndef test_call():\n    assert call_named(client, \"total\") == 10\n",
        2,
        "    return client.total()",
        "    return getattr(client, name)()",
    )?;
    assert_eq!(finding.class, ExposureClass::StaticUnknown);
    assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
    assert_eq!(
        finding.ripr.reveal.discriminate.summary,
        "Static limit `dynamic_dispatch` prevents a safe Python discriminator claim."
    );
    Ok(())
}
