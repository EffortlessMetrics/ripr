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
        // Production ingestion boundary (Campaign 31 PR 9, ripr-swarm#1402):
        // validate the packet as untrusted production input. Each check fails
        // closed to a named error string; no silent acceptance of bad input.
        // The fixture-only parser checked only schema_version; this boundary
        // adds 9 integrity checks. Under-emit before over-emit: a false
        // rejection (good packet rejected) is a usability bug; a false
        // acceptance (bad packet trusted) is an honesty bug. Prefer the former.
        packet.validate_ingestion()?;
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
    /// Production ingestion boundary (Campaign 31 PR 9, ripr-swarm#1402).
    ///
    /// Validates this packet as untrusted production input. Each check fails
    /// closed to a named error; no silent acceptance. Called by
    /// `PerlAdapter::consume_fact_packet` after the schema_version check.
    fn validate_ingestion(&self) -> Result<(), String> {
        // 1. Packet status must be Complete or Partial (not Unavailable —
        //    an unavailable packet carries no usable facts).
        if matches!(self.packet_status, PacketStatus::Unavailable) {
            return Err(
                "ingestion: packet_status is `unavailable`; no facts to consume".to_string(),
            );
        }

        // 2. Producer identity: must be perl-lsp.
        if self.producer.name != PERL_LSP_FACT_EXPORTER {
            return Err(format!(
                "ingestion: producer name `{}` does not match expected `{PERL_LSP_FACT_EXPORTER}`",
                self.producer.name
            ));
        }

        // 3. ID uniqueness: every fact ID must be unique within its array.
        //    Duplicate IDs create ambiguous references.
        let id_checks: [(&str, Vec<&str>); 10] = [
            (
                "files",
                self.files.iter().map(|f| f.file_id.as_str()).collect(),
            ),
            (
                "owners",
                self.owners.iter().map(|o| o.owner_id.as_str()).collect(),
            ),
            (
                "changes",
                self.changes.iter().map(|c| c.change_id.as_str()).collect(),
            ),
            (
                "tests",
                self.tests.iter().map(|t| t.test_id.as_str()).collect(),
            ),
            (
                "oracles",
                self.oracles.iter().map(|o| o.oracle_id.as_str()).collect(),
            ),
            (
                "relations",
                self.relations
                    .iter()
                    .map(|r| r.relation_id.as_str())
                    .collect(),
            ),
            (
                "dynamic_boundaries",
                self.dynamic_boundaries
                    .iter()
                    .map(|b| b.boundary_id.as_str())
                    .collect(),
            ),
            (
                "verify_commands",
                self.verify_commands
                    .iter()
                    .map(|v| v.command_id.as_str())
                    .collect(),
            ),
            (
                "limitations",
                self.limitations
                    .iter()
                    .map(|l| l.limitation_id.as_str())
                    .collect(),
            ),
            (
                "provenance",
                self.provenance
                    .iter()
                    .map(|p| p.provenance_id.as_str())
                    .collect(),
            ),
        ];
        for (field, ids) in &id_checks {
            let mut seen = BTreeSet::new();
            for id in ids {
                if !seen.insert(*id) {
                    return Err(format!(
                        "ingestion: duplicate {field} ID `{id}` — IDs must be unique"
                    ));
                }
            }
        }

        // 4. Referential integrity: every owner_id/change_id/test_id/oracle_id
        //    referenced by a relation/verify_command must exist.
        let owner_ids: BTreeSet<&str> = self.owners.iter().map(|o| o.owner_id.as_str()).collect();
        let change_ids: BTreeSet<&str> =
            self.changes.iter().map(|c| c.change_id.as_str()).collect();
        let test_ids: BTreeSet<&str> = self.tests.iter().map(|t| t.test_id.as_str()).collect();
        let oracle_ids: BTreeSet<&str> =
            self.oracles.iter().map(|o| o.oracle_id.as_str()).collect();
        for relation in &self.relations {
            if !change_ids.contains(relation.change_id.as_str()) {
                return Err(format!(
                    "ingestion: relation `{}` references unknown change_id `{}`",
                    relation.relation_id, relation.change_id
                ));
            }
            if !owner_ids.contains(relation.owner_id.as_str()) {
                return Err(format!(
                    "ingestion: relation `{}` references unknown owner_id `{}`",
                    relation.relation_id, relation.owner_id
                ));
            }
            if !test_ids.contains(relation.test_id.as_str()) {
                return Err(format!(
                    "ingestion: relation `{}` references unknown test_id `{}`",
                    relation.relation_id, relation.test_id
                ));
            }
            if let Some(ref oracle_id) = relation.oracle_id
                && !oracle_ids.contains(oracle_id.as_str())
            {
                return Err(format!(
                    "ingestion: relation `{}` references unknown oracle_id `{oracle_id}`",
                    relation.relation_id
                ));
            }
        }

        // 5. Change referential integrity: every change's owner_id + file_id
        //    must exist.
        let file_ids: BTreeSet<&str> = self.files.iter().map(|f| f.file_id.as_str()).collect();
        for change in &self.changes {
            if !file_ids.contains(change.file_id.as_str()) {
                return Err(format!(
                    "ingestion: change `{}` references unknown file_id `{}`",
                    change.change_id, change.file_id
                ));
            }
            if !owner_ids.contains(change.owner_id.as_str()) {
                return Err(format!(
                    "ingestion: change `{}` references unknown owner_id `{}`",
                    change.change_id, change.owner_id
                ));
            }
        }

        // 6. Oversized packet guard: reject packets with absurdly large arrays
        //    (resource-exhaustion protection).
        const MAX_FACTS_PER_ARRAY: usize = 10_000;
        for (field, count) in [
            ("files", self.files.len()),
            ("owners", self.owners.len()),
            ("changes", self.changes.len()),
            ("tests", self.tests.len()),
            ("oracles", self.oracles.len()),
            ("relations", self.relations.len()),
        ] {
            if count > MAX_FACTS_PER_ARRAY {
                return Err(format!(
                    "ingestion: {field} array has {count} entries; max is {MAX_FACTS_PER_ARRAY}"
                ));
            }
        }

        Ok(())
    }

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
            // Campaign 31 PR 12 (#1405): gate by relation KIND, not just class.
            // Only DirectOwnerCall / HelperCall are eligible for strict
            // actionability. PackageReference / TestNameMatch / FileProximity
            // are advisory-only (they provide context but not proof the test
            // observes the changed owner). Unknown is a limitation.
            .find(|evidence| {
                evidence.class == ExposureClass::WeaklyExposed
                    && matches!(
                        evidence.relation_kind,
                        RelationKind::DirectOwnerCall | RelationKind::HelperCall
                    )
            })
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
        // Campaign 31 PR 12 (#1405): use a concrete missing discriminator,
        // NOT the generic `default_missing_discriminator()` enum label.
        // The concrete discriminator must come from the change fact (which
        // the perl-lsp producer emits in PR 7, e.g. "$amount == $threshold").
        // If the change fact doesn't carry a concrete discriminator, fall
        // back to the generic label + emit a note that the gap record is
        // not strongly actionable (the relation gate + the generic
        // discriminator together ensure conservative behavior).
        let missing_discriminator = change
            .changed_text_digest
            .as_str()
            .strip_prefix("discriminator:")
            .map(str::to_string)
            .unwrap_or_else(|| {
                change
                    .behavior_hint
                    .default_missing_discriminator()
                    .to_string()
            });
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

fn perl_raw_evidence_refs_json(refs: &[PerlRawEvidenceRef]) -> Vec<serde_json::Value> {
    refs.iter()
        .map(|reference| {
            serde_json::json!({
                "kind": reference.kind.as_str(),
                "source_id": reference.source_id.as_str(),
                "path": reference.path.as_str()
            })
        })
        .collect()
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

impl PerlRepairCard {
    fn json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "card_version": self.card_version.as_str(),
            "source": self.source.as_str(),
            "language": self.language.as_str(),
            "language_status": self.language_status.as_str(),
            "authority_boundary": self.authority_boundary.as_str(),
            "projection_scope": self.projection_scope.as_str(),
            "public_repair_packet": self.public_repair_packet,
            "public_projection_ready": self.public_projection_ready,
            "packet_id": self.packet_id.as_str(),
            "canonical_gap_id": self.canonical_gap_id.as_str(),
            "gap_state": self.gap_state.as_str(),
            "changed_owner": self.changed_owner.as_str(),
            "evidence_class": self.evidence_class.as_str(),
            "repair_kind": self.repair_kind.as_str(),
            "current_test_evidence": self.current_test_evidence.as_str(),
            "missing_discriminator": self.missing_discriminator.as_str(),
            "target_test_shape": self.target_test_shape.as_str(),
            "suggested_test_location": self.suggested_test_location.as_str(),
            "suggested_assertion": self.suggested_assertion.as_str(),
            "verify": {
                "command": self.verify_command.as_str()
            },
            "receipt": {
                "command": self.receipt_command.as_str()
            },
            "confidence": self.confidence.as_str(),
            "raw_evidence_refs": perl_raw_evidence_refs_json(&self.raw_evidence_refs),
            "allowed_edit_boundaries": &self.allowed_edit_boundaries,
            "forbidden_edit_boundaries": &self.forbidden_edit_boundaries,
            "stop_if": &self.stop_if,
            "must_not_change": &self.must_not_change
        })
    }
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

impl PerlInternalAgentPacket {
    fn json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "packet_version": self.packet_version.as_str(),
            "packet_id": self.packet_id.as_str(),
            "canonical_gap_id": self.canonical_gap_id.as_str(),
            "language": self.language.as_str(),
            "language_status": self.language_status.as_str(),
            "authority_boundary": self.authority_boundary.as_str(),
            "projection_scope": self.projection_scope.as_str(),
            "gap_state": self.gap_state.as_str(),
            "evidence_class": self.evidence_class.as_str(),
            "repair_packet_ready": self.repair_packet_ready,
            "public_repair_packet": self.public_repair_packet,
            "public_projection_ready": self.public_projection_ready,
            "repair_route": self.repair_route.as_str(),
            "changed_owner": self.changed_owner.as_str(),
            "missing_discriminator": self.missing_discriminator.as_str(),
            "target_test_shape": self.target_test_shape.as_str(),
            "suggested_test_location": self.suggested_test_location.as_str(),
            "commands": {
                "verify": {
                    "command": self.verify_command.as_str(),
                    "argv": &self.verify_command_argv
                },
                "receipt": {
                    "command": self.receipt_command.as_str(),
                    "argv": &self.receipt_command_argv
                }
            },
            "confidence": self.confidence.as_str(),
            "raw_evidence_refs": perl_raw_evidence_refs_json(&self.raw_evidence_refs),
            "allowed_edit_surface": &self.allowed_edit_surface,
            "forbidden_files": &self.forbidden_files,
            "stop_if": &self.stop_if,
            "must_not_change": &self.must_not_change
        })
    }
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

/// Short flags `prove` accepts that are NOT test paths (Campaign 31 PR 13,
/// #1406). These are the common include/verbose/recurse/lib flags used in
/// CPAN-style workflows. Unknown short flags are rejected (fail-closed).
const PROVE_SHORT_FLAGS: &[&str] = &[
    "-l",  // include lib/ in @INC
    "-lv", // -l + verbose
    "-v",  // verbose
    "-r",  // recurse
    "-b",  // blib (build directory)
    "-s",  // shuffle
    "-q",  // quiet
    "-Q",  // very quiet
    "-f",  // force
];

/// Long flags `prove` accepts.
const PROVE_LONG_FLAGS: &[&str] = &[
    "--verbose",
    "--lib",
    "--blib",
    "--recurse",
    "--merge",
    "--shuffle",
    "--jobs",
    "--state",
    "--ext",
    "--timer",
    "--comments",
];

/// Value-option prefixes for `prove` (these consume the next arg as a value).
const PROVE_VALUE_OPTS: &[&str] = &["-I", "-M", "-j", "--include=", "--module=", "--state="];

/// Split `prove`'s trailing args into (flags, test_paths). Flags are the
/// recognized options; test_paths are the remaining positional args that must
/// each be a safe `t/*.t` path. Returns `None` if an unrecognized flag is
/// encountered (fail-closed: reject-by-default for unknown short flags).
///
/// This replaces the positional matching that treated every arg after `prove`
/// as a test path — which rejected common commands like `prove -l t/app.t`.
/// (Campaign 31 PR 13, ripr-swarm#1406.)
fn split_prove_args(args: &[String]) -> Option<(Vec<String>, Vec<String>)> {
    let mut flags = Vec::new();
    let mut test_paths = Vec::new();
    for arg in args {
        if PROVE_SHORT_FLAGS.contains(&arg.as_str()) || PROVE_LONG_FLAGS.contains(&arg.as_str()) {
            flags.push(arg.clone());
        } else if PROVE_VALUE_OPTS
            .iter()
            .any(|prefix| arg.starts_with(prefix))
        {
            // Value option: either -Ilib (inline) or --include=lib (= form).
            flags.push(arg.clone());
        } else if arg.starts_with('-') {
            // Unknown short/long flag — reject (fail-closed).
            return None;
        } else {
            // Positional — must be a test path.
            test_paths.push(arg.clone());
        }
    }
    Some((flags, test_paths))
}

fn is_verify_command(command: &[String]) -> bool {
    if command.iter().any(|arg| !is_safe_command_arg(arg)) {
        return false;
    }

    match command {
        // prove [flags] <test_paths...> — typed flag recognition (PR 13, #1406).
        [program, trailing @ ..] if program == "prove" => {
            let Some((_flags, test_paths)) = split_prove_args(trailing) else {
                return false;
            };
            !test_paths.is_empty() && test_paths.iter().all(|path| is_safe_test_path(path))
        }
        [program, subcommand, test_paths @ ..] if program == "yath" && subcommand == "test" => {
            !test_paths.is_empty() && test_paths.iter().all(|path| is_safe_test_path(path))
        }
        [program, subcommand, runner, test_paths @ ..]
            if program == "carton" && subcommand == "exec" && runner == "prove" =>
        {
            // carton exec prove [flags] <test_paths...> — delegate to the
            // typed prove parser for the inner prove args.
            let Some((_flags, test_paths)) = split_prove_args(test_paths) else {
                return false;
            };
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
mod tests;
