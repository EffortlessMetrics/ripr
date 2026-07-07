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

/// A default `AnalysisOptions` for packet-parsing tests: a throwaway temp
/// root, no base/diff. Coherence checks pass (no consumer base to compare
/// against); freshness checks no-op (no source files exist on disk under the
/// temp root, so the loop skips each file).
fn packet_test_options() -> crate::analysis::AnalysisOptions {
    use crate::analysis::{AnalysisMode, AnalysisOptions};
    AnalysisOptions {
        root: std::env::temp_dir().join("ripr-perl-packet-test"),
        base: None,
        diff_file: None,
        mode: AnalysisMode::Draft,
        include_unchanged_tests: false,
        resolve_tsconfig_paths: false,
        perl_facts_path: None,
    }
}

/// Convenience wrapper: consume a packet text with the default test options.
fn consume(packet_text: &str) -> Result<PerlFactPacket, String> {
    PerlAdapter.consume_fact_packet(packet_text, &packet_test_options())
}

fn blocking_boundary_kind_cases() -> [(BoundaryKind, &'static str); 15] {
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
        (BoundaryKind::PartialEmitter, "partial_emitter"),
        (BoundaryKind::Unknown, "unknown"),
    ]
}

fn blocking_limitation_kind_labels() -> [&'static str; 16] {
    [
        "dynamic_dispatch",
        "module_resolution_unknown",
        "generated_symbol",
        "role_composition",
        "monkeypatch_or_symbol_patch",
        "eval_or_string_code",
        "symbol_table_mutation",
        "framework_indirection",
        "unknown_helper",
        "unsupported_syntax",
        "missing_test_runner",
        "missing_diff_owner",
        "narrowed_representation",
        "packet_incomplete",
        "partial_emitter",
        "unknown",
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
    let packet = consume(EXACT_RETURN_PACKET)?;

    assert_eq!(packet.schema_version, crate::app::PERL_FACT_PACKET_SCHEMA);
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
    let err = match consume(
        &EXACT_RETURN_PACKET.replace("\"ripr-perl-facts-v1\"", "\"ripr-perl-facts-v2\""),
    ) {
        Ok(_) => return Err("unknown schema version should fail closed".to_string()),
        Err(err) => err,
    };

    assert!(err.contains("unsupported Perl fact packet schema"));
    assert!(err.contains(crate::app::PERL_FACT_PACKET_SCHEMA));

    Ok(())
}

#[test]
fn perl_fact_packet_adapter_parses_partial_dynamic_boundary_limitation() -> Result<(), String> {
    let packet = consume(PARTIAL_DYNAMIC_BOUNDARY_PACKET)?;

    assert_eq!(packet.packet_status, PacketStatus::Partial);
    assert_eq!(packet.dynamic_boundaries.len(), 1);
    assert_eq!(
        packet.dynamic_boundaries[0].kind,
        BoundaryKind::DynamicDispatch
    );
    assert_eq!(packet.limitations.len(), 1);
    assert_eq!(packet.limitations[0].kind, "dynamic_dispatch");
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
    let packet = consume(EXACT_RETURN_PACKET)?;
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
    let packet = consume(fixture)?;

    assert_eq!(packet.producer.name, "perl-lsp");
    assert_eq!(packet.schema_version, crate::app::PERL_FACT_PACKET_SCHEMA);
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
fn ingestion_accepts_canonical_and_compat_perl_fact_exporters() -> Result<(), String> {
    for producer in ["perl-ripr-facts", "perllsp", "perl-lsp"] {
        let packet = EXACT_RETURN_PACKET.replace(
            "\"name\": \"perl-lsp\"",
            &format!("\"name\": \"{producer}\""),
        );
        let consumed = consume(&bless_fingerprint(&packet))?;
        assert_eq!(consumed.producer.name, producer);
    }

    Ok(())
}

#[test]
fn perl_fact_packet_adapter_preserves_source_test_and_oracle_taxonomy() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let packet = consume(fixture)?;

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
    let packet = consume(fixture)?;

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

    let partial_text = fixture.replace(
        r#""packet_status": "complete""#,
        r#""packet_status": "partial""#,
    );
    let partial_packet = consume(&bless_fingerprint(&partial_text))?;
    assert_eq!(
        partial_packet.classify_change_from_related_tests("change:lib/My/App.pm:8:return"),
        ExposureClass::WeaklyExposed,
        "partial alpha packets with valid relation evidence still support draft exposure classification"
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
    let stale_owner_packet = consume(&bless_fingerprint(&stale_owner_text))?;
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
    let weak_packet = consume(&bless_fingerprint(&weak_text))?;
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
    let static_unknown_packet = consume(&bless_fingerprint(&static_unknown_text))?;
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
    let packet = consume(fixture)?;
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
    let mut packet = consume(fixture)?;
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
    let packet = consume(fixture)?;
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
    let packet = consume(fixture)?;
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
    let packet = consume(fixture)?;
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
    let packet = consume(fixture)?;
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

    let mut unrelated_owner_boundary = packet.clone();
    unrelated_owner_boundary
        .dynamic_boundaries
        .push(DynamicBoundaryFact {
            boundary_id: "boundary:lib/My/App.pm:helper:dynamic".to_string(),
            kind: BoundaryKind::DynamicDispatch,
            file_id: "file:lib/My/App.pm".to_string(),
            owner_id: Some("perl:lib/My/App.pm::My::App::dynamic_method".to_string()),
            range: RangeFact {
                start_line: 20,
                start_column: 5,
                end_line: 20,
                end_column: 14,
            },
            confidence: Confidence::Medium,
            provenance_refs: vec!["prov:dynamic-boundary:helper".to_string()],
        });
    assert_eq!(
        unrelated_owner_boundary
            .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        "owner-scoped dynamic boundaries in the same file must not block unrelated owners"
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
        kind: "framework_indirection".to_string(),
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

    let partial = consume(PARTIAL_DYNAMIC_BOUNDARY_PACKET)?;
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
        let mut packet = consume(fixture)?;
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

    for label in blocking_limitation_kind_labels() {
        let mut packet = consume(fixture)?;
        packet.limitations = vec![LimitationFact {
            limitation_id: format!("limitation:{label}:discount"),
            kind: label.to_string(),
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
fn perl_blocking_limitations_match_relevant_provenance_refs() -> Result<(), String> {
    let fixture = include_str!(
        "../../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
    );
    let context = complete_perl_actionability_context();
    let mut packet = consume(fixture)?;
    packet.limitations = vec![LimitationFact {
        limitation_id: "limitation:narrowed:discount".to_string(),
        kind: "narrowed_representation".to_string(),
        message: "producer narrowed the changed representation".to_string(),
        evidence_refs: vec!["prov:diff:return".to_string()],
    }];

    assert_eq!(
        packet.strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::DynamicBoundary),
        "a limitation tied through provenance refs must fail closed"
    );
    assert_eq!(
        packet.repair_card_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::DynamicBoundary),
        "a provenance-scoped limitation must not emit a repair card"
    );
    assert_eq!(
        packet.agent_packet_for_change("change:lib/My/App.pm:8:return", &context),
        Err(PerlActionabilityBlocker::DynamicBoundary),
        "a provenance-scoped limitation must not emit an agent packet"
    );

    Ok(())
}

#[test]
fn perl_owner_identity_is_packet_canonical_and_path_qualified() -> Result<(), String> {
    let packet = consume(EXACT_RETURN_PACKET)?;
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
    let packet = consume(EXACT_RETURN_PACKET)?;
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
    let original = consume(EXACT_RETURN_PACKET)?;
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
    let moved = consume(&bless_fingerprint(&moved_text))?;

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
    let packet = consume(&unknown_owner_text)?;

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
    let packet = consume(PARTIAL_DYNAMIC_BOUNDARY_PACKET)?;

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

// ── Relation-kind gating tests (Campaign 31 PR 12, #1405) ──
// These prove that strict_actionability_for_change now gates by relation
// KIND, not just exposure class. Only DirectOwnerCall / HelperCall relations
// are eligible; PackageReference / TestNameMatch / FileProximity are
// advisory-only (they fail the find() and return MissingStrongRelatedEvidence).

#[test]
fn relation_gate_accepts_direct_owner_call() {
    // Build a packet where the related test evidence has a DirectOwnerCall
    // relation kind — this must pass the strict-actionability gate.
    let adapter = PerlAdapter;
    let packet_text = EXACT_RETURN_PACKET.replace(
        "\"relation_kind\": \"file_proximity\"",
        "\"relation_kind\": \"direct_owner_call\"",
    );
    let result_parse = consume(&packet_text);
    assert!(result_parse.is_ok(), "valid packet must parse");
    let packet = match result_parse {
        Ok(p) => p,
        Err(_) => return,
    };
    let ctx = complete_perl_actionability_context();
    let result = packet.strict_actionability_for_change("change:lib/My/App.pm:15:return", &ctx);
    // DirectOwnerCall should pass the relation gate (may still fail on other
    // gates, but NOT on MissingStrongRelatedEvidence).
    assert!(
        result.is_ok()
            || !matches!(
                result.as_ref().err(),
                Some(PerlActionabilityBlocker::MissingStrongRelatedEvidence)
            ),
        "DirectOwnerCall must not be rejected for MissingStrongRelatedEvidence"
    );
}

#[test]
fn relation_gate_rejects_file_proximity_only() {
    // The reference packet uses DirectOwnerCall relations. To test the gate,
    // modify the relation_kind to file_proximity — this must fail strict
    // actionability with MissingStrongRelatedEvidence.
    let adapter = PerlAdapter;
    let packet_text = EXACT_RETURN_PACKET.replace(
        "\"relation_kind\": \"direct_owner_call\"",
        "\"relation_kind\": \"file_proximity\"",
    );
    let result_parse = consume(&packet_text);
    assert!(result_parse.is_ok(), "valid packet must parse");
    let packet = match result_parse {
        Ok(p) => p,
        Err(_) => return,
    };
    let ctx = complete_perl_actionability_context();
    let result = packet.strict_actionability_for_change("change:lib/My/App.pm:15:return", &ctx);
    assert!(
        matches!(
            result.as_ref().err(),
            Some(PerlActionabilityBlocker::MissingStrongRelatedEvidence)
        ),
        "file_proximity-only relation must fail strict actionability (advisory-only): {:?}",
        result
    );
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

// ── Ingestion boundary tests (Campaign 31 PR 9, #1402) ──
// These prove the 9 integrity checks reject bad packets. Each test
// constructs a modified packet (from the valid EXACT_RETURN_PACKET) that
// violates one check, then asserts consume_fact_packet returns Err.

#[test]
fn ingestion_rejects_unavailable_packet_status() {
    let packet = EXACT_RETURN_PACKET.replace(
        "\"packet_status\": \"complete\"",
        "\"packet_status\": \"unavailable\"",
    );
    let adapter = PerlAdapter;
    let result = consume(&packet);
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(
        result.is_err(),
        "unavailable packet_status must be rejected"
    );
    assert!(
        err.contains("packet_status is `unavailable`"),
        "error must name the check: {err}"
    );
}

#[test]
fn ingestion_rejects_wrong_producer_name() {
    let packet =
        EXACT_RETURN_PACKET.replace("\"name\": \"perl-lsp\"", "\"name\": \"bogus-producer\"");
    let adapter = PerlAdapter;
    let result = consume(&packet);
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(result.is_err(), "wrong producer name must be rejected");
    assert!(
        err.contains("producer name"),
        "error must name the producer check: {err}"
    );
}

#[test]
fn ingestion_rejects_dangling_relation_owner_id() {
    // Change ONLY the relation's owner_id to nonexistent, leaving the owners
    // array entry intact. Use replacen on the owner_id value string.
    let needle = "perl:lib/My/App.pm::My::App::discount";
    let count = EXACT_RETURN_PACKET.matches(needle).count();
    // Replace the LAST occurrence (the relation's owner_id) with a bogus value.
    let packet = EXACT_RETURN_PACKET.replacen(needle, "perl:NONEXISTENT::Owner", count);
    // Restore the earlier occurrences (owners array + change's owner_id) back.
    let packet = packet.replacen("perl:NONEXISTENT::Owner", needle, count - 1);
    let adapter = PerlAdapter;
    let result = consume(&packet);
    assert!(
        result.is_err(),
        "dangling relation owner_id must be rejected"
    );
}

#[test]
fn ingestion_rejects_dangling_relation_change_id() {
    // Change ONLY the relation's change_id to nonexistent, leaving the changes
    // array entry intact. Use replacen to replace just the last occurrence.
    let needle = "\"change_id\": \"change:lib/My/App.pm:15:return\"";
    let count = EXACT_RETURN_PACKET.matches(needle).count();
    // Replace the LAST occurrence (the relation ref) with a bogus value.
    let packet =
        EXACT_RETURN_PACKET.replacen(needle, "\"change_id\": \"change:NONEXISTENT\"", count);
    // Restore the earlier occurrences (changes array) back to original.
    let packet = packet.replacen("\"change_id\": \"change:NONEXISTENT\"", needle, count - 1);
    let adapter = PerlAdapter;
    let result = consume(&packet);
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(
        result.is_err(),
        "dangling relation change_id must be rejected"
    );
    assert!(
        err.contains("unknown change_id"),
        "error must name the referential-integrity check: {err}"
    );
}

#[test]
fn ingestion_accepts_unresolved_relation_change_id_sentinel() {
    // `change:unresolved` is the producer's explicit unbound-relation sentinel.
    // It is not a dangling change reference and must remain valid for relation-
    // only evidence packets.
    let needle = "\"change_id\": \"change:lib/My/App.pm:15:return\"";
    let count = EXACT_RETURN_PACKET.matches(needle).count();
    let packet =
        EXACT_RETURN_PACKET.replacen(needle, "\"change_id\": \"change:unresolved\"", count);
    let packet = packet.replacen("\"change_id\": \"change:unresolved\"", needle, count - 1);
    let result = consume(&bless_fingerprint(&packet));
    assert!(
        result.is_ok(),
        "`change:unresolved` must be accepted as intentional unbound relation evidence: {:?}",
        result.err()
    );
}

#[test]
fn ingestion_accepts_well_formed_reference_packet() {
    let adapter = PerlAdapter;
    let result = consume(EXACT_RETURN_PACKET);
    assert!(
        result.is_ok(),
        "the well-formed reference packet must pass all ingestion checks: {:?}",
        result.err()
    );
}

// ── Integrity hardening tests (Campaign 31 item 2) ──
// These prove the new ingestion checks reject bad packets, one check per test.
// Each test mutates EXACT_RETURN_PACKET to violate exactly one new check, then
// asserts consume() returns Err naming that check. Mutated packets are blessed
// (fingerprint recomputed) EXCEPT the fingerprint-mismatch test, which by
// definition must keep a stale fingerprint.

#[test]
fn ingestion_rejects_mismatched_packet_fingerprint() {
    // Declare a fingerprint that does NOT match the recomputed value. This is
    // the load-bearing tamper/stale detection: a packet whose declared
    // fingerprint disagrees with its content must be rejected.
    let packet = EXACT_RETURN_PACKET.replace(
        "\"packet_fingerprint\": \"sha256:d23dde44154c2ee8eddf3eae1dd87d585371396ac04608c651b30289a89e74f3\"",
        "\"packet_fingerprint\": \"sha256:0000000000000000000000000000000000000000000000000000000000000000\"",
    );
    let result = consume(&packet);
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(result.is_err(), "a mismatched fingerprint must be rejected");
    assert!(
        err.contains("packet_fingerprint mismatch"),
        "error must name the fingerprint check: {err}"
    );
}

#[test]
fn ingestion_rejects_base_mismatch_against_consumer_request() {
    // The consumer asks for base `feature/x`, but the packet was built for
    // `origin/main`. A cross-branch/cross-repo packet must be rejected.
    let mut options = packet_test_options();
    options.base = Some("feature/x".to_string());
    let result = PerlAdapter.consume_fact_packet(EXACT_RETURN_PACKET, &options);
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(result.is_err(), "a base mismatch must be rejected");
    assert!(
        err.contains("base mismatch"),
        "error must name the base coherence check: {err}"
    );
}

#[test]
fn ingestion_rejects_missing_packet_head() {
    // A complete packet must declare the head it was built against.
    let packet = EXACT_RETURN_PACKET.replace("\"head\": \"HEAD\",", "\"head\": null,");
    let result = consume(&bless_fingerprint(&packet));
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(result.is_err(), "a missing head must be rejected");
    assert!(
        err.contains("input.head"),
        "error must name the head coherence check: {err}"
    );
}

#[test]
fn ingestion_rejects_missing_test_facts_capability() {
    // The packet carries tests/oracles but the producer did not advertise
    // `test_facts`. Strip `test_facts` from the capabilities list.
    let packet = EXACT_RETURN_PACKET.replace(
        "\"capabilities\": [\"syntax\", \"workspace\", \"test_facts\"]",
        "\"capabilities\": [\"syntax\", \"workspace\"]",
    );
    let result = consume(&packet);
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(
        result.is_err(),
        "missing test_facts capability must be rejected"
    );
    assert!(
        err.contains("test_facts"),
        "error must name the capability check: {err}"
    );
}

#[test]
fn ingestion_rejects_malformed_id_with_whitespace() {
    // An ID with internal whitespace is not a stable token (SPEC-0064).
    let packet = EXACT_RETURN_PACKET.replace(
        "change:lib/My/App.pm:15:return",
        "change:lib/My/App.pm:15:bad return",
    );
    let result = consume(&bless_fingerprint(&packet));
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(
        result.is_err(),
        "a whitespace-containing ID must be rejected"
    );
    assert!(
        err.contains("malformed") && err.contains("whitespace"),
        "error must name the ID-format check: {err}"
    );
}

#[test]
fn ingestion_rejects_absolute_file_path() {
    // An absolute file path is not repo-relative (SPEC-0064 path_style). Uses
    // a non-home absolute path so the test fixture string does not trip the
    // repo's check-local-context home-path guard.
    let packet = EXACT_RETURN_PACKET.replace(
        "\"path\": \"lib/My/App.pm\"",
        "\"path\": \"/srv/lib/My/App.pm\"",
    );
    let result = consume(&packet);
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(result.is_err(), "an absolute file path must be rejected");
    assert!(
        err.contains("not repo-relative") && err.contains("absolute"),
        "error must name the path-normalization check: {err}"
    );
}

#[test]
fn ingestion_rejects_backslash_file_path() {
    // A backslash-separated path violates SPEC-0064 (`/`-separated required).
    let packet = EXACT_RETURN_PACKET.replace(
        "\"path\": \"lib/My/App.pm\"",
        "\"path\": \"lib\\\\My\\\\App.pm\"",
    );
    let result = consume(&packet);
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(result.is_err(), "a backslash path must be rejected");
    assert!(
        err.contains("backslash"),
        "error must name the path-separator check: {err}"
    );
}

#[test]
fn ingestion_rejects_stale_file_digest_against_on_disk_source() -> Result<(), String> {
    // Write a real file on disk under the test root at the packet's declared
    // path, then keep the packet's stale `sha256:source` digest. The freshness
    // check recomputes the digest from the on-disk bytes and must reject it.
    let temp =
        std::env::temp_dir().join(format!("ripr-perl-digest-freshness-{}", std::process::id()));
    std::fs::create_dir_all(temp.join("lib/My"))
        .map_err(|err| format!("create dir failed: {err}"))?;
    std::fs::write(
        temp.join("lib/My/App.pm"),
        "# real perl source with different content than the declared digest\n",
    )
    .map_err(|err| format!("write file failed: {err}"))?;
    let mut options = packet_test_options();
    options.root = temp.clone();
    let result = PerlAdapter.consume_fact_packet(EXACT_RETURN_PACKET, &options);
    let err = result.as_ref().err().map(String::as_str).unwrap_or("");
    assert!(result.is_err(), "a stale digest must be rejected");
    assert!(
        err.contains("stale digest"),
        "error must name the digest freshness check: {err}"
    );
    let _ = std::fs::remove_dir_all(&temp);
    Ok(())
}

// ── Managed-mode argv parity with SPEC-0064 (Campaign 31 item 4) ──
// The managed producer (`app::check::invoke_perl_lsp_producer`) builds its
// argv via `perl_facts_export_argv`, which MUST match the surface this
// module's `PerlLspFactExportRequest::render_command` builds. This test pins
// `render_command`'s output to the SPEC-0064-canonical surface; the companion
// test `perl_facts_export_argv_matches_spec_canonical_surface` in
// `app::check::tests` pins the same surface. If either diverges, one fails —
// single-source via test, not via shared code (the perl module is
// cfg(feature = "lang-perl") and the managed producer must compile without it).

#[test]
fn render_command_matches_spec_canonical_surface() -> Result<(), String> {
    let request = PerlLspFactExportRequest::new(
        ".",
        "out.json",
        [
            PerlFactClass::Owners,
            PerlFactClass::Changes,
            PerlFactClass::Tests,
            PerlFactClass::Oracles,
        ],
    )?
    .with_diff_range("origin/main", "HEAD");
    let command = request.render_command();
    assert_eq!(
        command.program, "perl-lsp",
        "the compatibility wrapper program name remains `perl-lsp`; canonical producer identity is `perl-ripr-facts`"
    );
    assert_eq!(
        command.argv,
        vec![
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
            "out.json",
        ]
    );
    assert!(
        !command.argv.iter().any(|arg| arg.starts_with("--ripr-")),
        "render_command must not emit the non-spec `--ripr-*` surface: {:?}",
        command.argv
    );
    Ok(())
}

// ── Two-binary proof harness regression corpus (Campaign 31 item 3) ──
// Three committed regression packets under
// fixtures/perl_cpan_alpha/expected/regression-packets/, one per honest
// outcome (actionable / already-observed / limited). These are REGRESSION
// FIXTURES, not producer proof: they prove the consumer pipeline is intact
// independent of the producer. Real producer proof requires the two-binary
// harness (tests/perl_two_binary_harness.rs) running against real perllsp
// output from perl-lsp-swarm Phase B. Each packet carries real inner digests
// for lib/Pricing.pm and t/pricing.t and is consumed with the real fixture
// root, so item 2's freshness check validates the on-disk source.

/// Resolve the perl_cpan_alpha/input fixture root (where lib/Pricing.pm and
/// t/pricing.t live on disk) for the freshness check.
fn cpan_alpha_input_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/perl_cpan_alpha/input")
}

/// Read one of the three committed regression packets.
fn cpan_alpha_regression_packet(name: &str) -> Result<String, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/perl_cpan_alpha/expected/regression-packets")
        .join(name);
    std::fs::read_to_string(&path).map_err(|err| format!("read regression packet {path:?}: {err}"))
}

#[test]
fn cpan_alpha_actionable_regression_packet_yields_reachable_unrevealed_candidate()
-> Result<(), String> {
    // Outcome 1 (actionable): the boundary change reaches the weak `ok()`
    // oracle but the oracle does not pin the exact boundary. The honest
    // static classification is ReachableUnrevealed — the changed owner is
    // reachable through a direct call but the weak oracle does not reveal
    // the changed behavior. This is the actionable candidate a bounded
    // test-only repair packet would target (the missing discriminator is the
    // boundary equality). NOT Exposed (that needs a strong aligned oracle).
    let text = cpan_alpha_regression_packet("actionable-weak-ok.json")?;
    let mut options = packet_test_options();
    options.root = cpan_alpha_input_root();
    let packet = PerlAdapter
        .consume_fact_packet(&text, &options)
        .map_err(|err| {
            format!("actionable packet must pass ingestion (incl. on-disk freshness): {err}")
        })?;
    let findings = packet_to_findings(&packet);
    let candidate = findings
        .iter()
        .find(|finding| {
            finding.class == ExposureClass::ReachableUnrevealed
                || finding.class == ExposureClass::WeaklyExposed
        })
        .ok_or_else(|| "the boundary change under a weak oracle must yield a reachable-but-not-revealed (actionable) finding, not Exposed".to_string())?;
    assert_ne!(
        candidate.class,
        ExposureClass::Exposed,
        "a weak oracle must not credit the change as Exposed"
    );
    Ok(())
}

#[test]
fn cpan_alpha_already_observed_regression_packet_suppresses_repair_gap() -> Result<(), String> {
    // Outcome 2 (already-observed): the exact `is()` oracle's observed_sink
    // aligns to the change's changed_observable, so the H2 classifier marks
    // the change already-observed (Exposed, no repair gap needed).
    let text = cpan_alpha_regression_packet("already-observed-exact-is.json")?;
    let mut options = packet_test_options();
    options.root = cpan_alpha_input_root();
    let packet = PerlAdapter
        .consume_fact_packet(&text, &options)
        .map_err(|err| format!("already-observed packet must pass ingestion: {err}"))?;
    let findings = packet_to_findings(&packet);
    let exposed = findings
        .iter()
        .find(|finding| finding.class == ExposureClass::Exposed)
        .ok_or_else(|| {
            "the boundary change with an aligned exact oracle must classify as Exposed (already-observed)"
                .to_string()
        })?;
    assert!(
        exposed.canonical_gap.is_none(),
        "an already-observed (Exposed) finding must NOT carry a repair gap"
    );
    Ok(())
}

#[test]
fn cpan_alpha_dynamic_limited_regression_packet_yields_named_limitation() -> Result<(), String> {
    // Outcome 3 (limited): the changed owner is reached only through dynamic
    // dispatch. The producer emits a dynamic_dispatch boundary + a named
    // limitation, NOT a repair packet. No finding may classify as Exposed or
    // carry a repair gap.
    let text = cpan_alpha_regression_packet("dynamic-limited.json")?;
    let mut options = packet_test_options();
    options.root = cpan_alpha_input_root();
    let packet = PerlAdapter
        .consume_fact_packet(&text, &options)
        .map_err(|err| format!("dynamic-limited packet must pass ingestion: {err}"))?;
    assert_eq!(
        packet.packet_status,
        crate::analysis::language::perl::PacketStatus::Partial,
        "a limited packet has status `partial`"
    );
    assert!(
        !packet.dynamic_boundaries.is_empty(),
        "a dynamic-limited packet must carry a dynamic boundary"
    );
    assert!(
        !packet.limitations.is_empty(),
        "a dynamic-limited packet must carry a named limitation"
    );
    let findings = packet_to_findings(&packet);
    let limited = findings
        .iter()
        .find(|finding| finding.class == ExposureClass::StaticUnknown)
        .ok_or_else(|| {
            "dynamic-limited packet must classify the changed owner as static_unknown with a named dynamic limit"
                .to_string()
        })?;
    assert_eq!(
        limited.static_limit_kind,
        Some(crate::domain::StaticLimitKind::DynamicDispatch),
        "dynamic-limited finding must carry the dynamic_dispatch static limit"
    );
    assert!(
        findings
            .iter()
            .all(|finding| finding.canonical_gap.is_none()),
        "a limited packet must not produce any repair-shaped finding"
    );
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────
// Campaign 31 step 2 — consumer-side contract-freeze parity tests.
//
// The producer (perl-lsp-swarm #3104) now emits `changed_observable`,
// `missing_discriminator` on changes and `observed_sink`,
// `expected_expression` on oracles, plus the `Test2::V1` framework wire name.
// These tests prove ripr's deserializer ACCEPTS a packet carrying the new
// fields (so real producer packets don't fail closed) and that the fields are
// accessible to H2 classification. They use #[serde(default)] so older
// packets without the fields still parse.
// ──────────────────────────────────────────────────────────────────────

#[test]
fn consumer_accepts_packet_with_frozen_contract_fields() -> Result<(), String> {
    // Take the base packet and add the new contract fields to its change + oracle.
    let packet = EXACT_RETURN_PACKET
        .replace(
            "\"changed_text_digest\": \"sha256:return\",",
            "\"changed_text_digest\": \"sha256:return\",\n      \"changed_observable\": \"$amount / 2\",\n      \"missing_discriminator\": \"$amount == 100\",",
        )
        .replace(
            "\"expression\": \"is($got, 10, 'discount threshold')\",",
            "\"expression\": \"is($got, 10, 'discount threshold')\",\n      \"observed_sink\": \"$got\",\n      \"expected_expression\": \"10\",",
        );
    let parsed = consume(&bless_fingerprint(&packet))?;
    let change = parsed
        .change("change:lib/My/App.pm:15:return")
        .ok_or_else(|| "missing change".to_string())?;
    assert_eq!(
        change.changed_observable.as_deref(),
        Some("$amount / 2"),
        "consumer must read changed_observable"
    );
    assert_eq!(
        change.missing_discriminator.as_deref(),
        Some("$amount == 100"),
        "consumer must read missing_discriminator"
    );
    let oracle = parsed
        .oracle("oracle:t/app.t:8:is")
        .ok_or_else(|| "missing oracle".to_string())?;
    assert_eq!(
        oracle.observed_sink.as_deref(),
        Some("$got"),
        "consumer must read observed_sink"
    );
    assert_eq!(
        oracle.expected_expression.as_deref(),
        Some("10"),
        "consumer must read expected_expression"
    );
    Ok(())
}

#[test]
fn consumer_accepts_test2_v1_framework_wire_name() -> Result<(), String> {
    // The producer (post #3104) emits "Test2::V1". ripr's TestFramework must
    // deserialize it — serde rejects unknown enum variants by default, so a
    // missing variant would fail closed on real producer packets.
    let packet = EXACT_RETURN_PACKET.replace("\"Test::More\"", "\"Test2::V1\"");
    let parsed = consume(&packet)?;
    assert_eq!(
        parsed.tests_for_framework(TestFramework::Test2V1).len(),
        1,
        "Test2::V1 framework must deserialize and be queryable"
    );
    assert!(
        TestFramework::Test2V1.supports_strict_actionability(),
        "Test2::V1 must support strict actionability (alpha requirement)"
    );
    Ok(())
}

#[test]
fn consumer_still_parses_legacy_packets_without_frozen_fields() -> Result<(), String> {
    // Backward compat: the base EXACT_RETURN_PACKET has NONE of the new fields.
    // It must still parse (#[serde(default)] on the new fields).
    let parsed = consume(EXACT_RETURN_PACKET)?;
    let change = parsed
        .change("change:lib/My/App.pm:15:return")
        .ok_or_else(|| "missing change".to_string())?;
    assert!(
        change.changed_observable.is_none(),
        "legacy packet without changed_observable must default to None, not fail"
    );
    assert!(
        change.missing_discriminator.is_none(),
        "legacy packet without missing_discriminator must default to None"
    );
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────
// PR H1 — mapping-integrity adversarial tests.
//
// These are the FIRST tests to exercise the production mapper
// `packet_to_findings` directly. Before H1, every `related_test.file` was
// built from the PRODUCTION source path (mod.rs:178), so the edit surface
// could point at `lib/*.pm`. These tests pin the H1 contract: the test file
// resolves through `test.file_id`, verify commands are selected by `test_id`,
// dynamic boundaries keep their conservative scope, advisory relations never
// promote past ReachableUnrevealed, and no canonical repair gap is built from
// a generic discriminator label.
// ──────────────────────────────────────────────────────────────────────

/// Drive the production mapper: consume a packet, then map to Findings.
fn findings_from_packet(text: &str) -> Result<Vec<crate::domain::Finding>, String> {
    let packet = consume(&bless_fingerprint(text))?;
    Ok(packet_to_findings(&packet))
}

/// Recompute and patch the `packet_fingerprint` field of a packet's JSON text
/// so a test-built (mutated) packet passes the production fingerprint check.
/// This mirrors exactly what a real producer must do after building a packet,
/// and keeps the production `validate_ingestion` check strict rather than
/// forking a test-only validation path. Returns the JSON with the fingerprint
/// rewritten to the recomputed value. Panics-free: a malformed packet is
/// returned unchanged so the downstream `consume()` reports the real error.
fn bless_fingerprint(text: &str) -> String {
    // Recompute from a single typed parse; bail out (return input unchanged)
    // if the packet does not parse, so `consume()` surfaces the genuine error.
    let Ok(packet) = serde_json::from_str::<PerlFactPacket>(text) else {
        return text.to_string();
    };
    let recomputed = packet.recompute_packet_fingerprint();
    rewrite_fingerprint_field(text, &recomputed)
}

/// Replace the value of the `"packet_fingerprint"` field in `text` with
/// `new_value`, preserving the surrounding JSON. Done by locating the
/// `"packet_fingerprint": "<old>"` substring and swapping `<old>`. Returns
/// `text` unchanged if the field is not found.
fn rewrite_fingerprint_field(text: &str, new_value: &str) -> String {
    let key = "\"packet_fingerprint\":";
    let Some(key_pos) = text.find(key) else {
        return text.to_string();
    };
    let value_start = key_pos + key.len();
    let Some(open_quote) = text[value_start..].find('"') else {
        return text.to_string();
    };
    let value_open = value_start + open_quote + 1;
    let Some(close_quote) = text[value_open..].find('"') else {
        return text.to_string();
    };
    let value_close = value_open + close_quote;
    let mut result = String::with_capacity(text.len() + new_value.len());
    result.push_str(&text[..value_open]);
    result.push_str(new_value);
    result.push_str(&text[value_close..]);
    result
}

// ──────────────────────────────────────────────────────────────────────
// PR H2 — classification semantics: the "already-observed" outcome.
//
// H2 uses oracle.observed_sink aligned to change.changed_observable to
// distinguish a test that ALREADY discriminates the changed sink (Exposed /
// already-observed — no test needed) from one that merely reaches the owner
// (WeaklyExposed). This is the discrimination distinction: owner-
// target identity is NOT observation (the false-exposed family). These tests
// pin both the promotion and the fail-closed behavior.
// ──────────────────────────────────────────────────────────────────────

/// A packet where a strong-exact oracle's `observed_sink` aligns exactly to
/// the change's `changed_observable` must classify `Exposed` (already-observed),
/// emit `perl_already_discriminated:` evidence, and carry NO repair gap
/// (no test needs adding). This is maintainer end-state outcome #2.
#[test]
fn h2_sink_aligned_oracle_classifies_exposed_and_suppresses_repair_gap() -> Result<(), String> {
    // changed_observable == observed_sink == "$amount / 2" (exact alignment).
    let packet = EXACT_RETURN_PACKET
        .replace(
            "\"changed_text_digest\": \"sha256:return\",",
            "\"changed_text_digest\": \"sha256:return\",\n      \"changed_observable\": \"$amount / 2\",",
        )
        .replace(
            "\"expression\": \"is($got, 10, 'discount threshold')\",",
            "\"expression\": \"is($got, 10, 'discount threshold')\",\n      \"observed_sink\": \"$amount / 2\",\n      \"expected_expression\": \"10\",",
        );
    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert_eq!(
        finding.class,
        crate::domain::ExposureClass::Exposed,
        "sink-aligned strong oracle must classify Exposed (already-observed)"
    );
    assert!(
        finding
            .evidence
            .iter()
            .any(|e| e.starts_with("perl_already_discriminated:")),
        "already-observed must emit perl_already_discriminated evidence"
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|e| e.starts_with("perl_suggested_test_location")),
        "already-observed must NOT suggest a test location (no test needed)"
    );
    assert!(
        finding.canonical_gap.is_none(),
        "already-observed must carry no repair gap"
    );
    assert!(
        finding.activation.missing_discriminators.is_empty(),
        "already-observed must have no missing discriminator"
    );
    Ok(())
}

/// A strong-exact oracle whose `observed_sink` does NOT match the change's
/// `changed_observable` must NOT classify Exposed. Owner-target identity is
/// not sink observation — this is the cardinal false-exposed guard.
#[test]
fn h2_non_aligned_sink_stays_weakly_exposed() -> Result<(), String> {
    // changed_observable = "$rate * 0.9" but observed_sink = "$amount / 2".
    // Same owner, strong oracle, but the oracle observes a DIFFERENT sink.
    let packet = EXACT_RETURN_PACKET
        .replace(
            "\"changed_text_digest\": \"sha256:return\",",
            "\"changed_text_digest\": \"sha256:return\",\n      \"changed_observable\": \"$rate * 0.9\",",
        )
        .replace(
            "\"expression\": \"is($got, 10, 'discount threshold')\",",
            "\"expression\": \"is($got, 10, 'discount threshold')\",\n      \"observed_sink\": \"$amount / 2\",\n      \"expected_expression\": \"10\",",
        );
    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert_ne!(
        finding.class,
        crate::domain::ExposureClass::Exposed,
        "non-aligned sink must NOT promote to Exposed; got {:?}",
        finding.class
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|e| e.starts_with("perl_already_discriminated:")),
        "non-aligned sink must not emit already-discriminated evidence"
    );
    Ok(())
}

/// An advisory relation (file_proximity) with a matching sink must NOT
/// classify Exposed. The relation-kind gate holds even when sinks align —
/// only DirectOwnerCall can prove observation.
#[test]
fn h2_advisory_relation_with_matching_sink_does_not_promote() -> Result<(), String> {
    // Matching sinks, but the relation is file_proximity (advisory).
    let packet = EXACT_RETURN_PACKET
        .replace(
            "\"changed_text_digest\": \"sha256:return\",",
            "\"changed_text_digest\": \"sha256:return\",\n      \"changed_observable\": \"$amount / 2\",",
        )
        .replace(
            "\"expression\": \"is($got, 10, 'discount threshold')\",",
            "\"expression\": \"is($got, 10, 'discount threshold')\",\n      \"observed_sink\": \"$amount / 2\",",
        )
        .replace("\"direct_owner_call\"", "\"file_proximity\"");
    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert_ne!(
        finding.class,
        crate::domain::ExposureClass::Exposed,
        "file_proximity relation must not promote to Exposed even with matching sink; got {:?}",
        finding.class
    );
    Ok(())
}

/// Token-substring coincidence must NOT pass as sink alignment. `buffer` vs
/// `buffered_stream` is the recurring false-exposed family — the sink must
/// match exactly, not as a substring.
#[test]
fn h2_token_substring_coincidence_does_not_align() -> Result<(), String> {
    // changed_observable = "buffer", observed_sink = "buffered_stream".
    // Substring coincidence — must NOT align.
    let packet = EXACT_RETURN_PACKET
        .replace(
            "\"changed_text_digest\": \"sha256:return\",",
            "\"changed_text_digest\": \"sha256:return\",\n      \"changed_observable\": \"buffer\",",
        )
        .replace(
            "\"expression\": \"is($got, 10, 'discount threshold')\",",
            "\"expression\": \"is($got, 10, 'discount threshold')\",\n      \"observed_sink\": \"buffered_stream\",",
        );
    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert_ne!(
        finding.class,
        crate::domain::ExposureClass::Exposed,
        "token-substring coincidence (buffer vs buffered_stream) must not align; got {:?}",
        finding.class
    );
    Ok(())
}

/// `return <expr>` alignment: a change whose observable is `return $x` aligns
/// to an oracle observing `$x` (the normalized form). This is the legitimate
/// aliasing the helper accepts.
#[test]
fn h2_return_prefix_aliasing_aligns() -> Result<(), String> {
    let packet = EXACT_RETURN_PACKET
        .replace(
            "\"changed_text_digest\": \"sha256:return\",",
            "\"changed_text_digest\": \"sha256:return\",\n      \"changed_observable\": \"return $discounted\",",
        )
        .replace(
            "\"expression\": \"is($got, 10, 'discount threshold')\",",
            "\"expression\": \"is($got, 10, 'discount threshold')\",\n      \"observed_sink\": \"$discounted\",",
        );
    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert_eq!(
        finding.class,
        crate::domain::ExposureClass::Exposed,
        "`return $x` observable must align to `$x` observed_sink (normalized)"
    );
    Ok(())
}

/// `t/app.t` must NEVER project the production source path as the related-test
/// file. Every `related_tests[*].file` must resolve to the test file.
#[test]
fn h1_production_source_path_never_becomes_related_test_file() -> Result<(), String> {
    let findings = findings_from_packet(EXACT_RETURN_PACKET)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding from EXACT_RETURN_PACKET".to_string())?;
    assert!(
        finding.related_tests.iter().all(|t| {
            let p = t.file.display().to_string();
            p == "t/app.t"
        }),
        "every related-test file must be the TEST path (t/app.t), never the \
         production source (lib/My/App.pm); got: {:?}",
        finding
            .related_tests
            .iter()
            .map(|t| t.file.display().to_string())
            .collect::<Vec<_>>()
    );
    // The probe location, by contrast, IS the production source — that's
    // correct and must remain so.
    assert_eq!(
        finding.probe.location.file.display().to_string(),
        "lib/My/App.pm",
        "probe location stays on the production source file"
    );
    Ok(())
}

/// The related-test line must come from the TestFact range, not the hardcoded
/// `1`. EXACT_RETURN_PACKET's test starts at line 4; its oracle at line 8.
#[test]
fn h1_related_test_line_uses_test_range_not_hardcoded_one() -> Result<(), String> {
    let findings = findings_from_packet(EXACT_RETURN_PACKET)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    let related = finding
        .related_tests
        .first()
        .ok_or_else(|| "expected a related test".to_string())?;
    assert_ne!(
        related.line, 1,
        "related-test line must not be the hardcoded 1"
    );
    assert!(
        related.line >= 4,
        "related-test line should reflect the test/oracle range (>=4); got {}",
        related.line
    );
    Ok(())
}

/// A `file_proximity` relation must NOT promote the change to `WeaklyExposed`.
/// It is advisory-only and is capped at `ReachableUnrevealed`, and carries no
/// repair-gap fields (no `perl_suggested_test_location`).
#[test]
fn h1_file_proximity_relation_is_not_eligible() -> Result<(), String> {
    let packet = EXACT_RETURN_PACKET.replace("\"direct_owner_call\"", "\"file_proximity\"");
    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert_ne!(
        finding.class,
        crate::domain::ExposureClass::WeaklyExposed,
        "file_proximity must not promote to WeaklyExposed; got {:?}",
        finding.class
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|e| e.starts_with("perl_suggested_test_location")),
        "file_proximity must not emit a repair-gap test location"
    );
    Ok(())
}

/// The emitted verify command must belong to the selected test (keyed by
/// `test_id`), never a different test's runner. EXACT_RETURN_PACKET's verify
/// command is `prove t/app.t` scoped to `test:t/app.t:test_discount_threshold`.
#[test]
fn h1_verify_command_belongs_to_selected_test() -> Result<(), String> {
    let findings = findings_from_packet(EXACT_RETURN_PACKET)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    let verify = finding
        .evidence
        .iter()
        .find_map(|e| e.strip_prefix("perl_verify_command: "))
        .ok_or_else(|| "expected a perl_verify_command".to_string())?;
    assert_eq!(
        verify, "prove t/app.t",
        "verify command must be the per-test command, not a global scan result"
    );
    Ok(())
}

/// A dynamic boundary on a DIFFERENT explicit owner AND a different file must
/// not block this change's classification. (A boundary on the *changed file*
/// conservatively blocks regardless of owner — that is tested separately by
/// `h1_ownerless_file_boundary_still_blocks`. This test isolates the
/// owner-match path: boundary is on another file+owner entirely.)
#[test]
fn h1_boundary_on_explicit_other_owner_does_not_block() -> Result<(), String> {
    // Add a second file for the boundary to live on (different from the changed
    // file and the test file), owned by a different owner.
    let packet = EXACT_RETURN_PACKET
        .replace(
            "\"dynamic_boundaries\": []",
            "\"dynamic_boundaries\": [{\
           \"boundary_id\":\"bnd:other-owner\",\
           \"kind\":\"dynamic_dispatch\",\
           \"file_id\":\"file:lib/Other.pm\",\
           \"owner_id\":\"perl:lib/Other.pm::Other::thing\",\
           \"range\":{\"start_line\":1,\"start_column\":1,\"end_line\":1,\"end_column\":2},\
           \"confidence\":\"medium\",\
           \"provenance_refs\":[\"prov:bnd:1\"]\
         }]",
        )
        .replace(
            "\"provenance_refs\":[\"prov:runner:1\"]\n      }\n    ]",
            "\"provenance_refs\":[\"prov:runner:1\"]\n      },\n      {\
           \"provenance_id\":\"prov:bnd:1\",\
           \"source\":\"semantic\",\
           \"file_id\":\"file:lib/Other.pm\",\
           \"range\":null,\
           \"confidence\":\"medium\"\
         }]",
        );
    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert_ne!(
        finding.class,
        crate::domain::ExposureClass::StaticUnknown,
        "a boundary on a different file+owner must not block this change; got {:?}",
        finding.class
    );
    Ok(())
}

/// An OWNERLESS file-level boundary on the changed file must STILL block
/// (conservative). This pins that H1 does NOT narrow to owner-only.
#[test]
fn h1_ownerless_file_boundary_still_blocks() -> Result<(), String> {
    // Boundary with no owner_id, on the changed file.
    let packet = EXACT_RETURN_PACKET.replace(
        "\"dynamic_boundaries\": []",
        "\"dynamic_boundaries\": [{\
           \"boundary_id\":\"bnd:file-level\",\
           \"kind\":\"dynamic_dispatch\",\
           \"file_id\":\"file:lib/My/App.pm\",\
           \"owner_id\":null,\
           \"range\":{\"start_line\":1,\"start_column\":1,\"end_line\":1,\"end_column\":2},\
           \"confidence\":\"medium\",\
           \"provenance_refs\":[\"prov:bnd:1\"]\
         }]",
    );
    let packet = packet.replace(
        "\"provenance_refs\":[\"prov:runner:1\"]\n      }\n    ]",
        "\"provenance_refs\":[\"prov:runner:1\"]\n      },\n      {\
           \"provenance_id\":\"prov:bnd:1\",\
           \"source\":\"semantic\",\
           \"file_id\":\"file:lib/My/App.pm\",\
           \"range\":null,\
           \"confidence\":\"medium\"\
         }]",
    );
    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert_eq!(
        finding.class,
        crate::domain::ExposureClass::StaticUnknown,
        "an ownerless file-level boundary on the changed file must still block; got {:?}",
        finding.class
    );
    Ok(())
}

#[test]
fn h1_dynamic_dispatch_limitation_blocks_related_finding_classification() -> Result<(), String> {
    let mut packet = consume(EXACT_RETURN_PACKET)?;
    packet.limitations = vec![LimitationFact {
        limitation_id: "limitation:dynamic-dispatch:relation".to_string(),
        kind: "dynamic_dispatch".to_string(),
        message: "producer could not bind the related call through dynamic dispatch".to_string(),
        evidence_refs: vec!["relation:change:discount-return:test:threshold".to_string()],
    }];

    let findings = packet_to_findings(&packet);
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert!(
        !finding.related_tests.is_empty(),
        "fixture must keep related evidence so this covers the related-evidence limitation branch"
    );
    assert_eq!(
        finding.class,
        crate::domain::ExposureClass::StaticUnknown,
        "a dynamic_dispatch limitation tied to related evidence must block strict finding classification"
    );
    Ok(())
}

#[test]
fn h1_dynamic_dispatch_limitation_suppresses_concrete_repair_gap() -> Result<(), String> {
    let text = EXACT_RETURN_PACKET.replace(
        "\"changed_text_digest\": \"sha256:return\"",
        "\"changed_text_digest\": \"discriminator:$amount == $threshold\"",
    );
    let mut packet = consume(&bless_fingerprint(&text))?;
    packet.limitations = vec![LimitationFact {
        limitation_id: "limitation:dynamic-dispatch:relation".to_string(),
        kind: "dynamic_dispatch".to_string(),
        message: "producer could not bind the related call through dynamic dispatch".to_string(),
        evidence_refs: vec!["relation:change:discount-return:test:threshold".to_string()],
    }];

    let findings = packet_to_findings(&packet);
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert_eq!(
        finding.class,
        crate::domain::ExposureClass::StaticUnknown,
        "a dynamic_dispatch limitation tied to related evidence must stay fail-closed"
    );
    assert!(
        finding.canonical_gap.is_none(),
        "static-limited Perl findings must not attach repair gap identities"
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|e| e.starts_with("perl_suggested_test_location")),
        "static-limited Perl findings must not suggest a repair test location"
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|e| e.starts_with("perl_suggested_assertion")),
        "static-limited Perl findings must not suggest a repair assertion"
    );
    Ok(())
}

#[test]
fn h1_dynamic_dispatch_limitation_blocks_no_static_path_classification() -> Result<(), String> {
    let mut packet = consume(EXACT_RETURN_PACKET)?;
    packet.relations.clear();
    packet.limitations = vec![LimitationFact {
        limitation_id: "limitation:dynamic-dispatch:change".to_string(),
        kind: "dynamic_dispatch".to_string(),
        message: "producer could not bind the changed call through dynamic dispatch".to_string(),
        evidence_refs: vec!["change:lib/My/App.pm:15:return".to_string()],
    }];

    let findings = packet_to_findings(&packet);
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert!(
        finding.related_tests.is_empty(),
        "fixture mutation must remove relation evidence so this covers the change-only limitation branch"
    );
    assert_eq!(
        finding.class,
        crate::domain::ExposureClass::StaticUnknown,
        "a change-scoped dynamic_dispatch limitation must outrank no_static_path"
    );
    Ok(())
}

/// A change whose `changed_text_digest` is NOT a `discriminator:`-prefixed
/// concrete discriminator must NOT carry a canonical repair gap. EXACT_RETURN_PACKET
/// uses `"sha256:return"` (generic), so it must yield `canonical_gap: None`.
#[test]
fn h1_generic_discriminator_produces_no_canonical_repair_gap() -> Result<(), String> {
    let findings = findings_from_packet(EXACT_RETURN_PACKET)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert!(
        finding.canonical_gap.is_none(),
        "a generic (non-`discriminator:`-prefixed) changed_text_digest must not \
         produce a canonical repair gap; got {:?}",
        finding.canonical_gap
    );
    assert!(
        !finding
            .evidence
            .iter()
            .any(|e| e.starts_with("perl_suggested_test_location")),
        "generic discriminator must not emit a repair-gap test location"
    );
    Ok(())
}

/// Conversely, when the producer DOES supply a concrete discriminator
/// (`discriminator:` prefix), the canonical gap IS attached. This proves the
/// gate opens for the real-producer case (PRs 4–7).
#[test]
fn h1_concrete_discriminator_attaches_canonical_gap() -> Result<(), String> {
    let packet = EXACT_RETURN_PACKET.replace(
        "\"changed_text_digest\": \"sha256:return\"",
        "\"changed_text_digest\": \"discriminator:$amount == $threshold\"",
    );
    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    assert!(
        finding.canonical_gap.is_some(),
        "a `discriminator:`-prefixed changed_text_digest must attach a canonical \
         repair gap; got None"
    );
    assert!(
        finding
            .evidence
            .iter()
            .any(|e| e.starts_with("perl_suggested_test_location")),
        "concrete discriminator must emit the repair-gap test location"
    );
    Ok(())
}

/// A dynamic boundary tied to a SECOND/subsequent related test's file must
/// STILL block the change — the boundary check must not depend on which
/// related evidence happens to be first (ordering independence, PR H1 followup
/// to the droid-review finding). Before the fix, only `.first()` was sampled,
/// so a boundary on the 2nd test was silently missed.
#[test]
fn h1_boundary_check_is_ordering_independent_across_related_tests() -> Result<(), String> {
    // Two related tests on two test files; the boundary is on the SECOND
    // test's file (file:t/second.t), not the first (file:t/app.t).
    let packet = EXACT_RETURN_PACKET
        // 1. Add the second test file.
        .replace(
            "],\n  \"owners\": [",
            ",{\n      \"file_id\": \"file:t/second.t\",\n      \"path\": \"t/second.t\",\n      \"role\": [\"test\"],\n      \"digest\": \"sha256:second\",\n      \"package_names\": [],\n      \"provenance_refs\": [\"prov:file-index:second\"]\n    }\n  ],\n  \"owners\": [",
        )
        // 2. Add the second test + second oracle + second relation pointing at
        //    the changed owner, with the boundary on the SECOND test's file.
        .replace(
            "\"dynamic_boundaries\": [],",
            "\"dynamic_boundaries\": [{\
               \"boundary_id\":\"bnd:second-file\",\
               \"kind\":\"dynamic_dispatch\",\
               \"file_id\":\"file:t/second.t\",\
               \"owner_id\":null,\
               \"range\":{\"start_line\":1,\"start_column\":1,\"end_line\":1,\"end_column\":2},\
               \"confidence\":\"medium\",\
               \"provenance_refs\":[\"prov:bnd:1\"]\
             }],",
        )
        .replace(
            "\"provenance_refs\":[\"prov:runner:1\"]\n      }\n    ]",
            "\"provenance_refs\":[\"prov:runner:1\"]\n      },\n      {\
               \"provenance_id\":\"prov:file-index:second\",\
               \"source\":\"workspace\",\
               \"file_id\":\"file:t/second.t\",\
               \"range\":null,\
               \"confidence\":\"high\"\
             },{\n               \"provenance_id\":\"prov:bnd:1\",\
               \"source\":\"semantic\",\
               \"file_id\":\"file:t/second.t\",\
               \"range\":null,\
               \"confidence\":\"medium\"\
             }]",
        );
    // Append a second test + oracle + relation (linked to the changed owner).
    let packet = packet.replace(
        "  \"tests\": [\n    {",
        "  \"tests\": [\n    {\n      \"test_id\": \"test:t/second.t:second_discount\",\n      \"file_id\": \"file:t/second.t\",\n      \"framework\": \"Test::More\",\n      \"name\": \"second_discount\",\n      \"range\": {\"start_line\": 3, \"start_column\": 1, \"end_line\": 8, \"end_column\": 2},\n      \"runner_hints\": [\"prove\"],\n      \"confidence\": \"medium\",\n      \"provenance_refs\": [\"prov:file-index:second\"]\n    },\n    {",
    );
    let packet = packet.replace(
        "  \"oracles\": [\n    {",
        "  \"oracles\": [\n    {\n      \"oracle_id\": \"oracle:t/second.t:4:is\",\n      \"test_id\": \"test:t/second.t:second_discount\",\n      \"kind\": \"exact_return_assertion\",\n      \"strength\": \"strong_exact\",\n      \"target_owner_id\": \"perl:lib/My/App.pm::My::App::discount\",\n      \"expression\": \"is($got, 10)\",\n      \"range\": {\"start_line\": 4, \"start_column\": 1, \"end_line\": 4, \"end_column\": 20},\n      \"confidence\": \"medium\",\n      \"provenance_refs\": [\"prov:file-index:second\"]\n    },\n    {",
    );
    let packet = packet.replace(
        "      \"provenance_refs\": [\"prov:relation:1\"]\n    }\n  ],\n  \"dynamic_boundaries\":",
        "      \"provenance_refs\": [\"prov:relation:1\"]\n    },\n    {\n      \"relation_id\": \"relation:change:discount:return:test:second\",\n      \"change_id\": \"change:lib/My/App.pm:15:return\",\n      \"owner_id\": \"perl:lib/My/App.pm::My::App::discount\",\n      \"test_id\": \"test:t/second.t:second_discount\",\n      \"oracle_id\": \"oracle:t/second.t:4:is\",\n      \"relation_kind\": \"direct_owner_call\",\n      \"reachability_hint\": \"reachable\",\n      \"confidence\": \"medium\",\n      \"provenance_refs\": [\"prov:file-index:second\"]\n    }\n  ],\n  \"dynamic_boundaries\":",
    );

    let findings = findings_from_packet(&packet)?;
    let finding = findings
        .first()
        .ok_or_else(|| "expected one finding".to_string())?;
    // There must be at least 2 related tests for ordering to matter at all.
    assert!(
        finding.related_tests.len() >= 2,
        "ordering test requires >=2 related tests; got {}",
        finding.related_tests.len()
    );
    assert_eq!(
        finding.class,
        crate::domain::ExposureClass::StaticUnknown,
        "a boundary on the SECOND related test's file must still block; got \
         {:?} (before the fix this returned non-StaticUnknown because only \
         .first() was sampled)",
        finding.class
    );
    Ok(())
}

const EXACT_RETURN_PACKET: &str = r#"{
  "schema_version": "ripr-perl-facts-v1",
  "packet_id": "perl-facts:repo:exact-return",
  "packet_status": "complete",
  "packet_fingerprint": "sha256:d23dde44154c2ee8eddf3eae1dd87d585371396ac04608c651b30289a89e74f3",
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
  "packet_fingerprint": "sha256:5004da4fad36c03cead176f176fc27f10f7d2ca62d4eefba0dc5b836a3bf64a0",
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
