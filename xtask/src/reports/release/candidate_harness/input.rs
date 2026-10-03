//! Bounded observed snapshots, not atomic or authenticated filesystem custody.
use super::safe_artifact_path;
use std::fs::{File, Metadata};
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

#[derive(Eq, PartialEq)]
struct Snapshot {
    len: u64,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64),
}

impl Snapshot {
    fn capture(metadata: &Metadata) -> Result<Self, String> {
        if !metadata.is_file() {
            return Err("custody input is not a regular file".to_string());
        }
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            len: metadata.len(),
            modified: metadata
                .modified()
                .map_err(|e| format!("input modification time: {e}"))?,
            #[cfg(unix)]
            identity: (metadata.dev(), metadata.ino()),
        })
    }
}

pub(super) fn read_snapshot(root: &Path, relative: &str, limit: u64) -> Result<Vec<u8>, String> {
    if !safe_artifact_path(Path::new(relative)) {
        return Err("custody input must be an ordinary root-relative path".to_string());
    }
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("resolve custody input {relative}: {e}"))?;
    if !path.starts_with(root) {
        return Err(format!("custody input escapes its root: {relative}"));
    }
    let inspect = || -> Result<Snapshot, String> {
        Snapshot::capture(
            &std::fs::symlink_metadata(root.join(relative))
                .map_err(|e| format!("inspect custody input {relative}: {e}"))?,
        )
    };
    let before = inspect()?;
    if before.len > limit {
        return Err(format!(
            "custody input {relative} exceeds its {limit}-byte budget"
        ));
    }
    let mut file = File::open(&path).map_err(|e| format!("open custody input {relative}: {e}"))?;
    let opened = Snapshot::capture(
        &file
            .metadata()
            .map_err(|e| format!("inspect opened input: {e}"))?,
    )?;
    if before != opened {
        return Err(format!("custody input changed while opening: {relative}"));
    }
    let bytes = read_limited(&mut file, limit)?;
    let after = Snapshot::capture(
        &file
            .metadata()
            .map_err(|e| format!("inspect read input: {e}"))?,
    )?;
    if opened != after
        || after != inspect()?
        || bytes.len() as u64 != after.len
        || root
            .join(relative)
            .canonicalize()
            .map_err(|e| format!("re-resolve input: {e}"))?
            != path
    {
        return Err(format!("custody input changed while reading: {relative}"));
    }
    Ok(bytes)
}

// The limit applies to bytes actually read, even when a file grows after stat.
// Snapshot metadata is observational: same-size/timestamp-preserving writes,
// path swaps between checks and later mutation cannot be excluded without locks.
fn read_limited(reader: &mut impl Read, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read custody input: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!(
            "custody input exceeds its {limit}-byte read budget"
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_read_limit_rejects_growth_without_consuming_the_tail() -> Result<(), String> {
        let mut reader = std::io::Cursor::new(b"123456789".to_vec());
        if read_limited(&mut reader, 4).is_ok() || reader.position() != 5 {
            return Err("limit+1 read consumed an unbounded tail or admitted growth".to_string());
        }
        let bytes = read_limited(&mut std::io::Cursor::new(b"1234"), 4)?;
        if bytes != b"1234" {
            return Err("exact read budget rejected".to_string());
        }
        Ok(())
    }
}
