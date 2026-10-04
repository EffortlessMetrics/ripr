use super::catalog::{adjacent_fields, production_records, test_field};
use super::fields::production_disposition;
use super::invariants::{registry_violations, sort_records};
use super::kinds::{IdentityKind, PortabilityClass, is_forbidden_portable_input};
use super::record::{AdjacentField, FieldRole, IdentityRecord};
use super::render::{render_canonical_json, render_markdown};
use super::{REQUIRED_TAXONOMY_KINDS, identity_field_disposition, identity_registry_violations};

const AGENT_REQUEST: &str = "schemas/ripr/ripr-agent-request.schema.json";
const AGENT_SUCCESS: &str = "schemas/ripr/ripr-agent-success.schema.json";
const REPAIR_ATTEMPT: &str = "schemas/ripr/repair-attempt.schema.json";
const FEEDBACK: &str = "crates/ripr/src/output/feedback.rs";

fn blank(kind: IdentityKind) -> IdentityRecord {
    IdentityRecord {
        kind,
        canonical_type: "ty",
        owner_path: "path",
        owner_issue: "issue",
        portability: PortabilityClass::Portable,
        semantic_inputs: &[],
        volatile_excluded: &[],
        parents: &[],
        children: &[],
        invalidation: "invalidation",
        persistence: "persistence",
        serialization: &[],
        competing_wrappers: &[],
    }
}

fn with_taxonomy(extra: &[IdentityRecord]) -> Vec<IdentityRecord> {
    let mut records = IdentityKind::REQUIRED_TAXONOMY
        .iter()
        .map(|kind| {
            extra
                .iter()
                .copied()
                .find(|record| record.kind == *kind)
                .unwrap_or_else(|| blank(*kind))
        })
        .collect::<Vec<_>>();
    for record in extra {
        if !records.iter().any(|existing| existing.kind == record.kind) {
            records.push(*record);
        }
    }
    records
}

fn catalogued(kind: IdentityKind) -> Result<IdentityRecord, String> {
    production_records()
        .iter()
        .copied()
        .find(|record| record.kind == kind)
        .ok_or_else(|| format!("{} is missing from the production catalog", kind.as_str()))
}

#[test]
fn production_registry_covers_required_taxonomy_and_is_internally_consistent() {
    let records = production_records();
    for kind in REQUIRED_TAXONOMY_KINDS {
        assert!(
            records.iter().any(|record| record.kind.as_str() == *kind),
            "missing required taxonomy identity {kind}"
        );
    }
    assert_eq!(identity_registry_violations(), Vec::<String>::new());
}

#[test]
fn missing_taxonomy_identity_fails_the_registry_check() {
    let records = with_taxonomy(&[])
        .into_iter()
        .filter(|record| record.kind != IdentityKind::RepairAttemptId)
        .collect::<Vec<_>>();
    let violations = registry_violations(&records, &[]);
    assert!(
        violations.iter().any(|violation| violation.contains("RepairAttemptId")
            && violation.contains("missing")),
        "{violations:?}"
    );
}

#[test]
fn contradictory_duplicate_owners_fail_the_registry_check() {
    let mut records = with_taxonomy(&[]);
    records.push(blank(IdentityKind::CommandId));
    let violations = registry_violations(&records, &[]);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("CommandId") && violation.contains("duplicate")),
        "{violations:?}"
    );
}

#[test]
fn same_surface_field_cannot_have_two_canonical_owners() {
    const LEFT: &[super::record::SerializationField] = &[test_field(
        "snapshot_id",
        FieldRole::Canonical,
        AGENT_REQUEST,
    )];
    const RIGHT: &[super::record::SerializationField] = &[test_field(
        "snapshot_id",
        FieldRole::Canonical,
        AGENT_REQUEST,
    )];
    let mut attempt = blank(IdentityKind::AnalysisAttemptId);
    attempt.serialization = LEFT;
    let mut completed = blank(IdentityKind::CompletedAnalysisSnapshotId);
    completed.serialization = RIGHT;
    let records = with_taxonomy(&[attempt, completed]);
    let violations = registry_violations(&records, &[]);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("snapshot_id")
                && violation.contains("contradictory owners")),
        "{violations:?}"
    );
}

#[test]
fn same_field_name_on_different_surfaces_can_keep_distinct_meanings() -> Result<(), String> {
    const REPAIR_ATTEMPT_FIELDS: &[super::record::SerializationField] = &[test_field(
        "attempt_id",
        FieldRole::CompatibilityAlias,
        REPAIR_ATTEMPT,
    )];
    let mut repair = catalogued(IdentityKind::RepairAttemptId)?;
    repair.serialization = REPAIR_ATTEMPT_FIELDS;
    let adjacent = [AdjacentField {
        name: "attempt_id",
        surfaces: &[FEEDBACK],
        reason: "caller-supplied slot",
    }];
    let records = with_taxonomy(&[repair]);
    let violations = registry_violations(&records, &adjacent);
    assert!(
        !violations.iter().any(
            |violation| violation.contains("attempt_id") && violation.contains("contradictory")
        ),
        "{violations:?}"
    );
    assert_eq!(
        production_disposition(&records, &adjacent, REPAIR_ATTEMPT, "attempt_id"),
        Some("RepairAttemptId")
    );
    assert_eq!(
        production_disposition(&records, &adjacent, FEEDBACK, "attempt_id"),
        Some("adjacent")
    );
    Ok(())
}

#[test]
fn repair_attempt_id_cannot_be_registered_as_a_snapshot_or_analysis_attempt_alias() {
    const SHARED: &[super::record::SerializationField] = &[test_field(
        "snapshot_id",
        FieldRole::Canonical,
        AGENT_SUCCESS,
    )];
    let mut repair = blank(IdentityKind::RepairAttemptId);
    repair.serialization = SHARED;
    let mut attempt = blank(IdentityKind::AnalysisAttemptId);
    attempt.serialization = SHARED;
    let records = with_taxonomy(&[repair, attempt]);
    let violations = registry_violations(&records, &[]);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("RepairAttemptId")
                && (violation.contains("AnalysisAttemptId") || violation.contains("collides"))),
        "{violations:?}"
    );
}

#[test]
fn completed_snapshot_cannot_own_agent_snapshot_id() {
    const FIELDS: &[super::record::SerializationField] = &[test_field(
        "snapshot_id",
        FieldRole::Canonical,
        AGENT_SUCCESS,
    )];
    let mut completed = blank(IdentityKind::CompletedAnalysisSnapshotId);
    completed.serialization = FIELDS;
    let records = with_taxonomy(&[completed]);
    let violations = registry_violations(&records, &[]);
    assert!(
        violations.iter().any(
            |violation| violation.contains("CompletedAnalysisSnapshotId")
                && violation.contains("snapshot_id")
        ),
        "{violations:?}"
    );
}

#[test]
fn production_catalog_keeps_agent_snapshot_id_as_analysis_attempt_not_completed_snapshot() {
    assert_eq!(
        identity_field_disposition(AGENT_SUCCESS, "snapshot_id"),
        Some("AnalysisAttemptId")
    );
    assert_eq!(
        identity_field_disposition("schemas/ripr/check.schema.json", "snapshot_identity"),
        Some("CompletedAnalysisSnapshotId")
    );
    assert_ne!(
        identity_field_disposition(AGENT_SUCCESS, "snapshot_id"),
        identity_field_disposition("schemas/ripr/check.schema.json", "snapshot_identity")
    );
}

#[test]
fn continuation_wire_names_are_one_authority_with_an_alias() -> Result<(), String> {
    assert_eq!(
        identity_field_disposition(AGENT_REQUEST, "continuation_id"),
        Some("ContinuationId")
    );
    assert_eq!(
        identity_field_disposition(AGENT_SUCCESS, "continuation_identity"),
        Some("ContinuationId")
    );
    let continuation = catalogued(IdentityKind::ContinuationId)?;
    assert!(continuation.serialization.iter().any(|field| {
        field.name == "continuation_identity" && field.role == FieldRole::CompatibilityAlias
    }));
    assert!(
        continuation
            .serialization
            .iter()
            .find(|field| field.name == "continuation_identity")
            .and_then(|field| field.removal_generation)
            .is_some()
    );
    Ok(())
}

#[test]
fn portable_identities_reject_scheduler_timestamp_pid_client_and_display_inputs() {
    const INPUTS: &[&str] = &["owner", "timestamp"];
    let mut canonical = blank(IdentityKind::CanonicalItemId);
    canonical.portability = PortabilityClass::Portable;
    canonical.semantic_inputs = INPUTS;
    let records = with_taxonomy(&[canonical]);
    let violations = registry_violations(&records, &[]);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("CanonicalItemId")
                && violation.contains("timestamp")),
        "{violations:?}"
    );
    for forbidden in [
        "timestamp",
        "pid",
        "client_name",
        "display",
        "markdown",
        "scheduler_generation",
        "effective_root",
    ] {
        assert!(is_forbidden_portable_input(forbidden), "{forbidden}");
    }
}

#[test]
fn equivalent_checkout_roots_preserve_portable_identities_and_keep_containment_separate()
-> Result<(), String> {
    for record in production_records() {
        if record.portability.is_portable() {
            assert!(
                !record
                    .semantic_inputs
                    .iter()
                    .any(|input| is_forbidden_portable_input(input)),
                "{} must not hash absolute checkout spelling",
                record.kind.as_str()
            );
        }
    }
    let input = catalogued(IdentityKind::InputIdentity)?;
    assert!(!input.portability.is_portable());
    assert!(
        input
            .serialization
            .iter()
            .any(|field| field.name == "root_identity" && field.role == FieldRole::Component),
        "concrete containment evidence must stay a separate InputIdentity component"
    );
    assert!(
        !input
            .semantic_inputs
            .iter()
            .any(|input| *input == "effective_root" || *input == "absolute_checkout"),
        "absolute checkout spelling is containment evidence, not a portable semantic input"
    );
    Ok(())
}

#[test]
fn alias_without_removal_generation_fails() {
    const FIELDS: &[super::record::SerializationField] = &[test_field(
        "gap_id",
        FieldRole::CompatibilityAlias,
        "crates/ripr/src/lsp/action_contract.rs",
    )];
    let mut canonical = blank(IdentityKind::CanonicalItemId);
    canonical.serialization = FIELDS;
    let records = with_taxonomy(&[canonical]);
    let violations = registry_violations(&records, &[]);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("gap_id")
                && violation.contains("removal generation")),
        "{violations:?}"
    );
}

#[test]
fn unknown_governed_field_has_no_disposition() {
    assert_eq!(
        identity_field_disposition(AGENT_SUCCESS, "brand_new_identity"),
        None
    );
}

#[test]
fn reordered_registry_entries_render_byte_stable_json_and_markdown() {
    let mut forward = production_records().to_vec();
    let mut reversed = production_records().to_vec();
    reversed.reverse();
    sort_records(&mut forward);
    sort_records(&mut reversed);
    assert_eq!(
        render_canonical_json(&forward, adjacent_fields()),
        render_canonical_json(&reversed, adjacent_fields())
    );
    assert_eq!(
        render_markdown(&forward, adjacent_fields()),
        render_markdown(&reversed, adjacent_fields())
    );
    let json = render_canonical_json(production_records(), adjacent_fields());
    let markdown = render_markdown(production_records(), adjacent_fields());
    for kind in REQUIRED_TAXONOMY_KINDS {
        assert!(json.contains(kind), "json missing {kind}");
        assert!(
            markdown.contains(&format!("`{kind}`")),
            "markdown missing {kind}"
        );
    }
}

#[test]
fn markdown_and_json_name_the_same_identities() {
    let json = render_canonical_json(production_records(), adjacent_fields());
    let markdown = render_markdown(production_records(), adjacent_fields());
    for record in production_records() {
        assert!(json.contains(record.kind.as_str()));
        assert!(markdown.contains(&format!("`{}`", record.kind.as_str())));
    }
}

#[test]
fn feedback_receipt_id_does_not_steal_repair_receipt_authority() -> Result<(), String> {
    assert_eq!(
        identity_field_disposition(FEEDBACK, "receipt_id"),
        Some("FeedbackReceiptId")
    );
    let receipt = catalogued(IdentityKind::ReceiptId)?;
    assert!(
        !receipt
            .serialization
            .iter()
            .any(|field| field.name == "receipt_id" && field.surface == FEEDBACK)
    );
    Ok(())
}

#[test]
fn required_taxonomy_kind_names_match_the_closed_enum() {
    assert_eq!(
        REQUIRED_TAXONOMY_KINDS.len(),
        IdentityKind::REQUIRED_TAXONOMY.len()
    );
    for (name, kind) in REQUIRED_TAXONOMY_KINDS
        .iter()
        .zip(IdentityKind::REQUIRED_TAXONOMY)
    {
        assert_eq!(*name, kind.as_str());
    }
    for kind in IdentityKind::ALL {
        let required = IdentityKind::REQUIRED_TAXONOMY.contains(kind);
        assert_eq!(
            REQUIRED_TAXONOMY_KINDS.contains(&kind.as_str()),
            required,
            "{}",
            kind.as_str()
        );
    }
}

#[test]
fn production_parent_child_graph_is_consistent() {
    let violations = registry_violations(production_records(), adjacent_fields());
    assert!(
        !violations.iter().any(|violation| {
            violation.contains("as a child") || violation.contains("as a parent")
        }),
        "{violations:?}"
    );
}

#[test]
fn one_sided_parent_child_edge_fails_the_registry_check() {
    const CHILDREN: &[IdentityKind] = &[IdentityKind::InstructionInstanceId];
    let mut canonical = blank(IdentityKind::CanonicalItemId);
    canonical.children = CHILDREN;
    let records = with_taxonomy(&[canonical]);
    let violations = registry_violations(&records, &[]);
    assert!(
        violations.iter().any(|violation| {
            violation.contains("CanonicalItemId")
                && violation.contains("InstructionInstanceId")
                && violation.contains("parent")
        }),
        "{violations:?}"
    );
}

#[test]
fn adjacent_field_cannot_also_be_canonical_on_the_same_surface() {
    const FIELDS: &[super::record::SerializationField] = &[test_field(
        "diff_identity",
        FieldRole::Canonical,
        AGENT_SUCCESS,
    )];
    let mut command = blank(IdentityKind::CommandId);
    command.serialization = FIELDS;
    let adjacent = [AdjacentField {
        name: "diff_identity",
        surfaces: &[AGENT_SUCCESS],
        reason: "git digest",
    }];
    let records = with_taxonomy(&[command]);
    let violations = registry_violations(&records, &adjacent);
    assert!(
        violations.iter().any(|violation| {
            violation.contains("diff_identity")
                && violation.contains("cannot also be a registered identity field")
        }),
        "{violations:?}"
    );
}

#[test]
fn serialization_on_an_ungoverned_surface_fails_closed() {
    const FIELDS: &[super::record::SerializationField] = &[test_field(
        "command_id",
        FieldRole::Canonical,
        "docs/not-a-governed-surface.json",
    )];
    let mut command = blank(IdentityKind::CommandId);
    command.serialization = FIELDS;
    let records = with_taxonomy(&[command]);
    let mut violations = Vec::new();
    super::invariants::require_catalog_surfaces_are_governed(
        &records,
        &[],
        super::GOVERNED_IDENTITY_SURFACES,
        &mut violations,
    );
    assert!(
        violations.iter().any(|violation| {
            violation.contains("ungoverned surface") && violation.contains("command_id")
        }),
        "{violations:?}"
    );
}

#[test]
fn repair_attempt_cannot_parent_or_child_an_analysis_attempt() {
    const CHILDREN: &[IdentityKind] = &[IdentityKind::AnalysisAttemptId];
    const PARENTS: &[IdentityKind] = &[IdentityKind::RepairAttemptId];
    let mut repair = blank(IdentityKind::RepairAttemptId);
    repair.children = CHILDREN;
    let mut attempt = blank(IdentityKind::AnalysisAttemptId);
    attempt.parents = PARENTS;
    let records = with_taxonomy(&[repair, attempt]);
    let violations = registry_violations(&records, &[]);
    assert!(
        violations.iter().any(|violation| {
            violation.contains("RepairAttemptId") && violation.contains("AnalysisAttemptId")
        }),
        "{violations:?}"
    );
}

#[test]
fn private_visibility_is_recorded_without_becoming_a_second_owner() {
    use super::catalog::test_field_private;
    const FIELDS: &[super::record::SerializationField] = &[test_field_private(
        "internal_result_id",
        FieldRole::Canonical,
        AGENT_SUCCESS,
    )];
    let mut diagnostic = blank(IdentityKind::DiagnosticResultId);
    diagnostic.serialization = FIELDS;
    let json = render_canonical_json(&with_taxonomy(&[diagnostic]), &[]);
    assert!(json.contains("\"visibility\": \"private\""));
    assert!(json.contains("internal_result_id"));
    assert_eq!(
        identity_field_disposition(AGENT_SUCCESS, "internal_result_id"),
        None,
        "production lookup must not treat a fixture-only private field as live authority"
    );
}

#[test]
fn json_escapes_quotes_and_newlines_and_stays_byte_stable() {
    const INVALIDATION: &str = "say \"hello\" and a\nnewline\tand\rcarriage";
    let mut command = blank(IdentityKind::CommandId);
    command.invalidation = INVALIDATION;
    let records = with_taxonomy(&[command]);
    let first = render_canonical_json(&records, &[]);
    let second = render_canonical_json(&records, &[]);
    assert_eq!(first, second);
    assert!(first.contains("say \\\"hello\\\" and a\\nnewline\\tand\\rcarriage"));
}

#[test]
fn reordered_adjacent_fields_render_byte_stable_json() {
    let forward = adjacent_fields().to_vec();
    let mut reversed = adjacent_fields().to_vec();
    reversed.reverse();
    assert_eq!(
        render_canonical_json(production_records(), &forward),
        render_canonical_json(production_records(), &reversed)
    );
}

#[test]
fn diagnostic_result_id_has_no_public_wire_field() -> Result<(), String> {
    let diagnostic = catalogued(IdentityKind::DiagnosticResultId)?;
    assert!(
        diagnostic
            .serialization
            .iter()
            .all(|field| field.visibility == super::record::FieldVisibility::Private),
        "DiagnosticResultId stays transport-private until a typed public wire field exists"
    );
    assert_eq!(
        identity_field_disposition("crates/ripr/src/lsp/action_contract.rs", "diagnostic_id"),
        Some("DiagnosticCodeId")
    );
    Ok(())
}

#[test]
fn gap_id_is_a_canonical_item_alias_not_a_second_identity() {
    assert_eq!(
        identity_field_disposition("crates/ripr/src/lsp/action_contract.rs", "gap_id"),
        Some("CanonicalItemId")
    );
    assert_eq!(
        identity_field_disposition("crates/ripr/src/lsp/action_contract.rs", "canonical_gap_id"),
        Some("CanonicalItemId")
    );
}

#[test]
fn production_catalog_covers_every_governed_surface() {
    let mut violations = Vec::new();
    super::invariants::require_catalog_surfaces_are_governed(
        production_records(),
        adjacent_fields(),
        super::GOVERNED_IDENTITY_SURFACES,
        &mut violations,
    );
    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn markdown_omits_private_canonical_fields() -> Result<(), String> {
    let markdown = render_markdown(production_records(), adjacent_fields());
    let json = render_canonical_json(production_records(), adjacent_fields());
    assert!(
        markdown
            .lines()
            .any(|line| line.contains("`DiagnosticResultId`") && line.contains(" | — | — |")),
        "{markdown}"
    );
    assert!(json.contains("\"visibility\": \"private\""));
    assert!(json.contains("document_id"));
    Ok(())
}

#[test]
fn truncated_surface_suffix_does_not_inherit_a_disposition() {
    assert_eq!(
        identity_field_disposition("schema.json", "snapshot_id"),
        None,
        "a truncated suffix must not steal a longer registered schema path"
    );
    assert_eq!(
        identity_field_disposition("error.schema.json", "snapshot_id"),
        None,
        "ripr-agent-error.schema.json must not match a bare error.schema.json suffix"
    );
    assert_eq!(
        identity_field_disposition("action_contract.rs", "action_id"),
        None,
        "a basename must not inherit the governed action_contract.rs surface"
    );
}

#[test]
fn checkout_prefixed_surface_still_matches_the_registered_path() {
    assert_eq!(
        identity_field_disposition(
            "/abs/checkout/crates/ripr/src/lsp/action_contract.rs",
            "action_id"
        ),
        Some("ActionId")
    );
    assert_eq!(
        identity_field_disposition(
            "checkout\\schemas/ripr/ripr-agent-success.schema.json",
            "snapshot_id"
        ),
        Some("AnalysisAttemptId")
    );
}

#[test]
fn same_surface_field_cannot_be_canonical_and_component() {
    const CANONICAL: &[super::record::SerializationField] = &[test_field(
        "snapshot_id",
        FieldRole::Canonical,
        AGENT_SUCCESS,
    )];
    const COMPONENT: &[super::record::SerializationField] = &[test_field(
        "snapshot_id",
        FieldRole::Component,
        AGENT_SUCCESS,
    )];
    let mut attempt = blank(IdentityKind::AnalysisAttemptId);
    attempt.serialization = CANONICAL;
    let mut feedback = blank(IdentityKind::FeedbackReceiptId);
    feedback.serialization = COMPONENT;
    let records = with_taxonomy(&[attempt, feedback]);
    let violations = registry_violations(&records, &[]);
    assert!(
        violations.iter().any(|violation| {
            violation.contains("snapshot_id")
                && violation.contains("contradictory owners")
                && violation.contains("AnalysisAttemptId")
                && violation.contains("FeedbackReceiptId")
        }),
        "{violations:?}"
    );
}

#[test]
fn missing_serialized_sibling_fails_the_registry_check() {
    let records = with_taxonomy(&[]);
    let violations = registry_violations(&records, &[]);
    assert!(
        violations
            .iter()
            .any(|violation| { violation.contains("FindingId") && violation.contains("missing") }),
        "{violations:?}"
    );
}
