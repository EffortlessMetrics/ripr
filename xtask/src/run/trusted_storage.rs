//! Monitored storage for trusted repository builds, not a filesystem sandbox.
//! Sizes are observed on known paths; writes between scans, open-unlinked files
//! and writes outside those paths are not subject to a hard quota here.

use std::fs;
use std::path::PathBuf;
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
        let all = roots.clone();
        roots.retain(|root| {
            !all.iter()
                .any(|other| other != root && root.starts_with(other))
        });
        let baseline = observed_bytes(&roots)?;
        Ok(Self {
            roots,
            baseline,
            maximum_growth,
            peak_growth: 0,
            next_scan: Instant::now(),
        })
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
    const ENTRIES: usize = 250_000;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut pending = roots.to_vec();
    let mut entries = 0;
    let mut bytes = 0u64;
    while let Some(path) = pending.pop() {
        if entries >= ENTRIES || Instant::now() >= deadline {
            return Err("trusted storage observation exceeded its entry/time bound".into());
        }
        entries += 1;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "trusted storage observation {}: {error}",
                    path.display()
                ));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "trusted storage observation refuses symlink {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(&path)
                .map_err(|error| format!("trusted storage directory {}: {error}", path.display()))?
            {
                if pending.len() >= ENTRIES || Instant::now() >= deadline {
                    return Err("trusted storage directory observation exceeded its bound".into());
                }
                pending.push(entry.map_err(|error| error.to_string())?.path());
            }
        } else if metadata.is_file() {
            bytes = bytes
                .checked_add(metadata.len())
                .ok_or("trusted storage size overflow")?;
        } else {
            return Err(format!(
                "trusted storage observation refuses nonregular path {}",
                path.display()
            ));
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            if monitor.peak_growth() != 16 || monitor.roots() != [root.clone()] {
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
