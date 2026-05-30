use crate::domain::{ExposureClass, Finding, LanguageId, ProbeFamily, RelatedTest};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PythonRepairCard {
    pub(crate) card_version: String,
    pub(crate) source: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) language: String,
    pub(crate) language_status: String,
    pub(crate) authority_boundary: String,
    pub(crate) changed_owner: String,
    pub(crate) changed_behavior: String,
    pub(crate) current_test_evidence: String,
    pub(crate) missing_discriminator: String,
    pub(crate) recommended_test_shape: String,
    pub(crate) suggested_assertion: String,
    pub(crate) suggested_test_file: String,
    pub(crate) suggested_test_name: String,
    pub(crate) suggested_test_node_id: Option<String>,
    pub(crate) verify_command: String,
    pub(crate) verify_command_confidence: String,
    pub(crate) receipt_command: Option<String>,
    pub(crate) receipt_status: String,
    pub(crate) receipt_guidance: String,
    pub(crate) stop_conditions: Vec<String>,
    pub(crate) limits: Vec<String>,
}

pub(crate) fn python_repair_card(finding: &Finding) -> Option<PythonRepairCard> {
    if finding.language != Some(LanguageId::Python) || finding.class != ExposureClass::WeaklyExposed
    {
        return None;
    }

    let gap = finding.canonical_gap.as_ref()?;
    let missing_discriminator = finding
        .activation
        .missing_discriminators
        .first()?
        .value
        .clone();
    let suggested_test_file = evidence_value(finding, "suggested_test_file: ")?.to_string();
    let suggested_test_name = evidence_value(finding, "suggested_test_name: ")?.to_string();
    let suggested_test_node_id =
        evidence_value(finding, "suggested_test_node_id: ").map(ToString::to_string);
    let verify_command = evidence_value(finding, "suggested_verify_command: ")?.to_string();
    let verify_command_confidence =
        evidence_value(finding, "suggested_verify_command_confidence: ")?.to_string();
    let related_test = strongest_related_test(finding)?;

    Some(PythonRepairCard {
        card_version: "python_repair_card.v1".to_string(),
        source: "check_python_preview".to_string(),
        canonical_gap_id: gap.id.clone(),
        language: "python".to_string(),
        language_status: "preview".to_string(),
        authority_boundary: "preview_advisory_only".to_string(),
        changed_owner: gap.owner.clone(),
        changed_behavior: changed_behavior(finding, gap.behavior_kind.as_str()),
        current_test_evidence: current_test_evidence(related_test),
        missing_discriminator: missing_discriminator.clone(),
        recommended_test_shape: recommended_test_shape(
            &finding.probe.family,
            &missing_discriminator,
            &verify_command,
        ),
        suggested_assertion: suggested_assertion(
            &finding.probe.family,
            &missing_discriminator,
            &verify_command,
        ),
        suggested_test_file,
        suggested_test_name,
        suggested_test_node_id,
        verify_command,
        verify_command_confidence,
        receipt_command: None,
        receipt_status: "unavailable_until_python_gap_ledger".to_string(),
        receipt_guidance: receipt_guidance(),
        stop_conditions: stop_conditions(),
        limits: limits(),
    })
}

fn changed_behavior(finding: &Finding, behavior_kind: &str) -> String {
    let expression = finding
        .probe
        .after
        .as_deref()
        .unwrap_or(finding.probe.expression.as_str())
        .trim();
    format!(
        "{behavior_kind} changed at {}:{}: `{expression}`",
        finding.probe.location.file.display(),
        finding.probe.location.line
    )
}

fn current_test_evidence(test: &RelatedTest) -> String {
    let oracle = test
        .oracle
        .as_deref()
        .map(|value| format!(": {value}"))
        .unwrap_or_default();
    format!(
        "{}:{} {} currently has oracle_strength={}, oracle_kind={}{}",
        test.file.display(),
        test.line,
        test.name,
        test.oracle_strength.as_str(),
        test.oracle_kind.as_str(),
        oracle
    )
}

fn recommended_test_shape(
    family: &ProbeFamily,
    missing_discriminator: &str,
    verify_command: &str,
) -> String {
    let framework = framework_label(verify_command);
    match family {
        ProbeFamily::Predicate => format!(
            "Add or strengthen a {framework} boundary assertion for `{missing_discriminator}`."
        ),
        ProbeFamily::ReturnValue => format!(
            "Add or strengthen a {framework} exact return-value assertion for `{missing_discriminator}`."
        ),
        ProbeFamily::ErrorPath => format!(
            "Add or strengthen a {framework} exception assertion for `{missing_discriminator}`."
        ),
        ProbeFamily::FieldConstruction => format!(
            "Add or strengthen a {framework} field/object assertion for `{missing_discriminator}`."
        ),
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => format!(
            "Add or strengthen a {framework} output/log/call-effect assertion for `{missing_discriminator}`."
        ),
        _ => format!(
            "Add or strengthen a {framework} focused assertion for `{missing_discriminator}`."
        ),
    }
}

fn suggested_assertion(
    family: &ProbeFamily,
    missing_discriminator: &str,
    verify_command: &str,
) -> String {
    match family {
        ProbeFamily::Predicate => {
            format!("Assert the owner result or effect at the boundary `{missing_discriminator}`.")
        }
        ProbeFamily::ReturnValue => {
            "Assert the returned value equals the expected value for the changed inputs."
                .to_string()
        }
        ProbeFamily::ErrorPath if verify_command.starts_with("python -m unittest ") => {
            unittest_exception_assertion(missing_discriminator)
        }
        ProbeFamily::ErrorPath => pytest_exception_assertion(missing_discriminator),
        ProbeFamily::FieldConstruction => {
            format!("Assert the returned object or field satisfies `{missing_discriminator}`.")
        }
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            format!(
                "Assert the changed output, log text, or call effect for `{missing_discriminator}`."
            )
        }
        _ => format!("Assert the changed behavior for `{missing_discriminator}`."),
    }
}

fn pytest_exception_assertion(missing_discriminator: &str) -> String {
    if let Some((exception, message)) = parse_exception_discriminator(missing_discriminator) {
        return format!("with pytest.raises({exception}, match={message:?}): ...");
    }
    "with pytest.raises(<expected exception>): ...".to_string()
}

fn unittest_exception_assertion(missing_discriminator: &str) -> String {
    if let Some((exception, message)) = parse_exception_discriminator(missing_discriminator) {
        return format!("with self.assertRaisesRegex({exception}, {message:?}): ...");
    }
    "with self.assertRaises(<expected exception>): ...".to_string()
}

fn parse_exception_discriminator(value: &str) -> Option<(&str, String)> {
    let rest = value.strip_prefix("raises ")?;
    let (exception, message) = rest.split_once(" matching ")?;
    let message = message.trim().trim_matches('"').to_string();
    if exception.trim().is_empty() || message.is_empty() {
        return None;
    }
    Some((exception.trim(), message))
}

fn framework_label(verify_command: &str) -> &'static str {
    if verify_command.starts_with("python -m unittest ") {
        "unittest"
    } else if verify_command.starts_with("pytest ") {
        "pytest"
    } else {
        "Python"
    }
}

fn stop_conditions() -> Vec<String> {
    vec![
        "Stop if imports, fixtures, or test setup cannot call the changed owner.".to_string(),
        "Stop if the expected value for the missing discriminator is ambiguous.".to_string(),
        "Stop if adding the test appears to require a production-code edit.".to_string(),
    ]
}

fn limits() -> Vec<String> {
    vec![
        "Syntax-first Python preview evidence only.".to_string(),
        "No source edits, generated tests, mutation execution, provider calls, or gate authority."
            .to_string(),
        "Verify success alone is not a gap-closure receipt.".to_string(),
    ]
}

fn receipt_guidance() -> String {
    "Save this `ripr check --format json` report, then run `ripr first-pr --check-output <check.json>` or `ripr reports gap-ledger --check-output <check.json>` to materialize a gap ledger with a concrete receipt command.".to_string()
}

fn strongest_related_test(finding: &Finding) -> Option<&RelatedTest> {
    finding
        .related_tests
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
}

fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix(prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{parse_exception_discriminator, pytest_exception_assertion};

    #[test]
    fn exception_assertion_uses_matching_message_when_available() {
        assert_eq!(
            pytest_exception_assertion("raises ValueError matching \"positive required\""),
            "with pytest.raises(ValueError, match=\"positive required\"): ..."
        );
    }

    #[test]
    fn exception_discriminator_parse_rejects_non_exception_text() {
        assert!(parse_exception_discriminator("amount == threshold").is_none());
    }
}
