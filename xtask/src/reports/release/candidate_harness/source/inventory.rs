//! Bounded raw Git-object inventory and observed checkout byte custody.
use super::super::{input::read_snapshot, safe_artifact_path};
use crate::run::{ByteCaptureBudget, TimedBytesOutput, capture_bytes_in_dir_with_budget};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

// Source budgets are independent of the manifest's 64 MiB retained-input budget.
pub(super) const MAX_SOURCE_FILE_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_SOURCE_BYTES: usize = 128 * 1024 * 1024;
pub(super) const MAX_SOURCE_FILES: usize = 16_384;
pub(super) const MAX_GIT_METADATA_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_GIT_STDERR_BYTES: usize = 1024 * 1024;
const BUDGET: SourceBudget = SourceBudget {
    file_bytes: MAX_SOURCE_FILE_BYTES,
    retained_bytes: MAX_SOURCE_BYTES,
    files: MAX_SOURCE_FILES,
};

#[derive(Clone, Copy)]
struct SourceBudget {
    file_bytes: usize,
    retained_bytes: usize,
    files: usize,
}
struct Entry {
    path: String,
    oid: String,
    size: usize,
}

pub(super) fn git_capture(
    root: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    stdout_bytes: usize,
    context: &str,
) -> Result<TimedBytesOutput, String> {
    // The global option applies equally to metadata and cat-file stdin batches.
    // Do not let ambient replacement refs turn OIDs into different objects.
    let args = std::iter::once("--no-replace-objects")
        .chain(args.iter().copied())
        .map(str::to_string)
        .collect::<Vec<_>>();
    capture_bytes_in_dir_with_budget(
        Path::new("git"),
        &args,
        (root, input),
        &["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"],
        ByteCaptureBudget {
            timeout: Duration::from_secs(30),
            stdout_bytes,
            stderr_bytes: MAX_GIT_STDERR_BYTES,
        },
        context,
    )
}

pub(super) fn committed_blobs(root: &Path, sha: &str) -> Result<BTreeMap<String, Vec<u8>>, String> {
    committed_blobs_with_budget(root, sha, BUDGET)
}
fn committed_blobs_with_budget(
    root: &Path,
    sha: &str,
    budget: SourceBudget,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let tree = super::git_bytes(root, &["ls-tree", "-rz", "--long", "--full-tree", sha])?;
    let entries = entries(&tree, budget)?;
    let mut input = Vec::new();
    let mut output_limit = 0usize;
    for entry in &entries {
        input.extend_from_slice(entry.oid.as_bytes());
        input.push(b'\n');
        output_limit = output_limit
            .checked_add(format!("{} blob {}\n", entry.oid, entry.size).len())
            .and_then(|n| n.checked_add(entry.size))
            .and_then(|n| n.checked_add(1))
            .ok_or("source batch output budget overflow")?;
    }
    let output = git_capture(
        root,
        &["cat-file", "--batch"],
        Some(&input),
        output_limit,
        "candidate committed source blobs",
    )?;
    if output.timed_out || !output.status.is_some_and(|status| status.success()) {
        return Err("candidate committed blob capture failed or timed out".to_string());
    }
    decode_batch(&output.stdout, &entries, budget)
}

fn entries(tree: &[u8], budget: SourceBudget) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    let mut total = 0usize;
    for entry in tree
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let separator = entry
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or("malformed Git tree entry")?;
        let header = std::str::from_utf8(entry.get(..separator).ok_or("Git tree header boundary")?)
            .map_err(|error| format!("Git tree header UTF-8: {error}"))?;
        let mut fields = header.split_whitespace();
        let mode = fields.next().ok_or("Git tree mode missing")?;
        let kind = fields.next().ok_or("Git tree kind missing")?;
        let oid = fields.next().ok_or("Git tree OID missing")?;
        let size = fields.next().ok_or("Git tree size missing")?;
        if fields.next().is_some() {
            return Err("unexpected Git tree header fields".to_string());
        }
        if !matches!(mode, "100644" | "100755") || kind != "blob" {
            continue;
        }
        super::super::live_head::require_hex("Git blob OID", oid, 40)?;
        let size = size
            .parse::<usize>()
            .map_err(|error| format!("Git tree blob size: {error}"))?;
        let path = std::str::from_utf8(entry.get(separator + 1..).ok_or("Git tree path boundary")?)
            .map_err(|error| format!("Git tree path UTF-8: {error}"))?;
        if !safe_artifact_path(Path::new(path)) {
            return Err("unsupported Git source path".to_string());
        }
        total = charge(total, entries.len(), size, path, budget)?;
        entries.push(Entry {
            path: path.to_string(),
            oid: oid.to_string(),
            size,
        });
    }
    Ok(entries)
}

fn charge(
    total: usize,
    previous_files: usize,
    size: usize,
    path: &str,
    budget: SourceBudget,
) -> Result<usize, String> {
    if previous_files >= budget.files {
        return Err(format!(
            "source inventory exceeds {} ordinary-blob budget",
            budget.files
        ));
    }
    if size > budget.file_bytes {
        return Err(format!(
            "source blob {path} exceeds {}-byte file budget",
            budget.file_bytes
        ));
    }
    total
        .checked_add(size)
        .filter(|sum| *sum <= budget.retained_bytes)
        .ok_or_else(|| {
            format!(
                "source inventory exceeds {}-byte aggregate retained-source budget",
                budget.retained_bytes
            )
        })
}

fn decode_batch(
    stdout: &[u8],
    entries: &[Entry],
    budget: SourceBudget,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut remaining = stdout;
    let mut blobs = BTreeMap::new();
    let mut total = 0usize;
    for entry in entries {
        let end = remaining
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or("Git batch header incomplete")?;
        let expected_header = format!("{} blob {}", entry.oid, entry.size);
        if remaining.get(..end) != Some(expected_header.as_bytes()) {
            return Err(
                "Git batch object identity/type/size differs from bounded tree inventory"
                    .to_string(),
            );
        }
        total = charge(total, blobs.len(), entry.size, &entry.path, budget)?;
        let body = remaining.get(end + 1..).ok_or("Git batch body boundary")?;
        let bytes = body.get(..entry.size).ok_or("Git batch body incomplete")?;
        if body.get(entry.size) != Some(&b'\n') {
            return Err("Git batch terminator missing".to_string());
        }
        if blobs.insert(entry.path.clone(), bytes.to_vec()).is_some() {
            return Err("duplicate committed source path".to_string());
        }
        remaining = body
            .get(entry.size + 1..)
            .ok_or("Git batch tail boundary")?;
    }
    if !remaining.is_empty() {
        return Err("unexpected Git batch tail".to_string());
    }
    Ok(blobs)
}

pub(super) fn verify_checkout(
    root: &Path,
    blobs: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    verify_checkout_with_budget(root, blobs, BUDGET)
}
fn verify_checkout_with_budget(
    root: &Path,
    blobs: &BTreeMap<String, Vec<u8>>,
    budget: SourceBudget,
) -> Result<(), String> {
    let mut total = 0usize;
    for (index, (path, expected)) in blobs.iter().enumerate() {
        total = charge(total, index, expected.len(), path, budget)?;
        // Read no more than the already admitted exact blob length plus one.
        // A growing/skipped checkout cannot allocate a second unbounded copy.
        let actual = read_snapshot(root, path, expected.len() as u64)?;
        if actual != *expected {
            return Err(format!(
                "selected checkout bytes differ from committed blob: {path}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
