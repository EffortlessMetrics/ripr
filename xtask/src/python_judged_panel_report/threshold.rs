//! Threshold policy: explicit input, explicit evaluation, non-authoritative
//! (RIPR-SPEC-0092). The report never selects a threshold from observed
//! results; it only evaluates the supplied policy as-is.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::python_judged_panel::parse_json_without_duplicate_keys;

use super::report::ErrorRate;
use super::{POLICY_KIND, REPORT_RERUN, REPORT_SCHEMA_VERSION, SPEC, THRESHOLD_AUTHORITY_NOTE};

// ---------------------------------------------------------------------------
// Threshold policy: explicit input, explicit evaluation, non-authoritative
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdPolicyFile {
    schema_version: String,
    kind: String,
    spec: String,
    rationale: String,
    #[serde(default)]
    authority: Option<String>,
    thresholds: Vec<ThresholdSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdSpec {
    metric: String,
    operator: String,
    value: f64,
}

const METRIC_FALSE_ACTIONABLE_RATE: &str = "false_actionable_rate";
const METRIC_FALSE_EXPOSED_RATE: &str = "false_exposed_rate";
const METRIC_ADJUDICATED_COUNT: &str = "adjudicated_count";

/// Evaluates the supplied policy as-is. The policy path is resolved against
/// the inventory root (the repository root in production) but echoed exactly
/// as given.
pub(super) fn evaluate_threshold_policy(
    root: &Path,
    policy_path: &str,
    selected: usize,
    adjudicated: usize,
    false_actionable: &ErrorRate,
    false_exposed: &ErrorRate,
) -> Result<Value, String> {
    let body = fs::read_to_string(root.join(policy_path)).map_err(|error| {
        format!("read threshold policy `{policy_path}`: {error}\nrerun: {REPORT_RERUN}")
    })?;
    let value = parse_json_without_duplicate_keys(&body)
        .map_err(|error| format!("parse threshold policy `{policy_path}`: {error}"))?;
    let policy: ThresholdPolicyFile = serde_json::from_value(value)
        .map_err(|error| format!("parse threshold policy `{policy_path}`: {error}"))?;
    if policy.schema_version != REPORT_SCHEMA_VERSION
        || policy.kind != POLICY_KIND
        || policy.spec != SPEC
    {
        return Err(format!(
            "threshold policy `{policy_path}` carries unknown identity (schema `{}`, kind `{}`, spec `{}`); expected {REPORT_SCHEMA_VERSION}/{POLICY_KIND}/{SPEC}",
            policy.schema_version, policy.kind, policy.spec
        ));
    }
    if policy.rationale.trim().is_empty() {
        return Err(format!(
            "threshold policy `{policy_path}` must carry a non-blank rationale; an unexplained threshold candidate cannot be evaluated"
        ));
    }
    if policy.thresholds.is_empty() {
        return Err(format!(
            "threshold policy `{policy_path}` must declare at least one threshold"
        ));
    }
    let mut seen = BTreeSet::new();
    let mut evaluations = Vec::new();
    for threshold in &policy.thresholds {
        if !seen.insert((threshold.metric.clone(), threshold.operator.clone())) {
            return Err(format!(
                "threshold policy `{policy_path}` declares duplicate metric `{}` with operator `{}`",
                threshold.metric, threshold.operator
            ));
        }
        let evaluation = match threshold.metric.as_str() {
            METRIC_FALSE_ACTIONABLE_RATE | METRIC_FALSE_EXPOSED_RATE => {
                if threshold.operator != "max" {
                    return Err(format!(
                        "threshold policy `{policy_path}`: metric `{}` requires operator `max`, found `{}`",
                        threshold.metric, threshold.operator
                    ));
                }
                if !(0.0..=1.0).contains(&threshold.value) {
                    return Err(format!(
                        "threshold policy `{policy_path}`: metric `{}` requires a value in [0, 1], found {}",
                        threshold.metric, threshold.value
                    ));
                }
                let (rate, axis) = if threshold.metric == METRIC_FALSE_ACTIONABLE_RATE {
                    (false_actionable, "false_actionable")
                } else {
                    (false_exposed, "false_exposed")
                };
                match rate.rate {
                    Some(measured) => json!({
                        "metric": threshold.metric,
                        "operator": "max",
                        "threshold_value": threshold.value,
                        "measured": measured,
                        "result": if measured <= threshold.value { "pass" } else { "fail" },
                        "reason": format!(
                            "{axis} numerator {} / denominator {} over the adjudicated rows named in the rate's denominator_case_ids",
                            rate.numerator, rate.denominator
                        ),
                    }),
                    None => json!({
                        "metric": threshold.metric,
                        "operator": "max",
                        "threshold_value": threshold.value,
                        "measured": Value::Null,
                        "result": "not_evaluable",
                        "reason": format!(
                            "no {axis} denominator: {} adjudicated row(s) with a decided {axis} label; no denominator means no rate",
                            rate.denominator
                        ),
                    }),
                }
            }
            METRIC_ADJUDICATED_COUNT => {
                if threshold.operator != "min" {
                    return Err(format!(
                        "threshold policy `{policy_path}`: metric `{METRIC_ADJUDICATED_COUNT}` requires operator `min`, found `{}`",
                        threshold.operator
                    ));
                }
                if threshold.value < 0.0 {
                    return Err(format!(
                        "threshold policy `{policy_path}`: metric `{METRIC_ADJUDICATED_COUNT}` requires a non-negative value, found {}",
                        threshold.value
                    ));
                }
                json!({
                    "metric": threshold.metric,
                    "operator": "min",
                    "threshold_value": threshold.value,
                    "measured": adjudicated,
                    "result": if adjudicated as f64 >= threshold.value { "pass" } else { "fail" },
                    "reason": format!(
                        "{adjudicated} of {selected} selected rows carry a complete two-role adjudication; disputed, inconclusive, pending, and stale rows never count"
                    ),
                })
            }
            other => {
                return Err(format!(
                    "threshold policy `{policy_path}` declares unknown metric `{other}`; expected {METRIC_FALSE_ACTIONABLE_RATE}, {METRIC_FALSE_EXPOSED_RATE}, or {METRIC_ADJUDICATED_COUNT}"
                ));
            }
        };
        evaluations.push(evaluation);
    }
    Ok(json!({
        "policy": {
            "path": policy_path,
            "rationale": policy.rationale,
            "authority": policy.authority.filter(|authority| !authority.trim().is_empty()),
        },
        "evaluations": evaluations,
        "authority_note": THRESHOLD_AUTHORITY_NOTE,
    }))
}
