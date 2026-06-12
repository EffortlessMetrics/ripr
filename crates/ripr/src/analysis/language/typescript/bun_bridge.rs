//! Bun bridge facts and cross-language analysis for the TypeScript preview adapter.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptBunArrayBufferFact {
    pub(crate) kind: TypeScriptBunArrayBufferFactKind,
    pub(crate) file: PathBuf,
    pub(crate) line: usize,
    pub(crate) text: String,
}

impl TypeScriptBunArrayBufferFact {
    pub(crate) fn evidence_line(&self) -> String {
        format!(
            "typescript_bun_ub_advisory_fact: {} at {}:{} ({})",
            self.kind.as_str(),
            normalized_path(&self.file),
            self.line,
            self.text
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TypeScriptBunArrayBufferFactKind {
    SharedArrayBuffer,
    ResizableArrayBuffer,
    ArrayBufferResize,
    ArrayBufferView,
    ViewBackedBlobInput,
    BlobArrayBufferObserver,
    MarkdownExternalCallsite,
    StableByteCopyOracle,
    MarkdownStrongOracle,
    WeakByteSmokeOracle,
    WeakByteSnapshotOracle,
    ByteOracleMentionOnly,
    MaxByteLengthMentionOnly,
}

impl TypeScriptBunArrayBufferFactKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SharedArrayBuffer => "shared_array_buffer",
            Self::ResizableArrayBuffer => "resizable_array_buffer",
            Self::ArrayBufferResize => "array_buffer_resize",
            Self::ArrayBufferView => "array_buffer_view",
            Self::ViewBackedBlobInput => "view_backed_blob_input",
            Self::BlobArrayBufferObserver => "blob_array_buffer_observer",
            Self::MarkdownExternalCallsite => "bun_markdown_callsite",
            Self::StableByteCopyOracle => "stable_byte_copy_oracle",
            Self::MarkdownStrongOracle => "markdown_strong_oracle",
            Self::WeakByteSmokeOracle => "weak_byte_smoke_oracle",
            Self::WeakByteSnapshotOracle => "weak_byte_snapshot_oracle",
            Self::ByteOracleMentionOnly => "byte_oracle_mention_only",
            Self::MaxByteLengthMentionOnly => "max_byte_length_mention_only",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptBunBridgeHint {
    pub(crate) profile_kind: TypeScriptBunBridgeProfileKind,
    pub(crate) confidence: TypeScriptBunBridgeConfidence,
    pub(crate) verdict: TypeScriptBunBridgeVerdict,
    pub(crate) rust_file: &'static str,
    pub(crate) rust_owner: &'static str,
    pub(crate) rust_boundary: &'static str,
    pub(crate) ts_test_file: PathBuf,
}

impl TypeScriptBunBridgeHint {
    pub(crate) fn evidence_lines(&self) -> Vec<String> {
        let missing = self.verdict.missing_discriminators();
        let missing = if missing.is_empty() {
            "none".to_string()
        } else {
            missing.join(",")
        };
        let mut lines = vec![
            format!(
                "typescript_bun_ub_bridge_hint: confidence={} rust_file={} rust_owner={} rust_boundary=\"{}\" ts_test_file={}",
                self.confidence.as_str(),
                self.rust_file,
                self.rust_owner,
                self.rust_boundary,
                normalized_path(&self.ts_test_file)
            ),
            format!(
                "typescript_bun_ub_bridge_verdict: {} missing_discriminators={} action={} suggested_test_file={} repair_packet_ready=false",
                self.verdict.as_str(),
                missing,
                self.verdict.expected_action(),
                self.suggested_test_file()
            ),
            format!(
                "typescript_bun_ub_cross_language_grip: state={} rust_grip=ungripped ts_verdict={} action={} authority=preview_advisory_only suggested_test_file={} repair_packet_ready=false",
                self.verdict.cross_language_state(),
                self.verdict.as_str(),
                self.verdict.expected_action(),
                self.suggested_test_file()
            ),
            "typescript_bun_ub_bridge_boundary: preview_advisory_only no_source_edits no_generated_tests no_runtime_bun_execution no_mutation_execution no_default_gates no_badge_baseline_zero_or_support_tier_authority".to_string(),
        ];
        if let Some(reason) = self.placement_reason() {
            lines.push(format!(
                "typescript_bun_ub_test_placement: rank=1 suggested_test_file={} reason=\"{}\" basis=configured_bridge_suggested_test_file,same_js_surface,same_boundary_vocabulary authority=preview_advisory_only repair_packet_ready=false",
                self.suggested_test_file(),
                reason
            ));
        }
        lines
    }

    pub(crate) fn suggested_test_file(&self) -> &'static str {
        match (self.profile_kind, self.verdict) {
            (
                TypeScriptBunBridgeProfileKind::BlobArrayBuffer,
                TypeScriptBunBridgeVerdict::TsMissingResizable
                | TypeScriptBunBridgeVerdict::TsMissingShared
                | TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable,
            ) => self.profile_kind.ts_test_file(),
            _ => "not_applicable",
        }
    }

    pub(crate) fn placement_reason(&self) -> Option<String> {
        self.profile_kind.placement_reason(self.verdict)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TypeScriptBunBridgeConfidence {
    ConfiguredHint,
    Unknown,
}

impl TypeScriptBunBridgeConfidence {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ConfiguredHint => "configured_hint",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TypeScriptBunBridgeVerdict {
    TsDiscriminated,
    TsMissingResizable,
    TsMissingShared,
    TsMissingSharedAndResizable,
    TsMissingExternalOracle,
    TsMentionNotObserver,
    BridgeUnknown,
}

impl TypeScriptBunBridgeVerdict {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::TsDiscriminated => "ts_discriminated",
            Self::TsMissingResizable => "ts_missing_resizable",
            Self::TsMissingShared => "ts_missing_shared",
            Self::TsMissingSharedAndResizable => "ts_missing_shared_and_resizable",
            Self::TsMissingExternalOracle => "ts_missing_external_oracle",
            Self::TsMentionNotObserver => "ts_mention_not_observer",
            Self::BridgeUnknown => "bridge_unknown",
        }
    }

    pub(crate) fn missing_discriminators(self) -> &'static [&'static str] {
        match self {
            Self::TsMissingResizable => &["resizable_array_buffer"],
            Self::TsMissingShared => &["shared_array_buffer"],
            Self::TsMissingSharedAndResizable => &["shared_array_buffer", "resizable_array_buffer"],
            Self::TsDiscriminated
            | Self::TsMissingExternalOracle
            | Self::TsMentionNotObserver
            | Self::BridgeUnknown => &[],
        }
    }

    pub(crate) fn expected_action(self) -> &'static str {
        match self {
            Self::TsDiscriminated => "no_missing_bridge_discriminator",
            Self::TsMissingResizable
            | Self::TsMissingShared
            | Self::TsMissingSharedAndResizable
            | Self::TsMissingExternalOracle => "route_cross_language_oracle_visibility_limitation",
            Self::TsMentionNotObserver => "do_not_credit_token_mention",
            Self::BridgeUnknown => "report_bridge_unknown_not_no_static_path",
        }
    }

    pub(crate) fn cross_language_state(self) -> &'static str {
        match self {
            Self::TsDiscriminated => "rust_ungripped_ts_discriminated",
            Self::TsMissingResizable
            | Self::TsMissingShared
            | Self::TsMissingSharedAndResizable => "rust_ungripped_ts_missing_discriminator",
            Self::TsMissingExternalOracle => "rust_ungripped_ts_missing_external_oracle",
            Self::TsMentionNotObserver => "ts_mention_not_observer",
            Self::BridgeUnknown => "bridge_unknown",
        }
    }

    pub(crate) fn exposure_class(self) -> ExposureClass {
        match self {
            Self::TsDiscriminated => ExposureClass::Exposed,
            Self::TsMissingResizable
            | Self::TsMissingShared
            | Self::TsMissingSharedAndResizable
            | Self::TsMissingExternalOracle => ExposureClass::StaticUnknown,
            Self::TsMentionNotObserver | Self::BridgeUnknown => ExposureClass::StaticUnknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptBunBridgeProfile {
    pub(crate) kind: TypeScriptBunBridgeProfileKind,
    pub(crate) confidence: TypeScriptBunBridgeConfidence,
    pub(crate) rust_file: &'static str,
    pub(crate) rust_owner: &'static str,
    pub(crate) rust_boundary: &'static str,
    pub(crate) ts_test_file: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypeScriptBunBridgeProfileKind {
    BlobArrayBuffer,
    ArrayBufferCopyToUnshared,
    MarkdownResizableArrayBuffer,
}

impl TypeScriptBunBridgeProfileKind {
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::BlobArrayBuffer => "Bun Blob",
            Self::ArrayBufferCopyToUnshared => "Bun ArrayBuffer copy_to_unshared",
            Self::MarkdownResizableArrayBuffer => "Bun MarkdownObject",
        }
    }

    pub(crate) fn ts_test_file(self) -> &'static str {
        match self {
            Self::BlobArrayBuffer | Self::ArrayBufferCopyToUnshared => {
                BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE
            }
            Self::MarkdownResizableArrayBuffer => BUN_MARKDOWN_RESIZABLE_TS_TEST_FILE,
        }
    }

    pub(crate) fn line_text_matches(self, line_text: &str) -> bool {
        match self {
            Self::BlobArrayBuffer => {
                line_text.contains("array_buffer.shared")
                    && line_text.contains("array_buffer.resizable")
            }
            Self::ArrayBufferCopyToUnshared => {
                line_text_matches_bun_copy_to_unshared_boundary(line_text)
            }
            Self::MarkdownResizableArrayBuffer => {
                line_text.contains("self.0.resizable") && line_text.contains("!self.0.shared")
            }
        }
    }

    pub(crate) fn expected_sinks(self) -> Vec<String> {
        match self {
            Self::BlobArrayBuffer | Self::ArrayBufferCopyToUnshared => {
                vec!["stable_byte_copy".to_string()]
            }
            Self::MarkdownResizableArrayBuffer => vec!["markdown_output".to_string()],
        }
    }

    pub(crate) fn required_oracles(self) -> Vec<String> {
        match self {
            Self::BlobArrayBuffer | Self::ArrayBufferCopyToUnshared => vec![
                "shared_array_buffer".to_string(),
                "resizable_array_buffer".to_string(),
                "stable_byte_copy_oracle".to_string(),
            ],
            Self::MarkdownResizableArrayBuffer => vec![
                "resizable_array_buffer".to_string(),
                "bun_markdown_callsite".to_string(),
                "markdown_strong_oracle".to_string(),
            ],
        }
    }

    pub(crate) fn placement_reason(self, verdict: TypeScriptBunBridgeVerdict) -> Option<String> {
        match (self, verdict) {
            (Self::BlobArrayBuffer, TypeScriptBunBridgeVerdict::TsMissingResizable) => Some(
                "existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer".to_string(),
            ),
            (Self::BlobArrayBuffer, TypeScriptBunBridgeVerdict::TsMissingShared) => Some(
                "existing Blob + ArrayBuffer integration tests live there; missing discriminator is SharedArrayBuffer".to_string(),
            ),
            (Self::BlobArrayBuffer, TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable) => Some(
                "existing Blob + ArrayBuffer integration tests live there; missing discriminators are SharedArrayBuffer and resizable ArrayBuffer".to_string(),
            ),
            _ => None,
        }
    }

    pub(crate) fn complete_observe_summary(self) -> &'static str {
        match self {
            Self::BlobArrayBuffer => {
                "TypeScript Blob ArrayBuffer integration evidence contains a stable-byte observer."
            }
            Self::ArrayBufferCopyToUnshared => {
                "TypeScript Blob ArrayBuffer copy evidence contains a stable-byte observer."
            }
            Self::MarkdownResizableArrayBuffer => {
                "TypeScript Bun markdown integration evidence contains a strong output observer."
            }
        }
    }

    pub(crate) fn complete_discriminate_summary(self) -> &'static str {
        match self {
            Self::BlobArrayBuffer => {
                "TypeScript evidence discriminates SharedArrayBuffer and resizable ArrayBuffer branches for the configured Rust seam."
            }
            Self::ArrayBufferCopyToUnshared => {
                "TypeScript evidence discriminates SharedArrayBuffer and resizable ArrayBuffer copy semantics for the configured Rust seam."
            }
            Self::MarkdownResizableArrayBuffer => {
                "TypeScript evidence discriminates a resizable non-shared ArrayBuffer for the configured Rust seam."
            }
        }
    }

    pub(crate) fn missing_resizable_summary(self) -> &'static str {
        match self {
            Self::BlobArrayBuffer => {
                "TypeScript evidence is missing the resizable ArrayBuffer discriminator for the configured Rust seam."
            }
            Self::ArrayBufferCopyToUnshared => {
                "TypeScript evidence is missing the resizable ArrayBuffer copy discriminator for the configured Rust seam."
            }
            Self::MarkdownResizableArrayBuffer => {
                "TypeScript evidence is missing the resizable non-shared ArrayBuffer discriminator for the configured Rust seam."
            }
        }
    }

    pub(crate) fn configured_bridge_sample(self, ts_test_file: &Path) -> String {
        match self {
            Self::BlobArrayBuffer => {
                format!(
                    "configured Bun Blob bridge to {}",
                    normalized_path(ts_test_file)
                )
            }
            Self::ArrayBufferCopyToUnshared => {
                format!(
                    "configured Bun copy_to_unshared bridge to {}",
                    normalized_path(ts_test_file)
                )
            }
            Self::MarkdownResizableArrayBuffer => {
                format!(
                    "configured Bun MarkdownObject bridge to {}",
                    normalized_path(ts_test_file)
                )
            }
        }
    }
}

pub(crate) const BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE: &str = "test/js/web/fetch/blob.test.ts";
pub(crate) const BUN_BLOB_ARRAY_BUFFER_RUST_FILE: &str = "src/jsc/Blob.rs";
pub(crate) const BUN_BLOB_ARRAY_BUFFER_RUST_OWNER: &str = "Blob::from_js_without_defer_gc";
pub(crate) const BUN_BLOB_ARRAY_BUFFER_RUST_BOUNDARY: &str =
    "array_buffer.shared || array_buffer.resizable";
pub(crate) const BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_RUST_FILE: &str = "src/jsc/array_buffer.rs";
pub(crate) const BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_RUST_OWNER: &str = "copy_to_unshared";
pub(crate) const BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_RUST_BOUNDARY: &str =
    "SharedArrayBuffer and resizable ArrayBuffer copy semantics";

pub(crate) const BUN_BLOB_ARRAY_BUFFER_BRIDGE_PROFILE: TypeScriptBunBridgeProfile =
    TypeScriptBunBridgeProfile {
        kind: TypeScriptBunBridgeProfileKind::BlobArrayBuffer,
        confidence: TypeScriptBunBridgeConfidence::ConfiguredHint,
        rust_file: BUN_BLOB_ARRAY_BUFFER_RUST_FILE,
        rust_owner: BUN_BLOB_ARRAY_BUFFER_RUST_OWNER,
        rust_boundary: BUN_BLOB_ARRAY_BUFFER_RUST_BOUNDARY,
        ts_test_file: BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE,
    };

pub(crate) const BUN_MARKDOWN_RESIZABLE_TS_TEST_FILE: &str = "test/js/bun/md/md-edge-cases.test.ts";
pub(crate) const BUN_MARKDOWN_RESIZABLE_RUST_FILE: &str = "src/runtime/api/MarkdownObject.rs";
pub(crate) const BUN_MARKDOWN_RESIZABLE_RUST_OWNER: &str = "MarkdownObject::to_string";
pub(crate) const BUN_MARKDOWN_RESIZABLE_RUST_BOUNDARY: &str = "self.0.resizable && !self.0.shared";

pub(crate) const BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_BRIDGE_PROFILE: TypeScriptBunBridgeProfile =
    TypeScriptBunBridgeProfile {
        confidence: TypeScriptBunBridgeConfidence::ConfiguredHint,
        kind: TypeScriptBunBridgeProfileKind::ArrayBufferCopyToUnshared,
        rust_file: BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_RUST_FILE,
        rust_owner: BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_RUST_OWNER,
        rust_boundary: BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_RUST_BOUNDARY,
        ts_test_file: BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE,
    };

pub(crate) const BUN_MARKDOWN_RESIZABLE_BRIDGE_PROFILE: TypeScriptBunBridgeProfile =
    TypeScriptBunBridgeProfile {
        kind: TypeScriptBunBridgeProfileKind::MarkdownResizableArrayBuffer,
        confidence: TypeScriptBunBridgeConfidence::ConfiguredHint,
        rust_file: BUN_MARKDOWN_RESIZABLE_RUST_FILE,
        rust_owner: BUN_MARKDOWN_RESIZABLE_RUST_OWNER,
        rust_boundary: BUN_MARKDOWN_RESIZABLE_RUST_BOUNDARY,
        ts_test_file: BUN_MARKDOWN_RESIZABLE_TS_TEST_FILE,
    };

pub(crate) const BUN_RUST_CROSS_LANGUAGE_BRIDGE_PROFILES: &[TypeScriptBunBridgeProfile] = &[
    BUN_BLOB_ARRAY_BUFFER_BRIDGE_PROFILE,
    BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_BRIDGE_PROFILE,
    BUN_MARKDOWN_RESIZABLE_BRIDGE_PROFILE,
];
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TypeScriptBunArrayBufferObservation {
    pub(crate) shared_array_buffer: bool,
    pub(crate) resizable_array_buffer: bool,
    pub(crate) view_backed_blob_input: bool,
    pub(crate) markdown_external_callsite: bool,
    pub(crate) stable_byte_copy_oracle: bool,
    pub(crate) markdown_strong_oracle: bool,
    pub(crate) max_byte_length_mention_only: bool,
}

impl TypeScriptBunArrayBufferObservation {
    pub(crate) fn has_complete_blob_observer(&self) -> bool {
        self.view_backed_blob_input && self.stable_byte_copy_oracle
    }

    pub(crate) fn has_partial_blob_observer(&self) -> bool {
        self.view_backed_blob_input || self.stable_byte_copy_oracle
    }

    pub(crate) fn has_all_bridge_discriminators(&self) -> bool {
        self.shared_array_buffer && self.resizable_array_buffer && self.has_complete_blob_observer()
    }

    pub(crate) fn has_complete_markdown_observer(&self) -> bool {
        self.markdown_external_callsite && self.markdown_strong_oracle
    }

    pub(crate) fn has_partial_markdown_observer(&self) -> bool {
        self.markdown_external_callsite || self.markdown_strong_oracle
    }

    pub(crate) fn bridge_verdict(
        &self,
        confidence: TypeScriptBunBridgeConfidence,
        profile_kind: TypeScriptBunBridgeProfileKind,
    ) -> Option<TypeScriptBunBridgeVerdict> {
        if profile_kind == TypeScriptBunBridgeProfileKind::MarkdownResizableArrayBuffer {
            return self.markdown_bridge_verdict(confidence);
        }
        self.blob_bridge_verdict(confidence)
    }

    pub(crate) fn blob_bridge_verdict(
        &self,
        confidence: TypeScriptBunBridgeConfidence,
    ) -> Option<TypeScriptBunBridgeVerdict> {
        if confidence == TypeScriptBunBridgeConfidence::Unknown {
            return self
                .has_all_bridge_discriminators()
                .then_some(TypeScriptBunBridgeVerdict::BridgeUnknown);
        }
        if self.max_byte_length_mention_only && !self.has_partial_blob_observer() {
            return Some(TypeScriptBunBridgeVerdict::TsMentionNotObserver);
        }
        if !self.has_complete_blob_observer() {
            if self.has_partial_blob_observer() {
                return Some(TypeScriptBunBridgeVerdict::TsMissingExternalOracle);
            }
            return None;
        }
        match (self.shared_array_buffer, self.resizable_array_buffer) {
            (true, true) => Some(TypeScriptBunBridgeVerdict::TsDiscriminated),
            (true, false) => Some(TypeScriptBunBridgeVerdict::TsMissingResizable),
            (false, true) => Some(TypeScriptBunBridgeVerdict::TsMissingShared),
            (false, false) => Some(TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable),
        }
    }

    pub(crate) fn markdown_bridge_verdict(
        &self,
        confidence: TypeScriptBunBridgeConfidence,
    ) -> Option<TypeScriptBunBridgeVerdict> {
        let complete = self.resizable_array_buffer && self.has_complete_markdown_observer();
        if confidence == TypeScriptBunBridgeConfidence::Unknown {
            return complete.then_some(TypeScriptBunBridgeVerdict::BridgeUnknown);
        }
        if !self.has_complete_markdown_observer() {
            if self.resizable_array_buffer || self.has_partial_markdown_observer() {
                return Some(TypeScriptBunBridgeVerdict::TsMissingExternalOracle);
            }
            return None;
        }
        if self.resizable_array_buffer {
            Some(TypeScriptBunBridgeVerdict::TsDiscriminated)
        } else {
            Some(TypeScriptBunBridgeVerdict::TsMissingResizable)
        }
    }
}

pub(crate) fn collect_related_bun_array_buffer_facts(
    candidates: &[TypeScriptRelatedCandidate<'_>],
) -> Vec<TypeScriptBunArrayBufferFact> {
    let mut facts = Vec::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.relation.uses_oracle())
    {
        for fact in bun_array_buffer_facts_for_test(candidate.test) {
            push_unique_bun_array_buffer_fact(&mut facts, fact);
        }
    }
    sort_bun_array_buffer_facts(&mut facts);
    facts
}

pub(crate) fn collect_related_bun_bridge_hints(
    facts: &[TypeScriptBunArrayBufferFact],
) -> Vec<TypeScriptBunBridgeHint> {
    let mut hints = Vec::new();
    for profile in [
        BUN_BLOB_ARRAY_BUFFER_BRIDGE_PROFILE,
        BUN_MARKDOWN_RESIZABLE_BRIDGE_PROFILE,
    ] {
        if let Some(hint) = bun_bridge_hint_for_profile(facts, profile) {
            push_unique_bun_bridge_hint(&mut hints, hint);
        }
    }
    sort_bun_bridge_hints(&mut hints);
    hints
}

pub(crate) fn collect_profile_bun_array_buffer_facts(
    all_tests: &[TypeScriptTest],
    profile: TypeScriptBunBridgeProfile,
) -> Vec<TypeScriptBunArrayBufferFact> {
    let mut facts = Vec::new();
    for test in all_tests
        .iter()
        .filter(|test| normalized_path(&test.file) == profile.ts_test_file)
    {
        for fact in bun_array_buffer_facts_for_test(test) {
            push_unique_bun_array_buffer_fact(&mut facts, fact);
        }
    }
    sort_bun_array_buffer_facts(&mut facts);
    facts
}

pub(crate) fn related_profile_bun_tests(
    all_tests: &[TypeScriptTest],
    profile: TypeScriptBunBridgeProfile,
) -> Vec<RelatedTest> {
    let mut related = all_tests
        .iter()
        .filter(|test| normalized_path(&test.file) == profile.ts_test_file)
        .filter(|test| !bun_array_buffer_facts_for_test(test).is_empty())
        .map(|test| {
            let strongest = strongest_assertion(&test.assertions);
            let (oracle_kind, oracle_strength, oracle_text) = match strongest {
                Some(assertion) => (
                    assertion.oracle_kind.clone(),
                    assertion.oracle_strength.clone(),
                    Some(assertion_oracle_text(assertion)),
                ),
                None => (OracleKind::Unknown, OracleStrength::Unknown, None),
            };
            RelatedTest {
                name: test.name.clone(),
                file: test.file.clone(),
                line: test.line,
                oracle: oracle_text,
                oracle_kind,
                oracle_strength,
            }
        })
        .collect::<Vec<_>>();
    related.sort_by(|left, right| {
        right
            .oracle_strength
            .rank()
            .cmp(&left.oracle_strength.rank())
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.name.cmp(&right.name))
    });
    related
}

pub(crate) fn bun_bridge_hint_for_profile(
    facts: &[TypeScriptBunArrayBufferFact],
    profile: TypeScriptBunBridgeProfile,
) -> Option<TypeScriptBunBridgeHint> {
    let observation = bun_array_buffer_observation_for_profile(facts, profile)?;
    let verdict = observation.bridge_verdict(profile.confidence, profile.kind)?;
    Some(TypeScriptBunBridgeHint {
        profile_kind: profile.kind,
        confidence: profile.confidence,
        verdict,
        rust_file: profile.rust_file,
        rust_owner: profile.rust_owner,
        rust_boundary: profile.rust_boundary,
        ts_test_file: PathBuf::from(profile.ts_test_file),
    })
}

pub(crate) fn bun_array_buffer_observation_for_profile(
    facts: &[TypeScriptBunArrayBufferFact],
    profile: TypeScriptBunBridgeProfile,
) -> Option<TypeScriptBunArrayBufferObservation> {
    let mut observation = TypeScriptBunArrayBufferObservation::default();
    let mut observed_any_profile_fact = false;
    for fact in facts
        .iter()
        .filter(|fact| normalized_path(&fact.file) == profile.ts_test_file)
    {
        match fact.kind {
            TypeScriptBunArrayBufferFactKind::SharedArrayBuffer => {
                observation.shared_array_buffer = true;
                observed_any_profile_fact = true;
            }
            TypeScriptBunArrayBufferFactKind::ResizableArrayBuffer => {
                observation.resizable_array_buffer = true;
                observed_any_profile_fact = true;
            }
            TypeScriptBunArrayBufferFactKind::ViewBackedBlobInput => {
                observation.view_backed_blob_input = true;
                observed_any_profile_fact = true;
            }
            TypeScriptBunArrayBufferFactKind::MarkdownExternalCallsite => {
                observation.markdown_external_callsite = true;
                observed_any_profile_fact = true;
            }
            TypeScriptBunArrayBufferFactKind::StableByteCopyOracle => {
                observation.stable_byte_copy_oracle = true;
                observed_any_profile_fact = true;
            }
            TypeScriptBunArrayBufferFactKind::MarkdownStrongOracle => {
                observation.markdown_strong_oracle = true;
                observed_any_profile_fact = true;
            }
            TypeScriptBunArrayBufferFactKind::MaxByteLengthMentionOnly => {
                observation.max_byte_length_mention_only = true;
                observed_any_profile_fact = true;
            }
            TypeScriptBunArrayBufferFactKind::ArrayBufferResize
            | TypeScriptBunArrayBufferFactKind::ArrayBufferView
            | TypeScriptBunArrayBufferFactKind::BlobArrayBufferObserver
            | TypeScriptBunArrayBufferFactKind::WeakByteSmokeOracle
            | TypeScriptBunArrayBufferFactKind::WeakByteSnapshotOracle
            | TypeScriptBunArrayBufferFactKind::ByteOracleMentionOnly => {}
        }
    }
    observed_any_profile_fact.then_some(observation)
}

pub(crate) fn bun_array_buffer_facts_for_test(
    test: &TypeScriptTest,
) -> Vec<TypeScriptBunArrayBufferFact> {
    extract_bun_array_buffer_facts_from_body(
        &test.file,
        &test.body_text,
        test.line,
        &test.assertions,
    )
}

pub(crate) fn extract_bun_array_buffer_facts_from_body(
    file: &Path,
    body_text: &str,
    start_line: usize,
    assertions: &[TypeScriptAssertion],
) -> Vec<TypeScriptBunArrayBufferFact> {
    let mut facts = Vec::new();

    push_bun_facts_for_shape(
        &mut facts,
        file,
        body_text,
        start_line,
        "new SharedArrayBuffer(",
        TypeScriptBunArrayBufferFactKind::SharedArrayBuffer,
    );
    push_resizable_array_buffer_facts(&mut facts, file, body_text, start_line);
    push_bun_facts_for_shape(
        &mut facts,
        file,
        body_text,
        start_line,
        ".resize(",
        TypeScriptBunArrayBufferFactKind::ArrayBufferResize,
    );
    for view_shape in [
        "new Uint8Array(",
        "new Uint8ClampedArray(",
        "new Uint16Array(",
        "new Uint32Array(",
        "new BigUint64Array(",
        "new Int8Array(",
        "new Int16Array(",
        "new Int32Array(",
        "new BigInt64Array(",
        "new Float32Array(",
        "new Float64Array(",
        "new DataView(",
    ] {
        push_bun_facts_for_shape(
            &mut facts,
            file,
            body_text,
            start_line,
            view_shape,
            TypeScriptBunArrayBufferFactKind::ArrayBufferView,
        );
    }
    push_bun_facts_for_shape(
        &mut facts,
        file,
        body_text,
        start_line,
        ".arrayBuffer(",
        TypeScriptBunArrayBufferFactKind::BlobArrayBufferObserver,
    );

    let has_view = facts
        .iter()
        .any(|fact| fact.kind == TypeScriptBunArrayBufferFactKind::ArrayBufferView);
    if has_view {
        push_view_backed_blob_input_facts(&mut facts, file, body_text, start_line);
    }

    push_byte_oracle_facts(&mut facts, file, body_text, start_line, assertions);
    push_markdown_oracle_facts(&mut facts, file, body_text, start_line, assertions);

    let has_view_backed_blob = facts
        .iter()
        .any(|fact| fact.kind == TypeScriptBunArrayBufferFactKind::ViewBackedBlobInput);
    let has_stable_byte_oracle = facts
        .iter()
        .any(|fact| fact.kind == TypeScriptBunArrayBufferFactKind::StableByteCopyOracle);
    if (!has_view_backed_blob || !has_stable_byte_oracle)
        && let Some(idx) = first_unquoted_token_index(body_text, "maxByteLength")
    {
        push_unique_bun_array_buffer_fact(
            &mut facts,
            bun_array_buffer_fact(
                file,
                body_text,
                start_line,
                idx,
                TypeScriptBunArrayBufferFactKind::MaxByteLengthMentionOnly,
            ),
        );
    }

    sort_bun_array_buffer_facts(&mut facts);
    facts
}

fn push_resizable_array_buffer_facts(
    facts: &mut Vec<TypeScriptBunArrayBufferFact>,
    file: &Path,
    body_text: &str,
    start_line: usize,
) {
    for idx in unquoted_shape_indices(body_text, "new ArrayBuffer(") {
        let Some(call_text) = delimited_call_text_at(body_text, idx, "new ArrayBuffer(") else {
            continue;
        };
        if contains_unquoted_token(call_text, "maxByteLength") {
            push_unique_bun_array_buffer_fact(
                facts,
                bun_array_buffer_fact(
                    file,
                    body_text,
                    start_line,
                    idx,
                    TypeScriptBunArrayBufferFactKind::ResizableArrayBuffer,
                ),
            );
        }
    }
}

fn push_bun_facts_for_shape(
    facts: &mut Vec<TypeScriptBunArrayBufferFact>,
    file: &Path,
    body_text: &str,
    start_line: usize,
    shape: &str,
    kind: TypeScriptBunArrayBufferFactKind,
) {
    for idx in unquoted_shape_indices(body_text, shape) {
        push_unique_bun_array_buffer_fact(
            facts,
            bun_array_buffer_fact(file, body_text, start_line, idx, kind),
        );
    }
}

fn push_view_backed_blob_input_facts(
    facts: &mut Vec<TypeScriptBunArrayBufferFact>,
    file: &Path,
    body_text: &str,
    start_line: usize,
) {
    for idx in unquoted_shape_indices(body_text, "new Blob(") {
        let Some(call_text) = delimited_call_text_at(body_text, idx, "new Blob(") else {
            continue;
        };
        if contains_unquoted_shape(call_text, "[") {
            push_unique_bun_array_buffer_fact(
                facts,
                bun_array_buffer_fact(
                    file,
                    body_text,
                    start_line,
                    idx,
                    TypeScriptBunArrayBufferFactKind::ViewBackedBlobInput,
                ),
            );
        }
    }
}

fn push_byte_oracle_facts(
    facts: &mut Vec<TypeScriptBunArrayBufferFact>,
    file: &Path,
    body_text: &str,
    start_line: usize,
    assertions: &[TypeScriptAssertion],
) {
    let Some(blob_read_idx) = first_blob_byte_read_index(body_text) else {
        return;
    };
    if assertions.iter().any(assertion_is_exact_value)
        && body_has_byte_or_text_observer(body_text)
        && let Some(idx) = first_exact_value_matcher_index(body_text).or(Some(blob_read_idx))
    {
        push_unique_bun_array_buffer_fact(
            facts,
            bun_array_buffer_fact(
                file,
                body_text,
                start_line,
                idx,
                TypeScriptBunArrayBufferFactKind::StableByteCopyOracle,
            ),
        );
        return;
    }
    if assertions.iter().any(assertion_is_snapshot)
        && let Some(idx) = first_snapshot_matcher_index(body_text).or(Some(blob_read_idx))
    {
        push_unique_bun_array_buffer_fact(
            facts,
            bun_array_buffer_fact(
                file,
                body_text,
                start_line,
                idx,
                TypeScriptBunArrayBufferFactKind::WeakByteSnapshotOracle,
            ),
        );
        return;
    }
    if assertions.iter().any(assertion_is_smoke)
        && let Some(idx) = first_smoke_matcher_index(body_text).or(Some(blob_read_idx))
    {
        push_unique_bun_array_buffer_fact(
            facts,
            bun_array_buffer_fact(
                file,
                body_text,
                start_line,
                idx,
                TypeScriptBunArrayBufferFactKind::WeakByteSmokeOracle,
            ),
        );
        return;
    }
    push_unique_bun_array_buffer_fact(
        facts,
        bun_array_buffer_fact(
            file,
            body_text,
            start_line,
            blob_read_idx,
            TypeScriptBunArrayBufferFactKind::ByteOracleMentionOnly,
        ),
    );
}

fn push_markdown_oracle_facts(
    facts: &mut Vec<TypeScriptBunArrayBufferFact>,
    file: &Path,
    body_text: &str,
    start_line: usize,
    assertions: &[TypeScriptAssertion],
) {
    let Some(markdown_idx) = first_bun_markdown_callsite_index(body_text) else {
        return;
    };
    push_unique_bun_array_buffer_fact(
        facts,
        bun_array_buffer_fact(
            file,
            body_text,
            start_line,
            markdown_idx,
            TypeScriptBunArrayBufferFactKind::MarkdownExternalCallsite,
        ),
    );
    if assertions.iter().any(assertion_is_exact_value)
        && let Some(idx) = first_exact_value_matcher_index(body_text).or(Some(markdown_idx))
    {
        push_unique_bun_array_buffer_fact(
            facts,
            bun_array_buffer_fact(
                file,
                body_text,
                start_line,
                idx,
                TypeScriptBunArrayBufferFactKind::MarkdownStrongOracle,
            ),
        );
    }
}

fn first_bun_markdown_callsite_index(body_text: &str) -> Option<usize> {
    [
        "Bun.markdown(",
        "Bun.md(",
        "Bun.Markdown(",
        "Bun.MarkdownObject(",
    ]
    .into_iter()
    .filter_map(|shape| first_unquoted_shape_index(body_text, shape))
    .min()
}

fn first_blob_byte_read_index(body_text: &str) -> Option<usize> {
    first_unquoted_shape_index(body_text, ".arrayBuffer(")
        .or_else(|| first_unquoted_shape_index(body_text, ".text("))
}

fn body_has_byte_or_text_observer(body_text: &str) -> bool {
    [
        "new Uint8Array(",
        "new Uint8ClampedArray(",
        "new DataView(",
        "Array.from(",
        "[...",
        ".text(",
    ]
    .into_iter()
    .any(|shape| contains_unquoted_shape(body_text, shape))
}

pub(crate) fn assertion_is_exact_value(assertion: &TypeScriptAssertion) -> bool {
    assertion.oracle_kind == OracleKind::ExactValue
        && assertion.oracle_strength.rank() >= OracleStrength::Strong.rank()
}

pub(crate) fn assertion_is_snapshot(assertion: &TypeScriptAssertion) -> bool {
    assertion.oracle_kind == OracleKind::Snapshot
}

pub(crate) fn assertion_is_smoke(assertion: &TypeScriptAssertion) -> bool {
    assertion.oracle_kind == OracleKind::SmokeOnly
}

pub(crate) fn first_exact_value_matcher_index(body_text: &str) -> Option<usize> {
    [".toEqual(", ".toStrictEqual(", ".toBe("]
        .into_iter()
        .filter_map(|shape| first_unquoted_shape_index(body_text, shape))
        .min()
}

fn first_snapshot_matcher_index(body_text: &str) -> Option<usize> {
    [".toMatchSnapshot(", ".toMatchInlineSnapshot("]
        .into_iter()
        .filter_map(|shape| first_unquoted_shape_index(body_text, shape))
        .min()
}

fn first_smoke_matcher_index(body_text: &str) -> Option<usize> {
    [
        ".toBeTruthy(",
        ".toBeFalsy(",
        ".toBeDefined(",
        ".toBeUndefined(",
        ".toBeNull(",
        ".toBeNaN(",
    ]
    .into_iter()
    .filter_map(|shape| first_unquoted_shape_index(body_text, shape))
    .min()
}

pub(crate) fn bun_array_buffer_fact(
    file: &Path,
    body_text: &str,
    start_line: usize,
    idx: usize,
    kind: TypeScriptBunArrayBufferFactKind,
) -> TypeScriptBunArrayBufferFact {
    TypeScriptBunArrayBufferFact {
        kind,
        file: file.to_path_buf(),
        line: line_for_body_offset(body_text, start_line, idx),
        text: source_line_at_offset(body_text, idx),
    }
}

pub(crate) fn line_for_body_offset(body_text: &str, start_line: usize, idx: usize) -> usize {
    start_line
        + body_text[..idx]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
}

pub(crate) fn source_line_at_offset(body_text: &str, idx: usize) -> String {
    let line_start = body_text[..idx].rfind('\n').map_or(0, |offset| offset + 1);
    let line_end = body_text[idx..]
        .find('\n')
        .map_or(body_text.len(), |offset| idx + offset);
    let mut line = body_text[line_start..line_end].trim().to_string();
    const MAX_FACT_TEXT: usize = 160;
    if line.len() > MAX_FACT_TEXT {
        line.truncate(MAX_FACT_TEXT);
        line.push_str("...");
    }
    line
}

pub(crate) fn delimited_call_text_at<'a>(
    body_text: &'a str,
    idx: usize,
    shape: &str,
) -> Option<&'a str> {
    let open_idx = idx + shape.len() - 1;
    if body_text.as_bytes().get(open_idx).copied()? != b'(' {
        return None;
    }
    let close_idx = matching_close_paren(body_text, open_idx)?;
    body_text.get(idx..=close_idx)
}

fn matching_close_paren(body_text: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut escaped = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_template = false;
    for (idx, ch) in body_text[open_idx..].char_indices() {
        let absolute = open_idx + idx;
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double && !in_template {
            in_single = !in_single;
            continue;
        }
        if ch == '"' && !in_single && !in_template {
            in_double = !in_double;
            continue;
        }
        if ch == '`' && !in_single && !in_double {
            in_template = !in_template;
            continue;
        }
        if in_single || in_double || in_template || inside_block_comment(body_text, absolute) {
            continue;
        }
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(absolute);
            }
        }
    }
    None
}

fn contains_unquoted_token(text: &str, token: &str) -> bool {
    first_unquoted_token_index(text, token).is_some()
}

fn first_unquoted_token_index(text: &str, token: &str) -> Option<usize> {
    unquoted_shape_indices(text, token)
        .into_iter()
        .find(|idx| has_token_boundary(text, *idx, token.len()))
}

fn first_unquoted_shape_index(text: &str, shape: &str) -> Option<usize> {
    unquoted_shape_indices(text, shape).into_iter().next()
}

fn unquoted_shape_indices(text: &str, shape: &str) -> Vec<usize> {
    text.match_indices(shape)
        .filter_map(|(idx, _)| {
            (!line_prefix_looks_like_comment_or_string(text, idx)
                && !inside_block_comment(text, idx))
            .then_some(idx)
        })
        .collect()
}

fn has_token_boundary(text: &str, idx: usize, len: usize) -> bool {
    text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_javascript_identifier_char(ch))
        && text
            .get(idx + len..)
            .and_then(|tail| tail.chars().next())
            .is_none_or(|ch| !is_javascript_identifier_char(ch))
}

fn push_unique_bun_array_buffer_fact(
    facts: &mut Vec<TypeScriptBunArrayBufferFact>,
    fact: TypeScriptBunArrayBufferFact,
) {
    if !facts.iter().any(|existing| existing == &fact) {
        facts.push(fact);
    }
}

fn sort_bun_array_buffer_facts(facts: &mut [TypeScriptBunArrayBufferFact]) {
    facts.sort_by(|left, right| {
        normalized_path(&left.file)
            .cmp(&normalized_path(&right.file))
            .then(left.line.cmp(&right.line))
            .then(left.kind.cmp(&right.kind))
            .then(left.text.cmp(&right.text))
    });
}

fn push_unique_bun_bridge_hint(
    hints: &mut Vec<TypeScriptBunBridgeHint>,
    hint: TypeScriptBunBridgeHint,
) {
    if !hints.iter().any(|existing| existing == &hint) {
        hints.push(hint);
    }
}

fn sort_bun_bridge_hints(hints: &mut [TypeScriptBunBridgeHint]) {
    hints.sort_by(|left, right| {
        normalized_path(&left.ts_test_file)
            .cmp(&normalized_path(&right.ts_test_file))
            .then(left.confidence.cmp(&right.confidence))
            .then(left.verdict.cmp(&right.verdict))
            .then(left.rust_file.cmp(right.rust_file))
            .then(left.rust_owner.cmp(right.rust_owner))
            .then(left.rust_boundary.cmp(right.rust_boundary))
    });
}

pub(crate) fn bun_cross_language_finding_for_changed_rust_line(
    file: &Path,
    line: usize,
    line_text: &str,
    all_tests: &[TypeScriptTest],
) -> Option<Finding> {
    BUN_RUST_CROSS_LANGUAGE_BRIDGE_PROFILES
        .iter()
        .find_map(|profile| {
            bun_cross_language_finding_for_changed_rust_line_with_profile(
                file, line, line_text, all_tests, *profile,
            )
        })
}

pub(crate) fn bun_cross_language_finding_for_changed_rust_line_with_profile(
    file: &Path,
    line: usize,
    line_text: &str,
    all_tests: &[TypeScriptTest],
    profile: TypeScriptBunBridgeProfile,
) -> Option<Finding> {
    if normalized_path(file) != profile.rust_file || !profile.kind.line_text_matches(line_text) {
        return None;
    }

    let facts = collect_profile_bun_array_buffer_facts(all_tests, profile);
    let hint = bun_bridge_hint_for_profile(&facts, profile)?;
    let class = hint.verdict.exposure_class();
    let missing_discriminators = hint
        .verdict
        .missing_discriminators()
        .iter()
            .map(|missing| MissingDiscriminatorFact {
                value: (*missing).to_string(),
                reason: format!(
                    "{} TypeScript preview evidence does not discriminate `{missing}` for Rust boundary `{}`.",
                    profile.kind.display_name(),
                    hint.rust_boundary
                ),
                flow_sink: None,
            })
        .collect::<Vec<_>>();
    let related_tests = related_profile_bun_tests(all_tests, profile);
    let id_path = normalized_path(file)
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect::<String>();
    let bun_owner_id = format!("rust:{}::{}", profile.rust_file, profile.rust_owner);
    let bun_probe_id = fingerprint_probe_id(
        "probe",
        &id_path,
        "typescript_bun_ub_cross_language_preview",
        &bun_owner_id,
        &normalize_expression(profile.rust_boundary),
        1,
    );
    let probe = Probe {
        id: bun_probe_id,
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: Some(SymbolId(bun_owner_id)),
        family: ProbeFamily::Predicate,
        delta: DeltaKind::Control,
        before: None,
        after: Some(line_text.to_string()),
        expression: profile.rust_boundary.to_string(),
        expected_sinks: profile.kind.expected_sinks(),
        required_oracles: profile.kind.required_oracles(),
    };
    let actionability = typescript_bun_cross_language_actionability(&hint);
    let mut evidence = vec![
        format!("owner: {}", profile.rust_owner),
        format!(
            "typescript_bun_ub_rust_seam: file={} line={} owner={} boundary=\"{}\"",
            profile.rust_file, line, profile.rust_owner, profile.rust_boundary
        ),
    ];
    for fact in &facts {
        evidence.push(fact.evidence_line());
    }
    evidence.extend(hint.evidence_lines());
    evidence.extend(typescript_bun_cross_language_actionability_evidence(
        &actionability,
        file,
        line,
        &facts,
        &hint,
        &probe.id.0,
    ));

    let mut missing = Vec::new();
    if !missing_discriminators.is_empty() {
        let missing_values = missing_discriminators
            .iter()
            .map(|missing| missing.value.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        missing.push(format!(
            "{} TypeScript preview is missing cross-language discriminator(s): {missing_values}.",
            profile.kind.display_name()
        ));
    }
    missing.push(actionability.missing_summary());

    let (reach_state, observe_state, discriminate_state) =
        bun_cross_language_stage_states(hint.verdict);
    Some(Finding {
        id: probe.id.0.clone(),
        canonical_gap: None,
        probe,
        class,
        ripr: RiprEvidence {
            reach: StageEvidence::new(
                reach_state,
                Confidence::Low,
                format!(
                    "Configured {} bridge maps Rust owner `{}` to TypeScript integration test `{}`.",
                    hint.profile_kind.display_name(),
                    hint.rust_owner,
                    normalized_path(&hint.ts_test_file)
                ),
            ),
            infect: StageEvidence::new(
                StageState::Unknown,
                Confidence::Low,
                "TypeScript cross-language preview does not model Rust-side infection.",
            ),
            propagate: StageEvidence::new(
                StageState::Unknown,
                Confidence::Low,
                "TypeScript cross-language preview does not prove FFI propagation.",
            ),
            reveal: RevealEvidence {
                observe: StageEvidence::new(
                    observe_state,
                    Confidence::Low,
                    bun_cross_language_observe_summary_for(hint.profile_kind, hint.verdict),
                ),
                discriminate: StageEvidence::new(
                    discriminate_state,
                    Confidence::Low,
                    bun_cross_language_discriminate_summary_for(hint.profile_kind, hint.verdict),
                ),
            },
        },
        confidence: bun_cross_language_confidence(hint.verdict),
        evidence,
        missing,
        flow_sinks: Vec::new(),
        activation: ActivationEvidence {
            observed_values: Vec::new(),
            missing_discriminators,
        },
        stop_reasons: bun_cross_language_stop_reasons(hint.verdict),
        related_tests,
        recommended_next_step: Some(bun_cross_language_recommendation(&hint)),
        language: Some(DomainLanguageId::TypeScript),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: Some(OwnerKind::Function),
        static_limit_kind: None,
    })
}

pub(crate) fn line_text_matches_bun_copy_to_unshared_boundary(line_text: &str) -> bool {
    if line_text.contains(BUN_ARRAY_BUFFER_COPY_TO_UNSHARED_RUST_OWNER) {
        return true;
    }
    let lower = line_text.to_ascii_lowercase();
    (lower.contains("sharedarraybuffer") && lower.contains("resizable"))
        || (lower.contains("shared") && lower.contains("resizable") && lower.contains("copy"))
}

pub(crate) fn typescript_bun_cross_language_actionability(
    hint: &TypeScriptBunBridgeHint,
) -> TypeScriptActionability {
    match hint.verdict {
        TypeScriptBunBridgeVerdict::TsDiscriminated => TypeScriptActionability {
            gap_state: "already_observed",
            category: "bun_ub_ts_discriminated",
            why_not_actionable: format!(
                "configured {} TypeScript preview evidence discriminates the profiled boundary; no repair packet should be emitted",
                hint.profile_kind.display_name()
            ),
            repair_route: "no new test suggested; keep the cross-language witness advisory and verify manually against the Bun change".to_string(),
            missing_fields: Vec::new(),
            evidence_needed:
                "none for a repair packet; retain the advisory TypeScript witness and manual Bun review boundary",
        },
        TypeScriptBunBridgeVerdict::TsMissingResizable
        | TypeScriptBunBridgeVerdict::TsMissingShared
        | TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable => TypeScriptActionability {
            gap_state: "static_limitation",
            category: "cross_language_oracle_visibility_unresolved",
            why_not_actionable: format!(
                "configured {} TypeScript preview evidence is missing external discriminator(s): {}; placement can name the configured TypeScript test file, but RIPR cannot emit a public repair packet without verification, receipt, and edit-surface evidence",
                hint.profile_kind.display_name(),
                hint.verdict.missing_discriminators().join(", ")
            ),
            repair_route: "analysis/cross-language-oracle-visibility".to_string(),
            missing_fields: vec![
                "verify_command",
                "receipt_command",
                "must_not_change",
                "allowed_edit_surface",
            ],
            evidence_needed:
                "the missing TypeScript discriminator in the configured test file plus verify command, receipt command, and edit constraints before repair-packet projection",
        },
        TypeScriptBunBridgeVerdict::TsMissingExternalOracle => TypeScriptActionability {
            gap_state: "static_limitation",
            category: "cross_language_oracle_visibility_unresolved",
            why_not_actionable: format!(
                "configured {} TypeScript preview facts include a partial external observer path, but the callsite or oracle edge is incomplete, so RIPR cannot safely credit the Rust seam or suggest a repair packet",
                hint.profile_kind.display_name()
            ),
            repair_route: "analysis/cross-language-oracle-visibility".to_string(),
            missing_fields: vec![
                "external_oracle_path",
                "verify_command",
                "receipt_command",
                "allowed_edit_surface",
                "raw_evidence_refs",
            ],
            evidence_needed:
                "external callsite, external oracle, binding or FFI route, verify command, receipt command, raw evidence refs, and edit constraints",
        },
        TypeScriptBunBridgeVerdict::TsMentionNotObserver => TypeScriptActionability {
            gap_state: "static_limitation",
            category: "cross_language_oracle_visibility_unresolved",
            why_not_actionable:
                "maxByteLength or byte-token evidence appears without a Blob input and stable-byte observer, so it cannot be credited to the Rust seam"
                    .to_string(),
            repair_route: "analysis/cross-language-oracle-visibility".to_string(),
            missing_fields: vec![
                "external_oracle_path",
                "verify_command",
                "raw_evidence_refs",
            ],
            evidence_needed:
                "Blob input, stable-byte observer, binding or FFI route, verify command, and raw evidence refs",
        },
        TypeScriptBunBridgeVerdict::BridgeUnknown => TypeScriptActionability {
            gap_state: "static_limitation",
            category: "cross_language_oracle_visibility_unresolved",
            why_not_actionable:
                "TypeScript discriminators are present, but the Rust bridge is unknown and must not be reported as no_static_path"
                    .to_string(),
            repair_route: "analysis/cross-language-oracle-visibility".to_string(),
            missing_fields: vec!["bridge_hint", "raw_evidence_refs"],
            evidence_needed: "configured bridge hint or generated bridge fact plus raw evidence refs",
        },
    }
}

pub(crate) fn bun_cross_language_stage_states(
    verdict: TypeScriptBunBridgeVerdict,
) -> (StageState, StageState, StageState) {
    match verdict {
        TypeScriptBunBridgeVerdict::TsDiscriminated => {
            (StageState::Yes, StageState::Yes, StageState::Yes)
        }
        TypeScriptBunBridgeVerdict::TsMissingResizable
        | TypeScriptBunBridgeVerdict::TsMissingShared
        | TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable => {
            (StageState::Yes, StageState::Unknown, StageState::Unknown)
        }
        TypeScriptBunBridgeVerdict::TsMissingExternalOracle => (
            StageState::Unknown,
            StageState::Unknown,
            StageState::Unknown,
        ),
        TypeScriptBunBridgeVerdict::TsMentionNotObserver
        | TypeScriptBunBridgeVerdict::BridgeUnknown => (
            StageState::Unknown,
            StageState::Unknown,
            StageState::Unknown,
        ),
    }
}

pub(crate) fn bun_cross_language_observe_summary_for(
    profile_kind: TypeScriptBunBridgeProfileKind,
    verdict: TypeScriptBunBridgeVerdict,
) -> &'static str {
    match verdict {
        TypeScriptBunBridgeVerdict::TsDiscriminated
        | TypeScriptBunBridgeVerdict::TsMissingResizable
        | TypeScriptBunBridgeVerdict::TsMissingShared
        | TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable => {
            profile_kind.complete_observe_summary()
        }
        TypeScriptBunBridgeVerdict::TsMentionNotObserver => {
            "TypeScript evidence is a token mention, not a Blob stable-byte observer."
        }
        TypeScriptBunBridgeVerdict::TsMissingExternalOracle => {
            "TypeScript evidence has a partial Blob observer path, but the stable external oracle path is incomplete."
        }
        TypeScriptBunBridgeVerdict::BridgeUnknown => {
            "TypeScript evidence has discriminators, but the Rust bridge is unknown."
        }
    }
}

pub(crate) fn bun_cross_language_discriminate_summary_for(
    profile_kind: TypeScriptBunBridgeProfileKind,
    verdict: TypeScriptBunBridgeVerdict,
) -> &'static str {
    match verdict {
        TypeScriptBunBridgeVerdict::TsDiscriminated => profile_kind.complete_discriminate_summary(),
        TypeScriptBunBridgeVerdict::TsMissingResizable => profile_kind.missing_resizable_summary(),
        TypeScriptBunBridgeVerdict::TsMissingShared => {
            "TypeScript evidence is missing the SharedArrayBuffer discriminator for the configured Rust seam."
        }
        TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable => {
            "TypeScript evidence is missing both SharedArrayBuffer and resizable ArrayBuffer discriminators for the configured Rust seam."
        }
        TypeScriptBunBridgeVerdict::TsMentionNotObserver => {
            "TypeScript token mentions are not stable-byte discriminators."
        }
        TypeScriptBunBridgeVerdict::TsMissingExternalOracle => {
            "TypeScript evidence cannot be credited until both the Blob callsite and stable-byte oracle are visible."
        }
        TypeScriptBunBridgeVerdict::BridgeUnknown => {
            "Bridge confidence is unknown, so TypeScript discriminators cannot yet be credited to the Rust seam."
        }
    }
}

pub(crate) fn bun_cross_language_confidence(verdict: TypeScriptBunBridgeVerdict) -> f32 {
    match verdict {
        TypeScriptBunBridgeVerdict::TsDiscriminated => 0.6,
        TypeScriptBunBridgeVerdict::TsMissingResizable
        | TypeScriptBunBridgeVerdict::TsMissingShared
        | TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable => 0.45,
        TypeScriptBunBridgeVerdict::TsMissingExternalOracle => 0.35,
        TypeScriptBunBridgeVerdict::TsMentionNotObserver
        | TypeScriptBunBridgeVerdict::BridgeUnknown => 0.3,
    }
}

pub(crate) fn bun_cross_language_stop_reasons(
    verdict: TypeScriptBunBridgeVerdict,
) -> Vec<StopReason> {
    match verdict {
        TypeScriptBunBridgeVerdict::TsMentionNotObserver
        | TypeScriptBunBridgeVerdict::BridgeUnknown
        | TypeScriptBunBridgeVerdict::TsMissingResizable
        | TypeScriptBunBridgeVerdict::TsMissingShared
        | TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable
        | TypeScriptBunBridgeVerdict::TsMissingExternalOracle => {
            vec![StopReason::StaticProbeUnknown]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn bun_cross_language_recommendation(hint: &TypeScriptBunBridgeHint) -> String {
    let placement_guidance = match hint.verdict {
        TypeScriptBunBridgeVerdict::TsDiscriminated => " no new test suggested;",
        TypeScriptBunBridgeVerdict::TsMissingResizable
        | TypeScriptBunBridgeVerdict::TsMissingShared
        | TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable => {
            " suggest the configured TypeScript observer file only as advisory placement;"
        }
        TypeScriptBunBridgeVerdict::TsMissingExternalOracle
        | TypeScriptBunBridgeVerdict::TsMentionNotObserver
        | TypeScriptBunBridgeVerdict::BridgeUnknown => {
            " route to `analysis/cross-language-oracle-visibility` before suggesting a test target;"
        }
    };
    format!(
        "TypeScript cross-language preview: state `{}` for Rust seam `{}` `{}`; action `{}`;{} suggested_test_file `{}`; authority preview/advisory only.",
        hint.verdict.cross_language_state(),
        hint.rust_owner,
        hint.rust_boundary,
        hint.verdict.expected_action(),
        placement_guidance,
        hint.suggested_test_file()
    )
}

pub(crate) fn typescript_bun_cross_language_actionability_evidence(
    actionability: &TypeScriptActionability,
    file: &Path,
    line: usize,
    facts: &[TypeScriptBunArrayBufferFact],
    hint: &TypeScriptBunBridgeHint,
    source_id: &str,
) -> Vec<String> {
    let raw_refs =
        typescript_bun_cross_language_raw_evidence_refs(file, line, facts, hint, source_id);
    let first_ref = raw_refs.first().cloned().unwrap_or_else(|| {
        typescript_bun_cross_language_raw_evidence_ref(
            "rust_seam",
            &normalized_path(file),
            line,
            "rust_boundary",
            source_id,
            Some(hint.rust_owner),
            hint.rust_boundary,
        )
    });
    let mut evidence = actionability.evidence(first_ref);
    let missing_graph_legs =
        bun_cross_language_missing_graph_legs(hint.verdict, facts, hint.profile_kind);
    if !missing_graph_legs.is_empty() {
        evidence.push(format!(
            "missing_graph_legs: {}",
            missing_graph_legs.join(", ")
        ));
    }
    if let Some(unlock_condition) =
        bun_cross_language_unlock_condition(hint.verdict, facts, hint.profile_kind)
    {
        evidence.push(format!("unlock_condition: {unlock_condition}"));
    }
    evidence.extend(raw_refs.into_iter().skip(1));
    evidence
}

pub(crate) fn typescript_bun_cross_language_raw_evidence_refs(
    file: &Path,
    line: usize,
    facts: &[TypeScriptBunArrayBufferFact],
    hint: &TypeScriptBunBridgeHint,
    source_id: &str,
) -> Vec<String> {
    let mut refs = vec![typescript_bun_cross_language_raw_evidence_ref(
        "rust_seam",
        &normalized_path(file),
        line,
        "rust_boundary",
        source_id,
        Some(hint.rust_owner),
        hint.rust_boundary,
    )];

    if hint.confidence == TypeScriptBunBridgeConfidence::ConfiguredHint {
        refs.push(typescript_bun_cross_language_raw_evidence_ref(
            "binding_edge",
            hint.rust_file,
            line,
            "configured_bridge",
            source_id,
            Some(hint.rust_owner),
            &hint
                .profile_kind
                .configured_bridge_sample(&hint.ts_test_file),
        ));
    }

    for kind in [
        TypeScriptBunArrayBufferFactKind::SharedArrayBuffer,
        TypeScriptBunArrayBufferFactKind::ResizableArrayBuffer,
    ] {
        if let Some(fact) = first_bun_array_buffer_fact(facts, kind) {
            refs.push(typescript_bun_fact_raw_evidence_ref(
                "boundary_discriminator",
                fact,
                source_id,
                Some(hint.rust_owner),
            ));
        }
    }

    if let Some(fact) =
        first_bun_array_buffer_fact(facts, TypeScriptBunArrayBufferFactKind::ViewBackedBlobInput)
    {
        refs.push(typescript_bun_fact_raw_evidence_ref(
            "external_callsite",
            fact,
            source_id,
            Some(hint.rust_owner),
        ));
    } else if let Some(fact) = first_bun_array_buffer_fact(
        facts,
        TypeScriptBunArrayBufferFactKind::MarkdownExternalCallsite,
    ) {
        refs.push(typescript_bun_fact_raw_evidence_ref(
            "external_callsite",
            fact,
            source_id,
            Some(hint.rust_owner),
        ));
    } else if let Some(fact) = first_bun_array_buffer_fact(
        facts,
        TypeScriptBunArrayBufferFactKind::MaxByteLengthMentionOnly,
    ) {
        refs.push(typescript_bun_fact_raw_evidence_ref(
            "external_mention",
            fact,
            source_id,
            Some(hint.rust_owner),
        ));
    }

    if let Some(fact) = first_bun_array_buffer_fact(
        facts,
        TypeScriptBunArrayBufferFactKind::StableByteCopyOracle,
    ) {
        refs.push(typescript_bun_fact_raw_evidence_ref(
            "external_oracle",
            fact,
            source_id,
            Some(hint.rust_owner),
        ));
    } else if let Some(fact) = first_bun_array_buffer_fact(
        facts,
        TypeScriptBunArrayBufferFactKind::MarkdownStrongOracle,
    ) {
        refs.push(typescript_bun_fact_raw_evidence_ref(
            "external_oracle",
            fact,
            source_id,
            Some(hint.rust_owner),
        ));
    } else if let Some(fact) = first_bun_array_buffer_fact(
        facts,
        TypeScriptBunArrayBufferFactKind::ByteOracleMentionOnly,
    ) {
        refs.push(typescript_bun_fact_raw_evidence_ref(
            "external_mention",
            fact,
            source_id,
            Some(hint.rust_owner),
        ));
    }

    refs
}

pub(crate) fn first_bun_array_buffer_fact(
    facts: &[TypeScriptBunArrayBufferFact],
    kind: TypeScriptBunArrayBufferFactKind,
) -> Option<&TypeScriptBunArrayBufferFact> {
    facts.iter().find(|fact| fact.kind == kind)
}

pub(crate) fn typescript_bun_fact_raw_evidence_ref(
    leg: &str,
    fact: &TypeScriptBunArrayBufferFact,
    source_id: &str,
    owner: Option<&str>,
) -> String {
    typescript_bun_cross_language_raw_evidence_ref(
        leg,
        &normalized_path(&fact.file),
        fact.line,
        fact.kind.as_str(),
        source_id,
        owner,
        &fact.text,
    )
}

pub(crate) fn typescript_bun_cross_language_raw_evidence_ref(
    leg: &str,
    file: &str,
    line: usize,
    kind: &str,
    source_id: &str,
    owner: Option<&str>,
    sample: &str,
) -> String {
    let mut parts = vec![
        format!("leg={}", raw_evidence_ref_value(leg)),
        format!("file={}", raw_evidence_ref_value(file)),
        format!("line={line}"),
        format!("kind={}", raw_evidence_ref_value(kind)),
        format!("source_id={}", raw_evidence_ref_value(source_id)),
    ];
    if let Some(owner) = owner {
        parts.push(format!("owner={}", raw_evidence_ref_value(owner)));
    }
    parts.push(format!("sample={}", raw_evidence_ref_value(sample)));
    format!("raw_evidence_ref: {}", parts.join(";"))
}

pub(crate) fn raw_evidence_ref_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\r' | '\n' | ';' => ' ',
            _ => ch,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub(crate) fn bun_cross_language_missing_graph_legs(
    verdict: TypeScriptBunBridgeVerdict,
    facts: &[TypeScriptBunArrayBufferFact],
    profile_kind: TypeScriptBunBridgeProfileKind,
) -> Vec<&'static str> {
    match verdict {
        TypeScriptBunBridgeVerdict::TsDiscriminated => Vec::new(),
        TypeScriptBunBridgeVerdict::TsMissingResizable => {
            vec!["boundary_discriminator:resizable_array_buffer"]
        }
        TypeScriptBunBridgeVerdict::TsMissingShared => {
            vec!["boundary_discriminator:shared_array_buffer"]
        }
        TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable => vec![
            "boundary_discriminator:shared_array_buffer",
            "boundary_discriminator:resizable_array_buffer",
        ],
        TypeScriptBunBridgeVerdict::TsMentionNotObserver => vec![
            "external_callsite:view_backed_blob_input",
            "external_oracle:stable_byte_copy",
        ],
        TypeScriptBunBridgeVerdict::TsMissingExternalOracle => {
            if profile_kind == TypeScriptBunBridgeProfileKind::MarkdownResizableArrayBuffer {
                let mut missing = Vec::new();
                if first_bun_array_buffer_fact(
                    facts,
                    TypeScriptBunArrayBufferFactKind::MarkdownExternalCallsite,
                )
                .is_none()
                {
                    missing.push("external_callsite:bun_markdown_callsite");
                }
                if first_bun_array_buffer_fact(
                    facts,
                    TypeScriptBunArrayBufferFactKind::MarkdownStrongOracle,
                )
                .is_none()
                {
                    missing.push("external_oracle:markdown_strong_oracle");
                }
                if missing.is_empty() {
                    missing.push("external_oracle_path");
                }
                return missing;
            }
            let mut missing = Vec::new();
            if first_bun_array_buffer_fact(
                facts,
                TypeScriptBunArrayBufferFactKind::ViewBackedBlobInput,
            )
            .is_none()
            {
                missing.push("external_callsite:view_backed_blob_input");
            }
            if first_bun_array_buffer_fact(
                facts,
                TypeScriptBunArrayBufferFactKind::StableByteCopyOracle,
            )
            .is_none()
            {
                missing.push("external_oracle:stable_byte_copy");
            }
            if missing.is_empty() {
                missing.push("external_oracle_path");
            }
            missing
        }
        TypeScriptBunBridgeVerdict::BridgeUnknown => vec!["binding_or_ffi_edge"],
    }
}

pub(crate) fn bun_cross_language_unlock_condition(
    verdict: TypeScriptBunBridgeVerdict,
    facts: &[TypeScriptBunArrayBufferFact],
    profile_kind: TypeScriptBunBridgeProfileKind,
) -> Option<String> {
    match verdict {
        TypeScriptBunBridgeVerdict::TsDiscriminated => None,
        TypeScriptBunBridgeVerdict::TsMissingResizable
        | TypeScriptBunBridgeVerdict::TsMissingShared
        | TypeScriptBunBridgeVerdict::TsMissingSharedAndResizable => Some(
            format!(
                "add or inspect the missing external TypeScript discriminator(s) in {} and keep repair-packet projection blocked until verify, receipt, and edit-surface evidence exists",
                profile_kind.ts_test_file()
            ),
        ),
        TypeScriptBunBridgeVerdict::TsMentionNotObserver => Some(
            "connect a Blob-backed external callsite and stable-byte oracle to the Rust seam before crediting token mentions".to_string(),
        ),
        TypeScriptBunBridgeVerdict::TsMissingExternalOracle => {
            if profile_kind == TypeScriptBunBridgeProfileKind::MarkdownResizableArrayBuffer {
                let missing_callsite = first_bun_array_buffer_fact(
                    facts,
                    TypeScriptBunArrayBufferFactKind::MarkdownExternalCallsite,
                )
                .is_none();
                let missing_oracle = first_bun_array_buffer_fact(
                    facts,
                    TypeScriptBunArrayBufferFactKind::MarkdownStrongOracle,
                )
                .is_none();
                let missing_edge = match (missing_callsite, missing_oracle) {
                    (true, false) => "a Bun markdown external callsite",
                    (false, true) => "a strong markdown output oracle",
                    (true, true) => "a Bun markdown external callsite and strong output oracle",
                    (false, false) => "the external oracle path",
                };
                return Some(format!(
                    "Connect the partial Bun markdown evidence to {missing_edge} before crediting the Rust seam or suggesting placement."
                ));
            }
            let missing_callsite = first_bun_array_buffer_fact(
                facts,
                TypeScriptBunArrayBufferFactKind::ViewBackedBlobInput,
            )
            .is_none();
            let missing_oracle = first_bun_array_buffer_fact(
                facts,
                TypeScriptBunArrayBufferFactKind::StableByteCopyOracle,
            )
            .is_none();
            let missing_edge = match (missing_callsite, missing_oracle) {
                (true, false) => "a Blob-backed external callsite",
                (false, true) => "a stable byte oracle",
                (true, true) => "a Blob-backed external callsite and stable byte oracle",
                (false, false) => "the external oracle path",
            };
            Some(format!(
                "Connect the partial Blob observer evidence to {missing_edge} before crediting the Rust seam or suggesting placement."
            ))
        }
        TypeScriptBunBridgeVerdict::BridgeUnknown => Some(
            "name the binding or FFI edge from the Rust seam to the external test before crediting external discriminators".to_string(),
        ),
    }
}
