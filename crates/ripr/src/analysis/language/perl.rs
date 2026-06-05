//! Fixture-only Perl fact packet adapter.
//!
//! This module is test-scoped for the first Perl implementation slice. It
//! consumes canned `ripr-perl-facts-v1` packets without launching `perl-lsp`,
//! a Perl runtime, or an LSP protocol session. Production routing lands only
//! after the fact packet and strict actionability slices are fixture-backed.

use serde::Deserialize;

const PERL_FACT_PACKET_SCHEMA: &str = "ripr-perl-facts-v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PerlAdapter;

impl PerlAdapter {
    fn consume_fact_packet(&self, text: &str) -> Result<PerlFactPacket, String> {
        let packet: PerlFactPacket = serde_json::from_str(text)
            .map_err(|err| format!("parse ripr-perl-facts-v1 packet: {err}"))?;
        if packet.schema_version != PERL_FACT_PACKET_SCHEMA {
            return Err(format!(
                "unsupported Perl fact packet schema `{}`; expected `{PERL_FACT_PACKET_SCHEMA}`",
                packet.schema_version
            ));
        }
        Ok(packet)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct PerlFactPacket {
    schema_version: String,
    packet_id: String,
    packet_status: PacketStatus,
    packet_fingerprint: String,
    producer: ProducerFact,
    root: RootFact,
    input: InputFact,
    files: Vec<FileFact>,
    owners: Vec<OwnerFact>,
    changes: Vec<ChangeFact>,
    tests: Vec<TestFact>,
    oracles: Vec<OracleFact>,
    relations: Vec<RelationFact>,
    dynamic_boundaries: Vec<DynamicBoundaryFact>,
    verify_commands: Vec<VerifyCommandFact>,
    limitations: Vec<LimitationFact>,
    provenance: Vec<ProvenanceFact>,
}

impl PerlFactPacket {
    fn file(&self, file_id: &str) -> Option<&FileFact> {
        self.files.iter().find(|file| file.file_id == file_id)
    }

    fn owner(&self, owner_id: &str) -> Option<&OwnerFact> {
        self.owners.iter().find(|owner| owner.owner_id == owner_id)
    }

    fn change(&self, change_id: &str) -> Option<&ChangeFact> {
        self.changes
            .iter()
            .find(|change| change.change_id == change_id)
    }

    fn oracle(&self, oracle_id: &str) -> Option<&OracleFact> {
        self.oracles
            .iter()
            .find(|oracle| oracle.oracle_id == oracle_id)
    }

    fn relation(&self, relation_id: &str) -> Option<&RelationFact> {
        self.relations
            .iter()
            .find(|relation| relation.relation_id == relation_id)
    }

    fn verify_command_for_test(&self, test_id: &str) -> Option<&VerifyCommandFact> {
        self.verify_commands
            .iter()
            .find(|command| command.test_id.as_deref() == Some(test_id))
    }

    fn canonical_owner_identity(&self, owner_id: &str) -> Option<CanonicalPerlOwnerIdentity> {
        let owner = self.owner(owner_id)?;
        if owner.kind == OwnerKind::Unknown || !owner.owner_id.starts_with("perl:") {
            return None;
        }
        let file = self.file(&owner.file_id)?;
        Some(CanonicalPerlOwnerIdentity {
            id: owner.owner_id.clone(),
            file_path: file.path.clone(),
            kind: owner.kind.as_str().to_string(),
            package: owner.package.clone(),
            name: owner.name.clone(),
        })
    }

    fn canonical_gap_identity_for_change(
        &self,
        change_id: &str,
    ) -> Option<CanonicalPerlGapIdentity> {
        if self.packet_status != PacketStatus::Complete {
            return None;
        }

        let change = self.change(change_id)?;
        if self
            .dynamic_boundaries
            .iter()
            .any(|boundary| boundary.owner_id.as_deref() == Some(change.owner_id.as_str()))
        {
            return None;
        }

        let owner = self.canonical_owner_identity(&change.owner_id)?;
        let behavior_kind = change.behavior_hint.as_str().to_string();
        let assertion_shape = self
            .relations
            .iter()
            .filter(|relation| relation.change_id == change.change_id)
            .find_map(|relation| {
                relation
                    .oracle_id
                    .as_deref()
                    .and_then(|oracle_id| self.oracle(oracle_id))
                    .map(|oracle| oracle.kind.assertion_shape().to_string())
            })
            .unwrap_or_else(|| change.behavior_hint.default_assertion_shape().to_string());
        let missing_discriminator = change
            .behavior_hint
            .default_missing_discriminator()
            .to_string();
        let id = canonical_perl_gap_id([
            owner.id.as_str(),
            behavior_kind.as_str(),
            missing_discriminator.as_str(),
            assertion_shape.as_str(),
        ]);

        Some(CanonicalPerlGapIdentity {
            id,
            owner_id: owner.id,
            behavior_kind,
            missing_discriminator,
            assertion_shape,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalPerlOwnerIdentity {
    id: String,
    file_path: String,
    kind: String,
    package: Option<String>,
    name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalPerlGapIdentity {
    id: String,
    owner_id: String,
    behavior_kind: String,
    missing_discriminator: String,
    assertion_shape: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PacketStatus {
    Complete,
    Partial,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct ProducerFact {
    name: String,
    version: String,
    capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct RootFact {
    repo_relative: String,
    vcs_head: Option<String>,
    path_style: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct InputFact {
    base: Option<String>,
    head: Option<String>,
    diff_id: Option<String>,
    requested_fact_classes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct FileFact {
    file_id: String,
    path: String,
    role: Vec<FileRole>,
    digest: String,
    package_names: Vec<String>,
    provenance_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum FileRole {
    Source,
    Test,
    Helper,
    Generated,
    Config,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct OwnerFact {
    owner_id: String,
    file_id: String,
    kind: OwnerKind,
    package: Option<String>,
    name: Option<String>,
    range: RangeFact,
    confidence: Confidence,
    provenance_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum OwnerKind {
    Package,
    Sub,
    Method,
    Script,
    ModuleInitializer,
    TestSub,
    Unknown,
}

impl OwnerKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Sub => "sub",
            Self::Method => "method",
            Self::Script => "script",
            Self::ModuleInitializer => "module_initializer",
            Self::TestSub => "test_sub",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct ChangeFact {
    change_id: String,
    file_id: String,
    owner_id: String,
    range: RangeFact,
    behavior_hint: BehaviorHint,
    changed_text_digest: String,
    provenance_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BehaviorHint {
    PredicateBoundary,
    ReturnValue,
    ExceptionPath,
    HashOrObjectField,
    OutputObserver,
    WarnObserver,
    LogObserver,
    CallEffect,
    Unknown,
}

impl BehaviorHint {
    fn as_str(self) -> &'static str {
        match self {
            Self::PredicateBoundary => "predicate_boundary",
            Self::ReturnValue => "return_value",
            Self::ExceptionPath => "exception_path",
            Self::HashOrObjectField => "hash_or_object_field",
            Self::OutputObserver => "output_observer",
            Self::WarnObserver => "warn_observer",
            Self::LogObserver => "log_observer",
            Self::CallEffect => "call_effect",
            Self::Unknown => "unknown",
        }
    }

    fn default_missing_discriminator(self) -> &'static str {
        match self {
            Self::PredicateBoundary => "predicate_boundary",
            Self::ReturnValue => "return_value",
            Self::ExceptionPath => "exception_observer",
            Self::HashOrObjectField => "hash_or_object_field",
            Self::OutputObserver => "output_observer",
            Self::WarnObserver => "warn_observer",
            Self::LogObserver => "log_observer",
            Self::CallEffect => "call_effect",
            Self::Unknown => "unknown_discriminator",
        }
    }

    fn default_assertion_shape(self) -> &'static str {
        match self {
            Self::PredicateBoundary => "predicate_boundary_assertion",
            Self::ReturnValue => "exact_return_assertion",
            Self::ExceptionPath => "exception_observer",
            Self::HashOrObjectField => "hash_or_object_field_assertion",
            Self::OutputObserver => "output_observer",
            Self::WarnObserver => "warn_observer",
            Self::LogObserver => "log_observer",
            Self::CallEffect => "side_effect_observer",
            Self::Unknown => "unknown_assertion",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct TestFact {
    test_id: String,
    file_id: String,
    framework: TestFramework,
    name: String,
    range: RangeFact,
    runner_hints: Vec<RunnerHint>,
    confidence: Confidence,
    provenance_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
enum TestFramework {
    #[serde(rename = "Test::More")]
    TestMore,
    #[serde(rename = "Test2::V0")]
    Test2V0,
    #[serde(rename = "Test2::Suite")]
    Test2Suite,
    #[serde(rename = "Test::Exception")]
    TestException,
    #[serde(rename = "Test::Fatal")]
    TestFatal,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RunnerHint {
    Prove,
    Yath,
    Carton,
    Dzil,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct OracleFact {
    oracle_id: String,
    test_id: String,
    kind: OracleKind,
    strength: OracleStrength,
    target_owner_id: Option<String>,
    expression: Option<String>,
    range: RangeFact,
    confidence: Confidence,
    provenance_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum OracleKind {
    ExactReturnAssertion,
    PredicateBoundaryAssertion,
    ExceptionObserver,
    HashOrObjectFieldAssertion,
    OutputObserver,
    WarnObserver,
    LogObserver,
    SmokeOk,
    MentionOnly,
    DiesOnly,
    UnknownHelper,
    DynamicFrameworkIndirection,
    Unknown,
}

impl OracleKind {
    fn assertion_shape(self) -> &'static str {
        match self {
            Self::ExactReturnAssertion => "exact_return_assertion",
            Self::PredicateBoundaryAssertion => "predicate_boundary_assertion",
            Self::ExceptionObserver => "exception_observer",
            Self::HashOrObjectFieldAssertion => "hash_or_object_field_assertion",
            Self::OutputObserver => "output_observer",
            Self::WarnObserver => "warn_observer",
            Self::LogObserver => "log_observer",
            Self::SmokeOk => "smoke_ok",
            Self::MentionOnly => "mention_only",
            Self::DiesOnly => "dies_only",
            Self::UnknownHelper => "unknown_helper",
            Self::DynamicFrameworkIndirection => "dynamic_framework_indirection",
            Self::Unknown => "unknown_assertion",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum OracleStrength {
    StrongExact,
    WeakSmoke,
    WeakBroad,
    MentionOnly,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct RelationFact {
    relation_id: String,
    change_id: String,
    owner_id: String,
    test_id: String,
    oracle_id: Option<String>,
    relation_kind: RelationKind,
    reachability_hint: ReachabilityHint,
    confidence: Confidence,
    provenance_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RelationKind {
    DirectOwnerCall,
    PackageReference,
    MethodReceiver,
    TestNameMatch,
    FileProximity,
    HelperCall,
    FixtureSetup,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ReachabilityHint {
    Reachable,
    WeaklyReachable,
    StaticUnknown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct DynamicBoundaryFact {
    boundary_id: String,
    kind: BoundaryKind,
    file_id: String,
    owner_id: Option<String>,
    range: RangeFact,
    confidence: Confidence,
    provenance_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct LimitationFact {
    limitation_id: String,
    kind: BoundaryKind,
    message: String,
    evidence_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BoundaryKind {
    DynamicDispatch,
    ModuleResolutionUnknown,
    GeneratedSymbol,
    RoleComposition,
    MonkeypatchOrSymbolPatch,
    EvalOrStringCode,
    SymbolTableMutation,
    FrameworkIndirection,
    UnknownHelper,
    UnsupportedSyntax,
    MissingTestRunner,
    MissingDiffOwner,
    PacketIncomplete,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct VerifyCommandFact {
    command_id: String,
    runner: Runner,
    argv: Vec<String>,
    scope: CommandScope,
    test_id: Option<String>,
    confidence: Confidence,
    preconditions: Vec<String>,
    provenance_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Runner {
    Prove,
    Yath,
    Carton,
    Dzil,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CommandScope {
    Test,
    File,
    Suite,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct ProvenanceFact {
    provenance_id: String,
    source: ProvenanceSource,
    file_id: Option<String>,
    range: Option<RangeFact>,
    confidence: Confidence,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ProvenanceSource {
    Syntax,
    Semantic,
    Workspace,
    ModuleResolution,
    TestDiscovery,
    OracleExtraction,
    RunnerDetection,
    Diff,
    OperatorConfig,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
struct RangeFact {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

fn canonical_perl_gap_id<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for part in parts {
        for byte in part.as_bytes().iter().chain([0].iter()) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    format!("gap:perl:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn perl_gap_identity_uses_owner_behavior_discriminator_and_assertion_shape()
    -> Result<(), String> {
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
            .replace("change:lib/My/App.pm:15:return", "change:lib/My/App.pm:99:return")
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
}
