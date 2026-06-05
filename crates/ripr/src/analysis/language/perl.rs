//! Fixture-only Perl fact packet adapter.
//!
//! This module is test-scoped for the first Perl implementation slice. It
//! consumes canned `ripr-perl-facts-v1` packets without launching `perl-lsp`,
//! a Perl runtime, or an LSP protocol session. Production routing lands only
//! after the fact packet and strict actionability slices are fixture-backed.

use crate::domain::ExposureClass;
use serde::Deserialize;
use std::collections::BTreeSet;

const PERL_FACT_PACKET_SCHEMA: &str = "ripr-perl-facts-v1";
const PERL_LSP_FACT_EXPORTER: &str = "perl-lsp";
const PERL_LSP_FACT_EXPORT_SUBCOMMAND: &str = "ripr-facts";

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct PerlLspFactExportRequest {
    root: String,
    out: String,
    base: Option<String>,
    head: Option<String>,
    requested_fact_classes: Vec<PerlFactClass>,
}

impl PerlLspFactExportRequest {
    fn new(
        root: impl Into<String>,
        out: impl Into<String>,
        requested_fact_classes: impl IntoIterator<Item = PerlFactClass>,
    ) -> Result<Self, String> {
        let root = stable_repo_path_arg(root.into(), "root")?;
        let out = stable_repo_path_arg(out.into(), "out")?;
        let requested_fact_classes = canonical_fact_classes(requested_fact_classes);
        if requested_fact_classes.is_empty() {
            return Err(
                "perl-lsp fact export request requires at least one fact class".to_string(),
            );
        }

        Ok(Self {
            root,
            out,
            base: None,
            head: None,
            requested_fact_classes,
        })
    }

    fn with_diff_range(
        mut self,
        base: impl Into<String>,
        head: impl Into<String>,
    ) -> PerlLspFactExportRequest {
        self.base = Some(base.into());
        self.head = Some(head.into());
        self
    }

    fn render_command(&self) -> PerlLspFactExportCommand {
        let mut argv = vec![
            PERL_LSP_FACT_EXPORT_SUBCOMMAND.to_string(),
            "--schema".to_string(),
            PERL_FACT_PACKET_SCHEMA.to_string(),
            "--root".to_string(),
            self.root.clone(),
        ];
        if let Some(base) = self.base.as_ref() {
            argv.push("--base".to_string());
            argv.push(base.clone());
        }
        if let Some(head) = self.head.as_ref() {
            argv.push("--head".to_string());
            argv.push(head.clone());
        }
        argv.push("--fact-classes".to_string());
        argv.push(fact_classes_arg(&self.requested_fact_classes));
        argv.push("--out".to_string());
        argv.push(self.out.clone());

        PerlLspFactExportCommand {
            program: PERL_LSP_FACT_EXPORTER.to_string(),
            argv,
        }
    }

    fn exporter_unavailable(reason: impl Into<String>) -> PerlLspFactExportUnavailable {
        PerlLspFactExportUnavailable {
            packet_status: PacketStatus::Unavailable,
            limitation_kind: BoundaryKind::PacketIncomplete,
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PerlLspFactExportCommand {
    program: String,
    argv: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PerlLspFactExportUnavailable {
    packet_status: PacketStatus,
    limitation_kind: BoundaryKind,
    reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PerlFactClass {
    Files,
    Owners,
    Changes,
    Tests,
    Oracles,
    Relations,
    DynamicBoundaries,
    VerifyCommands,
    Limitations,
    Provenance,
}

impl PerlFactClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Owners => "owners",
            Self::Changes => "changes",
            Self::Tests => "tests",
            Self::Oracles => "oracles",
            Self::Relations => "relations",
            Self::DynamicBoundaries => "dynamic_boundaries",
            Self::VerifyCommands => "verify_commands",
            Self::Limitations => "limitations",
            Self::Provenance => "provenance",
        }
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

    fn test(&self, test_id: &str) -> Option<&TestFact> {
        self.tests.iter().find(|test| test.test_id == test_id)
    }

    fn provenance(&self, provenance_id: &str) -> Option<&ProvenanceFact> {
        self.provenance
            .iter()
            .find(|provenance| provenance.provenance_id == provenance_id)
    }

    fn files_with_role(&self, role: FileRole) -> Vec<&FileFact> {
        self.files
            .iter()
            .filter(|file| file.role.contains(&role))
            .collect()
    }

    fn tests_for_framework(&self, framework: TestFramework) -> Vec<&TestFact> {
        self.tests
            .iter()
            .filter(|test| test.framework == framework)
            .collect()
    }

    fn oracles_for_kind(&self, kind: OracleKind) -> Vec<&OracleFact> {
        self.oracles
            .iter()
            .filter(|oracle| oracle.kind == kind)
            .collect()
    }

    fn strong_exact_oracles(&self) -> Vec<&OracleFact> {
        self.oracles
            .iter()
            .filter(|oracle| oracle.is_strong_exact())
            .collect()
    }

    fn advisory_oracles(&self) -> Vec<&OracleFact> {
        self.oracles
            .iter()
            .filter(|oracle| !oracle.is_strong_exact())
            .collect()
    }

    fn verify_command_runners(&self) -> BTreeSet<Runner> {
        self.verify_commands
            .iter()
            .map(|command| command.runner)
            .collect()
    }

    fn related_test_evidence_for_change(&self, change_id: &str) -> Vec<PerlRelatedTestEvidence> {
        if self.packet_status != PacketStatus::Complete {
            return Vec::new();
        }
        let Some(change) = self.change(change_id) else {
            return Vec::new();
        };

        self.relations
            .iter()
            .filter(|relation| relation.change_id == change_id)
            .filter(|relation| relation.owner_id == change.owner_id)
            .filter_map(|relation| self.related_test_evidence(relation))
            .collect()
    }

    fn classify_change_from_related_tests(&self, change_id: &str) -> ExposureClass {
        if self.packet_status != PacketStatus::Complete || self.change(change_id).is_none() {
            return ExposureClass::StaticUnknown;
        }

        let related = self.related_test_evidence_for_change(change_id);
        if related.is_empty() {
            return ExposureClass::NoStaticPath;
        }
        if related
            .iter()
            .any(|evidence| evidence.class == ExposureClass::WeaklyExposed)
        {
            return ExposureClass::WeaklyExposed;
        }
        if related
            .iter()
            .any(|evidence| evidence.class == ExposureClass::ReachableUnrevealed)
        {
            return ExposureClass::ReachableUnrevealed;
        }

        ExposureClass::StaticUnknown
    }

    fn related_test_evidence(&self, relation: &RelationFact) -> Option<PerlRelatedTestEvidence> {
        let test = self
            .tests
            .iter()
            .find(|test| test.test_id == relation.test_id)?;
        let test_file = self.file(&test.file_id)?;
        let oracle = relation
            .oracle_id
            .as_deref()
            .and_then(|oracle_id| self.oracle(oracle_id));
        let verify_fact = self.verify_command_for_test(&test.test_id);
        let verify_command = verify_fact.map(|command| command.argv.clone());
        let owner = self.owner(&relation.owner_id);
        let class = self.classify_related_relation(relation, oracle);
        let confidence = combined_confidence(
            [
                owner.map(|owner| owner.confidence),
                Some(relation.confidence),
                Some(test.confidence),
                oracle.map(|oracle| oracle.confidence),
                verify_fact.map(|command| command.confidence),
            ]
            .into_iter()
            .flatten(),
        );
        let mut evidence_refs = BTreeSet::new();
        evidence_refs.extend(relation.provenance_refs.iter().cloned());
        evidence_refs.extend(test.provenance_refs.iter().cloned());
        if let Some(oracle) = oracle {
            evidence_refs.extend(oracle.provenance_refs.iter().cloned());
        }

        Some(PerlRelatedTestEvidence {
            relation_id: relation.relation_id.clone(),
            change_id: relation.change_id.clone(),
            owner_id: relation.owner_id.clone(),
            test_id: test.test_id.clone(),
            test_path: test_file.path.clone(),
            test_name: test.name.clone(),
            test_framework: test.framework,
            oracle_id: oracle.map(|oracle| oracle.oracle_id.clone()),
            relation_kind: relation.relation_kind,
            reachability_hint: relation.reachability_hint,
            oracle_shape: oracle.map(|oracle| oracle.kind.assertion_shape().to_string()),
            oracle_strength: oracle.map(|oracle| oracle.strength),
            class,
            confidence,
            verify_command,
            verify_command_id: verify_fact.map(|command| command.command_id.clone()),
            evidence_refs: evidence_refs.into_iter().collect(),
        })
    }

    fn classify_related_relation(
        &self,
        relation: &RelationFact,
        oracle: Option<&OracleFact>,
    ) -> ExposureClass {
        match relation.reachability_hint {
            ReachabilityHint::StaticUnknown => return ExposureClass::StaticUnknown,
            ReachabilityHint::WeaklyReachable => return ExposureClass::ReachableUnrevealed,
            ReachabilityHint::Reachable => {}
        }

        let Some(oracle) = oracle else {
            return ExposureClass::ReachableUnrevealed;
        };
        if oracle.test_id == relation.test_id
            && oracle.target_owner_id.as_deref() == Some(relation.owner_id.as_str())
            && oracle.is_strong_exact()
        {
            ExposureClass::WeaklyExposed
        } else {
            ExposureClass::ReachableUnrevealed
        }
    }

    fn strict_actionability_for_change(
        &self,
        change_id: &str,
        context: &PerlActionabilityContext,
    ) -> Result<PerlStrictActionability, PerlActionabilityBlocker> {
        if self.packet_status != PacketStatus::Complete {
            return Err(PerlActionabilityBlocker::PacketNotComplete);
        }

        let change = self
            .change(change_id)
            .ok_or(PerlActionabilityBlocker::MissingChange)?;
        if self.has_blocking_dynamic_boundary(change, None) {
            return Err(PerlActionabilityBlocker::DynamicBoundary);
        }

        let owner = self
            .owner(&change.owner_id)
            .ok_or(PerlActionabilityBlocker::MissingCanonicalGapId)?;
        if !owner.confidence.is_strict_actionable() {
            return Err(PerlActionabilityBlocker::LowConfidence);
        }
        let repair_kind = change
            .behavior_hint
            .repair_kind()
            .ok_or(PerlActionabilityBlocker::UnsupportedBehavior)?;

        let related = self.related_test_evidence_for_change(change_id);
        let evidence = related
            .iter()
            .find(|evidence| evidence.class == ExposureClass::WeaklyExposed)
            .ok_or(PerlActionabilityBlocker::MissingStrongRelatedEvidence)?;
        if !evidence.confidence.is_strict_actionable() {
            return Err(PerlActionabilityBlocker::LowConfidence);
        }
        if !evidence.test_framework.supports_strict_actionability() {
            return Err(PerlActionabilityBlocker::UnsupportedTestFramework);
        }
        let expected_oracle_shape = change.behavior_hint.default_assertion_shape();
        if evidence.oracle_shape.as_deref() != Some(expected_oracle_shape) {
            return Err(PerlActionabilityBlocker::OracleShapeMismatch);
        }
        let gap = self
            .canonical_gap_identity_for_change_with_assertion_shape(
                change_id,
                expected_oracle_shape,
            )
            .ok_or(PerlActionabilityBlocker::MissingCanonicalGapId)?;
        if self.has_blocking_dynamic_boundary(change, Some(evidence))
            || self.has_blocking_limitation(change, evidence)
        {
            return Err(PerlActionabilityBlocker::DynamicBoundary);
        }

        let verify_command = evidence
            .verify_command
            .clone()
            .filter(|command| !command.is_empty())
            .ok_or(PerlActionabilityBlocker::MissingVerifyCommand)?;
        if !is_verify_command(&verify_command) {
            return Err(PerlActionabilityBlocker::MissingVerifyCommand);
        }
        let receipt_command = context
            .receipt_command
            .clone()
            .filter(|command| !command.is_empty())
            .ok_or(PerlActionabilityBlocker::MissingReceiptCommand)?;
        if !is_receipt_command(&receipt_command) {
            return Err(PerlActionabilityBlocker::InvalidReceiptCommand);
        }
        let source_file = self
            .file(&change.file_id)
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        let test = self
            .test(&evidence.test_id)
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        let test_file = self
            .file(&test.file_id)
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        if test_file.path != evidence.test_path || !test_file.role.contains(&FileRole::Test) {
            return Err(PerlActionabilityBlocker::MissingAllowedEditBoundary);
        }
        if !is_safe_repo_relative_path(&source_file.path)
            || !is_safe_repo_relative_path(&test_file.path)
        {
            return Err(PerlActionabilityBlocker::UnsafeEditBoundary);
        }
        if context
            .allowed_edit_boundaries
            .iter()
            .chain(context.forbidden_edit_boundaries.iter())
            .any(|path| !is_safe_repo_relative_path(path))
        {
            return Err(PerlActionabilityBlocker::UnsafeEditBoundary);
        }
        if !context
            .allowed_edit_boundaries
            .iter()
            .any(|path| path == &evidence.test_path)
        {
            return Err(PerlActionabilityBlocker::MissingAllowedEditBoundary);
        }
        if context
            .allowed_edit_boundaries
            .iter()
            .any(|path| path == &source_file.path)
        {
            return Err(PerlActionabilityBlocker::AllowedProductionEditBoundary);
        }
        if context
            .allowed_edit_boundaries
            .iter()
            .any(|path| path != &evidence.test_path)
        {
            return Err(PerlActionabilityBlocker::UnexpectedAllowedEditBoundary);
        }
        if !context
            .forbidden_edit_boundaries
            .iter()
            .any(|path| path == &source_file.path)
        {
            return Err(PerlActionabilityBlocker::MissingForbiddenEditBoundary);
        }
        if context.stop_if.is_empty() {
            return Err(PerlActionabilityBlocker::MissingStopIf);
        }
        if !has_required_must_not_change(&context.must_not_change) {
            return Err(PerlActionabilityBlocker::MissingMustNotChange);
        }

        let raw_evidence_refs = self.raw_actionability_refs(change, evidence)?;
        if raw_evidence_refs.is_empty() {
            return Err(PerlActionabilityBlocker::MissingEvidenceRefs);
        }

        Ok(PerlStrictActionability {
            packet_id: format!("perl-repair:{}", gap.id),
            canonical_gap_id: gap.id,
            gap_state: PerlGapState::Actionable,
            changed_owner_id: gap.owner_id,
            evidence_class: evidence.class.clone(),
            missing_discriminator: gap.missing_discriminator,
            repair_kind: repair_kind.to_string(),
            target_test_shape: format!(
                "{} {}",
                evidence.test_framework.as_str(),
                expected_oracle_shape
            ),
            suggested_test_location: format!("{}::{}", evidence.test_path, evidence.test_name),
            related_test_id: evidence.test_id.clone(),
            verify_command,
            receipt_command,
            confidence: evidence.confidence,
            raw_evidence_refs,
            allowed_edit_boundaries: context.allowed_edit_boundaries.clone(),
            forbidden_edit_boundaries: context.forbidden_edit_boundaries.clone(),
            stop_if: context.stop_if.clone(),
            must_not_change: context.must_not_change.clone(),
        })
    }

    fn repair_card_for_change(
        &self,
        change_id: &str,
        context: &PerlActionabilityContext,
    ) -> Result<PerlRepairCard, PerlActionabilityBlocker> {
        self.strict_actionability_for_change(change_id, context)
            .map(|actionability| actionability.repair_card())
    }

    fn agent_packet_for_change(
        &self,
        change_id: &str,
        context: &PerlActionabilityContext,
    ) -> Result<PerlInternalAgentPacket, PerlActionabilityBlocker> {
        self.strict_actionability_for_change(change_id, context)
            .map(|actionability| actionability.agent_packet())
    }

    fn raw_actionability_refs(
        &self,
        change: &ChangeFact,
        evidence: &PerlRelatedTestEvidence,
    ) -> Result<Vec<PerlRawEvidenceRef>, PerlActionabilityBlocker> {
        let mut refs = Vec::new();
        let mut provenance_ids = BTreeSet::new();
        let source_file = self
            .file(&change.file_id)
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        let owner = self
            .owner(&change.owner_id)
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        let owner_file = self
            .file(&owner.file_id)
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        let relation = self
            .relation(&evidence.relation_id)
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        let test = self
            .test(&evidence.test_id)
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        let test_file = self
            .file(&test.file_id)
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        let oracle = evidence
            .oracle_id
            .as_deref()
            .and_then(|oracle_id| self.oracle(oracle_id))
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;
        let verify = evidence
            .verify_command_id
            .as_deref()
            .and_then(|command_id| {
                self.verify_commands
                    .iter()
                    .find(|command| command.command_id == command_id)
            })
            .ok_or(PerlActionabilityBlocker::MissingEvidenceRefs)?;

        push_actionability_ref(
            &mut refs,
            &mut provenance_ids,
            "perl_change",
            &change.change_id,
            &source_file.path,
            &change.provenance_refs,
        )?;
        push_actionability_ref(
            &mut refs,
            &mut provenance_ids,
            "perl_source_file",
            &source_file.file_id,
            &source_file.path,
            &source_file.provenance_refs,
        )?;
        push_actionability_ref(
            &mut refs,
            &mut provenance_ids,
            "perl_owner",
            &owner.owner_id,
            &owner_file.path,
            &owner.provenance_refs,
        )?;
        push_actionability_ref(
            &mut refs,
            &mut provenance_ids,
            "perl_owner_file",
            &owner_file.file_id,
            &owner_file.path,
            &owner_file.provenance_refs,
        )?;
        push_actionability_ref(
            &mut refs,
            &mut provenance_ids,
            "perl_relation",
            &relation.relation_id,
            &test_file.path,
            &relation.provenance_refs,
        )?;
        push_actionability_ref(
            &mut refs,
            &mut provenance_ids,
            "perl_test",
            &test.test_id,
            &test_file.path,
            &test.provenance_refs,
        )?;
        push_actionability_ref(
            &mut refs,
            &mut provenance_ids,
            "perl_test_file",
            &test_file.file_id,
            &test_file.path,
            &test_file.provenance_refs,
        )?;
        push_actionability_ref(
            &mut refs,
            &mut provenance_ids,
            "perl_oracle",
            &oracle.oracle_id,
            &test_file.path,
            &oracle.provenance_refs,
        )?;
        push_actionability_ref(
            &mut refs,
            &mut provenance_ids,
            "perl_verify_command",
            &verify.command_id,
            &test_file.path,
            &verify.provenance_refs,
        )?;

        for provenance_id in provenance_ids {
            let provenance = self
                .provenance(&provenance_id)
                .ok_or(PerlActionabilityBlocker::MissingProvenanceRefs)?;
            let path = provenance
                .file_id
                .as_deref()
                .and_then(|file_id| self.file(file_id))
                .map(|file| file.path.clone())
                .unwrap_or_else(|| ".".to_string());
            refs.push(PerlRawEvidenceRef {
                kind: "perl_provenance".to_string(),
                source_id: provenance.provenance_id.clone(),
                path,
            });
        }

        Ok(refs)
    }

    fn has_blocking_dynamic_boundary(
        &self,
        change: &ChangeFact,
        evidence: Option<&PerlRelatedTestEvidence>,
    ) -> bool {
        let test_file_id = evidence.and_then(|evidence| {
            self.test(&evidence.test_id)
                .map(|test| test.file_id.as_str())
        });
        self.dynamic_boundaries.iter().any(|boundary| {
            boundary.owner_id.as_deref() == Some(change.owner_id.as_str())
                || boundary.file_id == change.file_id
                || test_file_id == Some(boundary.file_id.as_str())
        })
    }

    fn has_blocking_limitation(
        &self,
        change: &ChangeFact,
        evidence: &PerlRelatedTestEvidence,
    ) -> bool {
        let relevant_refs = self.actionability_evidence_ids(change, evidence);
        self.limitations.iter().any(|limitation| {
            limitation.kind.blocks_strict_actionability()
                && (limitation.evidence_refs.is_empty()
                    || limitation
                        .evidence_refs
                        .iter()
                        .any(|evidence_ref| relevant_refs.contains(evidence_ref)))
        })
    }

    fn actionability_evidence_ids(
        &self,
        change: &ChangeFact,
        evidence: &PerlRelatedTestEvidence,
    ) -> BTreeSet<String> {
        let mut ids = BTreeSet::from([
            change.change_id.clone(),
            change.file_id.clone(),
            change.owner_id.clone(),
            evidence.relation_id.clone(),
            evidence.test_id.clone(),
        ]);
        if let Some(test) = self.test(&evidence.test_id) {
            ids.insert(test.file_id.clone());
        }
        if let Some(oracle_id) = evidence.oracle_id.as_ref() {
            ids.insert(oracle_id.clone());
        }
        if let Some(verify_command_id) = evidence.verify_command_id.as_ref() {
            ids.insert(verify_command_id.clone());
        }
        ids
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
        let change = self.change(change_id)?;
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
        self.canonical_gap_identity_for_change_with_assertion_shape(change_id, &assertion_shape)
    }

    fn canonical_gap_identity_for_change_with_assertion_shape(
        &self,
        change_id: &str,
        assertion_shape: &str,
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
        let missing_discriminator = change
            .behavior_hint
            .default_missing_discriminator()
            .to_string();
        let id = canonical_perl_gap_id([
            owner.id.as_str(),
            behavior_kind.as_str(),
            missing_discriminator.as_str(),
            assertion_shape,
        ]);

        Some(CanonicalPerlGapIdentity {
            id,
            owner_id: owner.id,
            behavior_kind,
            missing_discriminator,
            assertion_shape: assertion_shape.to_string(),
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PerlActionabilityContext {
    receipt_command: Option<Vec<String>>,
    allowed_edit_boundaries: Vec<String>,
    forbidden_edit_boundaries: Vec<String>,
    stop_if: Vec<String>,
    must_not_change: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PerlStrictActionability {
    packet_id: String,
    canonical_gap_id: String,
    gap_state: PerlGapState,
    changed_owner_id: String,
    evidence_class: ExposureClass,
    missing_discriminator: String,
    repair_kind: String,
    target_test_shape: String,
    suggested_test_location: String,
    related_test_id: String,
    verify_command: Vec<String>,
    receipt_command: Vec<String>,
    confidence: Confidence,
    raw_evidence_refs: Vec<PerlRawEvidenceRef>,
    allowed_edit_boundaries: Vec<String>,
    forbidden_edit_boundaries: Vec<String>,
    stop_if: Vec<String>,
    must_not_change: Vec<String>,
}

impl PerlStrictActionability {
    fn repair_card(&self) -> PerlRepairCard {
        PerlRepairCard {
            card_version: "perl_repair_card.v1".to_string(),
            source: "perl_adapter_strict_actionability".to_string(),
            language: "perl".to_string(),
            language_status: "preview".to_string(),
            authority_boundary: "preview_advisory_only".to_string(),
            projection_scope: "internal_adapter_only".to_string(),
            public_repair_packet: false,
            public_projection_ready: false,
            packet_id: self.packet_id.clone(),
            canonical_gap_id: self.canonical_gap_id.clone(),
            gap_state: self.gap_state.as_str().to_string(),
            changed_owner: self.changed_owner_id.clone(),
            evidence_class: self.evidence_class.as_str().to_string(),
            repair_kind: self.repair_kind.clone(),
            current_test_evidence: format!(
                "{} currently weakly exposes {} through {}",
                self.related_test_id, self.changed_owner_id, self.target_test_shape
            ),
            missing_discriminator: self.missing_discriminator.clone(),
            target_test_shape: self.target_test_shape.clone(),
            suggested_test_location: self.suggested_test_location.clone(),
            suggested_assertion: perl_suggested_assertion(
                &self.repair_kind,
                &self.missing_discriminator,
            ),
            verify_command: command_string(&self.verify_command),
            receipt_command: command_string(&self.receipt_command),
            confidence: self.confidence.as_str().to_string(),
            raw_evidence_refs: self.raw_evidence_refs.clone(),
            allowed_edit_boundaries: self.allowed_edit_boundaries.clone(),
            forbidden_edit_boundaries: self.forbidden_edit_boundaries.clone(),
            stop_if: self.stop_if.clone(),
            must_not_change: self.must_not_change.clone(),
        }
    }

    fn agent_packet(&self) -> PerlInternalAgentPacket {
        PerlInternalAgentPacket {
            packet_version: "perl_internal_agent_packet.v1".to_string(),
            packet_id: self.packet_id.clone(),
            canonical_gap_id: self.canonical_gap_id.clone(),
            language: "perl".to_string(),
            language_status: "preview".to_string(),
            authority_boundary: "preview_advisory_only".to_string(),
            projection_scope: "internal_adapter_only".to_string(),
            gap_state: self.gap_state.as_str().to_string(),
            evidence_class: self.evidence_class.as_str().to_string(),
            repair_packet_ready: true,
            public_repair_packet: false,
            public_projection_ready: false,
            repair_route: self.repair_kind.clone(),
            changed_owner: self.changed_owner_id.clone(),
            missing_discriminator: self.missing_discriminator.clone(),
            target_test_shape: self.target_test_shape.clone(),
            suggested_test_location: self.suggested_test_location.clone(),
            verify_command: command_string(&self.verify_command),
            receipt_command: command_string(&self.receipt_command),
            verify_command_argv: self.verify_command.clone(),
            receipt_command_argv: self.receipt_command.clone(),
            confidence: self.confidence.as_str().to_string(),
            raw_evidence_refs: self.raw_evidence_refs.clone(),
            allowed_edit_surface: self.allowed_edit_boundaries.clone(),
            forbidden_files: self.forbidden_edit_boundaries.clone(),
            stop_if: self.stop_if.clone(),
            must_not_change: self.must_not_change.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PerlRepairCard {
    card_version: String,
    source: String,
    language: String,
    language_status: String,
    authority_boundary: String,
    projection_scope: String,
    public_repair_packet: bool,
    public_projection_ready: bool,
    packet_id: String,
    canonical_gap_id: String,
    gap_state: String,
    changed_owner: String,
    evidence_class: String,
    repair_kind: String,
    current_test_evidence: String,
    missing_discriminator: String,
    target_test_shape: String,
    suggested_test_location: String,
    suggested_assertion: String,
    verify_command: String,
    receipt_command: String,
    confidence: String,
    raw_evidence_refs: Vec<PerlRawEvidenceRef>,
    allowed_edit_boundaries: Vec<String>,
    forbidden_edit_boundaries: Vec<String>,
    stop_if: Vec<String>,
    must_not_change: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PerlInternalAgentPacket {
    packet_version: String,
    packet_id: String,
    canonical_gap_id: String,
    language: String,
    language_status: String,
    authority_boundary: String,
    projection_scope: String,
    gap_state: String,
    evidence_class: String,
    repair_packet_ready: bool,
    public_repair_packet: bool,
    public_projection_ready: bool,
    repair_route: String,
    changed_owner: String,
    missing_discriminator: String,
    target_test_shape: String,
    suggested_test_location: String,
    verify_command: String,
    receipt_command: String,
    verify_command_argv: Vec<String>,
    receipt_command_argv: Vec<String>,
    confidence: String,
    raw_evidence_refs: Vec<PerlRawEvidenceRef>,
    allowed_edit_surface: Vec<String>,
    forbidden_files: Vec<String>,
    stop_if: Vec<String>,
    must_not_change: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PerlGapState {
    Actionable,
}

impl PerlGapState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Actionable => "actionable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PerlRawEvidenceRef {
    kind: String,
    source_id: String,
    path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PerlActionabilityBlocker {
    PacketNotComplete,
    MissingChange,
    MissingCanonicalGapId,
    DynamicBoundary,
    UnsupportedBehavior,
    MissingStrongRelatedEvidence,
    OracleShapeMismatch,
    UnsupportedTestFramework,
    LowConfidence,
    MissingVerifyCommand,
    MissingReceiptCommand,
    InvalidReceiptCommand,
    MissingAllowedEditBoundary,
    AllowedProductionEditBoundary,
    UnexpectedAllowedEditBoundary,
    UnsafeEditBoundary,
    MissingForbiddenEditBoundary,
    MissingStopIf,
    MissingMustNotChange,
    MissingEvidenceRefs,
    MissingProvenanceRefs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PerlRelatedTestEvidence {
    relation_id: String,
    change_id: String,
    owner_id: String,
    test_id: String,
    test_path: String,
    test_name: String,
    test_framework: TestFramework,
    oracle_id: Option<String>,
    relation_kind: RelationKind,
    reachability_hint: ReachabilityHint,
    oracle_shape: Option<String>,
    oracle_strength: Option<OracleStrength>,
    class: ExposureClass,
    confidence: Confidence,
    verify_command: Option<Vec<String>>,
    verify_command_id: Option<String>,
    evidence_refs: Vec<String>,
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

    fn repair_kind(self) -> Option<&'static str> {
        match self {
            Self::PredicateBoundary => Some("add_predicate_boundary_assertion"),
            Self::ReturnValue => Some("add_exact_return_assertion"),
            Self::ExceptionPath => Some("add_exception_observer"),
            Self::HashOrObjectField => Some("add_hash_or_object_field_assertion"),
            Self::OutputObserver => Some("add_output_observer"),
            Self::WarnObserver => Some("add_warn_observer"),
            Self::LogObserver => Some("add_log_observer"),
            Self::CallEffect | Self::Unknown => None,
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

impl TestFramework {
    fn as_str(self) -> &'static str {
        match self {
            Self::TestMore => "Test::More",
            Self::Test2V0 => "Test2::V0",
            Self::Test2Suite => "Test2::Suite",
            Self::TestException => "Test::Exception",
            Self::TestFatal => "Test::Fatal",
            Self::Unknown => "unknown",
        }
    }

    fn supports_strict_actionability(self) -> bool {
        matches!(
            self,
            Self::TestMore
                | Self::Test2V0
                | Self::Test2Suite
                | Self::TestException
                | Self::TestFatal
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

    fn supports_strong_exact(self) -> bool {
        matches!(
            self,
            Self::ExactReturnAssertion
                | Self::PredicateBoundaryAssertion
                | Self::ExceptionObserver
                | Self::HashOrObjectFieldAssertion
                | Self::OutputObserver
                | Self::WarnObserver
                | Self::LogObserver
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum OracleStrength {
    StrongExact,
    WeakSmoke,
    WeakBroad,
    MentionOnly,
    Unknown,
}

impl OracleFact {
    fn is_strong_exact(&self) -> bool {
        self.strength == OracleStrength::StrongExact && self.kind.supports_strong_exact()
    }
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

impl BoundaryKind {
    fn blocks_strict_actionability(self) -> bool {
        matches!(
            self,
            Self::DynamicDispatch
                | Self::ModuleResolutionUnknown
                | Self::GeneratedSymbol
                | Self::RoleComposition
                | Self::MonkeypatchOrSymbolPatch
                | Self::EvalOrStringCode
                | Self::SymbolTableMutation
                | Self::FrameworkIndirection
                | Self::UnknownHelper
                | Self::UnsupportedSyntax
                | Self::MissingTestRunner
                | Self::MissingDiffOwner
                | Self::PacketIncomplete
                | Self::Unknown
        )
    }
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Runner {
    Prove,
    Yath,
    Carton,
    Dzil,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

impl Confidence {
    fn is_strict_actionable(self) -> bool {
        matches!(self, Self::High | Self::Medium)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Unknown => "unknown",
        }
    }
}

fn combined_confidence(confidences: impl IntoIterator<Item = Confidence>) -> Confidence {
    let mut saw_medium = false;
    for confidence in confidences {
        match confidence {
            Confidence::High => {}
            Confidence::Medium => saw_medium = true,
            Confidence::Low | Confidence::Unknown => return Confidence::Low,
        }
    }
    if saw_medium {
        Confidence::Medium
    } else {
        Confidence::High
    }
}

fn is_verify_command(command: &[String]) -> bool {
    if command.iter().any(|arg| !is_safe_command_arg(arg)) {
        return false;
    }

    match command {
        [program, test_paths @ ..] if program == "prove" => {
            !test_paths.is_empty() && test_paths.iter().all(|path| is_safe_test_path(path))
        }
        [program, subcommand, test_paths @ ..] if program == "yath" && subcommand == "test" => {
            !test_paths.is_empty() && test_paths.iter().all(|path| is_safe_test_path(path))
        }
        [program, subcommand, runner, test_paths @ ..]
            if program == "carton" && subcommand == "exec" && runner == "prove" =>
        {
            !test_paths.is_empty() && test_paths.iter().all(|path| is_safe_test_path(path))
        }
        [program, subcommand, test_flag, test_path]
            if program == "dzil" && subcommand == "test" && test_flag == "--test" =>
        {
            is_safe_test_path(test_path)
        }
        _ => false,
    }
}

fn is_receipt_command(command: &[String]) -> bool {
    if command.iter().any(|arg| !is_safe_command_arg(arg)) {
        return false;
    }

    if command.first().is_some_and(|program| program == "ripr")
        && command.get(1).is_some_and(|arg| arg == "agent")
        && command.get(2).is_some_and(|arg| arg == "receipt")
    {
        return has_required_agent_receipt_args(&command[3..]);
    }

    if command.first().is_some_and(|program| program == "cargo")
        && command.get(1).is_some_and(|arg| arg == "run")
        && command.get(2).is_some_and(|arg| arg == "-p")
        && command.get(3).is_some_and(|arg| arg == "ripr")
        && command.get(4).is_some_and(|arg| arg == "--")
        && command.get(5).is_some_and(|arg| arg == "agent")
        && command.get(6).is_some_and(|arg| arg == "receipt")
    {
        return has_required_agent_receipt_args(&command[7..]);
    }

    false
}

fn has_required_agent_receipt_args(args: &[String]) -> bool {
    let mut has_json = false;
    let mut has_verify_json = false;
    let mut has_seam_id = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                has_json = true;
                index += 1;
            }
            "--root" | "--verify-json" | "--seam-id" | "--test" | "--command" | "--out" => {
                let Some(value) = args.get(index + 1) else {
                    return false;
                };
                if value.trim().is_empty() || value.starts_with("--") {
                    return false;
                }
                match args[index].as_str() {
                    "--root" if !is_safe_receipt_root(value) => return false,
                    "--root" => {}
                    "--verify-json" if !is_safe_repo_relative_path(value) => return false,
                    "--verify-json" => {
                        has_verify_json = true;
                    }
                    "--test" | "--out" if !is_safe_repo_relative_path(value) => return false,
                    "--test" | "--out" => {}
                    "--seam-id" => has_seam_id = true,
                    "--command" => {}
                    _ => {}
                }
                index += 2;
            }
            _ => return false,
        }
    }

    has_json && has_verify_json && has_seam_id
}

fn is_safe_command_arg(arg: &str) -> bool {
    !arg.is_empty()
        && !arg.chars().any(char::is_control)
        && ![
            ';', '|', '&', '>', '<', '`', '$', '(', ')', '{', '}', '*', '?',
        ]
        .iter()
        .any(|metachar| arg.contains(*metachar))
}

fn is_safe_test_path(path: &str) -> bool {
    is_safe_repo_relative_path(path) && path.starts_with("t/") && path.ends_with(".t")
}

fn is_safe_receipt_root(path: &str) -> bool {
    path == "." || is_safe_repo_relative_path(path)
}

fn is_safe_repo_relative_path(path: &str) -> bool {
    !(path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains(':')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == ".."))
}

fn has_required_must_not_change(must_not_change: &[String]) -> bool {
    let mentions_production_code = must_not_change
        .iter()
        .any(|rule| rule.contains("Perl production code"));
    let mentions_suppression_or_intent = must_not_change
        .iter()
        .any(|rule| rule.contains("suppressions") || rule.contains("intent ledger"));
    mentions_production_code && mentions_suppression_or_intent
}

fn command_string(command: &[String]) -> String {
    command.join(" ")
}

fn perl_suggested_assertion(repair_kind: &str, missing_discriminator: &str) -> String {
    match repair_kind {
        "add_predicate_boundary_assertion" => {
            format!("add a boundary assertion for `{missing_discriminator}`")
        }
        "add_exact_return_assertion" => {
            format!("assert the exact returned `{missing_discriminator}` value")
        }
        "add_exception_observer" => {
            format!("assert the observed `{missing_discriminator}` exception")
        }
        "add_hash_or_object_field_assertion" => {
            format!("assert the changed `{missing_discriminator}` field")
        }
        "add_output_observer" => {
            format!("assert the emitted `{missing_discriminator}` output")
        }
        "add_warn_observer" => {
            format!("assert the emitted `{missing_discriminator}` warning")
        }
        "add_log_observer" => {
            format!("assert the emitted `{missing_discriminator}` log")
        }
        _ => format!("add a discriminating assertion for `{missing_discriminator}`"),
    }
}

fn push_actionability_ref(
    refs: &mut Vec<PerlRawEvidenceRef>,
    provenance_ids: &mut BTreeSet<String>,
    kind: &str,
    source_id: &str,
    path: &str,
    fact_provenance_refs: &[String],
) -> Result<(), PerlActionabilityBlocker> {
    if fact_provenance_refs.is_empty() {
        return Err(PerlActionabilityBlocker::MissingProvenanceRefs);
    }
    refs.push(PerlRawEvidenceRef {
        kind: kind.to_string(),
        source_id: source_id.to_string(),
        path: path.to_string(),
    });
    provenance_ids.extend(fact_provenance_refs.iter().cloned());
    Ok(())
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

fn canonical_fact_classes(
    requested_fact_classes: impl IntoIterator<Item = PerlFactClass>,
) -> Vec<PerlFactClass> {
    requested_fact_classes
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn fact_classes_arg(fact_classes: &[PerlFactClass]) -> String {
    fact_classes
        .iter()
        .map(|fact_class| fact_class.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

fn stable_repo_path_arg(path: String, field: &str) -> Result<String, String> {
    if path.is_empty() {
        return Err(format!(
            "perl-lsp fact export `{field}` path must not be empty"
        ));
    }
    if path.contains('\\')
        || path.starts_with('/')
        || path.contains(':')
        || path.split('/').any(|component| component == "..")
    {
        return Err(format!(
            "perl-lsp fact export `{field}` path must be repo-relative and use `/` separators"
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
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
            "../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-facts-v1.json"
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

        let value: serde_json::Value =
            serde_json::from_str(fixture).map_err(|err| err.to_string())?;
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
            "../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
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

        let value: serde_json::Value =
            serde_json::from_str(fixture).map_err(|err| err.to_string())?;
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
            "../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
        );
        let packet = PerlAdapter.consume_fact_packet(fixture)?;

        let return_related =
            packet.related_test_evidence_for_change("change:lib/My/App.pm:8:return");
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
            static_unknown_packet
                .classify_change_from_related_tests("change:lib/My/App.pm:8:return"),
            ExposureClass::StaticUnknown
        );

        let value: serde_json::Value =
            serde_json::from_str(fixture).map_err(|err| err.to_string())?;
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
            "../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
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
    fn perl_strict_actionability_uses_selected_strict_evidence_for_gap_identity()
    -> Result<(), String> {
        let fixture = include_str!(
            "../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
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
            "../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
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
    fn perl_repair_card_and_agent_packet_fail_closed_without_strict_actionability()
    -> Result<(), String> {
        let fixture = include_str!(
            "../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
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
            "../../../../../fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json"
        );
        let packet = PerlAdapter.consume_fact_packet(fixture)?;
        let context = complete_perl_actionability_context();

        let mut missing_receipt = context.clone();
        missing_receipt.receipt_command = None;
        assert_eq!(
            packet
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &missing_receipt),
            Err(PerlActionabilityBlocker::MissingReceiptCommand)
        );

        let mut invalid_receipt = context.clone();
        invalid_receipt.receipt_command =
            Some(vec!["target/ripr/workflow/agent-verify.json".to_string()]);
        assert_eq!(
            packet
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &invalid_receipt),
            Err(PerlActionabilityBlocker::InvalidReceiptCommand)
        );

        let mut non_receipt_command = context.clone();
        non_receipt_command.receipt_command = Some(vec!["cargo".to_string(), "test".to_string()]);
        assert_eq!(
            packet.strict_actionability_for_change(
                "change:lib/My/App.pm:8:return",
                &non_receipt_command
            ),
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
            packet
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &missing_allowed),
            Err(PerlActionabilityBlocker::MissingAllowedEditBoundary)
        );

        let mut production_allowed = context.clone();
        production_allowed
            .allowed_edit_boundaries
            .push("lib/My/App.pm".to_string());
        assert_eq!(
            packet.strict_actionability_for_change(
                "change:lib/My/App.pm:8:return",
                &production_allowed
            ),
            Err(PerlActionabilityBlocker::AllowedProductionEditBoundary)
        );

        let mut unsafe_allowed = context.clone();
        unsafe_allowed
            .allowed_edit_boundaries
            .push("../outside.pm".to_string());
        assert_eq!(
            packet
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &unsafe_allowed),
            Err(PerlActionabilityBlocker::UnsafeEditBoundary)
        );

        let mut unrelated_allowed = context.clone();
        unrelated_allowed
            .allowed_edit_boundaries
            .push("t/unrelated.t".to_string());
        assert_eq!(
            packet.strict_actionability_for_change(
                "change:lib/My/App.pm:8:return",
                &unrelated_allowed
            ),
            Err(PerlActionabilityBlocker::UnexpectedAllowedEditBoundary)
        );

        let mut missing_forbidden = context.clone();
        missing_forbidden.forbidden_edit_boundaries.clear();
        assert_eq!(
            packet.strict_actionability_for_change(
                "change:lib/My/App.pm:8:return",
                &missing_forbidden
            ),
            Err(PerlActionabilityBlocker::MissingForbiddenEditBoundary)
        );

        let mut wrong_forbidden = context.clone();
        wrong_forbidden.forbidden_edit_boundaries = vec!["badges/ripr-plus.json".to_string()];
        assert_eq!(
            packet
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &wrong_forbidden),
            Err(PerlActionabilityBlocker::MissingForbiddenEditBoundary)
        );

        let mut unsafe_forbidden = context.clone();
        unsafe_forbidden
            .forbidden_edit_boundaries
            .push("../outside.pm".to_string());
        assert_eq!(
            packet.strict_actionability_for_change(
                "change:lib/My/App.pm:8:return",
                &unsafe_forbidden
            ),
            Err(PerlActionabilityBlocker::UnsafeEditBoundary)
        );

        let mut missing_stop_if = context.clone();
        missing_stop_if.stop_if.clear();
        assert_eq!(
            packet
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &missing_stop_if),
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
        irrelevant_must_not_change.must_not_change =
            vec!["do not change unrelated files".to_string()];
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
            missing_verify
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
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
            invalid_verify
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
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
            shape_mismatch
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
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
            unknown_behavior
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
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
            low_confidence
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
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
            unknown_owner
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
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
            dynamic_boundary
                .strict_actionability_for_change("change:lib/My/App.pm:8:return", &context),
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
