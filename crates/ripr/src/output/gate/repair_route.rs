use super::model::{
    GateCandidate, GateRepairRoute, GateRepairRouteLimitation, GateRepairTarget, GateRouteFacts,
};
use crate::output::gap_decision_ledger::GapRecord;
use serde_json::Value;

pub(super) const GATE_REPAIR_AUTHORITY_BOUNDARY: &str = "static_ripr_evidence_only";
const INCOMPLETE_REPAIR_ROUTE: &str = "incomplete_repair_route";
const INCOMPLETE_REPAIR_ROUTE_DETAIL: &str =
    "The gate cannot provide a complete bounded repair route from current evidence.";

pub(super) enum GateRouteSource<'a> {
    ReviewCard(&'a Value),
    GapRecord(&'a GapRecord),
}

pub(super) fn normalize_route_source(source: GateRouteSource<'_>) -> GateRouteFacts {
    match source {
        GateRouteSource::ReviewCard(item) => review_card_route_facts(item),
        GateRouteSource::GapRecord(record) => gap_record_route_facts(record),
    }
}

pub(super) fn build_gate_repair_route(candidate: &GateCandidate) -> GateRepairRoute {
    let facts = &candidate.route_facts;
    let missing_fields = missing_route_fields(candidate);
    let limitation = (!missing_fields.is_empty()).then_some(GateRepairRouteLimitation {
        kind: INCOMPLETE_REPAIR_ROUTE,
        missing_fields,
        detail: INCOMPLETE_REPAIR_ROUTE_DETAIL,
    });

    GateRepairRoute {
        canonical_gap_id: facts.canonical_gap_id.clone(),
        seam_id: facts.seam_id.clone(),
        classification: facts.classification.clone(),
        changed_owner: facts.changed_owner.clone(),
        changed_behavior: facts.changed_behavior.clone(),
        missing_discriminator: facts.missing_discriminator.clone(),
        repair_target: facts.repair_target.clone(),
        test_intent: facts.test_intent.clone(),
        verify_command: facts.verify_command.clone(),
        receipt_command: facts.receipt_command.clone(),
        inspection_command: facts.inspection_command.clone(),
        authority_boundary: GATE_REPAIR_AUTHORITY_BOUNDARY.to_string(),
        limitation,
    }
}

pub(super) fn gate_repair_route_is_complete(candidate: &GateCandidate) -> bool {
    missing_route_fields(candidate).is_empty()
}

// NOTE (RIPR-SPEC-0087 §8, issue #2028): this 12-field completeness predicate
// is intentionally NOT the safe-for-repair-packet flip. The single authority
// for that flip is `analysis::repair_route::repair_packet_eligibility` over a
// `ClassifiedSeam`; the gate consumes `GateCandidate` facts projected from
// review cards and `GapRecord`s (a different input shape) and only decides
// whether a bounded repair route can be rendered complete. The GapRecord arm
// of that projection is itself gated by the shared
// `validate_agent_gap_record_packet` validator upstream. If a ClassifiedSeam
// ever becomes available here, delegate to the authority instead of extending
// this field list.
fn missing_route_fields(candidate: &GateCandidate) -> Vec<String> {
    let facts = &candidate.route_facts;
    let mut missing = Vec::new();
    push_missing(&mut missing, "canonical_gap_id", &facts.canonical_gap_id);
    push_missing(&mut missing, "seam_id", &facts.seam_id);
    push_missing(&mut missing, "file", &candidate.placement.path);
    if candidate.placement.line.is_none() {
        missing.push("line".to_string());
    }
    push_missing(&mut missing, "gap_state", &facts.gap_state);
    push_missing(&mut missing, "changed_owner", &facts.changed_owner);
    push_missing(&mut missing, "changed_behavior", &facts.changed_behavior);
    push_missing(
        &mut missing,
        "missing_discriminator",
        &facts.missing_discriminator,
    );
    if facts.repair_target.is_none() {
        missing.push("repair_target".to_string());
    }
    push_missing(&mut missing, "test_intent", &facts.test_intent);
    push_missing(&mut missing, "verify_command", &facts.verify_command);
    push_missing(&mut missing, "receipt_command", &facts.receipt_command);
    push_missing(
        &mut missing,
        "inspection_command",
        &facts.inspection_command,
    );
    missing
}

fn push_missing(missing: &mut Vec<String>, name: &str, value: &Option<String>) {
    if value.as_deref().is_none_or(|s| s.trim().is_empty()) {
        missing.push(name.to_string());
    }
}

fn review_card_route_facts(item: &Value) -> GateRouteFacts {
    GateRouteFacts {
        canonical_gap_id: super::canonical_gap_id_from_value(item),
        seam_id: string_field(item.get("seam_id")),
        gap_state: string_field(item.get("gap_state")),
        classification: string_field(item.get("grip_class"))
            .or_else(|| string_field(item.get("class"))),
        changed_owner: string_field(item.get("owner")),
        changed_behavior: string_field(item.pointer("/seam/expression")),
        missing_discriminator: string_field(item.get("missing_discriminator")),
        repair_target: review_card_repair_target(item),
        test_intent: string_field(item.pointer("/llm_guidance/prompt"))
            .or_else(|| string_field(item.pointer("/suggested_test/intent"))),
        verify_command: string_field(item.pointer("/llm_guidance/verify_command")),
        receipt_command: string_field(item.get("receipt_command")),
        inspection_command: string_field(item.pointer("/llm_guidance/command")),
    }
}

fn gap_record_route_facts(record: &GapRecord) -> GateRouteFacts {
    let route = record.repair_route.as_ref();
    GateRouteFacts {
        canonical_gap_id: non_empty(&record.canonical_gap_id),
        seam_id: record
            .seam_id
            .as_deref()
            .and_then(non_empty_str)
            .map(ToString::to_string),
        gap_state: non_empty(&record.gap_state),
        classification: None,
        changed_owner: record
            .anchor
            .as_ref()
            .and_then(|anchor| anchor.owner.as_deref())
            .and_then(non_empty_str)
            .map(ToString::to_string),
        changed_behavior: route
            .and_then(|route| route.changed_behavior.as_deref())
            .and_then(non_empty_str)
            .map(ToString::to_string),
        missing_discriminator: route
            .and_then(|route| route.missing_discriminator.as_deref())
            .and_then(non_empty_str)
            .map(ToString::to_string),
        repair_target: route.and_then(gap_record_repair_target),
        test_intent: route
            .and_then(|route| route.assertion_shape.as_deref())
            .and_then(non_empty_str)
            .map(ToString::to_string),
        verify_command: record
            .verification_commands
            .first()
            .and_then(|command| non_empty(command)),
        receipt_command: record.receipt_command.as_deref().and_then(non_empty),
        inspection_command: route
            .and_then(|route| route.inspection_command.as_deref())
            .and_then(non_empty_str)
            .map(ToString::to_string),
    }
}

fn review_card_repair_target(item: &Value) -> Option<GateRepairTarget> {
    review_card_related_test_target(item).or_else(|| review_card_production_caller_target(item))
}

fn review_card_related_test_target(item: &Value) -> Option<GateRepairTarget> {
    let related = item.pointer("/suggested_test/related_test")?;
    Some(GateRepairTarget::RelatedTest {
        name: non_empty(related.get("name")?.as_str()?)?,
        file: non_empty(related.get("file")?.as_str()?)?,
        line: related.get("line")?.as_u64()?,
    })
}

fn review_card_production_caller_target(item: &Value) -> Option<GateRepairTarget> {
    let caller = item.get("production_caller")?;
    Some(GateRepairTarget::ProductionCaller {
        owner: non_empty(caller.get("owner")?.as_str()?)?,
        file: string_field(caller.get("file")),
        line: caller.get("line").and_then(Value::as_u64),
    })
}

fn gap_record_repair_target(
    route: &crate::output::gap_decision_ledger::GapRepairRoute,
) -> Option<GateRepairTarget> {
    Some(GateRepairTarget::RelatedTest {
        name: non_empty(route.related_test.as_deref()?)?,
        file: non_empty(route.target_file.as_deref()?)?,
        line: route.target_line?,
    })
}

fn string_field(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .and_then(non_empty_str)
        .map(ToString::to_string)
}

fn non_empty(value: &str) -> Option<String> {
    non_empty_str(value).map(ToString::to_string)
}

fn non_empty_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::gap_decision_ledger::{GapAnchor, GapRepairRoute};
    use crate::output::gate::model::{GateCandidate, GatePlacement};
    use serde_json::{Value, json};

    const CURRENT_REVIEW_COMMENTS: &str = include_str!(
        "../../../../../fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json"
    );

    fn require_equal<T>(actual: T, expected: T, label: &str) -> Result<(), String>
    where
        T: std::fmt::Debug + PartialEq,
    {
        if actual == expected {
            Ok(())
        } else {
            Err(format!(
                "{label} mismatch: actual={actual:?} expected={expected:?}"
            ))
        }
    }

    fn require(condition: bool, message: &str) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(message.to_string())
        }
    }

    fn complete_candidate() -> GateCandidate {
        GateCandidate {
            source: "comments".to_string(),
            source_id: "ripr-review-seam-a".to_string(),
            gap_id: None,
            gap_kind: None,
            canonical_gap_id: Some("gap:shared".to_string()),
            seam_id: Some("seam-a".to_string()),
            gap_state: Some("actionable".to_string()),
            static_class: Some("weakly_gripped".to_string()),
            severity: Some("warning".to_string()),
            placement: GatePlacement {
                path: Some("crates/foo/src/lib.rs".to_string()),
                line: Some(88),
            },
            missing_discriminator: Some("call observation for Event::Ready".to_string()),
            route_facts: GateRouteFacts {
                canonical_gap_id: Some("gap:shared".to_string()),
                seam_id: Some("seam-a".to_string()),
                gap_state: Some("actionable".to_string()),
                classification: Some("weakly_gripped".to_string()),
                changed_owner: Some("foo::dispatch".to_string()),
                changed_behavior: Some("caller emits Event::Ready".to_string()),
                missing_discriminator: Some("call observation for Event::Ready".to_string()),
                repair_target: Some(GateRepairTarget::RelatedTest {
                    name: "dispatches_ready_event".to_string(),
                    file: "crates/foo/tests/dispatch.rs".to_string(),
                    line: 42,
                }),
                test_intent: Some("Exercise foo::dispatch and assert Event::Ready".to_string()),
                verify_command: Some("cargo test -p foo dispatches_ready_event".to_string()),
                receipt_command: Some("ripr receipt write --gap gap:shared".to_string()),
                inspection_command: Some(
                    "ripr agent brief --root . --seam-id seam-a --json".to_string(),
                ),
            },
            assertion_shape: Some("assert_eq!(event, Event::Ready)".to_string()),
            candidate_values: Vec::new(),
            recommended_test: Some(
                "crates/foo/tests/dispatch.rs::dispatches_ready_event".to_string(),
            ),
            repair_route: None,
            verification_commands: Vec::new(),
            nearby_test_changed: false,
            suppressed: false,
            configured_off: false,
            suppression_reason: None,
            summary_reason: None,
            gap_ledger_gate_candidate: false,
            gap_ledger_gate_reason: None,
            gap_ledger_safe_gate_predicate: false,
        }
    }

    fn current_review_item() -> Result<Value, String> {
        let document: Value = serde_json::from_str(CURRENT_REVIEW_COMMENTS)
            .map_err(|error| format!("parse current comments fixture: {error}"))?;
        document
            .get("comments")
            .and_then(Value::as_array)
            .and_then(|comments| comments.first())
            .cloned()
            .ok_or_else(|| "current comments fixture has no review card".to_string())
    }

    #[test]
    fn complete_route_projects_existing_identity_and_exact_inspection_command() -> Result<(), String>
    {
        let candidate = complete_candidate();
        let route = build_gate_repair_route(&candidate);

        require_equal(
            route.canonical_gap_id.as_deref(),
            Some("gap:shared"),
            "canonical gap id",
        )?;
        require_equal(route.seam_id.as_deref(), Some("seam-a"), "seam id")?;
        require_equal(
            route.inspection_command.as_deref(),
            Some("ripr agent brief --root . --seam-id seam-a --json"),
            "inspection command",
        )?;
        require_equal(
            route.authority_boundary.as_str(),
            GATE_REPAIR_AUTHORITY_BOUNDARY,
            "authority boundary",
        )?;
        require_equal(route.limitation, None, "limitation")?;
        require(
            gate_repair_route_is_complete(&candidate),
            "complete candidate should produce a complete route",
        )
    }

    #[test]
    fn incomplete_route_names_missing_fields_without_fabricating_fallbacks() -> Result<(), String> {
        let mut candidate = complete_candidate();
        candidate.route_facts.canonical_gap_id = None;
        candidate.route_facts.seam_id = None;
        candidate.route_facts.receipt_command = None;

        let route = build_gate_repair_route(&candidate);
        let limitation = route
            .limitation
            .as_ref()
            .ok_or_else(|| "limitation is required".to_string())?;

        require_equal(route.canonical_gap_id, None, "canonical gap id")?;
        require_equal(route.seam_id, None, "seam id")?;
        require_equal(limitation.kind, INCOMPLETE_REPAIR_ROUTE, "limitation kind")?;
        require_equal(
            limitation.missing_fields.clone(),
            vec![
                "canonical_gap_id".to_string(),
                "seam_id".to_string(),
                "receipt_command".to_string(),
            ],
            "missing fields",
        )?;
        require(
            !gate_repair_route_is_complete(&candidate),
            "incomplete candidate must not produce a complete route",
        )
    }

    #[test]
    fn explicit_production_caller_is_a_valid_tagged_target() -> Result<(), String> {
        let mut item = current_review_item()?;
        let related = item
            .pointer_mut("/suggested_test/related_test")
            .ok_or_else(|| "current review fixture lacks related test".to_string())?;
        *related = Value::Null;
        let object = item
            .as_object_mut()
            .ok_or_else(|| "current review card is not an object".to_string())?;
        object.insert(
            "production_caller".to_string(),
            json!({
                "owner": "foo::dispatch",
                "file": "crates/foo/src/lib.rs",
                "line": 88
            }),
        );

        let facts = normalize_route_source(GateRouteSource::ReviewCard(&item));

        require_equal(
            facts.repair_target,
            Some(GateRepairTarget::ProductionCaller {
                owner: "foo::dispatch".to_string(),
                file: Some("crates/foo/src/lib.rs".to_string()),
                line: Some(88),
            }),
            "tagged production caller",
        )
    }

    #[test]
    fn current_review_fixture_projects_complete_route_without_renderer_inference()
    -> Result<(), String> {
        let item = current_review_item()?;
        let expected_command = item
            .pointer("/llm_guidance/command")
            .and_then(Value::as_str)
            .ok_or_else(|| "current review fixture lacks llm_guidance.command".to_string())?;
        let candidate =
            crate::output::gate::candidate_from_guidance_item("comments", &item, false, false);
        let route = build_gate_repair_route(&candidate);
        let rendered = crate::output::gate::presentation::repair_route_json(&route);

        require_equal(
            rendered.get("canonical_gap_id"),
            Some(&Value::String("gap:dedf923a13a00573".to_string())),
            "rendered canonical gap id",
        )?;
        require_equal(
            rendered.get("seam_id"),
            Some(&Value::String("8f7fa8644fd12280".to_string())),
            "rendered seam id",
        )?;
        require_equal(
            rendered.pointer("/repair_target/file"),
            Some(&Value::String("tests/pricing.rs".to_string())),
            "rendered related test file",
        )?;
        require_equal(
            rendered.get("inspection_command"),
            Some(&Value::String(expected_command.to_string())),
            "producer-owned inspection command",
        )?;
        require_equal(
            rendered.get("changed_owner"),
            Some(&Value::String("pricing::discounted_total".to_string())),
            "changed owner",
        )?;
        require_equal(
            rendered.pointer("/repair_target/kind"),
            Some(&Value::String("related_test".to_string())),
            "predicate repair target kind",
        )?;
        require_equal(
            rendered.get("limitation"),
            Some(&Value::Null),
            "rendered limitation",
        )?;
        require(
            crate::output::gate::candidate_is_policy_eligible(&candidate),
            "complete PR-guidance route should remain policy eligible",
        )
    }

    #[test]
    fn same_line_seams_keep_distinct_producer_owned_inspection_commands() -> Result<(), String> {
        let first = current_review_item()?;
        let mut second = first.clone();
        let seam_id = second
            .get_mut("seam_id")
            .ok_or_else(|| "current review fixture lacks seam_id".to_string())?;
        *seam_id = Value::String("second-seam-on-line-88".to_string());
        let command = second
            .pointer_mut("/llm_guidance/command")
            .ok_or_else(|| "current review fixture lacks llm_guidance.command".to_string())?;
        *command = Value::String(
            "ripr agent brief --root . --seam-id second-seam-on-line-88 --json".to_string(),
        );

        let first_facts = normalize_route_source(GateRouteSource::ReviewCard(&first));
        let second_facts = normalize_route_source(GateRouteSource::ReviewCard(&second));

        require_equal(
            first.pointer("/placement/line"),
            second.pointer("/placement/line"),
            "shared source line",
        )?;
        require(
            first_facts.inspection_command != second_facts.inspection_command,
            "same-line seams must retain distinct exact inspection commands",
        )
    }

    #[test]
    fn generic_gap_record_evidence_ids_do_not_become_seam_identity() -> Result<(), String> {
        let record = GapRecord {
            canonical_gap_id: "gap:shared".to_string(),
            gap_state: "actionable".to_string(),
            anchor: Some(GapAnchor {
                file: Some("crates/foo/src/lib.rs".to_string()),
                line: Some(88),
                owner: Some("foo::dispatch".to_string()),
                ..GapAnchor::default()
            }),
            evidence_ids: vec!["evidence:generic-first-entry".to_string()],
            repair_route: Some(GapRepairRoute {
                changed_behavior: Some("caller emits Event::Ready".to_string()),
                missing_discriminator: Some("call observation for Event::Ready".to_string()),
                assertion_shape: Some("assert exact emitted event".to_string()),
                related_test: Some("dispatches_ready_event".to_string()),
                target_file: Some("crates/foo/tests/dispatch.rs".to_string()),
                target_line: Some(42),
                ..GapRepairRoute::default()
            }),
            verification_commands: vec!["cargo test -p foo dispatches_ready_event".to_string()],
            receipt_command: Some("ripr receipt write --gap gap:shared".to_string()),
            ..GapRecord::default()
        };

        let candidate = crate::output::gate::candidate_from_gap_record(&record);
        let route = build_gate_repair_route(&candidate);
        let limitation = route
            .limitation
            .as_ref()
            .ok_or_else(|| "gap-record route should be limited".to_string())?;

        require_equal(route.seam_id, None, "gap-record seam id")?;
        require(
            limitation
                .missing_fields
                .iter()
                .any(|field| field == "seam_id"),
            "generic evidence identity must leave seam_id missing",
        )
    }

    #[test]
    fn blank_gap_record_commands_render_as_null_and_remain_missing() -> Result<(), String> {
        let record = GapRecord {
            verification_commands: vec!["   ".to_string()],
            receipt_command: Some("\t".to_string()),
            ..GapRecord::default()
        };
        let candidate = crate::output::gate::candidate_from_gap_record(&record);
        let route = build_gate_repair_route(&candidate);
        let rendered = crate::output::gate::presentation::repair_route_json(&route);
        let limitation = route
            .limitation
            .as_ref()
            .ok_or_else(|| "blank commands must keep the route limited".to_string())?;

        require_equal(
            rendered.get("verify_command"),
            Some(&Value::Null),
            "blank verify command",
        )?;
        require_equal(
            rendered.get("receipt_command"),
            Some(&Value::Null),
            "blank receipt command",
        )?;
        require(
            limitation
                .missing_fields
                .iter()
                .any(|field| field == "verify_command"),
            "blank verify command must be named missing",
        )?;
        require(
            limitation
                .missing_fields
                .iter()
                .any(|field| field == "receipt_command"),
            "blank receipt command must be named missing",
        )
    }

    #[test]
    fn incomplete_route_is_advisory_only_and_renders_named_limitation() -> Result<(), String> {
        let item = current_review_item()?;
        let mut candidate =
            crate::output::gate::candidate_from_guidance_item("comments", &item, false, false);
        candidate.route_facts.verify_command = None;

        let route = build_gate_repair_route(&candidate);
        let rendered = crate::output::gate::presentation::repair_route_json(&route);

        require(
            !crate::output::gate::candidate_is_policy_eligible(&candidate),
            "incomplete route must fail closed out of policy eligibility",
        )?;
        require_equal(
            rendered.pointer("/limitation/kind"),
            Some(&Value::String(INCOMPLETE_REPAIR_ROUTE.to_string())),
            "limitation kind",
        )?;
        require_equal(
            rendered.pointer("/limitation/missing_fields"),
            Some(&json!(["verify_command"])),
            "limitation missing fields",
        )
    }
}
