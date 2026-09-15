//! Owned fixture copies keep host configuration outside corpus runs.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(0);

pub(super) struct FixtureWorkspace {
    root: PathBuf,
    removed: bool,
}

impl FixtureWorkspace {
    pub(super) fn create(fixture: &Path) -> Result<Self, String> {
        // Corpus and scaffold callers use relative paths. Preserve those exact
        // CLI paths instead of silently rewriting absolute or escaping roots.
        if fixture.as_os_str().is_empty()
            || fixture
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(format!(
                "fixture path must be a normal relative path: {}",
                fixture.display()
            ));
        }
        let parent = std::path::absolute("target/ripr/fixture-workspaces")
            .map_err(|error| format!("resolve fixture staging directory: {error}"))?;
        fs::create_dir_all(&parent)
            .map_err(|error| format!("create fixture staging directory: {error}"))?;
        let root = parent.join(format!(
            "{}-{}",
            std::process::id(),
            NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).map_err(|error| format!("create {}: {error}", root.display()))?;
        let workspace = Self {
            root,
            removed: false,
        };
        fs::create_dir(workspace.root.join(".git"))
            .map_err(|error| format!("create fixture configuration boundary: {error}"))?;
        // Copy the whole fixture so fixture-local parent config and sibling
        // inputs remain available. Never copy generated build/cache directories.
        copy_tree(fixture, &workspace.root.join(fixture))?;
        Ok(workspace)
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn cleanup(&mut self) -> Result<(), String> {
        fs::remove_dir_all(&self.root).map_err(|error| {
            format!("remove fixture workspace {}: {error}", self.root.display())
        })?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for FixtureWorkspace {
    fn drop(&mut self) {
        if !self.removed
            && let Err(error) = self.cleanup()
        {
            eprintln!("{error}");
        }
    }
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let source_kind = fs::symlink_metadata(source)
        .map_err(|error| format!("inspect fixture directory {}: {error}", source.display()))?
        .file_type();
    if !source_kind.is_dir() {
        return Err(format!(
            "unsupported fixture directory: {}",
            source.display()
        ));
    }
    fs::create_dir_all(destination)
        .map_err(|error| format!("create {}: {error}", destination.display()))?;
    for entry in
        fs::read_dir(source).map_err(|error| format!("read {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("read fixture entry: {error}"))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("inspect fixture entry: {error}"))?;
        if kind.is_dir() && entry.file_name() == "target" {
            continue;
        }
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)
                .map_err(|error| format!("copy fixture entry: {error}"))?;
        } else {
            return Err(format!(
                "unsupported fixture entry: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_retains_target_files_but_omits_target_directories() -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "ripr-fixture-copy-{}-{}",
            std::process::id(),
            NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).map_err(|error| format!("create copy fixture: {error}"))?;
        let mut owner = FixtureWorkspace {
            root: path,
            removed: false,
        };
        let result = (|| {
            let source = owner.root().join("source");
            fs::create_dir_all(source.join("nested/target"))
                .map_err(|error| format!("create target directory: {error}"))?;
            fs::write(source.join("target"), "source file")
                .map_err(|error| format!("write target file: {error}"))?;
            fs::write(source.join("nested/target/generated"), "cache")
                .map_err(|error| format!("write generated file: {error}"))?;
            let destination = owner.root().join("copy");
            copy_tree(&source, &destination)?;
            if fs::read_to_string(destination.join("target"))
                .map_err(|error| format!("read retained target file: {error}"))?
                != "source file"
                || destination.join("nested/target").exists()
            {
                return Err("fixture copy confused source files with generated directories".into());
            }
            if copy_tree(&source.join("target"), &owner.root().join("invalid")).is_ok() {
                return Err("fixture copy accepted a non-directory root".into());
            }
            Ok(())
        })();
        let cleanup = owner.cleanup();
        match (result, cleanup) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => Err(format!("{error}; {cleanup}")),
        }
    }
}
