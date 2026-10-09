//! The accepted Python eval-sweep manifest: loader and validator for the
//! canonical denominator (RIPR-SPEC-0086). The manifest is the accepted
//! contract: exactly eight uniquely identified subjects, a closed
//! deny-unknown schema, immutable repository identity (https url, pinned
//! 40-character sha), license and shape vocabularies, portable diff paths,
//! and optional identity fields whose absence is typed `incomplete` and
//! whose malformed presence fails. One loader owns these semantics:
//! `validate_accepted_manifest` is shared by check, refresh, and report, so
//! no route re-parses the manifest.

use std::collections::BTreeSet;

use serde_json::Value;

use super::{
    Diagnostic, KNOWN_SPEC, KNOWN_TIER, MANIFEST_SCHEMA_VERSION, as_object, check_git_sha,
    check_no_secrets, check_portable_path, check_sha256_digest, check_subject_url, fail,
    known_value_or_fail, opt_string, opt_string_array, reject_unknown_keys,
};

const MANIFEST_KIND: &str = "python_eval_sweep_manifest";
/// The closed accepted-manifest schema (deny-unknown). The canonical
/// `fixtures/python-eval-sweep/manifest.json` carries exactly the top-level
/// keys below, so every key it carries is owned and unknown keys are schema
/// rot, not forward compatibility.
const MANIFEST_KEYS: [&str; 8] = [
    "schema_version",
    "kind",
    "spec",
    "tier",
    "description",
    "limits",
    "synthetic_diff",
    "repos",
];
/// Owned per-subject keys: everything the canonical repos carry plus the
/// optional identity fields whose absence is typed incomplete.
const MANIFEST_REPO_KEYS: [&str; 11] = [
    "id",
    "url",
    "sha",
    "license",
    "shape",
    "synthetic_diff",
    "why",
    "tree_digest",
    "snapshot",
    "provenance",
    "retention_class",
];
const ACCEPTED_SUBJECT_COUNT: usize = 8;
/// Subject shape/layout tags (fixtures/python-eval-sweep/SPEC.md).
pub(super) const KNOWN_SHAPES: [&str; 5] = [
    "pytest_library",
    "unittest_library",
    "click_typer",
    "fastapi_web",
    "flask_web",
];
/// Subject ids become filesystem path components — the candidate subject dirs
/// and the raw/cache dirs the refresh route (#3566) derives from them — so
/// they must be safe identifiers: at most 64 characters, limited to
/// `[A-Za-z0-9._-]`, and never starting with `.` (which also bars `.`/`..`
/// and hidden components). The character set bars `/` and `\\`, so a hostile
/// id like `../../outside` fails here as a named diagnostic instead of
/// escaping candidate storage downstream.
const SUBJECT_ID_MAX_LENGTH: usize = 64;

fn check_subject_id(id: &str) -> Result<(), String> {
    if id.len() > SUBJECT_ID_MAX_LENGTH {
        return Err(fail(
            id,
            "id",
            format!(
                "subject id must be at most {SUBJECT_ID_MAX_LENGTH} characters, got {}",
                id.len()
            ),
        ));
    }
    if id.starts_with('.') {
        return Err(fail(
            id,
            "id",
            "subject id must not start with `.`: hidden or relative path components are not safe identifiers",
        ));
    }
    if !id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-')
    {
        return Err(fail(
            id,
            "id",
            "subject id must use only `[A-Za-z0-9._-]`: path separators, whitespace, and other characters are not safe identifiers",
        ));
    }
    Ok(())
}
// ---------------------------------------------------------------------------
// Accepted subject manifest
// ---------------------------------------------------------------------------

/// One accepted subject: the immutable identity the denominator is built from.
/// The optional identity fields carry the manifest-side values receipt rows
/// bind against (`None` = not recorded, typed incomplete). Fields are shared
/// with the refresh route (#3566), which consumes the validated subjects
/// without re-parsing the manifest.
#[derive(Debug, Clone)]
pub(crate) struct AcceptedSubject {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) sha: String,
    pub(crate) license: String,
    pub(crate) shape: String,
    /// The resolved synthetic-diff path (per-repo, or the manifest-level
    /// fallback) as a portable repo-relative path. Retained so the refresh
    /// route (#3566) consumes the resolved input identity instead of
    /// re-parsing the manifest.
    pub(crate) synthetic_diff: String,
    pub(crate) tree_digest: Option<String>,
    pub(crate) snapshot: Option<String>,
    pub(crate) provenance: Option<String>,
    pub(crate) retention_class: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AcceptedManifest {
    pub(crate) sha256: String,
    pub(crate) subjects: Vec<AcceptedSubject>,
    /// Identities the retained manifest does not carry by design; disclosed as
    /// `incomplete`, never invented.
    pub(crate) incomplete: Vec<Diagnostic>,
}

impl AcceptedManifest {
    pub(crate) fn subject(&self, id: &str) -> Option<&AcceptedSubject> {
        self.subjects.iter().find(|entry| entry.id == id)
    }

    pub(crate) fn ids(&self) -> Vec<String> {
        self.subjects.iter().map(|entry| entry.id.clone()).collect()
    }
}

/// Reads a `synthetic_diff` field at one manifest level. Absent or null is
/// `None`; a present value must be a string portable path. A malformed present
/// value fails at the level that records it (#3733 review) — a valid value at
/// the other level repairs ABSENCE, never malformedness.
fn synthetic_diff_at_level(
    owner: &str,
    object: &serde_json::Map<String, Value>,
) -> Result<Option<String>, String> {
    match object.get("synthetic_diff") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(path)) => {
            check_portable_path(owner, "synthetic_diff", path)?;
            Ok(Some(path.clone()))
        }
        Some(_) => Err(fail(
            owner,
            "synthetic_diff",
            "field must be a string diff path when present",
        )),
    }
}

/// Validates the accepted manifest: schema/kind/spec/tier, exactly eight
/// unique subjects, immutable repository identity, license and shape tags,
/// portable diff paths. Data-driven: any eight well-formed subjects pass.
/// Shared with the refresh route (#3566): refresh consumes the validated
/// subjects instead of re-parsing.
pub(crate) fn validate_accepted_manifest(
    value: &Value,
    sha256: String,
) -> Result<AcceptedManifest, String> {
    let top = as_object(value, "manifest", "manifest", "accepted manifest")?;
    reject_unknown_keys(top, &MANIFEST_KEYS, "manifest", "accepted manifest")?;
    for (field, expected) in [
        ("schema_version", MANIFEST_SCHEMA_VERSION),
        ("kind", MANIFEST_KIND),
        ("spec", KNOWN_SPEC),
        ("tier", KNOWN_TIER),
    ] {
        let actual = top.get(field).and_then(Value::as_str).ok_or_else(|| {
            fail(
                "manifest",
                field,
                "accepted manifest must declare this field",
            )
        })?;
        if actual != expected {
            return Err(fail(
                "manifest",
                field,
                format!("expected `{expected}`, got `{actual}`"),
            ));
        }
    }

    // Owned-but-unchecked top-level fields get their emitted-shape type
    // checks: `description` is a string, `limits` an array of strings, and a
    // present top-level `synthetic_diff` fallback is a portable path whether
    // or not any subject needs it (#3733 review).
    opt_string("manifest", top, "description")?;
    opt_string_array("manifest", top, "limits")?;
    let top_level_diff = synthetic_diff_at_level("manifest", top)?;

    let repos = value
        .get("repos")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fail(
                "manifest",
                "repos",
                "accepted manifest must contain a repos array",
            )
        })?;
    if repos.len() != ACCEPTED_SUBJECT_COUNT {
        return Err(fail(
            "manifest",
            "repos",
            format!(
                "the accepted sweep denominator is exactly {ACCEPTED_SUBJECT_COUNT} selected subjects, got {}",
                repos.len()
            ),
        ));
    }

    let mut subjects = Vec::new();
    let mut seen = BTreeSet::new();
    let mut incomplete = Vec::new();
    for repo in repos {
        let entry = as_object(repo, "manifest", "repos", "manifest repo entry")?;
        reject_unknown_keys(
            entry,
            &MANIFEST_REPO_KEYS,
            "manifest",
            "manifest repo entry",
        )?;
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                fail(
                    "manifest",
                    "repos[].id",
                    "subject id must be a non-empty string",
                )
            })?
            .to_string();
        // Case-insensitive uniqueness (ASCII): subject ids become filesystem
        // path components, and on case-insensitive filesystems (Windows,
        // default macOS) `Alpha` and `alpha` would resolve to one directory —
        // two distinct ids colliding into one candidate tree. The charset
        // already bars `~` (short-name aliases) and path separators;
        // case-collision is the remaining alias, so it is rejected here.
        if !seen.insert(id.to_ascii_lowercase()) {
            return Err(fail(
                &id,
                "id",
                "duplicate subject id in the accepted manifest (ids must be unique case-insensitively: ids are path components, and a case-insensitive filesystem would collide `Alpha` with `alpha`)",
            ));
        }
        check_subject_id(&id)?;

        let url = entry
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| fail(&id, "url", "subject must declare an https repository url"))?;
        check_subject_url(&id, "url", url)?;

        let sha = entry
            .get("sha")
            .and_then(Value::as_str)
            .ok_or_else(|| fail(&id, "sha", "subject must pin an immutable source SHA"))?;
        check_git_sha(&id, "sha", sha)?;

        let license = entry
            .get("license")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| fail(&id, "license", "subject must declare its license class"))?
            .to_string();

        // `why` is owned; when recorded it must be a non-empty string (its
        // emitted shape), not a wrong-typed stand-in. Absent stays data-driven.
        opt_string(&id, entry, "why")?;

        let shape = entry
            .get("shape")
            .and_then(Value::as_str)
            .ok_or_else(|| fail(&id, "shape", "subject must declare a shape/layout tag"))?;
        known_value_or_fail(&id, "shape", shape, &KNOWN_SHAPES, "shape/layout tag")?;

        // Diff identity: per-repo path with the manifest-level fallback the
        // run path resolves. Each present value is checked at the level that
        // records it (#3733 review): a malformed present value fails even
        // when the other level supplies a valid fallback — only absence falls
        // through. Portable and secret-free; existence is a run-time concern,
        // not an offline structural one. The resolved path is retained on the
        // subject for the refresh route (#3566).
        let diff = synthetic_diff_at_level(&id, entry)?
            .or_else(|| top_level_diff.clone())
            .ok_or_else(|| {
                fail(
                    &id,
                    "synthetic_diff",
                    "subject has no synthetic_diff and the manifest has no top-level fallback",
                )
            })?;

        // Optional identities (#3733 review): absent is typed incomplete; a
        // present value must be well-formed, because an explicit null, an
        // empty string, or a malformed digest is a garbage identity, not an
        // absent one — that fails instead of silently completing the artifact.
        // Well-formed values are retained on the subject for receipt binding.
        let mut identities = [None, None, None, None];
        for (index, field) in ["tree_digest", "snapshot", "provenance", "retention_class"]
            .into_iter()
            .enumerate()
        {
            match entry.get(field) {
                None => incomplete.push(Diagnostic::new(
                    &id,
                    field,
                    "identity not recorded in the retained manifest; typed incomplete, not invented",
                )),
                Some(Value::Null) => {
                    return Err(fail(
                        &id,
                        field,
                        "identity is explicitly null; omit the field to record it absent — a present null is not an absent identity",
                    ));
                }
                Some(value) => {
                    let text = value
                        .as_str()
                        .ok_or_else(|| fail(&id, field, "identity must be a string when present"))?;
                    if text.trim().is_empty() {
                        return Err(fail(
                            &id,
                            field,
                            "identity must be non-empty when present",
                        ));
                    }
                    match field {
                        "tree_digest" => check_sha256_digest(&id, field, text)?,
                        "snapshot" => check_no_secrets(&id, field, text)?,
                        _ => {}
                    }
                    identities[index] = Some(text.to_string());
                }
            }
        }

        let [tree_digest, snapshot, provenance, retention_class] = identities;
        subjects.push(AcceptedSubject {
            id,
            url: url.to_string(),
            sha: sha.to_string(),
            license,
            shape: shape.to_string(),
            synthetic_diff: diff,
            tree_digest,
            snapshot,
            provenance,
            retention_class,
        });
    }

    Ok(AcceptedManifest {
        sha256,
        subjects,
        incomplete,
    })
}
