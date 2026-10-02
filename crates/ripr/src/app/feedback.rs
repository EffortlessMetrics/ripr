//! Record and join local usefulness-feedback receipts (RIPR-SPEC-0179 / #4585).
//!
//! Recording writes one ignored JSON artifact under `target/ripr/feedback/`.
//! It does not load or mutate diagnostics, classification, baseline,
//! suppressions, gates, or gap closure. Export joins stored receipts onto
//! existing route-quality rows without creating a second attempt ledger.

use crate::atomic_file;
use crate::domain::{
    ActorKind, FEEDBACK_NOTE_MAX_BYTES, FEEDBACK_SCHEMA_VERSION, FeedbackJudgment, FeedbackPayload,
    FeedbackReceipt, ReferenceState, ResultIdentity, classify_reference,
};
use crate::output::feedback::{
    render_join_document, render_receipt_document, rendered_receipt_from_value,
};
use crate::output::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const FEEDBACK_DIRECTORY: &str = "target/ripr/feedback";
const DEFAULT_ROUTE_QUALITY: &str = "target/ripr/reports/route-quality.json";
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 128;

/// Best-effort note redaction patterns. These are not a guarantee of detecting
/// every secret.
const SECRET_PATTERNS: &[&str] = &[
    "akia",
    "begin private key",
    "begin rsa private key",
    "aws_secret_access_key",
    "ghp_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecordFeedbackOptions {
    pub root: PathBuf,
    pub payload: FeedbackPayload,
    pub idempotency_key: Option<String>,
    pub recorded_at: Option<String>,
    pub live_identity: Option<ResultIdentity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecordedFeedback {
    pub status: RecordStatus,
    pub receipt: FeedbackReceipt,
    pub path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecordStatus {
    Created,
    AlreadyRecorded,
}

impl RecordStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::AlreadyRecorded => "already_recorded",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExportFeedbackOptions {
    pub root: PathBuf,
    pub route_quality: Option<PathBuf>,
    pub live_identity: Option<ResultIdentity>,
}

/// Record one local usefulness-feedback receipt.
pub(crate) fn record_feedback(options: &RecordFeedbackOptions) -> Result<RecordedFeedback, String> {
    let payload = options.payload.clone().normalized();
    payload.validate()?;
    check_note_privacy(payload.note.as_deref())?;
    let root = confine_root(&options.root)?;
    let key = resolve_idempotency_key(&payload, options.idempotency_key.as_deref())?;
    let directory = feedback_directory(&root)?;
    let path = receipt_path(&directory, &key)?;
    if let Some(existing) = load_receipt_file(&path)? {
        let existing_payload = FeedbackPayload::from_receipt(&existing).normalized();
        if existing_payload == payload {
            return Ok(RecordedFeedback {
                status: RecordStatus::AlreadyRecorded,
                receipt: existing,
                path,
            });
        }
        return Err(format!(
            "feedback idempotency conflict for key `{key}`: payload differs from the existing receipt"
        ));
    }

    let recorded_at = match &options.recorded_at {
        Some(value) => value.clone(),
        None => unix_ms_now()?,
    };
    let feedback_id = feedback_id_for(&payload, &key);
    let reference_state = classify_reference(&payload.identity, options.live_identity.as_ref());
    let receipt = FeedbackReceipt {
        feedback_id,
        idempotency_key: key,
        identity: payload.identity.clone(),
        actor_kind: payload.actor_kind,
        review_status: payload.review_status,
        review_actor_kind: payload.review_actor_kind,
        reason: payload.reason,
        judgment_override: payload.judgment_override,
        note: payload.note.clone(),
        reference_state,
        recorded_at,
    };
    let rendered = render_receipt_document(&receipt, RecordStatus::Created.as_str())?;
    atomic_file::write(&path, rendered.as_bytes(), "usefulness-feedback receipt")?;
    Ok(RecordedFeedback {
        status: RecordStatus::Created,
        receipt,
        path,
    })
}

/// Load every well-formed receipt under the default feedback directory.
pub(crate) fn load_feedback_receipts(root: &Path) -> Result<Vec<FeedbackReceipt>, String> {
    let root = confine_root(root)?;
    let directory = root.join(FEEDBACK_DIRECTORY);
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut receipts = Vec::new();
    for entry in fs::read_dir(&directory)
        .map_err(|error| format!("read {} failed: {error}", directory.display()))?
    {
        let entry =
            entry.map_err(|error| format!("read {} failed: {error}", directory.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        if let Some(receipt) = load_receipt_file(&path)? {
            receipts.push(receipt);
        }
    }
    receipts.sort_by(|left, right| left.feedback_id.cmp(&right.feedback_id));
    Ok(receipts)
}

/// Join local receipts onto existing route-quality rows.
pub(crate) fn export_feedback_join(
    options: &ExportFeedbackOptions,
) -> Result<(String, serde_json::Value), String> {
    let root = confine_root(&options.root)?;
    let receipts = load_feedback_receipts(&root)?;
    let route_quality_path = options
        .route_quality
        .clone()
        .unwrap_or_else(|| root.join(DEFAULT_ROUTE_QUALITY));
    if route_quality_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("feedback export --route-quality must not contain `..`".to_string());
    }
    let route_quality = load_optional_json(&route_quality_path)?;
    let join = build_join(
        &receipts,
        route_quality.as_ref(),
        options.live_identity.as_ref(),
        &route_quality_path,
    );
    let rendered = render_join_document(&join)?;
    Ok((rendered, join))
}

pub(crate) fn feedback_directory(root: &Path) -> Result<PathBuf, String> {
    let directory = root.join(FEEDBACK_DIRECTORY);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("create {} failed: {error}", directory.display()))?;
    Ok(directory)
}

fn confine_root(root: &Path) -> Result<PathBuf, String> {
    if root.as_os_str().is_empty() {
        return Err("feedback --root must be a non-empty directory".to_string());
    }
    if root
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("feedback --root must not contain `..`".to_string());
    }
    if !root.exists() {
        fs::create_dir_all(root)
            .map_err(|error| format!("create {} failed: {error}", root.display()))?;
    }
    fs::canonicalize(root)
        .map_err(|error| format!("canonicalize {} failed: {error}", root.display()))
}

fn receipt_path(directory: &Path, key: &str) -> Result<PathBuf, String> {
    if key
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.'))
    {
        return Err(
            "feedback --idempotency-key may contain only ASCII letters, digits, `.`, `_`, and `-`"
                .to_string(),
        );
    }
    let path = directory.join(format!("{key}.json"));
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("feedback receipt path escaped the feedback directory".to_string());
    }
    Ok(path)
}

fn resolve_idempotency_key(
    payload: &FeedbackPayload,
    explicit: Option<&str>,
) -> Result<String, String> {
    if let Some(key) = explicit {
        if key.trim().is_empty() {
            return Err("feedback --idempotency-key requires a non-empty value".to_string());
        }
        if key.len() > MAX_IDEMPOTENCY_KEY_BYTES {
            return Err(format!(
                "feedback --idempotency-key exceeds {MAX_IDEMPOTENCY_KEY_BYTES} bytes"
            ));
        }
        return Ok(key.to_string());
    }
    Ok(format!("auto-{}", short_digest(&payload.canonical_bytes())))
}

fn feedback_id_for(payload: &FeedbackPayload, key: &str) -> String {
    format!(
        "fb-{}",
        short_digest(&format!("{key}\n{}", payload.canonical_bytes()))
    )
}

fn short_digest(bytes: &str) -> String {
    let digest = Sha256::digest(bytes.as_bytes());
    digest
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unix_ms_now() -> Result<String, String> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("clock error: {error}"))?
        .as_millis();
    Ok(format!("unix_ms:{ms}"))
}

fn check_note_privacy(note: Option<&str>) -> Result<(), String> {
    let Some(note) = note else {
        return Ok(());
    };
    if note.len() > FEEDBACK_NOTE_MAX_BYTES {
        return Err(format!(
            "feedback --note exceeds {FEEDBACK_NOTE_MAX_BYTES} bytes"
        ));
    }
    let lowered = note.to_ascii_lowercase();
    for pattern in SECRET_PATTERNS {
        if lowered.contains(pattern) {
            return Err(
                "feedback --note matches a known secret pattern; omit credentials and retry"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn load_receipt_file(path: &Path) -> Result<Option<FeedbackReceipt>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("read {} failed: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("parse {} failed: {error}", path.display()))?;
    Ok(Some(rendered_receipt_from_value(&value)?))
}

fn load_optional_json(path: &Path) -> Result<Option<serde_json::Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("read {} failed: {error}", path.display()))?;
    let value = serde_json::from_str(&text)
        .map_err(|error| format!("parse {} failed: {error}", path.display()))?;
    Ok(Some(value))
}

fn build_join(
    receipts: &[FeedbackReceipt],
    route_quality: Option<&serde_json::Value>,
    live_identity: Option<&ResultIdentity>,
    route_quality_path: &Path,
) -> serde_json::Value {
    let rows = route_quality_rows(route_quality);
    let mut unmatched = Vec::new();
    let mut row_feedback: Vec<Vec<&FeedbackReceipt>> = vec![Vec::new(); rows.len()];
    let mut rate_eligible_reviewed_human_total = 0usize;
    let mut rate_eligible_reviewed_human_useful = 0usize;
    for receipt in receipts {
        let mut matched = false;
        for (index, row) in rows.iter().enumerate() {
            if receipt_matches_row(receipt, row) {
                row_feedback[index].push(receipt);
                matched = true;
            }
        }
        if !matched {
            unmatched.push(receipt);
        }
        // A receipt can appear in several aggregate rows, but contributes at
        // most once to this receipt-based rate. Raw counts below retain every
        // opinion, including rejected, unmatched, and historical feedback.
        // No comparison subject is not evidence of staleness, but it also
        // cannot establish eligibility for a current-result usefulness rate.
        if matched
            && receipt.review_status == crate::domain::ReviewStatus::ReviewedAccepted
            && receipt.review_actor_kind == Some(ActorKind::Human)
            && live_identity.is_some_and(|live| {
                classify_reference(&receipt.identity, Some(live)) == ReferenceState::Current
            })
        {
            rate_eligible_reviewed_human_total += 1;
            if receipt.judgment() == FeedbackJudgment::Useful {
                rate_eligible_reviewed_human_useful += 1;
            }
        }
    }

    let mut row_values = Vec::new();
    let mut missing_feedback_rows = 0usize;
    for (row, attached) in rows.iter().zip(row_feedback.iter()) {
        if attached.is_empty() {
            missing_feedback_rows += 1;
        }
        row_values.push(serde_json::json!({
            "repair_kind": row.repair_kind,
            "language": row.language,
            "objective": {
                "repair_kind_attempted": row.attempted,
                "repair_kind_improved": row.improved,
            },
            "feedback": feedback_counts(attached, live_identity),
        }));
    }

    let reviewed_human_total = count_reviewed_human(receipts);
    let reviewed_human_useful_rate = if rate_eligible_reviewed_human_total == 0 {
        serde_json::Value::Null
    } else {
        serde_json::json!(
            rate_eligible_reviewed_human_useful as f64 / rate_eligible_reviewed_human_total as f64
        )
    };

    serde_json::json!({
        "schema_version": FEEDBACK_SCHEMA_VERSION,
        "kind": "usefulness_feedback_join",
        "status": "advisory",
        "route_quality_path": crate::output::path::display_path(route_quality_path),
        "route_quality_present": route_quality.is_some(),
        "rate_comparison_provided": live_identity.is_some(),
        "must_not_infer": [
            "helpful feedback does not establish correctness",
            "a negative opinion does not automatically establish a false positive",
            "silence and absent feedback are not votes",
            "unreviewed agent feedback is not human-approved",
            "unreviewed, stale, unmatched, and missing-feedback states are not success percentages",
            "recording feedback does not change diagnostics, classification, baseline, suppressions, gates, or gap closure"
        ],
        "denominators": {
            "receipts_total": receipts.len(),
            "reviewed_total": receipts.iter().filter(|receipt| receipt.review_status.is_reviewed()).count(),
            "reviewed_human_total": reviewed_human_total,
            "rate_eligible_reviewed_human_total": rate_eligible_reviewed_human_total,
            "rate_eligible_reviewed_human_useful": rate_eligible_reviewed_human_useful,
            "rate_excluded_reviewed_human_total": reviewed_human_total - rate_eligible_reviewed_human_total,
            "unreviewed_total": receipts.iter().filter(|receipt| !receipt.review_status.is_reviewed()).count(),
            "actor_agent_total": receipts.iter().filter(|receipt| receipt.actor_kind == ActorKind::Agent).count(),
            "unmatched_total": unmatched.len(),
            "historical_total": receipts.iter().filter(|receipt| classify_reference(&receipt.identity, live_identity) == ReferenceState::Historical).count(),
            "mismatched_total": receipts.iter().filter(|receipt| classify_reference(&receipt.identity, live_identity) == ReferenceState::Mismatched).count(),
            "missing_feedback_rows": missing_feedback_rows
        },
        "reviewed_human_useful_rate": reviewed_human_useful_rate,
        "judgments": {
            "useful": judgment_counts(receipts, FeedbackJudgment::Useful),
            "incorrect": judgment_counts(receipts, FeedbackJudgment::Incorrect),
            "unclear": judgment_counts(receipts, FeedbackJudgment::Unclear),
            "expensive": judgment_counts(receipts, FeedbackJudgment::Expensive),
            "intentional_no_action": judgment_counts(receipts, FeedbackJudgment::IntentionalNoAction)
        },
        "rows": row_values,
        "unmatched_receipts": unmatched.iter().map(|receipt| serde_json::json!({
            "feedback_id": receipt.feedback_id,
            "idempotency_key": receipt.idempotency_key,
            "snapshot_id": receipt.identity.snapshot_id,
            "canonical_item": receipt.identity.canonical_item,
            "reason": receipt.reason.as_str(),
            "judgment": receipt.judgment().as_str(),
            "actor_kind": receipt.actor_kind.as_str(),
            "review_status": receipt.review_status.as_str(),
        })).collect::<Vec<_>>(),
        "sample_feedback_ids": receipts.iter().take(8).map(|receipt| receipt.feedback_id.clone()).collect::<Vec<_>>(),
    })
}

struct RouteQualityRow {
    repair_kind: Option<String>,
    language: Option<String>,
    attempted: i64,
    improved: i64,
    sample_canonical_gap_ids: Vec<String>,
    sample_attempt_ids: Vec<String>,
}

fn route_quality_rows(route_quality: Option<&serde_json::Value>) -> Vec<RouteQualityRow> {
    let Some(document) = route_quality else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for key in [
        "repair_route_quality_latest",
        "language_repair_route_quality_latest",
    ] {
        let Some(array) = document.get(key).and_then(|value| value.as_array()) else {
            continue;
        };
        for value in array {
            rows.push(RouteQualityRow {
                repair_kind: value
                    .get("repair_kind")
                    .and_then(|item| item.as_str())
                    .map(str::to_string),
                language: value
                    .get("language")
                    .and_then(|item| item.as_str())
                    .map(str::to_string),
                attempted: value
                    .get("repair_kind_attempted")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
                improved: value
                    .get("repair_kind_improved")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0),
                sample_canonical_gap_ids: string_array(value.get("sample_canonical_gap_ids")),
                sample_attempt_ids: string_array(value.get("sample_attempt_ids")),
            });
        }
    }
    rows
}

fn string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|item| item.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
}

fn receipt_matches_row(receipt: &FeedbackReceipt, row: &RouteQualityRow) -> bool {
    if let Some(item) = &receipt.identity.canonical_item
        && row
            .sample_canonical_gap_ids
            .iter()
            .any(|candidate| candidate == item)
    {
        return true;
    }
    if let Some(attempt) = &receipt.identity.attempt_id
        && row
            .sample_attempt_ids
            .iter()
            .any(|candidate| candidate == attempt)
    {
        return true;
    }
    // Repair-kind / route-digest coincidence is not identity: many rows share
    // a kind. Unmatched receipts stay unmatched rather than attaching to every
    // similarly named route.
    false
}

fn feedback_counts(
    receipts: &[&FeedbackReceipt],
    live_identity: Option<&ResultIdentity>,
) -> serde_json::Value {
    serde_json::json!({
        "total": receipts.len(),
        "reviewed": receipts.iter().filter(|receipt| receipt.review_status.is_reviewed()).count(),
        "reviewed_human": receipts.iter().filter(|receipt| is_reviewed_human(receipt)).count(),
        "unreviewed": receipts.iter().filter(|receipt| !receipt.review_status.is_reviewed()).count(),
        "useful": receipts.iter().filter(|receipt| receipt.judgment() == FeedbackJudgment::Useful).count(),
        "incorrect": receipts.iter().filter(|receipt| receipt.judgment() == FeedbackJudgment::Incorrect).count(),
        "unclear": receipts.iter().filter(|receipt| receipt.judgment() == FeedbackJudgment::Unclear).count(),
        "expensive": receipts.iter().filter(|receipt| receipt.judgment() == FeedbackJudgment::Expensive).count(),
        "intentional_no_action": receipts.iter().filter(|receipt| receipt.judgment() == FeedbackJudgment::IntentionalNoAction).count(),
        "historical": receipts.iter().filter(|receipt| classify_reference(&receipt.identity, live_identity) == ReferenceState::Historical).count(),
        "mismatched": receipts.iter().filter(|receipt| classify_reference(&receipt.identity, live_identity) == ReferenceState::Mismatched).count(),
        "sample_feedback_ids": receipts.iter().take(3).map(|receipt| receipt.feedback_id.clone()).collect::<Vec<_>>(),
    })
}

fn judgment_counts(receipts: &[FeedbackReceipt], judgment: FeedbackJudgment) -> serde_json::Value {
    let matching: Vec<&FeedbackReceipt> = receipts
        .iter()
        .filter(|receipt| receipt.judgment() == judgment)
        .collect();
    serde_json::json!({
        "total": matching.len(),
        "reviewed": matching.iter().filter(|receipt| receipt.review_status.is_reviewed()).count(),
        "reviewed_human": matching.iter().filter(|receipt| is_reviewed_human(receipt)).count(),
        "unreviewed": matching.iter().filter(|receipt| !receipt.review_status.is_reviewed()).count(),
    })
}

fn is_reviewed_human(receipt: &FeedbackReceipt) -> bool {
    receipt.review_status.is_reviewed() && receipt.review_actor_kind == Some(ActorKind::Human)
}

fn count_reviewed_human(receipts: &[FeedbackReceipt]) -> usize {
    receipts
        .iter()
        .filter(|receipt| is_reviewed_human(receipt))
        .count()
}

pub(crate) fn record_result_json(recorded: &RecordedFeedback) -> Result<String, String> {
    let mut document = render_receipt_document(&recorded.receipt, recorded.status.as_str())?;
    let mut value: serde_json::Value = serde_json::from_str(&document)
        .map_err(|error| format!("parse recorded feedback JSON failed: {error}"))?;
    value["path"] = serde_json::json!(crate::output::path::display_path(&recorded.path));
    document = json::render_pretty_with_newline(&value, "usefulness-feedback receipt")?;
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FeedbackReason, ReviewStatus};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Result<Self, String> {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!(
                "ripr-feedback-{label}-{}-{stamp}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).map_err(|error| format!("create temp root: {error}"))?;
            Ok(Self { path })
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn payload(snapshot: &str, item: Option<&str>, reason: FeedbackReason) -> FeedbackPayload {
        FeedbackPayload {
            identity: ResultIdentity {
                snapshot_id: snapshot.to_string(),
                canonical_item: item.map(str::to_string),
                route_digest: Some("add_missing_test".to_string()),
                attempt_id: None,
                receipt_id: None,
            },
            actor_kind: ActorKind::Human,
            review_status: ReviewStatus::Unreviewed,
            review_actor_kind: None,
            reason,
            judgment_override: None,
            note: None,
        }
    }

    fn record(
        root: &Path,
        payload: FeedbackPayload,
        key: &str,
    ) -> Result<RecordedFeedback, String> {
        record_feedback(&RecordFeedbackOptions {
            root: root.to_path_buf(),
            payload,
            idempotency_key: Some(key.to_string()),
            recorded_at: Some("unix_ms:1".to_string()),
            live_identity: None,
        })
    }

    fn seed_policy_artifacts(root: &Path) -> Result<String, String> {
        let suppressions = root.join(".ripr/suppressions.toml");
        let baseline = root.join(".ripr/gate-baseline.json");
        let gate = root.join("target/ripr/reports/gate-decision.json");
        let suppressions_parent = suppressions
            .parent()
            .ok_or_else(|| "suppressions path has no parent".to_string())?;
        let gate_parent = gate
            .parent()
            .ok_or_else(|| "gate path has no parent".to_string())?;
        fs::create_dir_all(suppressions_parent).map_err(|e| e.to_string())?;
        fs::create_dir_all(gate_parent).map_err(|e| e.to_string())?;
        fs::write(&suppressions, "[[suppression]]\nid = \"keep\"\n").map_err(|e| e.to_string())?;
        fs::write(&baseline, "{\"schema_version\":\"0.1\"}\n").map_err(|e| e.to_string())?;
        fs::write(
            &gate,
            "{\"schema_version\":\"0.1\",\"decision\":\"pass\"}\n",
        )
        .map_err(|e| e.to_string())?;
        digest_tree(root)
    }

    fn digest_tree(root: &Path) -> Result<String, String> {
        let mut hasher = Sha256::new();
        for relative in [
            ".ripr/suppressions.toml",
            ".ripr/gate-baseline.json",
            "target/ripr/reports/gate-decision.json",
        ] {
            let bytes = fs::read(root.join(relative)).map_err(|error| error.to_string())?;
            hasher.update(relative.as_bytes());
            hasher.update(&bytes);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    #[test]
    fn recording_the_same_key_and_payload_is_idempotent() -> Result<(), String> {
        let root = TempRoot::new("idempotent")?;
        let payload = payload(
            "snap-1",
            Some("gap:alpha"),
            FeedbackReason::UsefulLimitation,
        );
        let first = record(&root.path, payload.clone(), "key-one")?;
        let second = record(&root.path, payload, "key-one")?;
        assert_eq!(first.status, RecordStatus::Created);
        assert_eq!(second.status, RecordStatus::AlreadyRecorded);
        assert_eq!(first.receipt.feedback_id, second.receipt.feedback_id);
        let files: Vec<_> = fs::read_dir(root.path.join(FEEDBACK_DIRECTORY))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        assert_eq!(
            files.len(),
            1,
            "repeat recording must not create a second file"
        );
        Ok(())
    }

    #[test]
    fn matching_explicit_judgment_is_idempotent_with_the_derived_class() -> Result<(), String> {
        let root = TempRoot::new("matching-judgment")?;
        let mut explicit = payload("snap-1", None, FeedbackReason::UsefulLimitation);
        explicit.judgment_override = Some(FeedbackJudgment::Useful);
        let first = record(&root.path, explicit, "shared")?;
        let derived = payload("snap-1", None, FeedbackReason::UsefulLimitation);
        let second = record(&root.path, derived, "shared")?;
        assert_eq!(first.status, RecordStatus::Created);
        assert_eq!(second.status, RecordStatus::AlreadyRecorded);
        assert!(first.receipt.judgment_override.is_none());
        let files: Vec<_> = fs::read_dir(root.path.join(FEEDBACK_DIRECTORY))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        assert_eq!(
            files.len(),
            1,
            "repeat recording must not create a second file"
        );
        Ok(())
    }

    #[test]
    fn same_key_with_a_different_payload_is_a_conflict() -> Result<(), String> {
        let root = TempRoot::new("conflict")?;
        let first = payload(
            "snap-1",
            Some("gap:alpha"),
            FeedbackReason::UsefulLimitation,
        );
        record(&root.path, first, "shared")?;
        let mut second = payload(
            "snap-1",
            Some("gap:alpha"),
            FeedbackReason::WrongDiscriminator,
        );
        second.note = None;
        let error = record(&root.path, second, "shared")
            .err()
            .ok_or_else(|| "conflict".to_string())?;
        assert!(error.contains("idempotency conflict"));
        Ok(())
    }

    #[test]
    fn useful_limitation_does_not_require_an_attempt_or_item() -> Result<(), String> {
        let root = TempRoot::new("no-action")?;
        let recorded = record(
            &root.path,
            payload("snap-1", None, FeedbackReason::UsefulLimitation),
            "limit",
        )?;
        assert!(recorded.receipt.identity.canonical_item.is_none());
        assert!(recorded.receipt.identity.attempt_id.is_none());
        assert_eq!(recorded.receipt.judgment(), FeedbackJudgment::Useful);
        assert_eq!(recorded.receipt.reference_state, ReferenceState::Current);
        Ok(())
    }

    #[test]
    fn recording_does_not_mutate_policy_or_gate_artifacts() -> Result<(), String> {
        let root = TempRoot::new("policy")?;
        let before = seed_policy_artifacts(&root.path)?;
        record(
            &root.path,
            payload("snap-1", Some("gap:alpha"), FeedbackReason::FalseActionable),
            "policy-key",
        )?;
        let after = digest_tree(&root.path)?;
        assert_eq!(before, after, "recording must not touch policy artifacts");
        Ok(())
    }

    #[test]
    fn a_later_attempt_does_not_relabel_historical_feedback() -> Result<(), String> {
        let root = TempRoot::new("historical")?;
        let mut stored = payload("snap-1", Some("gap:alpha"), FeedbackReason::TooSlow);
        stored.identity.attempt_id = Some("attempt-a".to_string());
        let recorded = record(&root.path, stored.clone(), "hist")?;
        let mut live = stored.identity.clone();
        live.attempt_id = Some("attempt-b".to_string());
        assert_eq!(
            classify_reference(&recorded.receipt.identity, Some(&live)),
            ReferenceState::Historical
        );
        Ok(())
    }

    #[test]
    fn mismatched_snapshot_stays_visible_and_is_not_current() -> Result<(), String> {
        let stored = ResultIdentity {
            snapshot_id: "snap-old".to_string(),
            canonical_item: Some("gap:alpha".to_string()),
            route_digest: Some("add_missing_test".to_string()),
            attempt_id: None,
            receipt_id: None,
        };
        let live = ResultIdentity {
            snapshot_id: "snap-new".to_string(),
            canonical_item: Some("gap:alpha".to_string()),
            route_digest: Some("add_missing_test".to_string()),
            attempt_id: None,
            receipt_id: None,
        };
        assert_eq!(
            classify_reference(&stored, Some(&live)),
            ReferenceState::Mismatched
        );
        Ok(())
    }

    #[test]
    fn agent_feedback_is_never_counted_as_human_reviewed() -> Result<(), String> {
        let root = TempRoot::new("agent")?;
        let mut agent = payload("snap-1", Some("gap:alpha"), FeedbackReason::WrongTarget);
        agent.actor_kind = ActorKind::Agent;
        record(&root.path, agent, "agent-key")?;
        let mut human = payload(
            "snap-1",
            Some("gap:alpha"),
            FeedbackReason::UsefulActionable,
        );
        human.review_status = ReviewStatus::ReviewedAccepted;
        human.review_actor_kind = Some(ActorKind::Human);
        record(&root.path, human, "human-key")?;
        let (_, join) = export_feedback_join(&ExportFeedbackOptions {
            root: root.path.clone(),
            route_quality: None,
            live_identity: None,
        })?;
        assert_eq!(join["denominators"]["actor_agent_total"], 1);
        assert_eq!(join["denominators"]["reviewed_human_total"], 1);
        assert_eq!(join["denominators"]["unreviewed_total"], 1);
        assert!(join["reviewed_human_useful_rate"].is_null());
        assert_eq!(
            join["denominators"]["rate_eligible_reviewed_human_total"],
            0
        );
        Ok(())
    }

    #[test]
    fn unreviewed_and_missing_feedback_do_not_become_success_rates() -> Result<(), String> {
        let root = TempRoot::new("rates")?;
        record(
            &root.path,
            payload(
                "snap-1",
                Some("gap:alpha"),
                FeedbackReason::UsefulActionable,
            ),
            "unreviewed",
        )?;
        let route_quality = root.path.join("route-quality.json");
        fs::write(
            &route_quality,
            r#"{
              "repair_route_quality_latest": [
                {
                  "repair_kind": "add_missing_test",
                  "repair_kind_attempted": 2,
                  "repair_kind_improved": 1,
                  "sample_canonical_gap_ids": ["gap:other"],
                  "sample_attempt_ids": []
                }
              ]
            }"#,
        )
        .map_err(|error| error.to_string())?;
        let (_, join) = export_feedback_join(&ExportFeedbackOptions {
            root: root.path.clone(),
            route_quality: Some(route_quality),
            live_identity: None,
        })?;
        assert!(join["reviewed_human_useful_rate"].is_null());
        assert_eq!(join["denominators"]["missing_feedback_rows"], 1);
        assert_eq!(join["denominators"]["unmatched_total"], 1);
        assert!(
            join["must_not_infer"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item
                    .as_str()
                    .is_some_and(|text| text.contains("success percentages"))))
        );
        Ok(())
    }

    #[test]
    fn join_attaches_to_existing_route_quality_rows_without_a_second_ledger() -> Result<(), String>
    {
        let root = TempRoot::new("join")?;
        record(
            &root.path,
            payload(
                "snap-1",
                Some("gap:alpha"),
                FeedbackReason::UsefulActionable,
            ),
            "join-key",
        )?;
        let route_quality = root.path.join("route-quality.json");
        fs::write(
            &route_quality,
            r#"{
              "repair_route_quality_latest": [
                {
                  "repair_kind": "add_missing_test",
                  "repair_kind_attempted": 4,
                  "repair_kind_improved": 1,
                  "sample_canonical_gap_ids": ["gap:alpha"],
                  "sample_attempt_ids": []
                }
              ]
            }"#,
        )
        .map_err(|error| error.to_string())?;
        let (_, join) = export_feedback_join(&ExportFeedbackOptions {
            root: root.path.clone(),
            route_quality: Some(route_quality),
            live_identity: None,
        })?;
        assert_eq!(join["kind"], "usefulness_feedback_join");
        assert_eq!(join["rows"][0]["objective"]["repair_kind_attempted"], 4);
        assert_eq!(join["rows"][0]["feedback"]["total"], 1);
        assert_eq!(join["unmatched_receipts"].as_array().map(Vec::len), Some(0));
        Ok(())
    }

    fn reviewed_payload(reason: FeedbackReason) -> FeedbackPayload {
        let mut reviewed = payload("snap-current", Some("gap:alpha"), reason);
        reviewed.review_status = ReviewStatus::ReviewedAccepted;
        reviewed.review_actor_kind = Some(ActorKind::Human);
        reviewed
    }

    fn rate_rows() -> serde_json::Value {
        serde_json::json!({
            "repair_route_quality_latest": [{
                "repair_kind": "add_missing_test",
                "repair_kind_attempted": 4,
                "repair_kind_improved": 1,
                "sample_canonical_gap_ids": ["gap:alpha"]
            }],
            "language_repair_route_quality_latest": [{
                "repair_kind": "add_missing_test",
                "language": "rust",
                "repair_kind_attempted": 4,
                "repair_kind_improved": 1,
                "sample_canonical_gap_ids": ["gap:alpha"]
            }]
        })
    }

    #[test]
    fn usefulness_rate_requires_a_comparison_and_a_matching_row() -> Result<(), String> {
        let root = TempRoot::new("rate-subject")?;
        let reviewed = reviewed_payload(FeedbackReason::UsefulActionable);
        let live = reviewed.identity.clone();
        let receipt = record(&root.path, reviewed, "reviewed")?.receipt;
        let rows = rate_rows();
        let path = root.path.join("route-quality.json");
        for (label, document, comparison) in [
            ("no comparison", Some(&rows), None),
            ("no route document", None, Some(&live)),
        ] {
            let join = build_join(std::slice::from_ref(&receipt), document, comparison, &path);
            assert!(join["reviewed_human_useful_rate"].is_null(), "{label}");
            assert_eq!(join["denominators"]["reviewed_human_total"], 1, "{label}");
            assert_eq!(
                join["denominators"]["rate_eligible_reviewed_human_total"], 0,
                "{label}"
            );
            assert_eq!(
                join["denominators"]["rate_excluded_reviewed_human_total"], 1,
                "{label}"
            );
        }
        let unrelated_rows = serde_json::json!({
            "repair_route_quality_latest": [{
                "sample_canonical_gap_ids": ["gap:unrelated"]
            }]
        });
        let join = build_join(&[receipt], Some(&unrelated_rows), Some(&live), &path);
        assert!(join["reviewed_human_useful_rate"].is_null());
        assert_eq!(join["denominators"]["unmatched_total"], 1);
        assert_eq!(join["denominators"]["missing_feedback_rows"], 1);
        Ok(())
    }

    #[test]
    fn usefulness_rate_excludes_each_ineligible_review_without_losing_counts() -> Result<(), String>
    {
        let root = TempRoot::new("rate-exclusions")?;
        let valid = reviewed_payload(FeedbackReason::UsefulActionable);
        let live = valid.identity.clone();
        let mut historical = valid.clone();
        historical.identity.attempt_id = Some("older-attempt".to_string());
        let mut mismatched = valid.clone();
        mismatched.identity.snapshot_id = "older-snapshot".to_string();
        let mut rejected = valid.clone();
        rejected.review_status = ReviewStatus::ReviewedRejected;
        let mut agent_review = valid.clone();
        agent_review.review_actor_kind = Some(ActorKind::Agent);
        let unreviewed = payload(
            "snap-current",
            Some("gap:alpha"),
            FeedbackReason::UsefulActionable,
        );
        let rows = rate_rows();
        for (key, feedback, human_total) in [
            ("historical", historical, 1),
            ("mismatched", mismatched, 1),
            ("rejected", rejected, 1),
            ("agent-review", agent_review, 0),
            ("unreviewed", unreviewed, 0),
        ] {
            let receipt = record(&root.path, feedback, key)?.receipt;
            let join = build_join(&[receipt], Some(&rows), Some(&live), &root.path);
            assert!(join["reviewed_human_useful_rate"].is_null(), "{key}");
            assert_eq!(join["denominators"]["receipts_total"], 1, "{key}");
            assert_eq!(
                join["denominators"]["reviewed_human_total"], human_total,
                "{key}"
            );
            assert_eq!(
                join["denominators"]["rate_eligible_reviewed_human_total"], 0,
                "{key}"
            );
            assert_eq!(join["judgments"]["useful"]["total"], 1, "{key}");
        }
        Ok(())
    }

    #[test]
    fn usefulness_rate_counts_eligible_receipts_once_across_aggregate_rows() -> Result<(), String> {
        let root = TempRoot::new("rate-mixed")?;
        let before_policy = seed_policy_artifacts(&root.path)?;
        let useful = reviewed_payload(FeedbackReason::UsefulActionable);
        let live = useful.identity.clone();
        record(&root.path, useful.clone(), "useful")?;
        record(
            &root.path,
            reviewed_payload(FeedbackReason::WrongTarget),
            "incorrect",
        )?;
        let mut stale = useful.clone();
        stale.identity.snapshot_id = "old-snapshot".to_string();
        record(&root.path, stale, "stale")?;
        let mut historical = useful.clone();
        historical.identity.attempt_id = Some("old-attempt".to_string());
        record(&root.path, historical, "historical")?;
        let mut rejected = useful;
        rejected.review_status = ReviewStatus::ReviewedRejected;
        record(&root.path, rejected, "rejected")?;
        let route_quality = root.path.join("route-quality.json");
        fs::write(&route_quality, rate_rows().to_string()).map_err(|error| error.to_string())?;
        let (rendered, join) = export_feedback_join(&ExportFeedbackOptions {
            root: root.path.clone(),
            route_quality: Some(route_quality),
            live_identity: Some(live),
        })?;
        let parsed: serde_json::Value =
            serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
        assert_eq!(parsed, join);
        assert_eq!(join["rate_comparison_provided"], true);
        assert_eq!(join["denominators"]["receipts_total"], 5);
        assert_eq!(join["denominators"]["reviewed_human_total"], 5);
        assert_eq!(
            join["denominators"]["rate_eligible_reviewed_human_total"],
            2
        );
        assert_eq!(
            join["denominators"]["rate_eligible_reviewed_human_useful"],
            1
        );
        assert_eq!(
            join["denominators"]["rate_excluded_reviewed_human_total"],
            3
        );
        assert_eq!(join["reviewed_human_useful_rate"], 0.5);
        assert_eq!(join["denominators"]["historical_total"], 1);
        assert_eq!(join["denominators"]["mismatched_total"], 1);
        assert_eq!(join["rows"].as_array().map(Vec::len), Some(2));
        assert_eq!(join["rows"][0]["objective"]["repair_kind_attempted"], 4);
        assert_eq!(join["rows"][0]["objective"]["repair_kind_improved"], 1);
        assert_eq!(join["rows"][0]["feedback"]["total"], 5);
        assert_eq!(join["rows"][1]["feedback"]["total"], 5);
        assert_eq!(before_policy, digest_tree(&root.path)?);
        Ok(())
    }

    #[test]
    fn usefulness_rate_keeps_a_current_negative_opinion_in_the_denominator() -> Result<(), String> {
        let root = TempRoot::new("rate-negative")?;
        let negative = reviewed_payload(FeedbackReason::WrongDiscriminator);
        let live = negative.identity.clone();
        let receipt = record(&root.path, negative, "negative")?.receipt;
        let join = build_join(&[receipt], Some(&rate_rows()), Some(&live), &root.path);
        assert_eq!(
            join["denominators"]["rate_eligible_reviewed_human_total"],
            1
        );
        assert_eq!(
            join["denominators"]["rate_eligible_reviewed_human_useful"],
            0
        );
        assert_eq!(join["reviewed_human_useful_rate"], 0.0);
        Ok(())
    }

    #[test]
    fn path_escape_and_secret_notes_fail_closed() -> Result<(), String> {
        let root = TempRoot::new("escape")?;
        let mut secret = payload("snap-1", None, FeedbackReason::Other);
        secret.judgment_override = Some(FeedbackJudgment::Unclear);
        secret.note = Some("token ghp_notarealsecretvalue".to_string());
        let error = record(&root.path, secret, "secret")
            .err()
            .ok_or_else(|| "secret note".to_string())?;
        assert!(error.contains("secret pattern"));

        let escaped = record_feedback(&RecordFeedbackOptions {
            root: root.path.clone(),
            payload: payload("snap-1", None, FeedbackReason::UsefulLimitation),
            idempotency_key: Some("../outside".to_string()),
            recorded_at: Some("unix_ms:1".to_string()),
            live_identity: None,
        })
        .err()
        .ok_or_else(|| "escaped key".to_string())?;
        assert!(
            escaped.contains("idempotency-key") || escaped.contains("escaped"),
            "{escaped}"
        );
        Ok(())
    }

    #[test]
    fn oversized_notes_fail_closed() -> Result<(), String> {
        let root = TempRoot::new("oversize")?;
        let mut payload = payload("snap-1", None, FeedbackReason::UnclearExplanation);
        payload.note = Some("n".repeat(FEEDBACK_NOTE_MAX_BYTES + 1));
        let error = record(&root.path, payload, "big")
            .err()
            .ok_or_else(|| "oversize".to_string())?;
        assert!(error.contains("exceeds"));
        Ok(())
    }

    #[test]
    fn leftover_temp_files_are_not_loaded_as_receipts() -> Result<(), String> {
        let root = TempRoot::new("tmp")?;
        record(
            &root.path,
            payload("snap-1", None, FeedbackReason::UsefulLimitation),
            "real",
        )?;
        let directory = root.path.join(FEEDBACK_DIRECTORY);
        fs::write(
            directory.join(".ripr-atomic-interrupted.tmp"),
            "{not-a-receipt",
        )
        .map_err(|error| error.to_string())?;
        let receipts = load_feedback_receipts(&root.path)?;
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].idempotency_key, "real");
        Ok(())
    }

    #[test]
    fn malformed_receipt_fails_closed() -> Result<(), String> {
        let root = TempRoot::new("malformed")?;
        let directory = root.path.join(FEEDBACK_DIRECTORY);
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        fs::write(directory.join("bad.json"), "{\"kind\":\"gate-decision\"}\n")
            .map_err(|error| error.to_string())?;
        let error = load_feedback_receipts(&root.path)
            .err()
            .ok_or_else(|| "malformed".to_string())?;
        assert!(error.contains("usefulness_feedback_receipt"));
        Ok(())
    }

    #[test]
    fn production_source_does_not_reach_policy_process_or_network_surfaces() {
        let source = include_str!("feedback.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        let spawn = concat!("Command", "::", "new");
        let process = concat!("std::", "process");
        let tcp = concat!("Tcp", "Stream");
        let udp = concat!("Udp", "Socket");
        let http = concat!("req", "west");
        for needle in [
            "output::gate",
            "output::suppressions",
            "output::baseline",
            spawn,
            process,
            tcp,
            udp,
            http,
        ] {
            assert!(
                !production.contains(needle),
                "feedback recorder production path contains {needle}"
            );
        }
        assert!(
            production.contains("atomic_file::write"),
            "recording must use the shared atomic writer"
        );
    }
}
