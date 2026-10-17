//! Materialize the retained corpus without nested Cargo packages in its inputs.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct SourceRoleCase {
    root: PathBuf,
    files: Vec<PathBuf>,
}

impl SourceRoleCase {
    pub(crate) fn load(name: &str) -> Result<Self, String> {
        // Enumerate the input closure so caches or unrelated files never enter
        // a case. Cargo.toml.fixture stays inert in the source distribution.
        let inputs: &[&str] = match name {
            "production_lib" | "cfg_variants" => &["Cargo.toml.fixture", "src/lib.rs"],
            "explicit_test_target" => &[
                "Cargo.toml.fixture",
                "src/lib.rs",
                "tests/custom/smoke_case.rs",
            ],
            "naming_lookalike" => &["Cargo.toml.fixture", "src/lib.rs", "src/price_test.rs"],
            _ => return Err(format!("unknown source-role corpus case: {name}")),
        };
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/source-role-corpus/cases")
            .join(name);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("source-role case timestamp: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-source-role-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&root)
            .map_err(|error| format!("create source-role case {}: {error}", root.display()))?;
        let mut case = Self {
            root,
            files: Vec::new(),
        };
        for &input in inputs {
            let relative = if input == "Cargo.toml.fixture" {
                "Cargo.toml"
            } else {
                input
            };
            let destination = case.root.join(relative);
            let parent = destination
                .parent()
                .ok_or_else(|| format!("corpus destination has no parent: {relative}"))?;
            fs::create_dir_all(parent).map_err(|error| {
                format!("create corpus directory {}: {error}", parent.display())
            })?;
            fs::copy(source.join(input), &destination).map_err(|error| {
                format!(
                    "materialize required source-role input {}: {error}",
                    source.join(input).display()
                )
            })?;
            case.files.push(destination);
        }
        Ok(case)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn files(&self) -> &[PathBuf] {
        &self.files
    }
}

impl Drop for SourceRoleCase {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
