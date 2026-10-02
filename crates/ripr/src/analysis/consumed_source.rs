//! Private commitments to raw Rust bytes actually loaded by one analysis.
//!
//! These are per-path observations, not a complete dependency identity. Cached
//! syntax text, decoded geometry and a later filesystem read cannot mint them.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Commitment {
    Captured(String),
    Unavailable,
    Conflicting,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ConsumedRustSources {
    paths: BTreeMap<PathBuf, Commitment>,
    #[cfg(test)]
    pub(crate) file_fact_cache: crate::analysis::seam_cache::FileFactCacheStats,
}

impl ConsumedRustSources {
    /// Observe the raw loaded buffer before parsing or consulting fact caches.
    pub(crate) fn record(&mut self, path: &Path, bytes: Option<&[u8]>) {
        if !is_normal_relative_path(path) {
            return;
        }
        let observed = bytes.map_or(Commitment::Unavailable, |bytes| {
            Commitment::Captured(format!("{:x}", Sha256::digest(bytes)))
        });
        self.paths
            .entry(path.to_path_buf())
            .and_modify(|previous| {
                if *previous != observed {
                    *previous = Commitment::Conflicting;
                }
            })
            .or_insert(observed);
    }

    /// Resolve an observed relative key. The consumer owns admission under the
    /// snapshot root; this carrier never reads or canonicalizes the filesystem.
    pub(crate) fn digest(&self, relative: &Path) -> Option<String> {
        if !is_normal_relative_path(relative) {
            return None;
        }
        match self.paths.get(relative)? {
            Commitment::Captured(digest) => Some(digest.clone()),
            Commitment::Unavailable | Commitment::Conflicting => None,
        }
    }
}

fn is_normal_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_encoding_and_conflicting_observations_remain_distinct() -> Result<(), String> {
        let path = Path::new("src/lib.rs");
        let mut sources = ConsumedRustSources::default();
        let raw = b"\xef\xbb\xbfpub fn f() {}\r\n\xff";
        sources.record(path, Some(raw));
        let expected = format!("{:x}", Sha256::digest(raw));
        if sources.digest(path).as_ref() != Some(&expected) {
            return Err("raw BOM, CRLF and invalid UTF-8 bytes were not retained".into());
        }
        sources.record(path, Some(raw));
        if sources.digest(path).as_ref() != Some(&expected) {
            return Err("identical repeated input became conflicting".into());
        }
        sources.record(path, Some(b"pub fn f() {}\n"));
        sources.record(path, Some(raw));
        if sources.digest(path).is_some() {
            return Err("conflicting observation regained authority".into());
        }
        Ok(())
    }

    #[test]
    fn missing_unknown_and_outside_paths_cannot_mint_commitments() -> Result<(), String> {
        let path = Path::new("src/lib.rs");
        let mut sources = ConsumedRustSources::default();
        sources.record(path, None);
        sources.record(path, Some(b"later bytes"));
        sources.record(Path::new("../outside.rs"), Some(b"outside"));
        for candidate in [
            path,
            Path::new("unknown.rs"),
            Path::new("../outside.rs"),
            Path::new("/outside.rs"),
            Path::new(""),
        ] {
            if sources.digest(candidate).is_some() {
                return Err(format!(
                    "unavailable input acquired authority: {}",
                    candidate.display()
                ));
            }
        }
        Ok(())
    }
}
