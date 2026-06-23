//! Perl adapter tests.
//!
//! Extracted from perl.rs as part of Campaign 31 PR 2 (the behaviour-preserving
//! monolith split). The tests are moved verbatim; the parent perl.rs now carries
//! only the production (cfg-test-gated) adapter code.

use super::*;

fn complete_perl_actionability_context() -> PerlActionabilityContext {
    PerlActionabilityContext {
        receipt_command: Some(
            [
                "ripr",
                "agent",
                "receipt",
                "--root",
                ".",
                "--verify-json",
                "target/ripr/workflow/agent-verify.json",
                "--seam-id",
                "perl-gap",
                "--json",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        ),
        allowed_edit_boundaries: vec!["t/app.t".to_string()],
        forbidden_edit_boundaries: vec![
            "lib/My/App.pm".to_string(),
            "badges/ripr-plus.json".to_string(),
        ],
        stop_if: vec![
            "perl-lsp packet status changes".to_string(),
            "related test no longer reaches owner".to_string(),
        ],
        must_not_change: vec![
            "do not edit Perl production code".to_string(),
            "do not add suppressions or intent ledger entries".to_string(),
        ],
    }
}

fn command_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

fn blocking_boundary_kind_cases() -> [(BoundaryKind, &'static str); 14] {
    [
        (BoundaryKind::DynamicDispatch, "dynamic_dispatch"),
        (
            BoundaryKind::ModuleResolutionUnknown,
            "module_resolution_unknown",
        ),
        (BoundaryKind::GeneratedSymbol, "generated_symbol"),
        (BoundaryKind::RoleComposition, "role_composition"),
        (
            BoundaryKind::MonkeypatchOrSymbolPatch,
            "monkeypatch_or_symbol_patch",
        ),
        (BoundaryKind::EvalOrStringCode, "eval_or_string_code"),
        (BoundaryKind::SymbolTableMutation, "symbol_table_mutation"),
        (BoundaryKind::FrameworkIndirection, "framework_indirection"),
        (BoundaryKind::UnknownHelper, "unknown_helper"),
        (BoundaryKind::UnsupportedSyntax, "unsupported_syntax"),
        (BoundaryKind::MissingTestRunner, "missing_test_runner"),
        (BoundaryKind::MissingDiffOwner, "missing_diff_owner"),
        (BoundaryKind::PacketIncomplete, "packet_incomplete"),
        (BoundaryKind::Unknown, "unknown"),
    ]
}

#[test]
fn perl_strict_command_guards_accept_only_bounded_verify_and_receipt_shapes() {
    assert!(is_verify_command(&command_args(&["prove", "t/app.t"])));
    assert!(is_verify_command(&command_args(&[
        "yath",
        "test",
        "t/app_test2.t"
    ])));
    assert!(is_verify_command(&command_args(&[
        "carton",
        "exec",
        "prove",
        "t/app_exception.t"
    ])));
    assert!(is_verify_command(&command_args(&[
        "dzil",
        "test",
        "--test",
        "t/app_fatal.t"
    ])));
    assert!(!is_verify_command(&command_args(&["cargo", "test"])));
    assert!(!is_verify_command(&command_args(&[
        "prove",
        "../outside.t"
    ])));
    assert!(!is_verify_command(&command_args(&[
        "prove", "t/app.t", "&&"
    ])));

    assert!(is_receipt_command(&command_args(&[
        "ripr",
        "agent",
        "receipt",
        "--root",
        ".",
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "perl-gap",
        "--json",
    ])));
    assert!(is_receipt_command(&command_args(&[
        "cargo",
        "run",
        "-p",
        "ripr",
        "--",
        "agent",
        "receipt",
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "perl-gap",
        "--test",
        "t/app.t",
        "--command",
        "prove",
        "--out",
        "target/ripr/reports/agent-receipt.json",
        "--json",
    ])));
    assert!(!is_receipt_command(&command_args(&[
        "ripr",
        "agent",
        "receipt",
        "--root",
        "../outside",
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "perl-gap",
        "--json",
    ])));
    assert!(!is_receipt_command(&command_args(&[
        "ripr",
        "agent",
        "receipt",
        "--verify-json",
        "../agent-verify.json",
        "--seam-id",
        "perl-gap",
        "--json",
    ])));
    assert!(!is_receipt_command(&command_args(&[
        "ripr",
        "agent",
        "receipt",
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--json",
    ])));
    assert!(!is_receipt_command(&command_args(&[
        "ripr",
        "agent",
        "receipt",
        "--verify-json",
        "target/ripr/workflow/agent-verify.json",
        "--seam-id",
        "--json",
        "--json",
    ])));

    assert!(is_safe_repo_relative_path(
        "target/ripr/reports/agent-receipt.json"
    ));
    assert!(!is_safe_repo_relative_path("../outside.pm"));
    assert!(!is_safe_repo_relative_path("crate:outside.pm"));
    assert!(!is_safe_repo_relative_path("t\\app.t"));
}

#[test]
fn perl_fact_packet_adapter_consumes_exact_return_fixture() -> Result<(), String> {
    let packet = PerlAdapter.consume_fact_packet(EXACT_RETURN_PACKET)?;

    assert_eq!(packet.schema_version, PERL_FACT_PACKET_SCHEMA);
    assert_eq!(packet.packet_status, PacketStatus::Complete);
    assert_eq!(packet.files.len(), 2);

    let owner = packet
        .owner("perl:lib/My/App.pm::My::App::discount")
        .ok_or_else(|| "missing owner fact".to_string())?;
    assert_eq!(owner.kind, OwnerKind::Sub);
    assert_eq!(owner.package.as_deref(), Some("My::App"));
    assert_eq!(owner.confidence, Confidence::High);

    let relation = packet
        .relation("relation:change:discount-return:test:threshold")
        .ok_or_else(|| "missing relation fact".to_string())?;
    assert_eq!(relation.relation_kind, RelationKind::DirectOwnerCall);
    assert_eq!(relation.reachability_hint, ReachabilityHint::Reachable);

    let command = packet
        .verify_command_for_test("test:t/app.t:test_discount_threshold")
        .ok_or_else(|| "missing verify command fact".to_string())?;
    assert_eq!(command.runner, Runner::Prove);
    assert_eq!(command.argv, ["prove", "t/app.t"]);

    Ok(())
}

#[test]
fn perl_fact_packet_adapter_rejects_unknown_schema_version() -> Result<(), String> {
    let err = match PerlAdapter.consume_fact_packet(
        &EXACT_RETURN_PACKET.replace("\"ripr-perl-facts-v1\"", "\"ripr-perl-facts-v2\""),
    ) {
        Ok(_) => return Err("unknown schema version should fail closed".to_string()),
        Err(err) => err,
    };

    assert!(err.contains("unsupported Perl fact packet schema"));
    assert!(err.contains(PERL_FACT_PACKET_SCHEMA));

    Ok(())
}

#[test]
fn perl_fact_packet_adapter_parses_partial_dynamic_boundary_limitation() -> Result<(), String> {
    let packet = PerlAdapter.consume_fact_packet(PARTIAL_DYNAMIC_BOUNDARY_PACKET)?;

    assert_eq!(packet.packet_status, PacketStatus::Partial);
    assert_eq!(packet.dynamic_boundaries.len(), 1);
    assert_eq!(
        packet.dynamic_boundaries[0].kind,
        BoundaryKind::DynamicDispatch
    );
    assert_eq!(packet.limitations.len(), 1);
    assert_eq!(packet.limitations[0].kind, BoundaryKind::DynamicDispatch);
    assert!(
        packet
            .verify_command_for_test("test:t/app.t:test_dynamic_discount")
            .is_none(),
        "partial dynamic-boundary fixture must not invent a verify command"
    );

    Ok(())
}

#[test]
fn perl_fact_packet_adapter_keeps_verify_command_as_fact_not_result() -> Result<(), String> {
    let packet = PerlAdapter.consume_fact_packet(EXACT_RETURN_PACKET)?;
    let command = packet
        .verify_command_for_test("test:t/app.t:test_discount_threshold")
        .ok_or_else(|| "missing verify command fact".to_string())?;

    assert_eq!(command.preconditions, ["prove_on_path"]);
    assert!(
        packet
            .provenance
            .iter()
            .any(|fact| fact.provenance_id == "prov:runner:1"),
        "runner detection is provenance, not an executed result"
    );

    Ok(())
}

#[test]
fn perllsp_exporter_fixture_is_consumed_without_actionable_gap_state() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-facts-v1.json"
    );
    let packet = PerlAdapter.consume_fact_packet(fixture)?;

    assert_eq!(packet.producer.name, "perl-lsp");
    assert_eq!(packet.schema_version, PERL_FACT_PACKET_SCHEMA);
    assert_eq!(packet.packet_status, PacketStatus::Complete);
    assert_eq!(
        packet.input.requested_fact_classes,
        ["owners", "changes", "tests", "oracles"]
    );
    assert!(
        packet
            .files
            .iter()
            .all(|file| !file.path.contains('\\') && !file.path.contains(':')),
        "exporter fixture paths must stay repo-relative"
    );

    let value: serde_json::Value = serde_json::from_str(fixture).map_err(|err| err.to_string())?;
    assert!(
        value.get("canonical_gap_id").is_none(),
        "perl-lsp packets must not emit RIPR-derived gap IDs"
    );
    assert!(
        value.get("gap_state").is_none(),
        "perl-lsp packets must not emit RIPR-derived actionability"
    );

    Ok(())
}

#[test]
fn perl_fact_packet_adapter_preserves_source_test_and_oracle_taxonomy() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let packet = PerlAdapter.consume_fact_packet(fixture)?;

    assert_eq!(packet.files_with_role(FileRole::Source).len(), 1);
    assert_eq!(packet.files_with_role(FileRole::Test).len(), 6);
    assert_eq!(packet.tests_for_framework(TestFramework::TestMore).len(), 1);
    assert_eq!(packet.tests_for_framework(TestFramework::Test2V0).len(), 1);
    assert_eq!(
        packet.tests_for_framework(TestFramework::Test2Suite).len(),
        1
    );
    assert_eq!(
        packet
            .tests_for_framework(TestFramework::TestException)
            .len(),
        1
    );
    assert_eq!(
        packet.tests_for_framework(TestFramework::TestFatal).len(),
        1
    );
    assert_eq!(packet.tests_for_framework(TestFramework::Unknown).len(), 1);

    assert_eq!(
        packet.verify_command_runners(),
        BTreeSet::from([Runner::Prove, Runner::Yath, Runner::Carton, Runner::Dzil])
    );

    let strong_shapes = packet
        .strong_exact_oracles()
        .into_iter()
        .map(|oracle| oracle.kind.assertion_shape())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        strong_shapes,
        BTreeSet::from([
            "exact_return_assertion",
            "predicate_boundary_assertion",
            "exception_observer",
            "hash_or_object_field_assertion",
            "output_observer",
            "warn_observer",
            "log_observer"
        ])
    );

    let advisory_shapes = packet
        .advisory_oracles()
        .into_iter()
        .map(|oracle| oracle.kind.assertion_shape())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        advisory_shapes,
        BTreeSet::from([
            "smoke_ok",
            "mention_only",
            "dies_only",
            "unknown_helper",
            "dynamic_framework_indirection"
        ])
    );

    for kind in [
        OracleKind::SmokeOk,
        OracleKind::MentionOnly,
        OracleKind::DiesOnly,
        OracleKind::UnknownHelper,
        OracleKind::DynamicFrameworkIndirection,
    ] {
        assert!(
            packet
                .oracles_for_kind(kind)
                .iter()
                .all(|oracle| !oracle.is_strong_exact()),
            "{kind:?} must stay advisory and non-strong in Perl preview facts"
        );
    }

    let value: serde_json::Value = serde_json::from_str(fixture).map_err(|err| err.to_string())?;
    assert!(
        value.get("canonical_gap_id").is_none(),
        "Perl source/test/oracle fixture must not emit RIPR-derived gap IDs"
    );
    assert!(
        value.get("gap_state").is_none(),
        "Perl source/test/oracle fixture must not emit RIPR-derived actionability"
    );

    Ok(())
}

#[test]
fn perl_related_test_linking_classifies_reachability_and_revealability() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let packet = PerlAdapter.consume_fact_packet(fixture)?;

    let return_related = packet.related_test_evidence_for_change("change:lib/My/App.pm:8:return");
    assert_eq!(return_related.len(), 1);
    let return_evidence = &return_related[0];
    assert_eq!(
        return_evidence.relation_id,
        "relation:return:discount-smoke"
    );
    assert_eq!(return_evidence.test_path, "t/app.t");
    assert_eq!(return_evidence.test_name, "discount_smoke");
    assert_eq!(return_evidence.relation_kind, RelationKind::DirectOwnerCall);
    assert_eq!(
        return_evidence.reachability_hint,
        ReachabilityHint::Reachable
    );
    assert_eq!(
        return_evidence.oracle_shape.as_deref(),
        Some("exact_return_assertion")
    );
    assert_eq!(
        return_evidence.oracle_strength,
        Some(OracleStrength::StrongExact)
    );
    assert_eq!(return_evidence.class, ExposureClass::WeaklyExposed);
    assert_eq!(
        return_evidence.verify_command.as_deref(),
        Some(&["prove".to_string(), "t/app.t".to_string()][..])
    );
    assert!(
        return_evidence
            .evidence_refs
            .contains(&"prov:relation:return".to_string())
    );
    assert!(
        return_evidence
            .evidence_refs
            .contains(&"prov:oracle:exact-return".to_string())
    );

    assert_eq!(
        packet.classify_change_from_related_tests("change:lib/My/App.pm:8:return"),
        ExposureClass::WeaklyExposed
    );
    assert_eq!(
        packet.classify_change_from_related_tests("change:lib/My/App.pm:14:predicate"),
        ExposureClass::WeaklyExposed
    );
    assert_eq!(
        packet.classify_change_from_related_tests("change:lib/My/App.pm:20:exception"),
        ExposureClass::WeaklyExposed
    );
    assert_eq!(
        packet.classify_change_from_related_tests("change:lib/My/App.pm:25:field"),
        ExposureClass::NoStaticPath,
        "unlinked Perl oracles must not imply related-test reachability"
    );

    let stale_owner_text = fixture.replace(
        r#""relation_id": "relation:return:discount-smoke",
      "change_id": "change:lib/My/App.pm:8:return",
      "owner_id": "perl:lib/My/App.pm::My::App::discount",
      "test_id": "test:t/app.t:discount_smoke",
      "oracle_id": "oracle:t/app.t:7:is""#,
        r#""relation_id": "relation:return:discount-smoke",
      "change_id": "change:lib/My/App.pm:8:return",
      "owner_id": "perl:lib/My/App.pm::My::App::eligible",
      "test_id": "test:t/app.t:discount_smoke",
      "oracle_id": "oracle:t/app.t:7:is""#,
    );
    let stale_owner_packet = PerlAdapter.consume_fact_packet(&stale_owner_text)?;
    assert!(
        stale_owner_packet
            .related_test_evidence_for_change("change:lib/My/App.pm:8:return")
            .is_empty(),
        "stale relation owners must not count as related-test evidence for the change"
    );
    assert_eq!(
        stale_owner_packet.classify_change_from_related_tests("change:lib/My/App.pm:8:return"),
        ExposureClass::NoStaticPath
    );

    let weak_text = fixture.replace(
        r#""relation_id": "relation:return:discount-smoke",
      "change_id": "change:lib/My/App.pm:8:return",
      "owner_id": "perl:lib/My/App.pm::My::App::discount",
      "test_id": "test:t/app.t:discount_smoke",
      "oracle_id": "oracle:t/app.t:7:is""#,
        r#""relation_id": "relation:return:discount-smoke",
      "change_id": "change:lib/My/App.pm:8:return",
      "owner_id": "perl:lib/My/App.pm::My::App::discount",
      "test_id": "test:t/app.t:discount_smoke",
      "oracle_id": "oracle:t/app.t:6:ok""#,
    );
    let weak_packet = PerlAdapter.consume_fact_packet(&weak_text)?;
    let weak_related =
        weak_packet.related_test_evidence_for_change("change:lib/My/App.pm:8:return");
    assert_eq!(weak_related.len(), 1);
    assert_eq!(
        weak_related[0].oracle_shape.as_deref(),
        Some("smoke_ok"),
        "the relation still names the related test but keeps the advisory oracle shape"
    );
    assert_eq!(
        weak_packet.classify_change_from_related_tests("change:lib/My/App.pm:8:return"),
        ExposureClass::ReachableUnrevealed
    );

    let static_unknown_text = fixture.replacen(
        r#""reachability_hint": "reachable""#,
        r#""reachability_hint": "static_unknown""#,
        1,
    );
    let static_unknown_packet = PerlAdapter.consume_fact_packet(&static_unknown_text)?;
    assert_eq!(
        static_unknown_packet.classify_change_from_related_tests("change:lib/My/App.pm:8:return"),
        ExposureClass::StaticUnknown
    );

    let value: serde_json::Value = serde_json::from_str(fixture).map_err(|err| err.to_string())?;
    assert!(
        value.get("repair_packet").is_none(),
        "Perl related-test linking must not emit repair packets before strict actionability"
    );
    assert!(
        value.get("gap_state").is_none(),
        "Perl related-test linking must not emit RIPR-derived actionability"
    );

    Ok(())
}

#[test]
fn perl_strict_actionability_requires_all_packet_and_context_fields() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let packet = PerlAdapter.consume_fact_packet(fixture)?;
    let context = complete_perl_actionability_context();
    let gap = packet
        .canonical_gap_identity_for_change("change:lib/My/App.pm:8:return")
        .ok_or_else(|| "missing canonical gap identity".to_string())?;

    let actionable = packet
        .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context)
        .map_err(|err| format!("{err:?}"))?;

    assert_eq!(actionable.packet_id, format!("perl-repair:{}", gap.id));
    assert_eq!(actionable.canonical_gap_id, gap.id);
    assert_eq!(actionable.gap_state, PerlGapState::Actionable);
    assert_eq!(
        actionable.changed_owner_id,
        "perl:lib/My/App.pm::My::App::discount"
    );
    assert_eq!(actionable.evidence_class, ExposureClass::WeaklyExposed);
    assert_eq!(actionable.missing_discriminator, "return_value");
    assert_eq!(actionable.repair_kind, "add_exact_return_assertion");
    assert_eq!(
        actionable.target_test_shape,
        "Test::More exact_return_assertion"
    );
    assert_eq!(
        actionable.suggested_test_location,
        "t/app.t::discount_smoke"
    );
    assert_eq!(actionable.related_test_id, "test:t/app.t:discount_smoke");
    assert_eq!(actionable.verify_command, ["prove", "t/app.t"]);
    assert_eq!(
        actionable.receipt_command,
        [
            "ripr",
            "agent",
            "receipt",
            "--root",
            ".",
            "--verify-json",
            "target/ripr/workflow/agent-verify.json",
            "--seam-id",
            "perl-gap",
            "--json"
        ]
    );
    assert_eq!(actionable.confidence, Confidence::Medium);
    assert_eq!(actionable.allowed_edit_boundaries, ["t/app.t"]);
    assert_eq!(
        actionable.forbidden_edit_boundaries,
        ["lib/My/App.pm", "badges/ripr-plus.json"]
    );
    assert_eq!(
        actionable.stop_if,
        [
            "perl-lsp packet status changes",
            "related test no longer reaches owner"
        ]
    );
    assert_eq!(
        actionable.must_not_change,
        [
            "do not edit Perl production code",
            "do not add suppressions or intent ledger entries"
        ]
    );
    assert!(actionable.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_change"
            && reference.source_id == "change:lib/My/App.pm:8:return"
            && reference.path == "lib/My/App.pm"
    }));
    assert!(actionable.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_source_file"
            && reference.source_id == "file:lib/My/App.pm"
            && reference.path == "lib/My/App.pm"
    }));
    assert!(actionable.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_owner_file"
            && reference.source_id == "file:lib/My/App.pm"
            && reference.path == "lib/My/App.pm"
    }));
    assert!(actionable.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_relation"
            && reference.source_id == "relation:return:discount-smoke"
            && reference.path == "t/app.t"
    }));
    assert!(actionable.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_test"
            && reference.source_id == "test:t/app.t:discount_smoke"
            && reference.path == "t/app.t"
    }));
    assert!(actionable.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_test_file"
            && reference.source_id == "file:t/app.t"
            && reference.path == "t/app.t"
    }));
    assert!(actionable.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_oracle"
            && reference.source_id == "oracle:t/app.t:7:is"
            && reference.path == "t/app.t"
    }));
    assert!(actionable.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_verify_command"
            && reference.source_id == "verify:t/app.t:prove"
            && reference.path == "t/app.t"
    }));
    assert!(actionable.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_provenance"
            && reference.source_id == "prov:diff:return"
            && reference.path == "lib/My/App.pm"
    }));

    Ok(())
}

#[test]
fn perl_strict_actionability_uses_selected_strict_evidence_for_gap_identity() -> Result<(), String>
{
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let mut packet = PerlAdapter.consume_fact_packet(fixture)?;
    packet.relations.insert(
        0,
        RelationFact {
            relation_id: "relation:return:smoke-first".to_string(),
            change_id: "change:lib/My/App.pm:8:return".to_string(),
            owner_id: "perl:lib/My/App.pm::My::App::discount".to_string(),
            test_id: "test:t/app.t:discount_smoke".to_string(),
            oracle_id: Some("oracle:t/app.t:6:ok".to_string()),
            relation_kind: RelationKind::DirectOwnerCall,
            reachability_hint: ReachabilityHint::Reachable,
            confidence: Confidence::High,
            provenance_refs: vec!["prov:relation:return".to_string()],
        },
    );

    let advisory_first_gap = packet
        .canonical_gap_identity_for_change("change:lib/My/App.pm:8:return")
        .ok_or_else(|| "missing canonical gap identity".to_string())?;
    assert_eq!(advisory_first_gap.assertion_shape, "smoke_ok");

    let expected_strict_gap = packet
        .canonical_gap_identity_for_change_with_assertion_shape(
            "change:lib/My/App.pm:8:return",
            "exact_return_assertion",
        )
        .ok_or_else(|| "missing strict canonical gap identity".to_string())?;
    let actionable = packet
        .strict_actionability_for_change(
            "change:lib/My/App.pm:8:return",
            &complete_perl_actionability_context(),
        )
        .map_err(|err| format!("{err:?}"))?;

    assert_ne!(actionable.canonical_gap_id, advisory_first_gap.id);
    assert_eq!(actionable.canonical_gap_id, expected_strict_gap.id);
    assert_eq!(
        actionable.target_test_shape,
        "Test::More exact_return_assertion"
    );

    Ok(())
}

#[test]
fn perl_repair_card_and_agent_packet_project_strict_actionability() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let packet = PerlAdapter.consume_fact_packet(fixture)?;
    let context = complete_perl_actionability_context();
    let card = packet
        .repair_card_for_change("change:lib/My/App.pm:8:return", &context)
        .map_err(|err| format!("{err:?}"))?;

    assert_eq!(card.card_version, "perl_repair_card.v1");
    assert_eq!(card.source, "perl_adapter_strict_actionability");
    assert_eq!(card.language, "perl");
    assert_eq!(card.language_status, "preview");
    assert_eq!(card.authority_boundary, "preview_advisory_only");
    assert_eq!(card.projection_scope, "internal_adapter_only");
    assert!(!card.public_repair_packet);
    assert!(!card.public_projection_ready);
    assert_eq!(card.gap_state, "actionable");
    assert_eq!(card.changed_owner, "perl:lib/My/App.pm::My::App::discount");
    assert_eq!(card.evidence_class, "weakly_exposed");
    assert_eq!(card.repair_kind, "add_exact_return_assertion");
    assert_eq!(card.missing_discriminator, "return_value");
    assert_eq!(card.target_test_shape, "Test::More exact_return_assertion");
    assert_eq!(card.suggested_test_location, "t/app.t::discount_smoke");
    assert_eq!(
        card.suggested_assertion,
        "assert the exact returned `return_value` value"
    );
    assert_eq!(card.verify_command, "prove t/app.t");
    assert_eq!(
        card.receipt_command,
        "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id perl-gap --json"
    );
    assert_eq!(card.confidence, "medium");
    assert_eq!(card.allowed_edit_boundaries, ["t/app.t"]);
    assert_eq!(
        card.forbidden_edit_boundaries,
        ["lib/My/App.pm", "badges/ripr-plus.json"]
    );
    assert!(
        card.current_test_evidence
            .contains("test:t/app.t:discount_smoke")
    );
    assert!(card.raw_evidence_refs.iter().any(|reference| {
        reference.kind == "perl_provenance"
            && reference.source_id == "prov:oracle:exact-return"
            && reference.path == "t/app.t"
    }));

    let agent_packet = packet
        .agent_packet_for_change("change:lib/My/App.pm:8:return", &context)
        .map_err(|err| format!("{err:?}"))?;
    assert_eq!(agent_packet.packet_version, "perl_internal_agent_packet.v1");
    assert_eq!(agent_packet.packet_id, card.packet_id);
    assert_eq!(agent_packet.canonical_gap_id, card.canonical_gap_id);
    assert_eq!(agent_packet.language, "perl");
    assert_eq!(agent_packet.language_status, "preview");
    assert_eq!(agent_packet.authority_boundary, "preview_advisory_only");
    assert_eq!(agent_packet.projection_scope, "internal_adapter_only");
    assert_eq!(agent_packet.gap_state, "actionable");
    assert_eq!(agent_packet.evidence_class, "weakly_exposed");
    assert!(agent_packet.repair_packet_ready);
    assert!(!agent_packet.public_repair_packet);
    assert!(!agent_packet.public_projection_ready);
    assert_eq!(agent_packet.repair_route, "add_exact_return_assertion");
    assert_eq!(agent_packet.changed_owner, card.changed_owner);
    assert_eq!(
        agent_packet.missing_discriminator,
        card.missing_discriminator
    );
    assert_eq!(agent_packet.target_test_shape, card.target_test_shape);
    assert_eq!(
        agent_packet.suggested_test_location,
        card.suggested_test_location
    );
    assert_eq!(agent_packet.verify_command, card.verify_command);
    assert_eq!(agent_packet.receipt_command, card.receipt_command);
    assert_eq!(agent_packet.verify_command_argv, ["prove", "t/app.t"]);
    assert_eq!(
        agent_packet.receipt_command_argv,
        [
            "ripr",
            "agent",
            "receipt",
            "--root",
            ".",
            "--verify-json",
            "target/ripr/workflow/agent-verify.json",
            "--seam-id",
            "perl-gap",
            "--json"
        ]
    );
    assert_eq!(agent_packet.confidence, card.confidence);
    assert_eq!(agent_packet.allowed_edit_surface, ["t/app.t"]);
    assert_eq!(
        agent_packet.forbidden_files,
        ["lib/My/App.pm", "badges/ripr-plus.json"]
    );
    assert_eq!(agent_packet.stop_if, card.stop_if);
    assert_eq!(agent_packet.must_not_change, card.must_not_change);
    assert_eq!(agent_packet.raw_evidence_refs, card.raw_evidence_refs);

    Ok(())
}

#[test]
fn perl_private_repair_card_and_agent_packet_json_preserve_internal_projection_boundary()
-> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let packet = PerlAdapter.consume_fact_packet(fixture)?;
    let context = complete_perl_actionability_context();
    let card = packet
        .repair_card_for_change("change:lib/My/App.pm:8:return", &context)
        .map_err(|err| format!("{err:?}"))?;
    let agent_packet = packet
        .agent_packet_for_change("change:lib/My/App.pm:8:return", &context)
        .map_err(|err| format!("{err:?}"))?;

    let card_json = card.json_value();
    assert_eq!(card_json["card_version"], "perl_repair_card.v1");
    assert_eq!(card_json["language"], "perl");
    assert_eq!(card_json["language_status"], "preview");
    assert_eq!(card_json["authority_boundary"], "preview_advisory_only");
    assert_eq!(card_json["projection_scope"], "internal_adapter_only");
    assert_eq!(card_json["public_repair_packet"], false);
    assert_eq!(card_json["public_projection_ready"], false);
    assert_eq!(card_json["gap_state"], "actionable");
    assert_eq!(
        card_json["canonical_gap_id"],
        card.canonical_gap_id.as_str()
    );
    assert_eq!(card_json["verify"]["command"], "prove t/app.t");
    assert_eq!(
        card_json["receipt"]["command"],
        "ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id perl-gap --json"
    );
    assert!(
        card_json["raw_evidence_refs"]
            .as_array()
            .is_some_and(|refs| refs.iter().any(|reference| {
                reference["kind"] == "perl_provenance"
                    && reference["source_id"] == "prov:oracle:exact-return"
                    && reference["path"] == "t/app.t"
            }))
    );

    let agent_json = agent_packet.json_value();
    assert_eq!(
        agent_json["packet_version"],
        "perl_internal_agent_packet.v1"
    );
    assert_eq!(agent_json["language"], "perl");
    assert_eq!(agent_json["language_status"], "preview");
    assert_eq!(agent_json["authority_boundary"], "preview_advisory_only");
    assert_eq!(agent_json["projection_scope"], "internal_adapter_only");
    assert_eq!(agent_json["repair_packet_ready"], true);
    assert_eq!(agent_json["public_repair_packet"], false);
    assert_eq!(agent_json["public_projection_ready"], false);
    assert_eq!(
        agent_json["commands"]["verify"]["argv"],
        serde_json::json!(["prove", "t/app.t"])
    );
    assert_eq!(
        agent_json["commands"]["receipt"]["argv"],
        serde_json::json!([
            "ripr",
            "agent",
            "receipt",
            "--root",
            ".",
            "--verify-json",
            "target/ripr/workflow/agent-verify.json",
            "--seam-id",
            "perl-gap",
            "--json"
        ])
    );
    assert_eq!(
        agent_json["allowed_edit_surface"],
        serde_json::json!(["t/app.t"])
    );
    assert_eq!(
        agent_json["forbidden_files"],
        serde_json::json!(["lib/My/App.pm", "badges/ripr-plus.json"])
    );

    Ok(())
}

#[test]
fn perl_repair_card_and_agent_packet_fail_closed_without_strict_actionability() -> Result<(), String>
{
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let packet = PerlAdapter.consume_fact_packet(fixture)?;
    let mut missing_receipt = complete_perl_actionability_context();
    missing_receipt.receipt_command = None;
    assert_eq!(
        packet.repair_card_for_change("change:lib/My/App.pm:8:return", &missing_receipt),
        Err(PerlActionabilityBlocker::MissingReceiptCommand)
    );
    assert_eq!(
        packet.agent_packet_for_change("change:lib/My/App.pm:8:return", &missing_receipt),
        Err(PerlActionabilityBlocker::MissingReceiptCommand)
    );

    let context = complete_perl_actionability_context();
    let mut weak_oracle = packet.clone();
    let relation = weak_oracle
        .relations
        .iter_mut()
        .find(|relation| relation.relation_id == "relation:return:discount-smoke")
        .ok_or_else(|| "missing return relation".to_string())?;
    relation.oracle_id = Some("oracle:t/app.t:6:ok".to_string());
    assert_eq!(
        weak_oracle.repair_card_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingStrongRelatedEvidence)
    );
    assert_eq!(
        weak_oracle.agent_packet_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingStrongRelatedEvidence)
    );

    Ok(())
}

#[test]
fn perl_strict_actionability_fails_closed_for_missing_or_weak_fields() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let packet = PerlAdapter.consume_fact_packet(fixture)?;
    let context = complete_perl_actionability_context();

    let mut missing_receipt = context.clone();
    missing_receipt.receipt_command = None;
    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &missing_receipt),
        Err(PerlActionabilityBlocker::MissingReceiptCommand)
    );

    let mut invalid_receipt = context.clone();
    invalid_receipt.receipt_command =
        Some(vec!["target/ripr/workflow/agent-verify.json".to_string()]);
    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &invalid_receipt),
        Err(PerlActionabilityBlocker::InvalidReceiptCommand)
    );

    let mut non_receipt_command = context.clone();
    non_receipt_command.receipt_command = Some(vec!["cargo".to_string(), "test".to_string()]);
    assert_eq!(
        packet
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &non_receipt_command),
        Err(PerlActionabilityBlocker::InvalidReceiptCommand)
    );

    let mut incomplete_receipt_command = context.clone();
    incomplete_receipt_command.receipt_command = Some(
        [
            "ripr",
            "agent",
            "receipt",
            "--root",
            ".",
            "--verify-json",
            "target/ripr/workflow/agent-verify.json",
            "--json",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
    );
    assert_eq!(
        packet.strict_actionability_for_change(
            "change:lib/My/App.pm:8:return",
            &incomplete_receipt_command
        ),
        Err(PerlActionabilityBlocker::InvalidReceiptCommand)
    );

    let mut outside_receipt_root = context.clone();
    outside_receipt_root.receipt_command = Some(
        [
            "ripr",
            "agent",
            "receipt",
            "--root",
            "../outside",
            "--verify-json",
            "target/ripr/workflow/agent-verify.json",
            "--seam-id",
            "perl-gap",
            "--json",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
    );
    assert_eq!(
        packet.strict_actionability_for_change(
            "change:lib/My/App.pm:8:return",
            &outside_receipt_root
        ),
        Err(PerlActionabilityBlocker::InvalidReceiptCommand)
    );

    let mut missing_allowed = context.clone();
    missing_allowed.allowed_edit_boundaries = vec!["t/other.t".to_string()];
    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &missing_allowed),
        Err(PerlActionabilityBlocker::MissingAllowedEditBoundary)
    );

    let mut production_allowed = context.clone();
    production_allowed
        .allowed_edit_boundaries
        .push("lib/My/App.pm".to_string());
    assert_eq!(
        packet
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &production_allowed),
        Err(PerlActionabilityBlocker::AllowedProductionEditBoundary)
    );

    let mut unsafe_allowed = context.clone();
    unsafe_allowed
        .allowed_edit_boundaries
        .push("../outside.pm".to_string());
    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &unsafe_allowed),
        Err(PerlActionabilityBlocker::UnsafeEditBoundary)
    );

    let mut unrelated_allowed = context.clone();
    unrelated_allowed
        .allowed_edit_boundaries
        .push("t/unrelated.t".to_string());
    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &unrelated_allowed),
        Err(PerlActionabilityBlocker::UnexpectedAllowedEditBoundary)
    );

    let mut missing_forbidden = context.clone();
    missing_forbidden.forbidden_edit_boundaries.clear();
    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &missing_forbidden),
        Err(PerlActionabilityBlocker::MissingForbiddenEditBoundary)
    );

    let mut wrong_forbidden = context.clone();
    wrong_forbidden.forbidden_edit_boundaries = vec!["badges/ripr-plus.json".to_string()];
    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &wrong_forbidden),
        Err(PerlActionabilityBlocker::MissingForbiddenEditBoundary)
    );

    let mut unsafe_forbidden = context.clone();
    unsafe_forbidden
        .forbidden_edit_boundaries
        .push("../outside.pm".to_string());
    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &unsafe_forbidden),
        Err(PerlActionabilityBlocker::UnsafeEditBoundary)
    );

    let mut missing_stop_if = context.clone();
    missing_stop_if.stop_if.clear();
    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &missing_stop_if),
        Err(PerlActionabilityBlocker::MissingStopIf)
    );

    let mut missing_must_not_change = context.clone();
    missing_must_not_change.must_not_change.clear();
    assert_eq!(
        packet.strict_actionability_for_change(
            "change:lib/My/App.pm:8:return",
            &missing_must_not_change
        ),
        Err(PerlActionabilityBlocker::MissingMustNotChange)
    );

    let mut irrelevant_must_not_change = context.clone();
    irrelevant_must_not_change.must_not_change = vec!["do not change unrelated files".to_string()];
    assert_eq!(
        packet.strict_actionability_for_change(
            "change:lib/My/App.pm:8:return",
            &irrelevant_must_not_change
        ),
        Err(PerlActionabilityBlocker::MissingMustNotChange)
    );

    let mut missing_verify = packet.clone();
    missing_verify
        .verify_commands
        .retain(|command| command.test_id.as_deref() != Some("test:t/app.t:discount_smoke"));
    assert_eq!(
        missing_verify.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingVerifyCommand)
    );

    let mut invalid_verify = packet.clone();
    let command = invalid_verify
        .verify_commands
        .iter_mut()
        .find(|command| command.command_id == "verify:t/app.t:prove")
        .ok_or_else(|| "missing prove verify command".to_string())?;
    command.argv = vec!["target/ripr/workflow/agent-verify.json".to_string()];
    assert_eq!(
        invalid_verify.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingVerifyCommand)
    );

    let mut outside_verify_path = packet.clone();
    let command = outside_verify_path
        .verify_commands
        .iter_mut()
        .find(|command| command.command_id == "verify:t/app.t:prove")
        .ok_or_else(|| "missing prove verify command".to_string())?;
    command.argv = vec!["prove".to_string(), "../outside.t".to_string()];
    assert_eq!(
        outside_verify_path
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingVerifyCommand)
    );

    let mut unsafe_verify_arg = packet.clone();
    let command = unsafe_verify_arg
        .verify_commands
        .iter_mut()
        .find(|command| command.command_id == "verify:t/app.t:prove")
        .ok_or_else(|| "missing prove verify command".to_string())?;
    command.argv = vec!["prove".to_string(), "t/app.t".to_string(), "&&".to_string()];
    assert_eq!(
        unsafe_verify_arg
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingVerifyCommand)
    );

    let mut low_verify = packet.clone();
    let command = low_verify
        .verify_commands
        .iter_mut()
        .find(|command| command.command_id == "verify:t/app.t:prove")
        .ok_or_else(|| "missing prove verify command".to_string())?;
    command.confidence = Confidence::Low;
    assert_eq!(
        low_verify.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::LowConfidence)
    );

    let mut weak_oracle = packet.clone();
    let relation = weak_oracle
        .relations
        .iter_mut()
        .find(|relation| relation.relation_id == "relation:return:discount-smoke")
        .ok_or_else(|| "missing return relation".to_string())?;
    relation.oracle_id = Some("oracle:t/app.t:6:ok".to_string());
    assert_eq!(
        weak_oracle.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingStrongRelatedEvidence)
    );

    for weak_kind in [
        OracleKind::MentionOnly,
        OracleKind::DiesOnly,
        OracleKind::UnknownHelper,
        OracleKind::DynamicFrameworkIndirection,
    ] {
        let mut weak_kind_packet = packet.clone();
        let oracle = weak_kind_packet
            .oracles
            .iter_mut()
            .find(|oracle| oracle.oracle_id == "oracle:t/app.t:7:is")
            .ok_or_else(|| "missing exact return oracle".to_string())?;
        oracle.kind = weak_kind;
        assert_eq!(
            weak_kind_packet
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
            Err(PerlActionabilityBlocker::MissingStrongRelatedEvidence)
        );
    }

    let mut shape_mismatch = packet.clone();
    let oracle = shape_mismatch
        .oracles
        .iter_mut()
        .find(|oracle| oracle.oracle_id == "oracle:t/app.t:7:is")
        .ok_or_else(|| "missing exact return oracle".to_string())?;
    oracle.kind = OracleKind::PredicateBoundaryAssertion;
    assert_eq!(
        shape_mismatch.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::OracleShapeMismatch)
    );

    let mut unsupported_framework = packet.clone();
    let test = unsupported_framework
        .tests
        .iter_mut()
        .find(|test| test.test_id == "test:t/app.t:discount_smoke")
        .ok_or_else(|| "missing discount smoke test".to_string())?;
    test.framework = TestFramework::Unknown;
    assert_eq!(
        unsupported_framework
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::UnsupportedTestFramework)
    );

    let mut unsupported_behavior = packet.clone();
    let change = unsupported_behavior
        .changes
        .iter_mut()
        .find(|change| change.change_id == "change:lib/My/App.pm:8:return")
        .ok_or_else(|| "missing return change".to_string())?;
    change.behavior_hint = BehaviorHint::CallEffect;
    assert_eq!(
        unsupported_behavior
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::UnsupportedBehavior)
    );

    let mut unknown_behavior = packet.clone();
    let change = unknown_behavior
        .changes
        .iter_mut()
        .find(|change| change.change_id == "change:lib/My/App.pm:8:return")
        .ok_or_else(|| "missing return change".to_string())?;
    change.behavior_hint = BehaviorHint::Unknown;
    assert_eq!(
        unknown_behavior.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::UnsupportedBehavior)
    );

    let mut low_confidence = packet.clone();
    let relation = low_confidence
        .relations
        .iter_mut()
        .find(|relation| relation.relation_id == "relation:return:discount-smoke")
        .ok_or_else(|| "missing return relation".to_string())?;
    relation.confidence = Confidence::Low;
    assert_eq!(
        low_confidence.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::LowConfidence)
    );

    let mut unknown_owner = packet.clone();
    let owner = unknown_owner
        .owners
        .iter_mut()
        .find(|owner| owner.owner_id == "perl:lib/My/App.pm::My::App::discount")
        .ok_or_else(|| "missing discount owner".to_string())?;
    owner.kind = OwnerKind::Unknown;
    assert_eq!(
        unknown_owner.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingCanonicalGapId)
    );

    let mut dynamic_boundary = packet.clone();
    dynamic_boundary
        .dynamic_boundaries
        .push(DynamicBoundaryFact {
            boundary_id: "boundary:lib/My/App.pm:discount:dynamic".to_string(),
            kind: BoundaryKind::DynamicDispatch,
            file_id: "file:lib/My/App.pm".to_string(),
            owner_id: Some("perl:lib/My/App.pm::My::App::discount".to_string()),
            range: RangeFact {
                start_line: 8,
                start_column: 5,
                end_line: 8,
                end_column: 14,
            },
            confidence: Confidence::Medium,
            provenance_refs: vec!["prov:dynamic-boundary:discount".to_string()],
        });
    assert_eq!(
        dynamic_boundary.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::DynamicBoundary)
    );

    let mut test_file_boundary = packet.clone();
    test_file_boundary
        .dynamic_boundaries
        .push(DynamicBoundaryFact {
            boundary_id: "boundary:t/app.t:framework".to_string(),
            kind: BoundaryKind::FrameworkIndirection,
            file_id: "file:t/app.t".to_string(),
            owner_id: None,
            range: RangeFact {
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
            },
            confidence: Confidence::Medium,
            provenance_refs: vec!["prov:test-discovery:more".to_string()],
        });
    assert_eq!(
        test_file_boundary
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::DynamicBoundary)
    );

    let mut relevant_limitation = packet.clone();
    relevant_limitation.limitations.push(LimitationFact {
        limitation_id: "limitation:framework-indirection:return".to_string(),
        kind: BoundaryKind::FrameworkIndirection,
        message: "dynamic framework indirection touches selected oracle".to_string(),
        evidence_refs: vec!["oracle:t/app.t:7:is".to_string()],
    });
    assert_eq!(
        relevant_limitation
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::DynamicBoundary)
    );

    let mut missing_provenance_refs = packet.clone();
    let change = missing_provenance_refs
        .changes
        .iter_mut()
        .find(|change| change.change_id == "change:lib/My/App.pm:8:return")
        .ok_or_else(|| "missing return change".to_string())?;
    change.provenance_refs.clear();
    assert_eq!(
        missing_provenance_refs
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingProvenanceRefs)
    );

    let mut missing_file_provenance_refs = packet.clone();
    let test_file = missing_file_provenance_refs
        .files
        .iter_mut()
        .find(|file| file.file_id == "file:t/app.t")
        .ok_or_else(|| "missing test file fact".to_string())?;
    test_file.provenance_refs.clear();
    assert_eq!(
        missing_file_provenance_refs
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingProvenanceRefs)
    );

    let mut unresolved_provenance = packet.clone();
    unresolved_provenance
        .provenance
        .retain(|provenance| provenance.provenance_id != "prov:diff:return");
    assert_eq!(
        unresolved_provenance
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::MissingProvenanceRefs)
    );

    assert_eq!(
        packet.strict_actionability_for_change("missing-change", &context),
        Err(PerlActionabilityBlocker::MissingChange)
    );

    let partial = PerlAdapter.consume_fact_packet(PARTIAL_DYNAMIC_BOUNDARY_PACKET)?;
    assert_eq!(
        partial.strict_actionability_for_change("change:lib/My/App.pm:22:call", &context),
        Err(PerlActionabilityBlocker::PacketNotComplete)
    );

    Ok(())
}

#[test]
fn perl_dynamic_boundary_kinds_fail_closed_before_canonical_gap_debt() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let context = complete_perl_actionability_context();

    for (kind, label) in blocking_boundary_kind_cases() {
        let mut packet = PerlAdapter.consume_fact_packet(fixture)?;
        let change = packet
            .change("change:lib/My/App.pm:8:return")
            .ok_or_else(|| "missing return change".to_string())?;
        packet.dynamic_boundaries = vec![DynamicBoundaryFact {
            boundary_id: format!("limit:lib/My/App.pm:{label}:8"),
            kind,
            file_id: "file:lib/My/App.pm".to_string(),
            owner_id: Some("perl:lib/My/App.pm::My::App::discount".to_string()),
            range: change.range,
            confidence: Confidence::High,
            provenance_refs: vec!["prov:syntax:discount".to_string()],
        }];

        assert!(
            packet
                .canonical_gap_identity_for_change("change:lib/My/App.pm:8:return")
                .is_none(),
            "{label} must not become canonical gap debt"
        );
        assert_eq!(
            packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
            Err(PerlActionabilityBlocker::DynamicBoundary),
            "{label} must fail closed before strict actionability"
        );
        assert_eq!(
            packet.repair_card_for_change("change:lib/My/App.pm:8:return", &context),
            Err(PerlActionabilityBlocker::DynamicBoundary),
            "{label} must not emit a repair card"
        );
        assert_eq!(
            packet.agent_packet_for_change("change:lib/My/App.pm:8:return", &context),
            Err(PerlActionabilityBlocker::DynamicBoundary),
            "{label} must not emit an agent packet"
        );
    }

    Ok(())
}

#[test]
fn perl_blocking_limitation_kinds_fail_closed_before_repair_packets() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let context = complete_perl_actionability_context();

    for (kind, label) in blocking_boundary_kind_cases() {
        let mut packet = PerlAdapter.consume_fact_packet(fixture)?;
        packet.limitations = vec![LimitationFact {
            limitation_id: format!("limitation:{label}:discount"),
            kind,
            message: format!("{label} blocks strict Perl actionability"),
            evidence_refs: vec!["change:lib/My/App.pm:8:return".to_string()],
        }];

        assert!(
            packet
                .canonical_gap_identity_for_change("change:lib/My/App.pm:8:return")
                .is_some(),
            "{label} limitation should block repair projection without hiding raw gap identity"
        );
        assert_eq!(
            packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
            Err(PerlActionabilityBlocker::DynamicBoundary),
            "{label} limitation must fail closed before strict actionability"
        );
        assert_eq!(
            packet.repair_card_for_change("change:lib/My/App.pm:8:return", &context),
            Err(PerlActionabilityBlocker::DynamicBoundary),
            "{label} limitation must not emit a repair card"
        );
        assert_eq!(
            packet.agent_packet_for_change("change:lib/My/App.pm:8:return", &context),
            Err(PerlActionabilityBlocker::DynamicBoundary),
            "{label} limitation must not emit an agent packet"
        );
    }

    Ok(())
}

#[test]
fn perl_owner_identity_is_packet_canonical_and_path_qualified() -> Result<(), String> {
    let packet = PerlAdapter.consume_fact_packet(EXACT_RETURN_PACKET)?;
    let owner = packet
        .canonical_owner_identity("perl:lib/My/App.pm::My::App::discount")
        .ok_or_else(|| "missing canonical owner identity".to_string())?;

    assert_eq!(owner.id, "perl:lib/My/App.pm::My::App::discount");
    assert_eq!(owner.file_path, "lib/My/App.pm");
    assert_eq!(owner.kind, "sub");
    assert_eq!(owner.package.as_deref(), Some("My::App"));
    assert_eq!(owner.name.as_deref(), Some("discount"));

    Ok(())
}

#[test]
fn perl_gap_identity_uses_owner_behavior_discriminator_and_assertion_shape() -> Result<(), String> {
    let packet = PerlAdapter.consume_fact_packet(EXACT_RETURN_PACKET)?;
    let gap = packet
        .canonical_gap_identity_for_change("change:lib/My/App.pm:15:return")
        .ok_or_else(|| "missing canonical gap identity".to_string())?;

    assert_eq!(gap.owner_id, "perl:lib/My/App.pm::My::App::discount");
    assert_eq!(gap.behavior_kind, "return_value");
    assert_eq!(gap.missing_discriminator, "return_value");
    assert_eq!(gap.assertion_shape, "exact_return_assertion");
    assert_eq!(
        gap.id,
        canonical_perl_gap_id([
            "perl:lib/My/App.pm::My::App::discount",
            "return_value",
            "return_value",
            "exact_return_assertion"
        ])
    );

    Ok(())
}

#[test]
fn perl_gap_identity_is_stable_across_locator_and_fact_id_movement() -> Result<(), String> {
    let original = PerlAdapter.consume_fact_packet(EXACT_RETURN_PACKET)?;
    let moved_text = EXACT_RETURN_PACKET
        .replace(
            "change:lib/My/App.pm:15:return",
            "change:lib/My/App.pm:99:return",
        )
        .replace(
            "test:t/app.t:test_discount_threshold",
            "test:t/app.t:test_discount_threshold_moved",
        )
        .replace("oracle:t/app.t:8:is", "oracle:t/app.t:88:is")
        .replace(
            r#""range": {"start_line": 15, "start_column": 10, "end_line": 15, "end_column": 18}"#,
            r#""range": {"start_line": 99, "start_column": 10, "end_line": 99, "end_column": 18}"#,
        );
    let moved = PerlAdapter.consume_fact_packet(&moved_text)?;

    let original_gap = original
        .canonical_gap_identity_for_change("change:lib/My/App.pm:15:return")
        .ok_or_else(|| "missing original canonical gap identity".to_string())?;
    let moved_gap = moved
        .canonical_gap_identity_for_change("change:lib/My/App.pm:99:return")
        .ok_or_else(|| "missing moved canonical gap identity".to_string())?;

    assert_eq!(original_gap.id, moved_gap.id);
    assert_eq!(original_gap.owner_id, moved_gap.owner_id);
    assert_eq!(original_gap.behavior_kind, moved_gap.behavior_kind);

    Ok(())
}

#[test]
fn perl_gap_identity_fails_closed_for_unknown_owner() -> Result<(), String> {
    let unknown_owner_text =
        EXACT_RETURN_PACKET.replacen(r#""kind": "sub""#, r#""kind": "unknown""#, 1);
    let packet = PerlAdapter.consume_fact_packet(&unknown_owner_text)?;

    assert!(
        packet
            .canonical_owner_identity("perl:lib/My/App.pm::My::App::discount")
            .is_none(),
        "unknown owners must not become canonical owner identities"
    );
    assert!(
        packet
            .canonical_gap_identity_for_change("change:lib/My/App.pm:15:return")
            .is_none(),
        "unknown owners must not become canonical gap debt"
    );

    Ok(())
}

#[test]
fn perl_gap_identity_fails_closed_for_partial_dynamic_boundary_packet() -> Result<(), String> {
    let packet = PerlAdapter.consume_fact_packet(PARTIAL_DYNAMIC_BOUNDARY_PACKET)?;

    assert!(
        packet
            .canonical_gap_identity_for_change("change:lib/My/App.pm:22:call")
            .is_none(),
        "partial dynamic-boundary packets must not receive canonical gap debt"
    );

    Ok(())
}

#[test]
fn perl_identity_mapping_tables_cover_supported_values() {
    let owner_cases = [
        (OwnerKind::Package, "package"),
        (OwnerKind::Sub, "sub"),
        (OwnerKind::Method, "method"),
        (OwnerKind::Script, "script"),
        (OwnerKind::ModuleInitializer, "module_initializer"),
        (OwnerKind::TestSub, "test_sub"),
        (OwnerKind::Unknown, "unknown"),
    ];
    for (kind, expected) in owner_cases {
        assert_eq!(kind.as_str(), expected);
    }

    let behavior_cases = [
        (
            BehaviorHint::PredicateBoundary,
            "predicate_boundary",
            "predicate_boundary",
            "predicate_boundary_assertion",
        ),
        (
            BehaviorHint::ReturnValue,
            "return_value",
            "return_value",
            "exact_return_assertion",
        ),
        (
            BehaviorHint::ExceptionPath,
            "exception_path",
            "exception_observer",
            "exception_observer",
        ),
        (
            BehaviorHint::HashOrObjectField,
            "hash_or_object_field",
            "hash_or_object_field",
            "hash_or_object_field_assertion",
        ),
        (
            BehaviorHint::OutputObserver,
            "output_observer",
            "output_observer",
            "output_observer",
        ),
        (
            BehaviorHint::WarnObserver,
            "warn_observer",
            "warn_observer",
            "warn_observer",
        ),
        (
            BehaviorHint::LogObserver,
            "log_observer",
            "log_observer",
            "log_observer",
        ),
        (
            BehaviorHint::CallEffect,
            "call_effect",
            "call_effect",
            "side_effect_observer",
        ),
        (
            BehaviorHint::Unknown,
            "unknown",
            "unknown_discriminator",
            "unknown_assertion",
        ),
    ];
    for (hint, expected_kind, expected_discriminator, expected_shape) in behavior_cases {
        assert_eq!(hint.as_str(), expected_kind);
        assert_eq!(hint.default_missing_discriminator(), expected_discriminator);
        assert_eq!(hint.default_assertion_shape(), expected_shape);
    }

    let oracle_cases = [
        (OracleKind::ExactReturnAssertion, "exact_return_assertion"),
        (
            OracleKind::PredicateBoundaryAssertion,
            "predicate_boundary_assertion",
        ),
        (OracleKind::ExceptionObserver, "exception_observer"),
        (
            OracleKind::HashOrObjectFieldAssertion,
            "hash_or_object_field_assertion",
        ),
        (OracleKind::OutputObserver, "output_observer"),
        (OracleKind::WarnObserver, "warn_observer"),
        (OracleKind::LogObserver, "log_observer"),
        (OracleKind::SmokeOk, "smoke_ok"),
        (OracleKind::MentionOnly, "mention_only"),
        (OracleKind::DiesOnly, "dies_only"),
        (OracleKind::UnknownHelper, "unknown_helper"),
        (
            OracleKind::DynamicFrameworkIndirection,
            "dynamic_framework_indirection",
        ),
        (OracleKind::Unknown, "unknown_assertion"),
    ];
    for (kind, expected) in oracle_cases {
        assert_eq!(kind.assertion_shape(), expected);
    }
}

#[test]
fn perl_lsp_export_request_renders_deterministic_batch_command() -> Result<(), String> {
    let request = PerlLspFactExportRequest::new(
        ".",
        "target/ripr/reports/perl-facts.json",
        [
            PerlFactClass::Tests,
            PerlFactClass::Owners,
            PerlFactClass::Oracles,
            PerlFactClass::Changes,
            PerlFactClass::Owners,
        ],
    )?
    .with_diff_range("origin/main", "HEAD");

    let command = request.render_command();

    assert_eq!(command.program, "perl-lsp");
    assert_eq!(
        command.argv,
        [
            "ripr-facts",
            "--schema",
            "ripr-perl-facts-v1",
            "--root",
            ".",
            "--base",
            "origin/main",
            "--head",
            "HEAD",
            "--fact-classes",
            "owners,changes,tests,oracles",
            "--out",
            "target/ripr/reports/perl-facts.json"
        ]
    );

    Ok(())
}

#[test]
fn perl_lsp_export_request_covers_all_fact_class_labels() {
    let cases = [
        (PerlFactClass::Files, "files"),
        (PerlFactClass::Owners, "owners"),
        (PerlFactClass::Changes, "changes"),
        (PerlFactClass::Tests, "tests"),
        (PerlFactClass::Oracles, "oracles"),
        (PerlFactClass::Relations, "relations"),
        (PerlFactClass::DynamicBoundaries, "dynamic_boundaries"),
        (PerlFactClass::VerifyCommands, "verify_commands"),
        (PerlFactClass::Limitations, "limitations"),
        (PerlFactClass::Provenance, "provenance"),
    ];

    for (fact_class, expected) in cases {
        assert_eq!(fact_class.as_str(), expected);
    }
}

#[test]
fn perl_lsp_export_request_rejects_host_specific_paths() -> Result<(), String> {
    let host_qualified_root = PerlLspFactExportRequest::new(
        "host:repo",
        "target/ripr/reports/perl-facts.json",
        [PerlFactClass::Owners],
    );
    let backslash_out = PerlLspFactExportRequest::new(
        ".",
        r"target\ripr\reports\perl-facts.json",
        [PerlFactClass::Owners],
    );
    let parent_path_out = PerlLspFactExportRequest::new(
        ".",
        "../target/ripr/reports/perl-facts.json",
        [PerlFactClass::Owners],
    );

    assert!(
        matches!(host_qualified_root, Err(ref message) if message.contains("repo-relative")),
        "host-qualified roots must not enter deterministic exporter requests"
    );
    assert!(
        matches!(backslash_out, Err(ref message) if message.contains("repo-relative")),
        "backslash paths must not enter deterministic exporter requests"
    );
    assert!(
        matches!(parent_path_out, Err(ref message) if message.contains("repo-relative")),
        "parent-relative paths must not leave the repository"
    );

    Ok(())
}

#[test]
fn perl_lsp_exporter_unavailable_stays_non_actionable() {
    let unavailable =
        PerlLspFactExportRequest::exporter_unavailable("perl-lsp exporter was not found");

    assert_eq!(unavailable.packet_status, PacketStatus::Unavailable);
    assert_eq!(unavailable.limitation_kind, BoundaryKind::PacketIncomplete);
    assert!(unavailable.reason.contains("not found"));
}

// ── Typed runner-command validation (Campaign 31 PR 13, #1406) ──
// These tests prove the positional matching is replaced by a typed model
// that recognizes prove flags (-l/-lv/-Ilib/-v/etc.) before trailing test
// paths. Red-then-green: these would FAIL before PR 13 because the old
// code treated every arg after "prove" as a test path.

#[test]
fn typed_prove_accepts_lib_flag() {
    let cmd = vec!["prove".to_string(), "-l".to_string(), "t/app.t".to_string()];
    assert!(is_verify_command(&cmd), "prove -l t/app.t must validate");
}

#[test]
fn typed_prove_accepts_lib_verbose_flag() {
    let cmd = vec![
        "prove".to_string(),
        "-lv".to_string(),
        "t/app.t".to_string(),
    ];
    assert!(is_verify_command(&cmd), "prove -lv t/app.t must validate");
}

#[test]
fn typed_prove_accepts_include_flag() {
    let cmd = vec![
        "prove".to_string(),
        "-Ilib".to_string(),
        "t/app.t".to_string(),
    ];
    assert!(is_verify_command(&cmd), "prove -Ilib t/app.t must validate");
}

#[test]
fn typed_prove_accepts_long_verbose_flag() {
    let cmd = vec![
        "prove".to_string(),
        "--verbose".to_string(),
        "t/app.t".to_string(),
    ];
    assert!(
        is_verify_command(&cmd),
        "prove --verbose t/app.t must validate"
    );
}

#[test]
fn typed_carton_exec_prove_accepts_lib_flag() {
    let cmd = vec![
        "carton".to_string(),
        "exec".to_string(),
        "prove".to_string(),
        "-l".to_string(),
        "t/app.t".to_string(),
    ];
    assert!(
        is_verify_command(&cmd),
        "carton exec prove -l t/app.t must validate"
    );
}

#[test]
fn typed_prove_rejects_unknown_flag() {
    let cmd = vec!["prove".to_string(), "-Z".to_string(), "t/app.t".to_string()];
    assert!(
        !is_verify_command(&cmd),
        "prove -Z t/app.t must reject (unknown flag, fail-closed)"
    );
}

#[test]
fn typed_prove_rejects_non_repo_relative_test_path() {
    let cmd = vec![
        "prove".to_string(),
        "-l".to_string(),
        "/etc/passwd".to_string(),
    ];
    assert!(
        !is_verify_command(&cmd),
        "prove -l /etc/passwd must reject (non-repo-relative path)"
    );
}

#[test]
fn typed_prove_rejects_no_test_path() {
    let cmd = vec!["prove".to_string(), "-l".to_string()];
    assert!(
        !is_verify_command(&cmd),
        "prove -l (no test path) must reject"
    );
}

#[test]
fn typed_prove_still_accepts_bare_command() {
    let cmd = vec!["prove".to_string(), "t/app.t".to_string()];
    assert!(
        is_verify_command(&cmd),
        "prove t/app.t (no flags) must still validate"
    );
}

#[test]
fn typed_prove_accepts_multiple_flags_and_paths() {
    let cmd = vec![
        "prove".to_string(),
        "-l".to_string(),
        "-v".to_string(),
        "t/app.t".to_string(),
        "t/discount.t".to_string(),
    ];
    assert!(
        is_verify_command(&cmd),
        "prove -l -v t/app.t t/discount.t must validate"
    );
}

const EXACT_RETURN_PACKET: &str = r#"{
  "schema_version": "ripr-perl-facts-v1",
  "packet_id": "perl-facts:repo:exact-return",
  "packet_status": "complete",
  "packet_fingerprint": "sha256:exact-return",
  "producer": {
    "name": "perl-lsp",
    "version": "0.0.0-fixture",
    "capabilities": ["syntax", "workspace", "test_facts"]
  },
  "root": {
    "repo_relative": ".",
    "vcs_head": "abc123",
    "path_style": "repo_relative"
  },
  "input": {
    "base": "origin/main",
    "head": "HEAD",
    "diff_id": "sha256:diff",
    "requested_fact_classes": ["owners", "tests", "oracles"]
  },
  "files": [
    {
      "file_id": "file:lib/My/App.pm",
      "path": "lib/My/App.pm",
      "role": ["source"],
      "digest": "sha256:source",
      "package_names": ["My::App"],
      "provenance_refs": ["prov:file-index:source"]
    },
    {
      "file_id": "file:t/app.t",
      "path": "t/app.t",
      "role": ["test"],
      "digest": "sha256:test",
      "package_names": [],
      "provenance_refs": ["prov:file-index:test"]
    }
  ],
  "owners": [
    {
      "owner_id": "perl:lib/My/App.pm::My::App::discount",
      "file_id": "file:lib/My/App.pm",
      "kind": "sub",
      "package": "My::App",
      "name": "discount",
      "range": {"start_line": 12, "start_column": 1, "end_line": 20, "end_column": 2},
      "confidence": "high",
      "provenance_refs": ["prov:syntax:discount"]
    }
  ],
  "changes": [
    {
      "change_id": "change:lib/My/App.pm:15:return",
      "file_id": "file:lib/My/App.pm",
      "owner_id": "perl:lib/My/App.pm::My::App::discount",
      "range": {"start_line": 15, "start_column": 10, "end_line": 15, "end_column": 18},
      "behavior_hint": "return_value",
      "changed_text_digest": "sha256:return",
      "provenance_refs": ["prov:diff:1"]
    }
  ],
  "tests": [
    {
      "test_id": "test:t/app.t:test_discount_threshold",
      "file_id": "file:t/app.t",
      "framework": "Test::More",
      "name": "test_discount_threshold",
      "range": {"start_line": 4, "start_column": 1, "end_line": 12, "end_column": 2},
      "runner_hints": ["prove"],
      "confidence": "medium",
      "provenance_refs": ["prov:test-discovery:1"]
    }
  ],
  "oracles": [
    {
      "oracle_id": "oracle:t/app.t:8:is",
      "test_id": "test:t/app.t:test_discount_threshold",
      "kind": "exact_return_assertion",
      "strength": "strong_exact",
      "target_owner_id": "perl:lib/My/App.pm::My::App::discount",
      "expression": "is($got, 10, 'discount threshold')",
      "range": {"start_line": 8, "start_column": 1, "end_line": 8, "end_column": 37},
      "confidence": "medium",
      "provenance_refs": ["prov:oracle:1"]
    }
  ],
  "relations": [
    {
      "relation_id": "relation:change:discount-return:test:threshold",
      "change_id": "change:lib/My/App.pm:15:return",
      "owner_id": "perl:lib/My/App.pm::My::App::discount",
      "test_id": "test:t/app.t:test_discount_threshold",
      "oracle_id": "oracle:t/app.t:8:is",
      "relation_kind": "direct_owner_call",
      "reachability_hint": "reachable",
      "confidence": "medium",
      "provenance_refs": ["prov:relation:1"]
    }
  ],
  "dynamic_boundaries": [],
  "verify_commands": [
    {
      "command_id": "verify:t/app.t:prove",
      "runner": "prove",
      "argv": ["prove", "t/app.t"],
      "scope": "file",
      "test_id": "test:t/app.t:test_discount_threshold",
      "confidence": "medium",
      "preconditions": ["prove_on_path"],
      "provenance_refs": ["prov:runner:1"]
    }
  ],
  "limitations": [],
  "provenance": [
    {
      "provenance_id": "prov:file-index:source",
      "source": "workspace",
      "file_id": "file:lib/My/App.pm",
      "range": null,
      "confidence": "high"
    },
    {
      "provenance_id": "prov:file-index:test",
      "source": "workspace",
      "file_id": "file:t/app.t",
      "range": null,
      "confidence": "high"
    },
    {
      "provenance_id": "prov:syntax:discount",
      "source": "syntax",
      "file_id": "file:lib/My/App.pm",
      "range": {"start_line": 12, "start_column": 1, "end_line": 20, "end_column": 2},
      "confidence": "high"
    },
    {
      "provenance_id": "prov:diff:1",
      "source": "diff",
      "file_id": "file:lib/My/App.pm",
      "range": {"start_line": 15, "start_column": 10, "end_line": 15, "end_column": 18},
      "confidence": "high"
    },
    {
      "provenance_id": "prov:test-discovery:1",
      "source": "test_discovery",
      "file_id": "file:t/app.t",
      "range": {"start_line": 4, "start_column": 1, "end_line": 12, "end_column": 2},
      "confidence": "medium"
    },
    {
      "provenance_id": "prov:oracle:1",
      "source": "oracle_extraction",
      "file_id": "file:t/app.t",
      "range": {"start_line": 8, "start_column": 1, "end_line": 8, "end_column": 37},
      "confidence": "medium"
    },
    {
      "provenance_id": "prov:relation:1",
      "source": "semantic",
      "file_id": "file:t/app.t",
      "range": {"start_line": 8, "start_column": 1, "end_line": 8, "end_column": 37},
      "confidence": "medium"
    },
    {
      "provenance_id": "prov:runner:1",
      "source": "runner_detection",
      "file_id": "file:t/app.t",
      "range": null,
      "confidence": "medium"
    }
  ]
}"#;

const PARTIAL_DYNAMIC_BOUNDARY_PACKET: &str = r#"{
  "schema_version": "ripr-perl-facts-v1",
  "packet_id": "perl-facts:repo:dynamic-boundary",
  "packet_status": "partial",
  "packet_fingerprint": "sha256:dynamic-boundary",
  "producer": {
    "name": "perl-lsp",
    "version": "0.0.0-fixture",
    "capabilities": ["syntax", "workspace"]
  },
  "root": {
    "repo_relative": ".",
    "vcs_head": "abc123",
    "path_style": "repo_relative"
  },
  "input": {
    "base": "origin/main",
    "head": "HEAD",
    "diff_id": "sha256:diff",
    "requested_fact_classes": ["owners", "tests", "oracles"]
  },
  "files": [
    {
      "file_id": "file:lib/My/App.pm",
      "path": "lib/My/App.pm",
      "role": ["source"],
      "digest": "sha256:source",
      "package_names": ["My::App"],
      "provenance_refs": ["prov:file-index:source"]
    }
  ],
  "owners": [
    {
      "owner_id": "perl:lib/My/App.pm::My::App::discount",
      "file_id": "file:lib/My/App.pm",
      "kind": "sub",
      "package": "My::App",
      "name": "discount",
      "range": {"start_line": 12, "start_column": 1, "end_line": 24, "end_column": 2},
      "confidence": "medium",
      "provenance_refs": ["prov:syntax:discount"]
    }
  ],
  "changes": [
    {
      "change_id": "change:lib/My/App.pm:22:call",
      "file_id": "file:lib/My/App.pm",
      "owner_id": "perl:lib/My/App.pm::My::App::discount",
      "range": {"start_line": 22, "start_column": 3, "end_line": 22, "end_column": 19},
      "behavior_hint": "call_effect",
      "changed_text_digest": "sha256:call",
      "provenance_refs": ["prov:diff:1"]
    }
  ],
  "tests": [
    {
      "test_id": "test:t/app.t:test_dynamic_discount",
      "file_id": "file:t/app.t",
      "framework": "Test::More",
      "name": "test_dynamic_discount",
      "range": {"start_line": 4, "start_column": 1, "end_line": 12, "end_column": 2},
      "runner_hints": ["unknown"],
      "confidence": "low",
      "provenance_refs": ["prov:test-discovery:1"]
    }
  ],
  "oracles": [
    {
      "oracle_id": "oracle:t/app.t:9:ok",
      "test_id": "test:t/app.t:test_dynamic_discount",
      "kind": "smoke_ok",
      "strength": "weak_smoke",
      "target_owner_id": "perl:lib/My/App.pm::My::App::discount",
      "expression": "ok($result)",
      "range": {"start_line": 9, "start_column": 1, "end_line": 9, "end_column": 12},
      "confidence": "low",
      "provenance_refs": ["prov:oracle:1"]
    }
  ],
  "relations": [],
  "dynamic_boundaries": [
    {
      "boundary_id": "limit:lib/My/App.pm:dynamic-dispatch:22",
      "kind": "dynamic_dispatch",
      "file_id": "file:lib/My/App.pm",
      "owner_id": "perl:lib/My/App.pm::My::App::discount",
      "range": {"start_line": 22, "start_column": 3, "end_line": 22, "end_column": 19},
      "confidence": "high",
      "provenance_refs": ["prov:semantic:dynamic:1"]
    }
  ],
  "verify_commands": [],
  "limitations": [
    {
      "limitation_id": "limitation:dynamic-dispatch:discount",
      "kind": "dynamic_dispatch",
      "message": "dynamic dispatch blocks strict Perl actionability",
      "evidence_refs": ["limit:lib/My/App.pm:dynamic-dispatch:22"]
    }
  ],
  "provenance": [
    {
      "provenance_id": "prov:file-index:source",
      "source": "workspace",
      "file_id": "file:lib/My/App.pm",
      "range": null,
      "confidence": "high"
    },
    {
      "provenance_id": "prov:syntax:discount",
      "source": "syntax",
      "file_id": "file:lib/My/App.pm",
      "range": {"start_line": 12, "start_column": 1, "end_line": 24, "end_column": 2},
      "confidence": "medium"
    },
    {
      "provenance_id": "prov:diff:1",
      "source": "diff",
      "file_id": "file:lib/My/App.pm",
      "range": {"start_line": 22, "start_column": 3, "end_line": 22, "end_column": 19},
      "confidence": "high"
    },
    {
      "provenance_id": "prov:test-discovery:1",
      "source": "test_discovery",
      "file_id": "file:t/app.t",
      "range": {"start_line": 4, "start_column": 1, "end_line": 12, "end_column": 2},
      "confidence": "low"
    },
    {
      "provenance_id": "prov:oracle:1",
      "source": "oracle_extraction",
      "file_id": "file:t/app.t",
      "range": {"start_line": 9, "start_column": 1, "end_line": 9, "end_column": 12},
      "confidence": "low"
    },
    {
      "provenance_id": "prov:semantic:dynamic:1",
      "source": "semantic",
      "file_id": "file:lib/My/App.pm",
      "range": {"start_line": 22, "start_column": 3, "end_line": 22, "end_column": 19},
      "confidence": "high"
    }
  ]
}"#;
