use crate::domain::{LanguageId, LanguageStatus, StaticLimitKind};
use crate::output::first_useful_action::DEFAULT_FIRST_USEFUL_ACTION_OUT;
use crate::output::gap_decision_ledger::DEFAULT_GAP_DECISION_LEDGER_OUT;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Component, Path};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GapArtifactValidationContext<'a> {
    pub(super) root: &'a Path,
    pub(super) enabled_languages: &'a [LanguageId],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ValidatedGapArtifact {
    pub(super) kind: GapArtifactKind,
    pub(super) root: Option<String>,
    pub(super) identities: Vec<GapArtifactIdentity>,
    pub(super) language: Option<LanguageId>,
    pub(super) language_status: Option<LanguageStatus>,
    pub(super) gap_state: Option<String>,
    pub(super) related_paths: Vec<String>,
    pub(super) verify_commands: Vec<String>,
    pub(super) receipt_commands: Vec<String>,
    pub(super) static_limit_kinds: Vec<String>,
    pub(super) has_text_static_limit: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GapArtifactKind {
    EvidenceRecord,
    FirstUsefulAction,
    GapDecisionLedger,
    AgentReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GapArtifactIdentity {
    pub(super) canonical_gap_id: Option<String>,
    pub(super) seam_id: Option<String>,
    pub(super) finding_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum GapArtifactRejection {
    DisabledLanguage(String),
    MalformedArtifact(&'static str),
    MalformedCommandPayload(String),
    MissingIdentity,
    OutOfWorkspacePath(String),
    StaleArtifact,
    UnavailableLanguage(String),
    UnsupportedSchema(String),
    UnsupportedStaticLimitKind(String),
    UnsupportedKind(String),
    WrongRoot(String),
}

impl ValidatedGapArtifact {
    pub(super) fn is_safe_projection_input(&self) -> bool {
        let kind_is_supported = matches!(
            self.kind,
            GapArtifactKind::EvidenceRecord
                | GapArtifactKind::FirstUsefulAction
                | GapArtifactKind::GapDecisionLedger
                | GapArtifactKind::AgentReceipt
        );
        let root_is_present_or_deferred = self
            .root
            .as_ref()
            .is_none_or(|root| !root.trim().is_empty());
        let identity_is_present = self.identities.iter().any(|identity| {
            identity.canonical_gap_id.is_some()
                || identity.seam_id.is_some()
                || identity.finding_id.is_some()
        });
        let language_is_supported = self.language.is_none_or(|language| language.is_available());
        let status_is_supported = self.language_status.is_none_or(|status| {
            matches!(status, LanguageStatus::Stable | LanguageStatus::Preview)
        });
        let gap_state_is_present_or_deferred = self
            .gap_state
            .as_ref()
            .is_none_or(|state| !state.trim().is_empty());
        let path_payloads_are_present = self
            .related_paths
            .iter()
            .all(|path| !path.trim().is_empty());
        let command_payloads_are_present = self
            .verify_commands
            .iter()
            .chain(self.receipt_commands.iter())
            .all(|command| !command.trim().is_empty());
        let static_limits_are_structured_or_text = self
            .static_limit_kinds
            .iter()
            .all(|kind| known_static_limit_kind(kind))
            || self.has_text_static_limit;

        kind_is_supported
            && root_is_present_or_deferred
            && identity_is_present
            && language_is_supported
            && status_is_supported
            && gap_state_is_present_or_deferred
            && path_payloads_are_present
            && command_payloads_are_present
            && static_limits_are_structured_or_text
    }
}

pub(super) fn validate_workspace_gap_artifacts(
    root: &Path,
    enabled_languages: &[LanguageId],
) -> Vec<ValidatedGapArtifact> {
    let context = GapArtifactValidationContext {
        root,
        enabled_languages,
    };
    [
        DEFAULT_FIRST_USEFUL_ACTION_OUT,
        DEFAULT_GAP_DECISION_LEDGER_OUT,
    ]
    .iter()
    .filter_map(|relative| validate_workspace_gap_artifact(root, relative, &context))
    .collect()
}

fn validate_workspace_gap_artifact(
    root: &Path,
    relative: &str,
    context: &GapArtifactValidationContext<'_>,
) -> Option<ValidatedGapArtifact> {
    let text = std::fs::read_to_string(root.join(relative)).ok()?;
    let value = serde_json::from_str::<Value>(&text).ok()?;
    validate_gap_artifact(&value, context).ok()
}

pub(super) fn validate_gap_artifact(
    artifact: &Value,
    context: &GapArtifactValidationContext<'_>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let object = artifact
        .as_object()
        .ok_or(GapArtifactRejection::MalformedArtifact(
            "gap artifact root must be a JSON object",
        ))?;
    validate_schema(object.get("schema_version").and_then(Value::as_str))?;
    let kind = artifact_kind(artifact)?;
    let report_root = object.get("root").and_then(Value::as_str);
    validate_report_root(context.root, report_root)?;
    validate_freshness(artifact)?;

    let mut validation = match kind {
        GapArtifactKind::FirstUsefulAction => validate_first_useful_action(artifact, report_root)?,
        GapArtifactKind::GapDecisionLedger => validate_gap_decision_ledger(artifact, report_root)?,
        GapArtifactKind::EvidenceRecord => validate_evidence_record(artifact, report_root)?,
        GapArtifactKind::AgentReceipt => validate_agent_receipt(artifact, report_root)?,
    };

    validate_language(context, validation.language, validation.language_status)?;
    validate_paths(context.root, &validation.related_paths)?;
    validate_commands(context.root, &validation.verify_commands)?;
    validate_commands(context.root, &validation.receipt_commands)?;
    validate_static_limits(artifact)?;
    normalize_static_limits(artifact, &mut validation);
    if validation.identities.is_empty() {
        return Err(GapArtifactRejection::MissingIdentity);
    }
    Ok(validation)
}

fn validate_first_useful_action(
    artifact: &Value,
    report_root: Option<&str>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let selected = artifact.get("selected");
    let target = artifact.get("target");
    let commands = artifact.get("commands");
    let identity = identity_from_sources(&[selected, Some(artifact)])
        .ok_or(GapArtifactRejection::MissingIdentity)?;
    let related_paths = string_values(&[
        path_value(target, &["file"]),
        path_value(target, &["related_test"]),
        path_value(selected, &["path"]),
    ]);
    let verify_commands = string_values(&[path_value(commands, &["verify"])]);
    let receipt_commands = string_values(&[path_value(commands, &["receipt"])]);
    Ok(ValidatedGapArtifact {
        kind: GapArtifactKind::FirstUsefulAction,
        root: report_root.map(ToOwned::to_owned),
        identities: vec![identity],
        language: language_from_value(artifact)?,
        language_status: language_status_from_value(artifact)?,
        gap_state: artifact
            .get("status")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        related_paths,
        verify_commands,
        receipt_commands,
        static_limit_kinds: Vec::new(),
        has_text_static_limit: false,
    })
}

fn validate_gap_decision_ledger(
    artifact: &Value,
    report_root: Option<&str>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let records = artifact
        .get("records")
        .or_else(|| artifact.get("gap_records"))
        .and_then(Value::as_array)
        .ok_or(GapArtifactRejection::MalformedArtifact(
            "gap decision ledger must contain records",
        ))?;
    let mut identities = Vec::new();
    let mut related_paths = Vec::new();
    let mut verify_commands = Vec::new();
    let mut receipt_commands = Vec::new();
    let mut language = None;
    let mut language_status = None;
    let mut gap_state = None;
    for record in records {
        identities.push(
            identity_from_sources(&[Some(record)]).ok_or(GapArtifactRejection::MissingIdentity)?,
        );
        language = language.or(language_from_value(record)?);
        language_status = language_status.or(language_status_from_value(record)?);
        gap_state = gap_state.or_else(|| {
            record
                .get("gap_state")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        });
        let route = record.get("repair_route");
        let anchor = record.get("anchor");
        let receipt = record.get("receipt");
        related_paths.extend(string_values(&[
            path_value(route, &["target_file"]),
            path_value(route, &["related_test"]),
            path_value(anchor, &["file"]),
            path_value(receipt, &["path"]),
        ]));
        if let Some(commands) = record
            .get("verification_commands")
            .and_then(Value::as_array)
        {
            verify_commands.extend(
                commands
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned),
            );
        }
        if let Some(command) = record.get("receipt_command").and_then(Value::as_str) {
            receipt_commands.push(command.to_string());
        }
    }
    Ok(ValidatedGapArtifact {
        kind: GapArtifactKind::GapDecisionLedger,
        root: report_root.map(ToOwned::to_owned),
        identities,
        language,
        language_status,
        gap_state,
        related_paths,
        verify_commands,
        receipt_commands,
        static_limit_kinds: Vec::new(),
        has_text_static_limit: false,
    })
}

fn validate_evidence_record(
    artifact: &Value,
    report_root: Option<&str>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let identity = identity_from_sources(&[
        Some(artifact),
        artifact.get("canonical_item"),
        artifact.get("identity"),
    ])
    .ok_or(GapArtifactRejection::MissingIdentity)?;
    let location = artifact.get("location");
    let canonical_item = artifact.get("canonical_item");
    let recommendation = artifact.get("recommendation");
    let related_paths = string_values(&[
        path_value(location, &["file"]),
        path_value(canonical_item, &["related_test", "file"]),
        path_value(recommendation, &["recommended_test", "file"]),
        path_value(recommendation, &["nearest_test_to_imitate", "file"]),
    ]);
    let verify_commands = string_values(&[
        path_value(canonical_item, &["verify_command"]),
        path_value(recommendation, &["verify_command"]),
    ]);
    Ok(ValidatedGapArtifact {
        kind: GapArtifactKind::EvidenceRecord,
        root: report_root.map(ToOwned::to_owned),
        identities: vec![identity],
        language: language_from_value(artifact)?,
        language_status: language_status_from_value(artifact)?,
        gap_state: path_value(canonical_item, &["gap_state"])
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        related_paths,
        verify_commands,
        receipt_commands: Vec::new(),
        static_limit_kinds: Vec::new(),
        has_text_static_limit: false,
    })
}

fn validate_agent_receipt(
    artifact: &Value,
    report_root: Option<&str>,
) -> Result<ValidatedGapArtifact, GapArtifactRejection> {
    let identity = identity_from_sources(&[Some(artifact), artifact.get("selected")])
        .ok_or(GapArtifactRejection::MissingIdentity)?;
    let inputs = artifact.get("inputs");
    let related_paths = string_values(&[
        path_value(inputs, &["receipt"]),
        path_value(inputs, &["verify"]),
        path_value(artifact.get("target"), &["file"]),
    ]);
    Ok(ValidatedGapArtifact {
        kind: GapArtifactKind::AgentReceipt,
        root: report_root.map(ToOwned::to_owned),
        identities: vec![identity],
        language: language_from_value(artifact)?,
        language_status: language_status_from_value(artifact)?,
        gap_state: artifact
            .get("movement")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        related_paths,
        verify_commands: Vec::new(),
        receipt_commands: Vec::new(),
        static_limit_kinds: Vec::new(),
        has_text_static_limit: false,
    })
}

fn validate_schema(schema_version: Option<&str>) -> Result<(), GapArtifactRejection> {
    match schema_version {
        Some("0.1") => Ok(()),
        Some(version) => Err(GapArtifactRejection::UnsupportedSchema(version.to_string())),
        None => Err(GapArtifactRejection::UnsupportedSchema(
            "missing".to_string(),
        )),
    }
}

fn artifact_kind(artifact: &Value) -> Result<GapArtifactKind, GapArtifactRejection> {
    let kind = artifact.get("kind").and_then(Value::as_str);
    match kind {
        Some("first_useful_action") => Ok(GapArtifactKind::FirstUsefulAction),
        Some("gap_decision_ledger") => Ok(GapArtifactKind::GapDecisionLedger),
        Some("agent_receipt") => Ok(GapArtifactKind::AgentReceipt),
        Some(other) => Err(GapArtifactRejection::UnsupportedKind(other.to_string())),
        None if artifact.get("evidence_path").is_some() && artifact.get("seam_id").is_some() => {
            Ok(GapArtifactKind::EvidenceRecord)
        }
        None => Err(GapArtifactRejection::UnsupportedKind("missing".to_string())),
    }
}

fn validate_report_root(
    root: &Path,
    report_root: Option<&str>,
) -> Result<(), GapArtifactRejection> {
    let Some(report_root) = report_root else {
        return Ok(());
    };
    if report_root == "." || normalize_path(root) == normalize_path(&root.join(report_root)) {
        return Ok(());
    }
    let candidate = Path::new(report_root);
    if candidate.is_absolute() && normalize_path(candidate) == normalize_path(root) {
        return Ok(());
    }
    Err(GapArtifactRejection::WrongRoot(report_root.to_string()))
}

fn validate_freshness(artifact: &Value) -> Result<(), GapArtifactRejection> {
    for path in [
        &["status"][..],
        &["freshness"][..],
        &["editor_context", "freshness"][..],
    ] {
        if path_value(Some(artifact), path).and_then(Value::as_str) == Some("stale") {
            return Err(GapArtifactRejection::StaleArtifact);
        }
    }
    Ok(())
}

fn validate_language(
    context: &GapArtifactValidationContext<'_>,
    language: Option<LanguageId>,
    language_status: Option<LanguageStatus>,
) -> Result<(), GapArtifactRejection> {
    let Some(language) = language else {
        return Ok(());
    };
    if !language.is_available() {
        return Err(GapArtifactRejection::UnavailableLanguage(
            language.as_str().to_string(),
        ));
    }
    if !context.enabled_languages.contains(&language) {
        return Err(GapArtifactRejection::DisabledLanguage(
            language.as_str().to_string(),
        ));
    }
    if language_status == Some(LanguageStatus::Preview) && language == LanguageId::Rust {
        return Err(GapArtifactRejection::MalformedArtifact(
            "rust gap artifacts must not be labeled preview",
        ));
    }
    Ok(())
}

fn validate_paths(root: &Path, paths: &[String]) -> Result<(), GapArtifactRejection> {
    for path in paths {
        if !workspace_path_is_safe(root, path) {
            return Err(GapArtifactRejection::OutOfWorkspacePath(path.clone()));
        }
    }
    Ok(())
}

fn validate_commands(root: &Path, commands: &[String]) -> Result<(), GapArtifactRejection> {
    for command in commands {
        if !command_payload_is_safe(root, command) {
            return Err(GapArtifactRejection::MalformedCommandPayload(
                command.clone(),
            ));
        }
    }
    Ok(())
}

fn validate_static_limits(artifact: &Value) -> Result<(), GapArtifactRejection> {
    for kind in static_limit_kind_values(artifact) {
        if !known_static_limit_kind(&kind) {
            return Err(GapArtifactRejection::UnsupportedStaticLimitKind(kind));
        }
    }
    Ok(())
}

fn normalize_static_limits(artifact: &Value, validation: &mut ValidatedGapArtifact) {
    let mut seen = BTreeSet::new();
    for kind in static_limit_kind_values(artifact) {
        if seen.insert(kind.clone()) {
            validation.static_limit_kinds.push(kind);
        }
    }
    validation.has_text_static_limit = contains_text_static_limit(artifact);
}

fn identity_from_sources(sources: &[Option<&Value>]) -> Option<GapArtifactIdentity> {
    let canonical_gap_id = string_from_sources(sources, &["canonical_gap_id"])
        .or_else(|| string_from_sources(sources, &["identity", "canonical_gap_id"]))
        .or_else(|| string_from_sources(sources, &["evidence_record", "canonical_gap_id"]));
    let seam_id = string_from_sources(sources, &["seam_id"]);
    let finding_id = string_from_sources(sources, &["finding_id"]);
    if canonical_gap_id.is_none() && seam_id.is_none() && finding_id.is_none() {
        return None;
    }
    Some(GapArtifactIdentity {
        canonical_gap_id,
        seam_id,
        finding_id,
    })
}

fn language_from_value(value: &Value) -> Result<Option<LanguageId>, GapArtifactRejection> {
    let language = value.get("language").and_then(Value::as_str);
    match language {
        None => Ok(None),
        Some("rust") => Ok(Some(LanguageId::Rust)),
        Some("typescript") | Some("javascript") => Ok(Some(LanguageId::TypeScript)),
        Some("python") => Ok(Some(LanguageId::Python)),
        Some(other) => Err(GapArtifactRejection::MalformedArtifact(match other {
            "" => "language must not be empty",
            _ => "unsupported language",
        })),
    }
}

fn language_status_from_value(
    value: &Value,
) -> Result<Option<LanguageStatus>, GapArtifactRejection> {
    match value.get("language_status").and_then(Value::as_str) {
        None => Ok(None),
        Some("stable") => Ok(Some(LanguageStatus::Stable)),
        Some("preview") => Ok(Some(LanguageStatus::Preview)),
        Some(_) => Err(GapArtifactRejection::MalformedArtifact(
            "unsupported language_status",
        )),
    }
}

fn string_values(values: &[Option<&Value>]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| value.and_then(Value::as_str))
        .filter_map(non_empty)
        .collect()
}

fn string_from_sources(sources: &[Option<&Value>], path: &[&str]) -> Option<String> {
    sources
        .iter()
        .find_map(|source| path_value(*source, path).and_then(Value::as_str))
        .and_then(non_empty)
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn path_value<'a>(value: Option<&'a Value>, path: &[&str]) -> Option<&'a Value> {
    let mut current = value?;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn workspace_path_is_safe(root: &Path, raw: &str) -> bool {
    let path_text = path_part(raw).trim();
    if path_text.is_empty() || path_text.contains('\n') || path_text.contains('\r') {
        return false;
    }
    let path = Path::new(path_text);
    if path.is_absolute() {
        return is_inside_workspace(root, path);
    }
    !path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

fn path_part(raw: &str) -> &str {
    raw.split_once("::").map_or(raw, |(path, _)| path)
}

fn command_payload_is_safe(root: &Path, command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() || trimmed.contains('\n') || trimmed.contains('\r') {
        return false;
    }
    if trimmed.contains("../") || trimmed.contains("..\\") {
        return false;
    }
    let tokens = command_tokens(trimmed);
    for index in 0..tokens.len() {
        if tokens[index] == "--root" {
            let Some(root_arg) = tokens.get(index + 1) else {
                return false;
            };
            if root_arg == "." {
                return true;
            }
            if normalize_path(Path::new(root_arg)) == normalize_path(root) {
                return true;
            }
            return workspace_path_is_safe(root, root_arg);
        }
    }
    true
}

fn command_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for character in command.chars() {
        match (quote, character) {
            (Some(active), value) if value == active => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, value) if value.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_inside_workspace(root: &Path, path: &Path) -> bool {
    let root = normalize_path(root);
    let path = normalize_path(path);
    path == root || path.starts_with(&(root + "/"))
}

fn normalize_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

fn static_limit_kind_values(value: &Value) -> Vec<String> {
    let mut values = Vec::new();
    collect_static_limit_kind_values(value, &mut values);
    values
}

fn collect_static_limit_kind_values(value: &Value, values: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(kind) = object.get("static_limit_kind").and_then(Value::as_str) {
                values.push(kind.to_string());
            }
            for child in object.values() {
                collect_static_limit_kind_values(child, values);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_static_limit_kind_values(item, values);
            }
        }
        _ => {}
    }
}

fn contains_text_static_limit(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            object.iter().any(|(key, child)| {
                key.starts_with("static_limit")
                    && key != "static_limit_kind"
                    && child.as_str().is_some_and(|text| !text.trim().is_empty())
            }) || object.values().any(contains_text_static_limit)
        }
        Value::Array(items) => items.iter().any(contains_text_static_limit),
        _ => false,
    }
}

fn known_static_limit_kind(kind: &str) -> bool {
    [
        StaticLimitKind::DynamicDispatch,
        StaticLimitKind::Metaprogramming,
        StaticLimitKind::MissingImportGraph,
        StaticLimitKind::DecoratorIndirection,
        StaticLimitKind::MockedModule,
        StaticLimitKind::UnsupportedSyntax,
    ]
    .iter()
    .any(|known| known.as_str() == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root() -> PathBuf {
        PathBuf::from("/workspace")
    }

    fn context(enabled_languages: &[LanguageId]) -> GapArtifactValidationContext<'_> {
        GapArtifactValidationContext {
            root: Path::new("/workspace"),
            enabled_languages,
        }
    }

    fn first_action() -> Value {
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "first_useful_action",
            "root": ".",
            "status": "actionable",
            "selected": {
                "seam_id": "seam-1",
                "path": "src/pricing.rs",
                "line": 88
            },
            "target": {
                "file": "tests/pricing.rs",
                "related_test": "tests/pricing.rs::below_threshold"
            },
            "commands": {
                "verify": "ripr agent verify --root . --json",
                "receipt": "ripr agent receipt --root . --json"
            }
        })
    }

    fn temp_root(label: &str) -> Result<PathBuf, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system time before epoch: {err}"))?;
        Ok(std::env::temp_dir().join(format!(
            "ripr-lsp-gap-artifacts-{label}-{}-{}",
            std::process::id(),
            now.as_nanos()
        )))
    }

    fn preview_gap_ledger() -> Value {
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "gap_decision_ledger",
            "root": ".",
            "status": "advisory",
            "records": [
                {
                    "gap_id": "gap:py:pricing",
                    "canonical_gap_id": "gap:py:pricing",
                    "kind": "MissingBoundaryAssertion",
                    "language": "python",
                    "language_status": "preview",
                    "gap_state": "actionable",
                    "repairability": "repairable",
                    "static_limit_kind": "missing_import_graph",
                    "repair_route": {
                        "route_kind": "AddBoundaryAssertion",
                        "target_file": "tests/test_pricing.py",
                        "related_test": "tests/test_pricing.py::test_discount_boundary"
                    },
                    "verification_commands": [
                        "ripr agent verify --root . --json"
                    ],
                    "receipt_command": "ripr agent receipt --root . --json"
                }
            ]
        })
    }

    #[test]
    fn first_useful_action_validates_read_only_projection_inputs() -> Result<(), String> {
        let artifact = first_action();
        let validated = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]))
            .map_err(|err| format!("{err:?}"))?;

        assert_eq!(validated.kind, GapArtifactKind::FirstUsefulAction);
        assert_eq!(validated.root.as_deref(), Some("."));
        assert_eq!(validated.identities[0].seam_id.as_deref(), Some("seam-1"));
        assert_eq!(validated.gap_state.as_deref(), Some("actionable"));
        assert_eq!(validated.related_paths.len(), 3);
        assert_eq!(validated.verify_commands.len(), 1);
        assert_eq!(validated.receipt_commands.len(), 1);
        Ok(())
    }

    #[test]
    fn preview_gap_ledger_validates_only_when_language_is_enabled() -> Result<(), String> {
        let artifact = preview_gap_ledger();
        let disabled = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]));
        assert_eq!(
            disabled,
            Err(GapArtifactRejection::DisabledLanguage("python".to_string()))
        );

        let enabled =
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust, LanguageId::Python]))
                .map_err(|err| format!("{err:?}"))?;
        assert_eq!(enabled.kind, GapArtifactKind::GapDecisionLedger);
        assert_eq!(enabled.language, Some(LanguageId::Python));
        assert_eq!(enabled.language_status, Some(LanguageStatus::Preview));
        assert_eq!(
            enabled.static_limit_kinds,
            vec!["missing_import_graph".to_string()]
        );
        assert_eq!(
            enabled.identities[0].canonical_gap_id.as_deref(),
            Some("gap:py:pricing")
        );
        Ok(())
    }

    #[test]
    fn validation_rejects_wrong_root() {
        let mut artifact = first_action();
        artifact["root"] = json!("/other/workspace");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::WrongRoot(
                "/other/workspace".to_string()
            ))
        );
    }

    #[test]
    fn validation_rejects_unsupported_schema() {
        let mut artifact = first_action();
        artifact["schema_version"] = json!("9.9");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::UnsupportedSchema("9.9".to_string()))
        );
    }

    #[test]
    fn validation_rejects_missing_identity() {
        let mut artifact = first_action();
        artifact["selected"] = json!({"path": "src/pricing.rs"});

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MissingIdentity)
        );
    }

    #[test]
    fn validation_rejects_stale_artifact() {
        let mut artifact = first_action();
        artifact["status"] = json!("stale");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::StaleArtifact)
        );
    }

    #[test]
    fn validation_rejects_out_of_workspace_related_paths() {
        let mut artifact = first_action();
        artifact["target"]["file"] = json!("../outside.rs");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::OutOfWorkspacePath(
                "../outside.rs".to_string()
            ))
        );
    }

    #[test]
    fn validation_rejects_malformed_command_payloads() {
        let mut artifact = first_action();
        artifact["commands"]["verify"] = json!("ripr agent verify --root ../outside --json");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust])),
            Err(GapArtifactRejection::MalformedCommandPayload(
                "ripr agent verify --root ../outside --json".to_string()
            ))
        );
    }

    #[test]
    fn validation_rejects_unregistered_static_limit_kind() {
        let mut artifact = preview_gap_ledger();
        artifact["records"][0]["static_limit_kind"] = json!("runtime_magic");

        assert_eq!(
            validate_gap_artifact(&artifact, &context(&[LanguageId::Rust, LanguageId::Python])),
            Err(GapArtifactRejection::UnsupportedStaticLimitKind(
                "runtime_magic".to_string()
            ))
        );
    }

    #[test]
    fn evidence_record_validates_text_static_limits_without_parsing_action_semantics()
    -> Result<(), String> {
        let artifact = json!({
            "schema_version": "0.1",
            "seam_id": "seam-2",
            "canonical_gap_id": "gap:rust:seam-2",
            "language": "rust",
            "language_status": "stable",
            "location": {
                "file": "src/lib.rs",
                "line": 12
            },
            "canonical_item": {
                "gap_state": "static_limitation",
                "verify_command": "ripr agent verify --root . --json"
            },
            "evidence_path": {},
            "static_limit_detail": "helper indirection hides the observed value"
        });

        let validated = validate_gap_artifact(&artifact, &context(&[LanguageId::Rust]))
            .map_err(|err| format!("{err:?}"))?;
        assert_eq!(validated.kind, GapArtifactKind::EvidenceRecord);
        assert!(validated.has_text_static_limit);
        assert!(validated.static_limit_kinds.is_empty());
        Ok(())
    }

    #[test]
    fn command_root_accepts_quoted_workspace_path() {
        let workspace = root();
        let command = "ripr agent verify --root \"/workspace\" --json";

        assert!(command_payload_is_safe(&workspace, command));
    }

    #[test]
    fn workspace_gap_artifact_loader_validates_known_report_paths() -> Result<(), String> {
        let root = temp_root("loader")?;
        let report_path = root.join(DEFAULT_FIRST_USEFUL_ACTION_OUT);
        let parent = report_path
            .parent()
            .ok_or_else(|| format!("missing parent for {}", report_path.display()))?;
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
        fs::write(&report_path, first_action().to_string())
            .map_err(|err| format!("write {}: {err}", report_path.display()))?;

        let artifacts = validate_workspace_gap_artifacts(&root, &[LanguageId::Rust]);
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].kind, GapArtifactKind::FirstUsefulAction);

        fs::remove_dir_all(&root).map_err(|err| format!("remove {}: {err}", root.display()))?;
        Ok(())
    }
}
