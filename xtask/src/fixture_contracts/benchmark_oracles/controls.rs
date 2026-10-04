//! Historical semantic controls share the corpus owner, not its static denominator.

use std::collections::BTreeMap;

use super::*;

pub(in crate::fixture_contracts) fn append_controls_disclosure(
    root: &Path,
    corpus: &Value,
    mut disclosure: crate::PolicyDisclosure,
    violations: &mut Vec<String>,
) -> crate::PolicyDisclosure {
    let Some(value) = corpus.get("semantic_oracle_controls") else {
        return disclosure;
    };
    let Some(controls) = value.as_array() else {
        violations.push("semantic_oracle_controls must be an array".to_string());
        disclosure.items.push(
            "Historical semantic controls: NOT_ESTABLISHED (collection unavailable)".to_string(),
        );
        return disclosure;
    };
    let mut seen = BTreeSet::new();
    let mut subjects = BTreeMap::new();
    let mut case_roles: BTreeMap<&str, BTreeSet<(&str, &str)>> = BTreeMap::new();
    let mut rejected_cases = BTreeSet::new();
    let (mut valid, mut invalid, mut rejected, mut local, mut external) = (0, 0, 0, 0, 0);
    let mut details = Vec::new();
    for control in controls {
        let id = control["id"].as_str().unwrap_or("unknown");
        let variant = control["semantic_oracle"]["variant"]
            .as_str()
            .unwrap_or("unknown");
        let declared_id = control["id"].as_str().filter(|id| !id.trim().is_empty());
        if let Some(id) = declared_id {
            let _ = case_roles.entry(id).or_default();
        }
        let outcome = validate_control(root, control, &mut seen);
        match outcome {
            Ok((status, custody)) => {
                if status == "valid" {
                    valid += 1;
                } else {
                    invalid += 1;
                }
                let _ = case_roles.entry(id).or_default().insert((variant, status));
                let oracle = &control["semantic_oracle"];
                let subject = serde_json::json!({"answer_key": oracle["answer_key"], "native_pairing": oracle["native_pairing"], "observed_static": control["observed_static"]});
                if let Some(previous) = subjects.get(id) {
                    if previous != &subject {
                        let _ = rejected_cases.insert(id);
                        violations.push(format!("Historical semantic case {id}: same-case controls disagree on key, pairing or historical static subject"));
                    }
                } else {
                    let _ = subjects.insert(id, subject);
                }
                local += custody.local;
                external += custody.external;
                details.push(format!("Historical control {id}/{variant}: reviewed expected-behavior declaration {status}; retained historical static capture checked, without a normative static classification."));
            }
            Err(error) => {
                rejected += 1;
                if let Some(id) = declared_id {
                    let _ = rejected_cases.insert(id);
                }
                violations.push(format!(
                    "Historical semantic control {id}/{variant}: {error}"
                ));
                details.push(format!("Historical control {id}/{variant}: rejected"));
            }
        }
    }
    let required_roles = BTreeSet::from(REVIEWED_ROLES);
    let mut complete_cases = 0;
    let mut incomplete_cases = 0;
    for (id, roles) in case_roles {
        if roles == required_roles && !rejected_cases.contains(id) {
            complete_cases += 1;
        } else {
            incomplete_cases += 1;
            violations.push(format!("Historical semantic case {id}: incomplete reviewed role pair; requires one accepted corrected/valid and original/invalid view with identical subjects and no rejected views"));
        }
    }
    disclosure.items.push(format!(
        "Benchmark cases: {}. Historical semantic controls: views={}, historical_cases={}, valid={valid}, invalid={invalid}, rejected={rejected}.",
        corpus["cases"].as_array().map_or(0, Vec::len), controls.len(), complete_cases
    ));
    disclosure.items.push(format!("Historical semantic case completeness: complete_cases={complete_cases}, incomplete_cases={incomplete_cases}. Valid/invalid counts retain individually reviewed row judgments; incomplete or mismatched cases cannot count as complete historical cases."));
    disclosure.items.push(format!("Historical control native test custody: {local} local artifact byte checks; {external} external artifact references NOT_REVERIFIED. The native custody totals cover only the six primary test observations per reviewed view. Matcher executables and linked libraries are external and NOT_REVERIFIED. Historical static executable bytes are also NOT_REVERIFIED; only their retained producer/capture identities are checked. Semantic review accepts only its exact answer-key/native-pairing subject; it does not accept the attached static analysis. Controls do not enter static/calibration cases, pilot selection or frozen denominators. The weak variant is a removal control only."));
    disclosure.items.extend(details.into_iter().take(20));
    disclosure
}

fn validate_control(
    root: &Path,
    control: &Value,
    seen: &mut BTreeSet<(String, String)>,
) -> Result<(&'static str, captures::Custody), String> {
    if !control.is_object() {
        return Err("control must be an object".to_string());
    }
    let id = text(control, "id")?;
    let _ = text(control, "fixture_reference")?;
    let oracle = &control["semantic_oracle"];
    if !oracle.is_object()
        || !REVIEWED_ROLES
            .iter()
            .any(|(_, status)| oracle["status"].as_str() == Some(*status))
    {
        return Err(
            "control requires an explicit reviewed semantic_oracle; legacy absence is not accepted"
                .to_string(),
        );
    }
    let variant = text(oracle, "variant")?;
    if !seen.insert((id.to_string(), variant.to_string())) {
        return Err("duplicate historical case/variant control".to_string());
    }
    let result = validate_declaration(root, control)?;
    let key = retained_json(root, &oracle["answer_key"])?;
    let pairing = retained_json(root, &oracle["native_pairing"])?;
    observed_static::validate(root, &key, &pairing, &control["observed_static"])?;
    Ok(result)
}
