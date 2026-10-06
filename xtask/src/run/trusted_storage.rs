//! Monitored storage for trusted repository builds, not a filesystem sandbox.
//! Sizes are observed on known paths; writes between scans, open-unlinked files
//! and writes outside those paths are not subject to a hard quota here.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub(crate) struct TrustedStorageMonitor {
    roots: Vec<PathBuf>,
    baseline: u64,
    maximum_growth: u64,
    peak_growth: u64,
    next_scan: Instant,
}

impl TrustedStorageMonitor {
    pub(crate) fn new(mut roots: Vec<PathBuf>, maximum_growth: u64) -> Result<Self, String> {
        if maximum_growth == 0 || roots.is_empty() {
            return Err("trusted storage monitoring requires paths and a positive bound".into());
        }
        if roots.iter().any(|root| !root.is_absolute()) {
            return Err("trusted storage monitoring requires absolute known paths".into());
        }
        roots.sort();
        roots.dedup();
        let baseline = observed_bytes(&roots)?;
        Ok(Self {
            roots,
            baseline,
            maximum_growth,
            peak_growth: 0,
            next_scan: Instant::now(),
        })
    }

    /// Establish only the already-declared build directories before ownership
    /// starts. Every component is checked without following an existing link.
    pub(crate) fn establish_roots(
        roots: &[PathBuf],
        required_existing: &[PathBuf],
    ) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut metadata = |path: &Path| fs::symlink_metadata(path);
        require_declared_roots(required_existing, &mut metadata, deadline)?;
        for root in roots {
            if !root.is_absolute() {
                return Err("trusted storage monitoring requires absolute known paths".into());
            }
            for path in root.ancestors().collect::<Vec<_>>().into_iter().rev() {
                if Instant::now() >= deadline {
                    return Err("trusted storage root establishment exceeded its time bound".into());
                }
                if !path.is_absolute() {
                    continue;
                }
                match fs::symlink_metadata(path) {
                    Ok(metadata) => require_root_directory(path, &metadata)?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        if required_existing
                            .iter()
                            .any(|required| required.starts_with(path))
                        {
                            return Err(format!(
                                "trusted storage required existing root disappeared {}: {error}",
                                path.display()
                            ));
                        }
                        match fs::create_dir(path) {
                            Ok(()) => {}
                            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                            Err(error) => {
                                return Err(format!(
                                    "trusted storage create declared root {}: {error}",
                                    path.display()
                                ));
                            }
                        }
                        let metadata = fs::symlink_metadata(path).map_err(|error| {
                            format!(
                                "trusted storage established root {}: {error}",
                                path.display()
                            )
                        })?;
                        require_root_directory(path, &metadata)?;
                    }
                    Err(error) => {
                        return Err(format!(
                            "trusted storage declared root {}: {error}",
                            path.display()
                        ));
                    }
                }
            }
        }
        require_declared_roots(required_existing, &mut metadata, deadline)
    }

    pub(crate) fn check(&mut self) -> Result<(), String> {
        if Instant::now() < self.next_scan {
            return Ok(());
        }
        self.check_now()
    }

    pub(crate) fn check_now(&mut self) -> Result<(), String> {
        let growth = observed_bytes(&self.roots)?.saturating_sub(self.baseline);
        self.peak_growth = self.peak_growth.max(growth);
        self.next_scan = Instant::now() + Duration::from_secs(1);
        if growth > self.maximum_growth {
            return Err(format!(
                "trusted build observed storage growth {growth} exceeds {} bytes; monitored paths only, not a hard quota",
                self.maximum_growth
            ));
        }
        Ok(())
    }

    pub(crate) fn peak_growth(&self) -> u64 {
        self.peak_growth
    }

    pub(crate) fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

fn observed_bytes(roots: &[PathBuf]) -> Result<u64, String> {
    observed_bytes_with(
        roots,
        |path| fs::symlink_metadata(path),
        |path| fs::read_dir(path),
    )
}

fn require_root_directory(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "trusted storage declared root must be a directory without a symlink {}",
            path.display()
        ));
    }
    Ok(())
}

fn require_declared_roots(
    roots: &[PathBuf],
    metadata: &mut impl FnMut(&Path) -> std::io::Result<fs::Metadata>,
    deadline: Instant,
) -> Result<(), String> {
    for (index, root) in roots.iter().enumerate() {
        if index >= 250_000 || Instant::now() >= deadline {
            return Err("trusted storage observation exceeded its entry/time bound".into());
        }
        let observed = metadata(root).map_err(|error| {
            format!("trusted storage declared root {}: {error}", root.display())
        })?;
        require_root_directory(root, &observed)?;
    }
    Ok(())
}

fn observed_bytes_with(
    roots: &[PathBuf],
    mut metadata: impl FnMut(&Path) -> std::io::Result<fs::Metadata>,
    mut read_dir: impl FnMut(&Path) -> std::io::Result<fs::ReadDir>,
) -> Result<u64, String> {
    const ENTRIES: usize = 250_000;
    let deadline = Instant::now() + Duration::from_secs(5);
    // Keep every declared root mandatory, including nested roots. Deduplicate
    // traversal coverage only; a missing declared root is never a temp entry.
    require_declared_roots(roots, &mut metadata, deadline)?;
    let mut pending: Vec<_> = roots
        .iter()
        .filter(|root| {
            !roots
                .iter()
                .any(|other| other != *root && root.starts_with(other))
        })
        .cloned()
        .collect();
    let mut entries = 0;
    let mut bytes = 0u64;
    while let Some(path) = pending.pop() {
        if entries >= ENTRIES || Instant::now() >= deadline {
            return Err("trusted storage observation exceeded its entry/time bound".into());
        }
        entries += 1;
        let observed = match metadata(&path) {
            Ok(metadata) => metadata,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && !roots.contains(&path) =>
            {
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "trusted storage observation {}: {error}",
                    path.display()
                ));
            }
        };
        if observed.file_type().is_symlink() {
            return Err(format!(
                "trusted storage observation refuses symlink {}",
                path.display()
            ));
        }
        if observed.is_dir() {
            let listing = match read_dir(&path) {
                Ok(listing) => listing,
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound && !roots.contains(&path) =>
                {
                    continue;
                }
                Err(error) => {
                    return Err(format!(
                        "trusted storage directory {}: {error}",
                        path.display()
                    ));
                }
            };
            for entry in listing {
                if pending.len() >= ENTRIES || Instant::now() >= deadline {
                    return Err("trusted storage directory observation exceeded its bound".into());
                }
                pending.push(entry.map_err(|error| error.to_string())?.path());
            }
        } else if observed.is_file() {
            bytes = bytes
                .checked_add(observed.len())
                .ok_or("trusted storage size overflow")?;
        } else {
            return Err(format!(
                "trusted storage observation refuses nonregular path {}",
                path.display()
            ));
        }
    }
    require_declared_roots(roots, &mut metadata, deadline)?;
    if Instant::now() >= deadline {
        return Err("trusted storage observation exceeded its entry/time bound".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_storage_fixture(run: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-preparation-storage-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        fs::create_dir(&root).map_err(|error| error.to_string())?;
        let result = run(&root);
        let cleanup = fs::remove_dir_all(&root).map_err(|error| error.to_string());
        result.and(cleanup)
    }

    fn require_storage_error<T>(result: Result<T, String>, needle: &str) -> Result<(), String> {
        match result {
            Err(reason) if reason.contains(needle) => Ok(()),
            Err(reason) => Err(format!("wrong storage refusal: {reason}")),
            Ok(_) => Err(format!("storage observation failed to refuse {needle}")),
        }
    }

    #[test]
    fn materialized_build_preparation_storage_tolerates_discovered_directory_disappearance()
    -> Result<(), String> {
        with_storage_fixture(|root| {
            let child = root.join("rmeta-temporary");
            fs::create_dir(&child).map_err(|error| error.to_string())?;
            fs::write(root.join("retained"), [0; 13]).map_err(|error| error.to_string())?;
            let mut removed_after_metadata = false;
            let bytes = observed_bytes_with(
                &[root.to_path_buf()],
                |path| fs::symlink_metadata(path),
                |path| {
                    if path == child {
                        fs::remove_dir(&child)?;
                        removed_after_metadata = true;
                    }
                    fs::read_dir(path)
                },
            )?;
            assert!(removed_after_metadata);
            assert_eq!(bytes, 13);
            Ok(())
        })
    }

    #[test]
    fn materialized_build_preparation_storage_tolerates_discovered_metadata_disappearance()
    -> Result<(), String> {
        with_storage_fixture(|root| {
            let child = root.join("temporary");
            fs::write(&child, [0; 7]).map_err(|error| error.to_string())?;
            fs::write(root.join("retained"), [0; 13]).map_err(|error| error.to_string())?;
            let mut removed_after_listing = false;
            let bytes = observed_bytes_with(
                &[root.to_path_buf()],
                |path| {
                    if path == child {
                        fs::remove_file(&child)?;
                        removed_after_listing = true;
                    }
                    fs::symlink_metadata(path)
                },
                |path| fs::read_dir(path),
            )?;
            assert!(removed_after_listing);
            assert_eq!(bytes, 13);
            Ok(())
        })
    }

    #[test]
    fn materialized_build_preparation_storage_refuses_owned_root_disappearance()
    -> Result<(), String> {
        with_storage_fixture(|root| {
            let owned = root.join("owned");
            require_storage_error(
                observed_bytes(std::slice::from_ref(&owned)),
                "declared root",
            )?;
            fs::create_dir(&owned).map_err(|error| error.to_string())?;
            require_storage_error(
                observed_bytes_with(
                    std::slice::from_ref(&owned),
                    |path| fs::symlink_metadata(path),
                    |path| {
                        fs::remove_dir(path)?;
                        fs::read_dir(path)
                    },
                ),
                "trusted storage directory",
            )?;
            fs::create_dir(&owned).map_err(|error| error.to_string())?;
            let mut monitor = TrustedStorageMonitor::new(vec![owned.clone()], 16)?;
            fs::remove_dir(&owned).map_err(|error| error.to_string())?;
            require_storage_error(monitor.check_now(), "declared root")
        })
    }

    #[test]
    fn materialized_build_preparation_storage_keeps_nested_owned_roots_mandatory()
    -> Result<(), String> {
        with_storage_fixture(|root| {
            let nested = root.join("nested");
            fs::create_dir(&nested).map_err(|error| error.to_string())?;
            let mut monitor =
                TrustedStorageMonitor::new(vec![root.to_path_buf(), nested.clone()], 16)?;
            fs::remove_dir(&nested).map_err(|error| error.to_string())?;
            require_storage_error(monitor.check_now(), "declared root")?;
            fs::create_dir(&nested).map_err(|error| error.to_string())?;
            require_storage_error(
                observed_bytes_with(
                    &[root.to_path_buf(), nested.clone()],
                    |path| fs::symlink_metadata(path),
                    |path| {
                        if path == root {
                            fs::remove_dir(&nested)?;
                        }
                        fs::read_dir(path)
                    },
                ),
                "declared root",
            )
        })
    }

    #[test]
    fn materialized_build_preparation_storage_refuses_permissions_and_other_io()
    -> Result<(), String> {
        with_storage_fixture(|root| {
            let child = root.join("child");
            fs::create_dir(&child).map_err(|error| error.to_string())?;
            for denied in [root, child.as_path()] {
                for kind in [
                    std::io::ErrorKind::PermissionDenied,
                    std::io::ErrorKind::Other,
                ] {
                    require_storage_error(
                        observed_bytes_with(
                            &[root.to_path_buf()],
                            |path| fs::symlink_metadata(path),
                            |path| {
                                if path == denied {
                                    return Err(std::io::Error::new(
                                        kind,
                                        "injected storage refusal",
                                    ));
                                }
                                fs::read_dir(path)
                            },
                        ),
                        "injected storage refusal",
                    )?;
                    require_storage_error(
                        observed_bytes_with(
                            &[root.to_path_buf()],
                            |path| {
                                if path == denied {
                                    return Err(std::io::Error::new(
                                        kind,
                                        "injected metadata refusal",
                                    ));
                                }
                                fs::symlink_metadata(path)
                            },
                            |path| fs::read_dir(path),
                        ),
                        "injected metadata refusal",
                    )?;
                }
            }
            Ok(())
        })
    }

    #[test]
    fn materialized_build_preparation_storage_establishes_only_cold_build_roots()
    -> Result<(), String> {
        with_storage_fixture(|root| {
            let build = root.join("cold").join("build");
            let required = root.join("packet");
            fs::create_dir(&required).map_err(|error| error.to_string())?;
            TrustedStorageMonitor::establish_roots(
                std::slice::from_ref(&build),
                &[root.to_path_buf(), required.clone()],
            )?;
            let mut monitor =
                TrustedStorageMonitor::new(vec![build.clone(), required.clone()], 16)?;
            monitor.check_now()?;
            fs::remove_dir(&required).map_err(|error| error.to_string())?;
            require_storage_error(
                TrustedStorageMonitor::establish_roots(
                    std::slice::from_ref(&build),
                    std::slice::from_ref(&required),
                ),
                "declared root",
            )?;
            assert!(!required.exists());
            require_storage_error(monitor.check_now(), "declared root")
        })
    }

    #[test]
    fn materialized_build_preparation_storage_counts_growth_without_double_counting()
    -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-preparation-storage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos()
        ));
        fs::create_dir(&root).map_err(|error| error.to_string())?;
        let result = (|| {
            let nested = root.join("nested");
            fs::create_dir(&nested).map_err(|error| error.to_string())?;
            fs::write(root.join("existing"), [0; 8]).map_err(|error| error.to_string())?;
            let mut monitor = TrustedStorageMonitor::new(vec![root.clone(), nested.clone()], 16)?;
            fs::write(nested.join("new"), [0; 16]).map_err(|error| error.to_string())?;
            monitor.check_now()?;
            if monitor.peak_growth() != 16 || monitor.roots() != [root.clone(), nested.clone()] {
                return Err("trusted storage growth was missed or counted twice".into());
            }
            fs::write(nested.join("new"), [0; 17]).map_err(|error| error.to_string())?;
            if monitor.check_now().is_ok() {
                return Err("trusted storage overflow did not stop preparation".into());
            }
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&root).map_err(|error| error.to_string());
        result.and(cleanup)
    }

    #[test]
    fn materialized_build_preparation_storage_refuses_unknown_observation() -> Result<(), String> {
        if TrustedStorageMonitor::new(vec![std::path::Path::new("relative").to_path_buf()], 16)
            .is_ok()
            || TrustedStorageMonitor::new(Vec::new(), 16).is_ok()
            || TrustedStorageMonitor::new(vec![std::env::temp_dir()], 0).is_ok()
        {
            return Err("unknown or unbounded trusted storage scope was admitted".into());
        }
        Ok(())
    }
}
