//! Source-state subject stamp for editor-consumed gap artifacts (#4544).
//!
//! A gap decision ledger or actionable-gaps report names files and lines in
//! the workspace it was computed from. Nothing about a line number says which
//! revision it belongs to, so after a branch switch, edit, or commit an editor
//! consumer could place branch A's gap at `anchor.file:anchor.line` on branch
//! B's file and present it as current.
//!
//! The stamp is taken where the analysis runs, not where a derived report is
//! written: `ripr check --format json` and `--format repo-exposure-json` add a
//! `source_subject` with the content digest of every file their gap-bearing
//! output names, read in the analysis run. The gap ledger writer and the xtask
//! actionable-gaps writer only copy digests from that input stamp
//! ([`derive_source_subject`]); they never hash the workspace, so a report
//! written after a checkout or an edit cannot vouch for records computed
//! before it. The LSP validator in `lsp::gap_artifacts` recomputes the
//! digests from the current workspace and is the only consumer authority.

mod shared;

pub(crate) use shared::{
    GapSourceSubject, GapSourceSubjectFile, SOURCE_SUBJECT_DIGEST_ALGORITHM,
    actionable_packet_named_paths, actionable_packet_subject_paths, derive_source_subject,
    subject_relative_path,
};

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::Path;

/// The result of comparing a stamp with the current workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceSubjectCheck {
    /// Every stamped file still has its stamped content.
    Current,
    /// A stamped file changed, was deleted, or appeared since the analysis
    /// read it. Carries the repo-relative path of the first differing file.
    Stale(String),
    /// The artifact cannot be matched to the current workspace: the stamp is
    /// missing, malformed, uses an unknown digest, omits a file the records
    /// name, or a stamped file cannot be read.
    Unverifiable(&'static str),
}

/// `sha256:<hex>` of one workspace file, `Ok(None)` when it does not exist.
pub(crate) fn source_file_digest(root: &Path, relative: &str) -> Result<Option<String>, String> {
    match std::fs::read(root.join(relative)) {
        Ok(bytes) => Ok(Some(format!("sha256:{:x}", Sha256::digest(bytes)))),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("read source subject file {relative} failed: {err}")),
    }
}

/// Files a gap decision ledger record makes claims about: the anchor the
/// diagnostic is placed on and the repair route's target and related test.
pub(crate) fn gap_record_subject_paths(root: &Path, record: &Value) -> BTreeSet<String> {
    [
        &["anchor", "file"][..],
        &["repair_route", "target_file"][..],
        &["repair_route", "related_test"][..],
    ]
    .iter()
    .filter_map(|path| {
        path.iter()
            .try_fold(record, |value, key| value.get(*key))
            .and_then(Value::as_str)
    })
    .filter_map(|raw| subject_relative_path(root, raw))
    .collect()
}

/// Every workspace file an analysis output value names under a path-bearing
/// key (`file`, `path`, `source_file`, `target_file`, `target_test`,
/// `related_test`, `test`, `related_test_or_observer`), at any depth. Used by
/// the repo-exposure producer so its stamp covers whatever a derived ledger
/// record or actionable-gaps packet later names from the same evidence.
pub(crate) fn named_files_in_value(root: &Path, value: &Value, files: &mut BTreeSet<String>) {
    const PATH_KEYS: [&str; 8] = [
        "file",
        "path",
        "source_file",
        "target_file",
        "target_test",
        "related_test",
        "test",
        "related_test_or_observer",
    ];
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if PATH_KEYS.contains(&key.as_str()) {
                    collect_path_strings(root, child, files);
                }
                named_files_in_value(root, child, files);
            }
        }
        Value::Array(values) => {
            for child in values {
                named_files_in_value(root, child, files);
            }
        }
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn collect_path_strings(root: &Path, value: &Value, files: &mut BTreeSet<String>) {
    match value {
        Value::String(raw) => {
            if let Some(path) = subject_relative_path(root, raw) {
                files.insert(path);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_path_strings(root, child, files);
            }
        }
        Value::Object(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Stamp the given repo-relative files with their current content digests.
/// Only analysis producers call this, in the run that read the files.
pub(crate) fn stamp_source_subject(
    root: &Path,
    paths: &BTreeSet<String>,
) -> Result<GapSourceSubject, String> {
    let files = paths
        .iter()
        .map(|path| {
            Ok(GapSourceSubjectFile {
                path: path.clone(),
                digest: source_file_digest(root, path)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(GapSourceSubject {
        digest_algorithm: SOURCE_SUBJECT_DIGEST_ALGORITHM.to_string(),
        files,
    })
}

/// Append a top-level `"source_subject"` member to a rendered JSON object
/// document, keeping the document's own formatting. Returns the document
/// unchanged when there is nothing to stamp or a file cannot be read, so a
/// derived report falls back to `unverifiable_subject` instead of carrying a
/// partial stamp.
pub(crate) fn append_source_subject_member(
    rendered: String,
    root: &Path,
    paths: &BTreeSet<String>,
) -> String {
    if paths.is_empty() {
        return rendered;
    }
    let Ok(stamp) = stamp_source_subject(root, paths) else {
        return rendered;
    };
    let Ok(stamp) = serde_json::to_string(&stamp) else {
        return rendered;
    };
    let trimmed = rendered.trim_end();
    let Some(body) = trimmed.strip_suffix('}') else {
        return rendered;
    };
    let body = body.trim_end();
    let separator = if body.ends_with('{') { "" } else { "," };
    let newline = if rendered.ends_with('\n') { "\n" } else { "" };
    format!("{body}{separator}\n  \"source_subject\": {stamp}\n}}{newline}")
}

/// Compare an artifact's `source_subject` with the current workspace.
/// `required` is every file the artifact's records name; each must be
/// stamped, so a stamp cannot vouch for a subset of the claim.
pub(crate) fn check_source_subject(
    root: &Path,
    stamp: Option<&Value>,
    required: &BTreeSet<String>,
) -> SourceSubjectCheck {
    let Some(stamp) = stamp else {
        return SourceSubjectCheck::Unverifiable("source_subject_missing");
    };
    let Ok(subject) = serde_json::from_value::<GapSourceSubject>(stamp.clone()) else {
        return SourceSubjectCheck::Unverifiable("source_subject_malformed");
    };
    if subject.digest_algorithm != SOURCE_SUBJECT_DIGEST_ALGORITHM {
        return SourceSubjectCheck::Unverifiable("source_subject_unsupported_digest");
    }
    let mut stamped = BTreeSet::new();
    for file in &subject.files {
        if subject_relative_path(root, &file.path).as_deref() != Some(file.path.as_str()) {
            return SourceSubjectCheck::Unverifiable("source_subject_malformed");
        }
        stamped.insert(file.path.as_str());
    }
    if required.iter().any(|path| !stamped.contains(path.as_str())) {
        return SourceSubjectCheck::Unverifiable("source_subject_incomplete");
    }
    for file in &subject.files {
        match source_file_digest(root, &file.path) {
            Ok(current) if current == file.digest => {}
            Ok(_) => return SourceSubjectCheck::Stale(file.path.clone()),
            Err(_) => return SourceSubjectCheck::Unverifiable("source_subject_unreadable"),
        }
    }
    SourceSubjectCheck::Current
}

#[cfg(test)]
pub(crate) fn with_source_subject_for_test(root: &Path, mut artifact: Value) -> Value {
    let mut paths = BTreeSet::new();
    for key in ["records", "gap_records"] {
        if let Some(records) = artifact.get(key).and_then(Value::as_array) {
            for record in records {
                paths.extend(gap_record_subject_paths(root, record));
            }
        }
    }
    if let Some(packets) = artifact.get("packets").and_then(Value::as_array) {
        for packet in packets {
            paths.extend(actionable_packet_subject_paths(root, packet));
        }
    }
    if let (Ok(stamp), Some(object)) = (
        stamp_source_subject(root, &paths)
            .and_then(|stamp| serde_json::to_value(stamp).map_err(|err| err.to_string())),
        artifact.as_object_mut(),
    ) {
        object.insert("source_subject".to_string(), stamp);
    }
    artifact
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> Result<std::path::PathBuf, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system time before epoch: {err}"))?;
        let root = std::env::temp_dir().join(format!(
            "ripr-gap-source-subject-{label}-{}-{}",
            std::process::id(),
            now.as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("mkdir: {err}"))?;
        Ok(root)
    }

    fn stamp_value(root: &Path, paths: &BTreeSet<String>) -> Result<Value, String> {
        serde_json::to_value(stamp_source_subject(root, paths)?).map_err(|err| err.to_string())
    }

    #[test]
    fn subject_relative_path_normalizes_selectors_and_rejects_escape() {
        let root = Path::new("/repo");
        assert_eq!(
            subject_relative_path(root, "tests/pricing.rs::discount_threshold"),
            Some("tests/pricing.rs".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "./src\\lib.rs"),
            Some("src/lib.rs".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "/repo/src/lib.rs"),
            Some("src/lib.rs".to_string())
        );
        assert_eq!(subject_relative_path(root, "/elsewhere/src/lib.rs"), None);
        assert_eq!(subject_relative_path(root, "../src/lib.rs"), None);
        assert_eq!(subject_relative_path(root, "  "), None);
        assert_eq!(subject_relative_path(root, ""), None);
        assert_eq!(subject_relative_path(root, "not a path"), None);
    }

    #[test]
    fn subject_relative_path_preserves_whitespace_identity() {
        let root = Path::new("/repo");
        assert_eq!(
            subject_relative_path(root, " leading.py"),
            Some(" leading.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "leading.py"),
            Some("leading.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "leading.py "),
            Some("leading.py ".to_string())
        );
        assert_eq!(
            subject_relative_path(root, " spaced/discount.py"),
            Some(" spaced/discount.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "foo bar.py"),
            Some("foo bar.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, " quoted\t.py"),
            Some(" quoted\t.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, " leading.py::discount_threshold"),
            Some(" leading.py".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "./ spaced\\file.rs"),
            Some(" spaced/file.rs".to_string())
        );
        assert_eq!(
            subject_relative_path(root, "/repo/ leading.py"),
            Some(" leading.py".to_string())
        );
        assert_ne!(
            subject_relative_path(root, " leading.py"),
            subject_relative_path(root, "leading.py")
        );
    }

    #[test]
    fn subject_relative_path_rejects_prefix_and_root_escape() {
        let root = Path::new("/repo");
        assert_eq!(subject_relative_path(root, "/outside.py"), None);
        assert_eq!(subject_relative_path(root, "../outside.py"), None);
        // `check-local-context` forbids contiguous drive-letter path literals.
        let drive_relative = format!("{}:outside.py", 'C');
        let drive_absolute = format!("{}:/outside.py", 'C');
        if cfg!(windows) {
            assert_eq!(
                subject_relative_path(root, &drive_relative),
                None,
                "drive-relative identity must not join outside the workspace"
            );
            assert_eq!(subject_relative_path(root, &drive_absolute), None);
            assert_eq!(
                check_source_subject(
                    root,
                    Some(&json!({
                        "digest_algorithm": "sha256",
                        "files": [{"path": drive_relative, "digest": null}]
                    })),
                    &BTreeSet::new()
                ),
                SourceSubjectCheck::Unverifiable("source_subject_malformed"),
                "currentness must fail closed before reading a prefixed stamp"
            );
        } else {
            assert_eq!(
                subject_relative_path(root, &drive_relative).as_deref(),
                Some(drive_relative.as_str()),
                "a drive-letter lookalike is an ordinary Unix filename"
            );
            assert_eq!(
                subject_relative_path(root, &drive_absolute).as_deref(),
                Some(drive_absolute.as_str()),
                "a drive-absolute lookalike is a Unix directory named like a drive"
            );
        }
    }

    #[test]
    fn source_subject_stays_current_until_a_stamped_file_changes() -> Result<(), String> {
        let root = temp_root("edit")?;
        std::fs::write(root.join("src/lib.rs"), "fn a() {}\n").map_err(|err| err.to_string())?;
        let paths = BTreeSet::from(["src/lib.rs".to_string()]);
        let stamp = stamp_value(&root, &paths)?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Current
        );
        std::fs::write(root.join("src/lib.rs"), "fn b() {}\n").map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale("src/lib.rs".to_string())
        );
        std::fs::remove_file(root.join("src/lib.rs")).map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale("src/lib.rs".to_string())
        );
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn source_subject_keeps_whitespace_paths_and_stales_only_the_changed_identity()
    -> Result<(), String> {
        let root = temp_root("whitespace-identity")?;
        let spaced = " leading.py";
        let plain = "leading.py";
        let nested = " spaced/discount.py";
        std::fs::create_dir_all(root.join(" spaced")).map_err(|err| err.to_string())?;
        std::fs::write(root.join(spaced), "def spaced():\n    return 1\n")
            .map_err(|err| err.to_string())?;
        std::fs::write(root.join(plain), "def plain():\n    return 2\n")
            .map_err(|err| err.to_string())?;
        std::fs::write(root.join(nested), "def nested():\n    return 3\n")
            .map_err(|err| err.to_string())?;

        let paths = BTreeSet::from([spaced.to_string(), plain.to_string(), nested.to_string()]);
        let spaced_only = BTreeSet::from([spaced.to_string()]);
        let stamp = stamp_value(&root, &paths)?;
        let spaced_stamp = stamp_value(&root, &spaced_only)?;
        let stamped_paths: Vec<&str> = stamp["files"]
            .as_array()
            .ok_or("stamp files missing")?
            .iter()
            .filter_map(|file| file["path"].as_str())
            .collect();
        assert_eq!(
            stamped_paths,
            vec![spaced, nested, plain],
            "BTreeSet order is lexical; identities must stay exact"
        );
        assert_ne!(stamp["files"][0]["digest"], stamp["files"][2]["digest"]);
        assert_eq!(
            stamp["files"][0]["digest"].as_str(),
            source_file_digest(&root, spaced)?.as_deref()
        );
        assert_eq!(
            stamp["files"][2]["digest"].as_str(),
            source_file_digest(&root, plain)?.as_deref()
        );
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Current
        );

        std::fs::write(root.join(plain), "def plain():\n    return 9\n")
            .map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&spaced_stamp), &spaced_only),
            SourceSubjectCheck::Current,
            "editing the namesake must not stale the whitespace-bearing subject"
        );
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale(plain.to_string())
        );

        std::fs::write(root.join(plain), "def plain():\n    return 2\n")
            .map_err(|err| err.to_string())?;
        std::fs::write(root.join(spaced), "def spaced():\n    return 8\n")
            .map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale(spaced.to_string())
        );

        let named = json!({
            "file": spaced,
            "related_test": format!("{plain}::discount_threshold"),
            "nested": {"path": nested}
        });
        let mut collected = BTreeSet::new();
        named_files_in_value(&root, &named, &mut collected);
        assert_eq!(collected, paths);

        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn source_subject_absent_file_stamp_goes_stale_when_the_file_appears() -> Result<(), String> {
        let root = temp_root("appear")?;
        let paths = BTreeSet::from(["src/new.rs".to_string()]);
        let stamp = stamp_value(&root, &paths)?;
        assert_eq!(stamp["files"][0]["digest"], Value::Null);
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Current
        );
        std::fs::write(root.join("src/new.rs"), "fn c() {}\n").map_err(|err| err.to_string())?;
        assert_eq!(
            check_source_subject(&root, Some(&stamp), &paths),
            SourceSubjectCheck::Stale("src/new.rs".to_string())
        );
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn source_subject_missing_malformed_or_incomplete_is_unverifiable() -> Result<(), String> {
        let root = temp_root("unverifiable")?;
        std::fs::write(root.join("src/lib.rs"), "fn a() {}\n").map_err(|err| err.to_string())?;
        let paths = BTreeSet::from(["src/lib.rs".to_string()]);
        assert_eq!(
            check_source_subject(&root, None, &paths),
            SourceSubjectCheck::Unverifiable("source_subject_missing")
        );
        assert_eq!(
            check_source_subject(&root, Some(&json!({"files": "x"})), &paths),
            SourceSubjectCheck::Unverifiable("source_subject_malformed")
        );
        assert_eq!(
            check_source_subject(
                &root,
                Some(&json!({"digest_algorithm": "md5", "files": []})),
                &paths
            ),
            SourceSubjectCheck::Unverifiable("source_subject_unsupported_digest")
        );
        assert_eq!(
            check_source_subject(
                &root,
                Some(&json!({"digest_algorithm": "sha256", "files": []})),
                &paths
            ),
            SourceSubjectCheck::Unverifiable("source_subject_incomplete")
        );
        assert_eq!(
            check_source_subject(
                &root,
                Some(&json!({
                    "digest_algorithm": "sha256",
                    "files": [{"path": "../outside.rs", "digest": null}]
                })),
                &BTreeSet::new()
            ),
            SourceSubjectCheck::Unverifiable("source_subject_malformed")
        );
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn subject_paths_name_the_anchor_route_and_packet_files() {
        let root = Path::new("/repo");
        let record = json!({
            "anchor": {"file": "src/pricing.rs", "line": 42},
            "repair_route": {
                "target_file": "tests/pricing.rs",
                "related_test": "tests/pricing.rs::discount_threshold"
            }
        });
        assert_eq!(
            gap_record_subject_paths(root, &record),
            BTreeSet::from(["src/pricing.rs".to_string(), "tests/pricing.rs".to_string()])
        );
        let packet = json!({
            "source_file": "src/pricing.rs",
            "primary_anchor": {"file": "src/lib.rs", "line": 3}
        });
        assert_eq!(
            actionable_packet_subject_paths(root, &packet),
            BTreeSet::from(["src/lib.rs".to_string(), "src/pricing.rs".to_string()])
        );
    }

    #[test]
    fn packet_subject_paths_cover_every_related_test_or_observer_shape() {
        let root = Path::new("/repo");
        for (shape, observer) in [
            ("string", json!("tests/observer.rs::observes")),
            (
                "object",
                json!({"file": "tests/observer.rs", "test": "observes"}),
            ),
            (
                "array",
                json!([
                    "not a path",
                    {"related_test": "tests/observer.rs::observes"},
                    ["tests/nested.rs::deeper"]
                ]),
            ),
        ] {
            let packet = json!({
                "source_file": "src/pricing.rs",
                "target_test": "tests/pricing.rs::discount_threshold",
                "target_file": "tests/pricing_extra.rs",
                "related_test_or_observer": observer
            });
            let paths = actionable_packet_subject_paths(root, &packet);
            assert!(paths.contains("tests/observer.rs"), "{shape}: {paths:?}");
            assert!(paths.contains("src/pricing.rs"), "{shape}: {paths:?}");
            assert!(paths.contains("tests/pricing.rs"), "{shape}: {paths:?}");
            assert!(
                paths.contains("tests/pricing_extra.rs"),
                "{shape}: {paths:?}"
            );
            assert_eq!(
                shape == "array",
                paths.contains("tests/nested.rs"),
                "{shape}"
            );
        }
    }

    #[test]
    fn absolute_paths_resolve_against_a_relative_root() -> Result<(), String> {
        // `cargo test` runs in the crate directory, which holds `src/lib.rs`.
        let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
        let absolute = cwd.join("src/lib.rs").display().to_string();
        assert_eq!(
            subject_relative_path(Path::new("."), &absolute).as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(
            subject_relative_path(Path::new("src/.."), &absolute).as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(
            subject_relative_path(Path::new("src"), &absolute).as_deref(),
            Some("lib.rs")
        );
        Ok(())
    }

    #[test]
    fn derive_copies_digests_and_never_hashes() -> Result<(), String> {
        let input = json!({
            "digest_algorithm": "sha256",
            "files": [
                {"path": "src/lib.rs", "digest": "sha256:aa"},
                {"path": "tests/it.rs", "digest": null},
                {"path": "src/unused.rs", "digest": "sha256:bb"}
            ]
        });
        let required = BTreeSet::from(["src/lib.rs".to_string(), "tests/it.rs".to_string()]);
        // `/nonexistent` cannot be read, so a copied digest cannot come from hashing.
        let root = Path::new("/nonexistent-ripr-root");
        let derived = derive_source_subject(Some(&input), root, root, &required)?;
        assert_eq!(
            serde_json::to_value(&derived).map_err(|err| err.to_string())?,
            json!({
                "digest_algorithm": "sha256",
                "files": [
                    {"path": "src/lib.rs", "digest": "sha256:aa"},
                    {"path": "tests/it.rs", "digest": null}
                ]
            })
        );
        // An input produced under a parent root is rebased onto the output root.
        let rebased = derive_source_subject(
            Some(&json!({
                "digest_algorithm": "sha256",
                "files": [{"path": "crate/src/lib.rs", "digest": "sha256:cc"}]
            })),
            Path::new("/nonexistent-ripr-root"),
            Path::new("/nonexistent-ripr-root/crate"),
            &BTreeSet::from(["src/lib.rs".to_string()]),
        )?;
        assert_eq!(rebased.files[0].digest.as_deref(), Some("sha256:cc"));

        for (stamp, reason) in [
            (None, "input_source_subject_missing"),
            (Some(json!({"files": 1})), "input_source_subject_malformed"),
            (
                Some(json!({"digest_algorithm": "md5", "files": []})),
                "input_source_subject_unsupported_digest",
            ),
            (
                Some(json!({"digest_algorithm": "sha256", "files": [
                    {"path": "src/lib.rs", "digest": "sha256:aa"}
                ]})),
                "input_source_subject_incomplete",
            ),
        ] {
            assert_eq!(
                derive_source_subject(stamp.as_ref(), root, root, &required),
                Err(reason)
            );
        }
        assert_eq!(
            derive_source_subject(None, root, root, &BTreeSet::new()).map(|s| s.files.len()),
            Ok(0)
        );

        let whitespace = json!({
            "digest_algorithm": "sha256",
            "files": [{"path": " leading.py", "digest": "sha256:aa"}]
        });
        let required_whitespace = BTreeSet::from([" leading.py".to_string()]);
        let derived_whitespace =
            derive_source_subject(Some(&whitespace), root, root, &required_whitespace)?;
        assert_eq!(derived_whitespace.files[0].path, " leading.py");
        assert_eq!(
            derived_whitespace.files[0].digest.as_deref(),
            Some("sha256:aa")
        );
        Ok(())
    }

    #[test]
    fn append_source_subject_member_stamps_named_files_or_leaves_the_document() -> Result<(), String>
    {
        let root = temp_root("append")?;
        std::fs::write(root.join("src/lib.rs"), "abc\n").map_err(|err| err.to_string())?;
        let paths = BTreeSet::from(["src/lib.rs".to_string()]);
        let rendered = "{\n  \"schema_version\": \"0.1\"\n}\n".to_string();
        let stamped = append_source_subject_member(rendered.clone(), &root, &paths);
        let value: Value = serde_json::from_str(&stamped).map_err(|err| err.to_string())?;
        assert_eq!(value["schema_version"], json!("0.1"));
        assert_eq!(value["source_subject"], stamp_value(&root, &paths)?);
        assert!(stamped.ends_with("}\n"));
        assert_eq!(
            append_source_subject_member(rendered.clone(), &root, &BTreeSet::new()),
            rendered
        );
        assert_eq!(
            append_source_subject_member("[]".to_string(), &root, &paths),
            "[]"
        );

        let mut named = BTreeSet::new();
        named_files_in_value(
            &root,
            &json!({
                "file": "src/lib.rs",
                "evidence": [{"related_test": "tests/it.rs::case", "name": "src/ignored.rs"}],
                "related_tests": [{"test": "not a path"}]
            }),
            &mut named,
        );
        assert_eq!(
            named,
            BTreeSet::from(["src/lib.rs".to_string(), "tests/it.rs".to_string()])
        );
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }
}
