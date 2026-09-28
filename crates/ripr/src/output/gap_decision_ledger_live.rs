use crate::output::gap_decision_ledger::{self, GapRecord};

/// Parsed gap-ledger records plus the producer provenance needed to decide
/// whether those persisted records still describe the selected checkout.
#[derive(Clone, Debug)]
pub(crate) struct ParsedGapRecordSourceWithProvenance {
    pub(crate) root: Option<String>,
    pub(crate) source_kind: Option<String>,
    pub(crate) records_path: Option<String>,
    pub(crate) source_identity_error: Option<String>,
    pub(crate) records: Vec<GapRecord>,
}

pub(crate) fn parse_gap_record_source_with_provenance_json(
    contents: &str,
) -> Result<ParsedGapRecordSourceWithProvenance, String> {
    let parsed = gap_decision_ledger::parse_gap_record_source_json(contents)?;
    let value: serde_json::Value =
        serde_json::from_str(contents).map_err(|error| format!("invalid JSON: {error}"))?;
    let inputs = value.get("inputs").and_then(serde_json::Value::as_object);

    let (source_kind, source_kind_error) = provenance_string(inputs, "source_kind");
    let (records_path, records_path_error) = provenance_string(inputs, "records");
    let source_identity_error = source_kind_error.or(records_path_error);

    Ok(ParsedGapRecordSourceWithProvenance {
        root: parsed.root,
        source_kind,
        records_path,
        source_identity_error,
        records: parsed.records,
    })
}

fn provenance_string(
    inputs: Option<&serde_json::Map<String, serde_json::Value>>,
    field: &str,
) -> (Option<String>, Option<String>) {
    let Some(inputs) = inputs else {
        return (
            None,
            Some("gap ledger is missing producer provenance under inputs".to_string()),
        );
    };
    let Some(value) = inputs.get(field) else {
        return (None, Some(format!("gap ledger inputs.{field} is missing")));
    };
    let Some(value) = value.as_str() else {
        return (
            None,
            Some(format!("gap ledger inputs.{field} must be a string")),
        );
    };
    let value = value.trim();
    if value.is_empty() {
        return (
            None,
            Some(format!("gap ledger inputs.{field} must be nonblank")),
        );
    }
    (Some(value.to_string()), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_preserves_source_kind_and_records_path() -> Result<(), String> {
        let source = parse_gap_record_source_with_provenance_json(
            r#"{
                "root": ".",
                "generated_at": "test",
                "inputs": {
                    "source_kind": "repo_exposure",
                    "records": "target/ripr/reports/repo-exposure.json"
                },
                "records": []
            }"#,
        )?;
        assert_eq!(source.source_kind.as_deref(), Some("repo_exposure"));
        assert_eq!(
            source.records_path.as_deref(),
            Some("target/ripr/reports/repo-exposure.json")
        );
        assert_eq!(source.source_identity_error, None);
        Ok(())
    }

    #[test]
    fn legacy_ledger_keeps_missing_provenance_explicit() -> Result<(), String> {
        let source = parse_gap_record_source_with_provenance_json(r#"{"root":".","records":[]}"#)?;
        assert_eq!(source.source_kind, None);
        assert_eq!(source.records_path, None);
        assert!(
            source
                .source_identity_error
                .as_deref()
                .is_some_and(|reason| reason.contains("missing producer provenance"))
        );
        Ok(())
    }
}
