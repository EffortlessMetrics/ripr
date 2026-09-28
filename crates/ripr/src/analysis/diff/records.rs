//! NUL-delimited Git path-record decoding (`--name-only -z`,
//! `--name-status -z`).
//!
//! Single owner for path-record decoding shared by the product and xtask
//! routes (#4006). Callers must pass `-z` output: NUL-separated records with
//! no C-quoting. Decoding is strict and lossless — non-UTF-8 records fail
//! instead of collapsing through lossy conversion. Empty records and status
//! records with missing path fields fail instead of vanishing; a missing
//! trailing NUL after the final non-empty field is tolerated. The decoder
//! applies no confinement or normalization policy: consumers own what a
//! decoded path may reference.
//!
//! Wire shapes (verified against real `git diff` output):
//!
//! ```text
//! name-only -z:   path\0path\0
//! name-status -z: STATUS\0path\0 | STATUS\0old\0new\0   (renames/copies)
//! ```
//!
//! Consumer migration (slice 2, after this shared authority lands): the
//! line-delimited inventories move to these decoders together with their
//! `-z` flag. Slice 2a (xtask): `back_sync` (`ls-tree`/`--name-status`),
//! `proof_route`, `review_comments`, `source_promotion`, `release_scope`,
//! `main` (`changed_files_vs_base`), alongside the landed xtask
//! `pr_evidence` (#4036). Slice 2b (product): `repair_attempt`
//! (`git_paths`) and product `pr_evidence` (`changed_files`). Slice 3
//! (xtask repo-ops): `main` (`collect_pr_changes`, `collect_worktree_status_changes`)
//! through the status-record decoder plus a local porcelain-`-z` decoder.
//! Out of scope:
//! `first_pr` (range validation only, no path decoding), the `init`
//! template receipt (a NUL count, not an inventory), `edit_cage` and
//! `precommit_v2` (already `-z` through their own strict local decoders;
//! folding those into this authority is a separate claim). This module must
//! not grow per-consumer argv or filtering rules.

use std::path::PathBuf;

/// One decoded `--name-status -z` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRecord {
    /// Raw status token (`M`, `A`, `D`, `R100`, `C75`, ...), verbatim.
    pub status: String,
    /// Changed path (rename/copy target).
    pub path: PathBuf,
    /// Rename/copy source; `None` for non-rename records.
    pub renamed_from: Option<PathBuf>,
}

/// Strict decoding failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathRecordError {
    /// A record is not valid UTF-8. `record` is the zero-based record index;
    /// `offset` is the byte offset of the first invalid byte within the record.
    NonUtf8Path { record: usize, offset: usize },
    /// A record is empty. Empty paths never name a changed file; accepting
    /// one would silently drop inventory the caller believes it holds.
    EmptyPath { record: usize },
    /// A rename/copy record is missing its paired path.
    TruncatedRecord { record: usize, status: String },
    /// A status field is not a Git status token. Callers must pass
    /// `--name-status -z` output: feeding a framed path inventory (such as
    /// `--name-only -z` bytes) fails here instead of decoding `Ok` with a
    /// path misattributed as a status.
    InvalidStatus { record: usize, status: String },
}

impl std::fmt::Display for PathRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonUtf8Path { record, offset } => write!(
                f,
                "path record {record} is not valid UTF-8 (first invalid byte at offset {offset})"
            ),
            Self::EmptyPath { record } => {
                write!(f, "path record {record} is empty")
            }
            Self::TruncatedRecord { record, status } => write!(
                f,
                "path record {record} with status {status:?} is missing its paired path"
            ),
            Self::InvalidStatus { record, status } => write!(
                f,
                "path record {record} has invalid status token {status:?}"
            ),
        }
    }
}

impl std::error::Error for PathRecordError {}

/// Decode a `--name-only -z` inventory: paths in record order.
///
/// Empty input decodes to an empty inventory (a real zero-change run).
pub fn parse_git_path_records(output: &[u8]) -> Result<Vec<PathBuf>, PathRecordError> {
    let mut paths = Vec::new();
    for (record, field) in nul_fields(output).into_iter().enumerate() {
        if field.is_empty() {
            return Err(PathRecordError::EmptyPath { record });
        }
        paths.push(decode_record_path(field, record)?);
    }
    Ok(paths)
}

/// Decode a `--name-status -z` inventory: `(status, path, rename-source)`
/// records in record order.
///
/// Empty input decodes to an empty inventory (a real zero-change run).
/// Rename/copy records (`R`/`C` status markers) consume their paired source
/// path; a missing pair fails instead of attributing the change to half a
/// record.
pub fn parse_git_status_records(output: &[u8]) -> Result<Vec<StatusRecord>, PathRecordError> {
    let mut fields = nul_fields(output).into_iter();
    let mut records = Vec::new();
    // `record` is the zero-based status-record index (not the NUL-field
    // index): one record spans two fields, three for renames/copies.
    let mut record = 0usize;
    while let Some(status_field) = fields.next() {
        if status_field.is_empty() {
            return Err(PathRecordError::EmptyPath { record });
        }
        let status = decode_record_str(status_field, record)?.to_string();
        let kind = status_kind(&status).ok_or_else(|| PathRecordError::InvalidStatus {
            record,
            status: status.clone(),
        })?;
        // A rename/copy record is `STATUS\0old\0new\0`: the paired target
        // follows as the next NUL-delimited field. A missing pair fails
        // instead of attributing the change to half a record.
        if matches!(kind, 'R' | 'C') {
            let origin = next_record_str(&mut fields, record, &status)?;
            let target = next_record_str(&mut fields, record, &status)?;
            records.push(StatusRecord {
                status,
                path: PathBuf::from(target),
                renamed_from: Some(PathBuf::from(origin)),
            });
        } else {
            let path = next_record_str(&mut fields, record, &status)?;
            records.push(StatusRecord {
                status,
                path: PathBuf::from(path),
                renamed_from: None,
            });
        }
        record += 1;
    }
    Ok(records)
}

/// Fetch and decode the next field of status record `record`.
///
/// A missing field fails as [`PathRecordError::TruncatedRecord`] and an
/// empty one as [`PathRecordError::EmptyPath`]: callers never observe half
/// a record.
fn next_record_str<'output>(
    fields: &mut impl Iterator<Item = &'output [u8]>,
    record: usize,
    status: &str,
) -> Result<&'output str, PathRecordError> {
    let field = fields
        .next()
        .ok_or_else(|| PathRecordError::TruncatedRecord {
            record,
            status: status.to_string(),
        })?;
    if field.is_empty() {
        return Err(PathRecordError::EmptyPath { record });
    }
    decode_record_str(field, record)
}

/// Split `-z` output into record fields.
///
/// Exactly one trailing NUL terminates the final record; empty input decodes
/// to zero fields (a real zero-change run). Any other empty field survives
/// here and fails downstream as [`PathRecordError::EmptyPath`]: an empty
/// record never names a changed file.
fn nul_fields(output: &[u8]) -> Vec<&[u8]> {
    if output.is_empty() {
        return Vec::new();
    }
    let mut fields: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    if output.last() == Some(&0) {
        fields.pop();
    }
    fields
}

/// Return the status letter when `status` is a Git status token.
///
/// Accepts exactly one status letter (`A`, `C`, `D`, `M`, `R`, `T`, `U`,
/// `X`, `B` — the `--name-status` alphabet, including rename/copy scores
/// such as `R100`) followed only by ASCII digits. Anything else is
/// misframed input, not a status.
fn status_kind(status: &str) -> Option<char> {
    let mut chars = status.chars();
    let kind = chars.next()?;
    let valid = matches!(kind, 'A' | 'C' | 'D' | 'M' | 'R' | 'T' | 'U' | 'X' | 'B')
        && chars.all(|c| c.is_ascii_digit());
    valid.then_some(kind)
}

/// Decode one NUL-delimited record field as a verbatim UTF-8 path fragment.
///
/// Tab, space, and other ASCII control bytes are kept as-is: `-z` output is
/// never quoted, so any byte except NUL can be path content. Returns a
/// borrow: callers that need ownership convert once at the boundary.
fn decode_record_str(field: &[u8], record: usize) -> Result<&str, PathRecordError> {
    std::str::from_utf8(field).map_err(|err| PathRecordError::NonUtf8Path {
        record,
        offset: err.valid_up_to(),
    })
}

/// Decode one `--name-only -z` record field into a path.
fn decode_record_path(field: &[u8], record: usize) -> Result<PathBuf, PathRecordError> {
    decode_record_str(field, record).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(output: &[u8]) -> Result<Vec<PathBuf>, String> {
        parse_git_path_records(output).map_err(|err| err.to_string())
    }

    fn statuses(output: &[u8]) -> Result<Vec<StatusRecord>, String> {
        parse_git_status_records(output).map_err(|err| err.to_string())
    }

    #[test]
    fn name_only_decodes_rename_space_and_unicode() -> Result<(), String> {
        let output = "renamed.txt\0sp ace.txt\0uni-é.txt\0".as_bytes();
        let decoded = paths(output)?;
        assert_eq!(
            decoded,
            vec![
                PathBuf::from("renamed.txt"),
                PathBuf::from("sp ace.txt"),
                PathBuf::from("uni-é.txt"),
            ]
        );
        Ok(())
    }

    #[test]
    fn name_only_empty_input_is_empty_inventory() -> Result<(), String> {
        assert_eq!(paths(b"")?, Vec::<PathBuf>::new());
        Ok(())
    }

    #[test]
    fn name_only_tolerates_missing_trailing_nul() -> Result<(), String> {
        let decoded = paths(b"a.txt\0b.txt")?;
        assert_eq!(
            decoded,
            vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")]
        );
        Ok(())
    }

    #[test]
    fn name_only_rejects_interior_empty_record() -> Result<(), String> {
        match paths(b"a.txt\0\0b.txt\0") {
            Err(err) => {
                assert_eq!(err, PathRecordError::EmptyPath { record: 1 }.to_string());
                Ok(())
            }
            Ok(decoded) => Err(format!(
                "interior empty record must fail, decoded {decoded:?}"
            )),
        }
    }

    #[test]
    fn name_only_rejects_non_utf8_record() -> Result<(), String> {
        let output = b"ok.txt\0\xffbad\0";
        match paths(output) {
            Err(err) => {
                assert_eq!(
                    err,
                    PathRecordError::NonUtf8Path {
                        record: 1,
                        offset: 0
                    }
                    .to_string()
                );
                Ok(())
            }
            Ok(decoded) => Err(format!("non-UTF-8 record must fail, decoded {decoded:?}")),
        }
    }

    #[test]
    fn name_only_keeps_tab_and_control_bytes_verbatim() -> Result<(), String> {
        let decoded = paths(b"a\tb.txt\0")?;
        assert_eq!(decoded, vec![PathBuf::from("a\tb.txt")]);
        Ok(())
    }

    #[test]
    fn status_decodes_modify_and_rename_pair() -> Result<(), String> {
        // Real `git diff --name-status -z` bytes: status fields are
        // NUL-separated with no tabs; renames carry old then new.
        let output = b"R100\0plain.txt\0renamed.txt\0M\0sp ace.txt\0";
        let decoded = statuses(output)?;
        assert_eq!(
            decoded,
            vec![
                StatusRecord {
                    status: "R100".to_string(),
                    path: PathBuf::from("renamed.txt"),
                    renamed_from: Some(PathBuf::from("plain.txt")),
                },
                StatusRecord {
                    status: "M".to_string(),
                    path: PathBuf::from("sp ace.txt"),
                    renamed_from: None,
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn status_empty_input_is_empty_inventory() -> Result<(), String> {
        assert_eq!(statuses(b"")?, Vec::<StatusRecord>::new());
        Ok(())
    }

    #[test]
    fn status_rejects_truncated_rename_pair() -> Result<(), String> {
        match statuses(b"R100\0only-old.txt\0") {
            Err(err) => {
                assert_eq!(
                    err,
                    PathRecordError::TruncatedRecord {
                        record: 0,
                        status: "R100".to_string()
                    }
                    .to_string()
                );
                Ok(())
            }
            Ok(decoded) => Err(format!(
                "truncated rename pair must fail, decoded {decoded:?}"
            )),
        }
    }

    #[test]
    fn status_rejects_non_utf8_path() -> Result<(), String> {
        match statuses(b"M\0\xffbad\0") {
            Err(err) => {
                assert_eq!(
                    err,
                    PathRecordError::NonUtf8Path {
                        record: 0,
                        offset: 0
                    }
                    .to_string()
                );
                Ok(())
            }
            Ok(decoded) => Err(format!("non-UTF-8 path must fail, decoded {decoded:?}")),
        }
    }

    #[test]
    fn status_rejects_misframed_path_as_status() -> Result<(), String> {
        // `--name-only -z` bytes fed to the status decoder must fail, not
        // decode `Ok` with a path misattributed as a status.
        match statuses(b"src/a.rs\0src/b.rs\0") {
            Err(err) => {
                assert_eq!(
                    err,
                    PathRecordError::InvalidStatus {
                        record: 0,
                        status: "src/a.rs".to_string()
                    }
                    .to_string()
                );
                Ok(())
            }
            Ok(decoded) => Err(format!(
                "misframed path-as-status must fail, decoded {decoded:?}"
            )),
        }
    }

    #[test]
    fn status_rejects_rename_shaped_misframing() -> Result<(), String> {
        match statuses(b"README.md\0x\0y\0") {
            Err(err) => {
                assert_eq!(
                    err,
                    PathRecordError::InvalidStatus {
                        record: 0,
                        status: "README.md".to_string()
                    }
                    .to_string()
                );
                Ok(())
            }
            Ok(decoded) => Err(format!(
                "rename-shaped misframing must fail, decoded {decoded:?}"
            )),
        }
    }

    #[test]
    fn status_rejects_truncated_non_rename_record() -> Result<(), String> {
        match statuses(b"M\0") {
            Err(err) => {
                assert_eq!(
                    err,
                    PathRecordError::TruncatedRecord {
                        record: 0,
                        status: "M".to_string()
                    }
                    .to_string()
                );
                Ok(())
            }
            Ok(decoded) => Err(format!(
                "truncated non-rename record must fail, decoded {decoded:?}"
            )),
        }
    }

    #[test]
    fn status_decodes_copy_pair_with_source() -> Result<(), String> {
        let output = b"C75\0template.txt\0copy.txt\0";
        let decoded = statuses(output)?;
        assert_eq!(
            decoded,
            vec![StatusRecord {
                status: "C75".to_string(),
                path: PathBuf::from("copy.txt"),
                renamed_from: Some(PathBuf::from("template.txt")),
            }]
        );
        Ok(())
    }
}
