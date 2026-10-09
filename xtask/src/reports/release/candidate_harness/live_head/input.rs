//! Manifest-specific budget over the shared observed-snapshot reader.
const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const MAX_RETAINED_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn read_owned(
    root: &std::path::Path,
    relative: &str,
    remaining: u64,
) -> Result<Vec<u8>, String> {
    let limit = remaining.min(MAX_INPUT_BYTES);
    if limit == 0 {
        return Err("direct manifest exceeds 64 MiB aggregate retained-byte budget".to_string());
    }
    let bytes = super::super::input::read_snapshot(root, relative, limit)
        .map_err(|error| format!("direct manifest {error} (16 MiB per file, 64 MiB aggregate)"))?;
    if bytes.is_empty() {
        return Err("direct manifest input is empty".to_string());
    }
    Ok(bytes)
}
