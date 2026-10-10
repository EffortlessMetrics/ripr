//! Retained, bounded I/O for a parent-owned complete-execution source stage.
//!
//! These handles prove opened filesystem objects and bound this module's
//! retained paths and source writes. They are not analyzer admission or saved
//! evidence authority. The parent owns stage cleanup; no recursive Drop exists.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
mod artifact_io;
#[cfg(all(test, target_os = "linux", feature = "lang-rust"))]
pub(crate) use artifact_io::{
    ArtifactBudget, ArtifactClosureData, ArtifactDirectory, ArtifactFileData,
    ArtifactPayloadData, ArtifactSlot, FinishedPayloads,
};

use serde::{Deserialize, Serialize};

const SOURCE_BYTES_MAX: u64 = 512 * 1024 * 1024;
const FILE_BYTES_MAX: u64 = 256 * 1024 * 1024;

// SAME reviewed kernel ABI flags as parent complete_stage.rs.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const DIRECTORY: i32 = 0x10000;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const NOFOLLOW: i32 = 0x20000;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const DIRECTORY: i32 = 0x4000;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const NOFOLLOW: i32 = 0x8000;
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
const NONBLOCK: i32 = 0x800;

/// Serializable identity DATA. Opening and checking the actual object is separate.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectoryIdentity {
    pub(crate) path: String,
    pub(crate) dev: u64,
    pub(crate) ino: u64,
}

/// A privately held directory descriptor. Closing it never deletes its path.
pub(crate) struct RetainedDirectory {
    file: File,
    identity: DirectoryIdentity,
    path_limit: u64,
}

fn check_deadline(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        return Err("staged source deadline expired".into());
    }
    Ok(())
}

fn absolute_path(path: &str, limit: u64) -> Result<&Path, String> {
    if limit == 0 || path.len() as u64 > limit || !path.starts_with('/') {
        return Err("staged directory path exceeds its bound or is not absolute".into());
    }
    if path.as_bytes().contains(&0)
        || (path != "/"
            && path[1..]
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == ".."))
    {
        return Err("staged directory path is not canonical".into());
    }
    Ok(Path::new(path))
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn open_no_follow(path: &Path, directory: bool, create: bool) -> Result<File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    // Keep the parent staging flags and pinned std descriptor ownership.
    let flags = NOFOLLOW | NONBLOCK | if directory { DIRECTORY } else { 0 };
    let mut options = OpenOptions::new();
    options.custom_flags(flags);
    if create {
        options.write(true).create_new(true);
    } else {
        options.read(true);
    }
    options
        .open(path)
        .map_err(|error| format!("staged no-follow open failed: {error}"))
}

#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn open_no_follow(_: &Path, _: bool, _: bool) -> Result<File, String> {
    Err("staged descriptor I/O requires qualified Linux".into())
}

#[cfg(target_os = "linux")]
fn object_identity(metadata: &Metadata) -> Result<(u64, u64), String> {
    use std::os::unix::fs::MetadataExt;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(not(target_os = "linux"))]
fn object_identity(_: &Metadata) -> Result<(u64, u64), String> {
    Err("staged descriptor identity requires qualified Linux".into())
}

#[cfg(target_os = "linux")]
fn descriptor_path(file: &File) -> Result<PathBuf, String> {
    use std::os::fd::AsRawFd;
    Ok(PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd())))
}

#[cfg(not(target_os = "linux"))]
fn descriptor_path(_: &File) -> Result<PathBuf, String> {
    Err("staged descriptor paths require qualified Linux".into())
}

fn file_metadata(file: &File, directory: bool) -> Result<Metadata, String> {
    let metadata = file
        .metadata()
        .map_err(|error| format!("staged descriptor metadata failed: {error}"))?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err("staged descriptor has the wrong object type".into());
    }
    Ok(metadata)
}

impl RetainedDirectory {
    pub(crate) fn open_absolute(
        identity: &DirectoryIdentity,
        path_limit: u64,
        deadline: Instant,
    ) -> Result<Self, String> {
        check_deadline(deadline)?;
        let path = absolute_path(&identity.path, path_limit)?;
        let mut file = open_no_follow(Path::new("/"), true, false)?;
        for component in path.components() {
            if let Component::Normal(name) = component {
                check_deadline(deadline)?;
                let next = descriptor_path(&file)?.join(name);
                file = open_no_follow(&next, true, false)?;
                file_metadata(&file, true)?;
            }
        }
        let metadata = file_metadata(&file, true)?;
        if object_identity(&metadata)? != (identity.dev, identity.ino) {
            return Err("staged directory identity mismatch".into());
        }
        check_deadline(deadline)?;
        Ok(Self {
            file,
            identity: identity.clone(),
            path_limit,
        })
    }

    pub(crate) fn identity(&self) -> &DirectoryIdentity {
        &self.identity
    }

    /// The path is an identifier, never a substitute for the held descriptor.
    pub(crate) fn path_identifier(&self) -> &str {
        &self.identity.path
    }

    pub(crate) fn verify_current(&self, deadline: Instant) -> Result<(), String> {
        check_deadline(deadline)?;
        if object_identity(&file_metadata(&self.file, true)?)?
            != (self.identity.dev, self.identity.ino)
        {
            return Err("held staged directory identity changed".into());
        }
        let reopened = Self::open_absolute(&self.identity, self.path_limit, deadline)?;
        if object_identity(&file_metadata(&reopened.file, true)?)?
            != object_identity(&file_metadata(&self.file, true)?)?
        {
            return Err("staged directory path no longer names its held object".into());
        }
        check_deadline(deadline)
    }

    pub(crate) fn open_child(
        &self,
        name: &str,
        expected: &DirectoryIdentity,
        deadline: Instant,
    ) -> Result<Self, String> {
        self.verify_current(deadline)?;
        let expected_path = absolute_path(&expected.path, self.path_limit)?;
        if name.is_empty()
            || name.contains('/')
            || name == "."
            || name == ".."
            || expected_path.parent() != Some(Path::new(&self.identity.path))
            || expected_path.file_name() != Some(std::ffi::OsStr::new(name))
        {
            return Err("staged role is not the exact direct child".into());
        }
        let file = open_no_follow(&descriptor_path(&self.file)?.join(name), true, false)?;
        if object_identity(&file_metadata(&file, true)?)? != (expected.dev, expected.ino) {
            return Err("staged child directory identity mismatch".into());
        }
        self.verify_current(deadline)?;
        check_deadline(deadline)?;
        Ok(Self {
            file,
            identity: expected.clone(),
            path_limit: self.path_limit,
        })
    }

    /// Stop on the first entry; never collect an unbounded directory.
    pub(crate) fn require_empty(&self, deadline: Instant) -> Result<(), String> {
        self.verify_current(deadline)?;
        let mut entries = fs::read_dir(descriptor_path(&self.file)?)
            .map_err(|error| format!("staged directory listing failed: {error}"))?;
        if let Some(entry) = entries.next() {
            entry.map_err(|error| format!("staged directory entry failed: {error}"))?;
            return Err("staged role directory is not empty".into());
        }
        self.verify_current(deadline)
    }

    /// The startup stage has exactly these three roles, with no other entries.
    pub(crate) fn require_role_entries(&self, deadline: Instant) -> Result<(), String> {
        self.verify_current(deadline)?;
        let mut seen = [false; 3];
        let mut count = 0_u64;
        for entry in fs::read_dir(descriptor_path(&self.file)?)
            .map_err(|error| format!("staged role listing failed: {error}"))?
        {
            check_deadline(deadline)?;
            count = count.checked_add(1).ok_or("staged role count overflow")?;
            if count > 3 {
                return Err("staged root has an unexpected entry".into());
            }
            let entry = entry.map_err(|error| format!("staged role entry failed: {error}"))?;
            let name = entry.file_name();
            let slot = if name == "source" {
                0
            } else if name == "spool" {
                1
            } else if name == "artifacts" {
                2
            } else {
                return Err("staged root has an unexpected role".into());
            };
            if seen[slot] {
                return Err("staged root has a duplicate role".into());
            }
            seen[slot] = true;
        }
        if seen != [true; 3] {
            return Err("staged root is missing a role".into());
        }
        self.verify_current(deadline)
    }
}

/// Explicit caller-admitted bounds; this type has no environment/default policy.
pub(crate) struct SourceBudget {
    max_source_bytes: u64,
    max_entries: u64,
    max_path_bytes: u64,
    max_file_bytes: u64,
}

impl SourceBudget {
    pub(crate) fn new(
        max_source_bytes: u64,
        max_entries: u64,
        max_path_bytes: u64,
        max_file_bytes: u64,
    ) -> Result<Self, String> {
        if max_source_bytes == 0
            || max_source_bytes > SOURCE_BYTES_MAX
            || max_entries == 0
            || max_path_bytes == 0
            || max_file_bytes == 0
            || max_file_bytes > FILE_BYTES_MAX
        {
            return Err("invalid staged source budget".into());
        }
        for bound in [
            max_source_bytes,
            max_entries,
            max_path_bytes,
            max_file_bytes,
        ] {
            usize::try_from(bound)
                .map_err(|error| format!("staged source bound exceeds native usize: {error}"))?;
        }
        Ok(Self {
            max_source_bytes,
            max_entries,
            max_path_bytes,
            max_file_bytes,
        })
    }
}

enum SourceEntry {
    Directory {
        identity: Option<(u64, u64)>,
    },
    File {
        ordinal: u64,
        identity: Option<(u64, u64)>,
        bytes: u64,
        finished: bool,
    },
}

struct SourceUsage {
    entries: BTreeMap<PathBuf, SourceEntry>,
    path_bytes: u64,
    source_bytes: u64,
    sealed: bool,
    fault: Option<String>,
}

/// Held I/O and bounded materialization state, not an execution capability.
pub(crate) struct SourceAnchor {
    root: RetainedDirectory,
    budget: SourceBudget,
    deadline: Instant,
    usage: Mutex<SourceUsage>,
}

fn relative_path(path: &Path, limit: u64, allow_root: bool) -> Result<u64, String> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let bytes = path.as_os_str().as_bytes();
        let size = u64::try_from(bytes.len())
            .map_err(|error| format!("staged path size overflow: {error}"))?;
        if size > limit {
            return Err("staged source path byte bound exceeded".into());
        }
        if bytes.is_empty() && allow_root {
            return Ok(0);
        }
        if bytes.is_empty()
            || bytes.contains(&0)
            || bytes
                .split(|byte| *byte == b'/')
                .any(|part| part.is_empty() || part == b"." || part == b"..")
        {
            return Err("staged source path is not a canonical relative path".into());
        }
        Ok(size)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (path, limit, allow_root);
        Err("staged relative paths require qualified Linux".into())
    }
}

impl SourceAnchor {
    pub(crate) fn new(
        root: RetainedDirectory,
        budget: SourceBudget,
        deadline: Instant,
    ) -> Result<Self, String> {
        root.require_empty(deadline)?;
        let mut entries = BTreeMap::new();
        entries.insert(
            PathBuf::new(),
            SourceEntry::Directory {
                identity: Some((root.identity.dev, root.identity.ino)),
            },
        );
        Ok(Self {
            root,
            budget,
            deadline,
            usage: Mutex::new(SourceUsage {
                entries,
                path_bytes: 0,
                source_bytes: 0,
                sealed: false,
                fault: None,
            }),
        })
    }

    pub(crate) fn path_identifier(&self) -> &str {
        self.root.path_identifier()
    }

    /// Returns the original held deadline without creating a new budget.
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Observes actual untouched source I/O; the caller owns one-shot lifecycle.
    pub(crate) fn verify_fresh(&self) -> Result<(), String> {
        let result = (|| {
            self.root.verify_current(self.deadline)?;
            let usage = self.lock_usage()?;
            if usage.sealed {
                return Err("staged source materialization is already sealed".into());
            }
            if usage.entries.len() != 1 || usage.path_bytes != 0 || usage.source_bytes != 0 {
                return Err("staged source materialization is not fresh".into());
            }
            match usage.entries.get(Path::new("")) {
                Some(SourceEntry::Directory {
                    identity: Some(identity),
                }) if *identity == (self.root.identity.dev, self.root.identity.ino) => {}
                _ => return Err("staged source initial root reservation is invalid".into()),
            }
            // Retain the state lock through real emptiness/currentness checks.
            // Success neither reserves the source nor permits another lifecycle.
            self.root.require_empty(self.deadline)?;
            check_deadline(self.deadline)
        })();
        self.remember(result)
    }

    pub(crate) fn verify_roots(&self) -> Result<(), String> {
        let result = (|| {
            self.root.verify_current(self.deadline)?;
            drop(self.lock_usage()?);
            Ok(())
        })();
        self.remember(result)
    }

    fn lock_usage(&self) -> Result<std::sync::MutexGuard<'_, SourceUsage>, String> {
        let usage = self
            .usage
            .lock()
            .map_err(|error| format!("staged source state lock failed: {error}"))?;
        if let Some(fault) = &usage.fault {
            return Err(format!("staged source is unqualified: {fault}"));
        }
        Ok(usage)
    }

    fn fault(&self, error: &str) {
        if let Ok(mut usage) = self.usage.lock()
            && usage.fault.is_none()
        {
            usage.fault = Some(error.to_string());
        }
    }

    fn remember<T>(&self, result: Result<T, String>) -> Result<T, String> {
        if let Err(error) = &result {
            self.fault(error);
        }
        result
    }

    fn admit(&self, usage: &mut SourceUsage, relative: &Path, bytes: u64) -> Result<u64, String> {
        check_deadline(self.deadline)?;
        let path_bytes = relative_path(relative, self.budget.max_path_bytes, false)?;
        if usage.sealed {
            return Err("staged source materialization is already sealed".into());
        }
        if usage.entries.contains_key(relative) {
            return Err("duplicate staged source path".into());
        }
        let parent = relative
            .parent()
            .ok_or("staged source path has no parent")?;
        if !matches!(
            usage.entries.get(parent),
            Some(SourceEntry::Directory { identity: Some(_) })
        ) {
            return Err("staged source parent is not an admitted directory".into());
        }
        let count = u64::try_from(usage.entries.len())
            .map_err(|error| format!("staged source entry count overflow: {error}"))?
            .checked_add(1)
            .ok_or("staged source entry count overflow")?;
        let paths = usage
            .path_bytes
            .checked_add(path_bytes)
            .ok_or("staged source path byte count overflow")?;
        let source = usage
            .source_bytes
            .checked_add(bytes)
            .ok_or("staged source byte count overflow")?;
        if count > self.budget.max_entries {
            return Err("staged source entry bound exceeded".into());
        }
        if paths > self.budget.max_path_bytes {
            return Err("staged source retained path bound exceeded".into());
        }
        if bytes > self.budget.max_file_bytes || source > self.budget.max_source_bytes {
            return Err("staged source declared byte bound exceeded".into());
        }
        // These counters are admitted before our retained PathBuf/map growth.
        usage.path_bytes = paths;
        usage.source_bytes = source;
        Ok(count)
    }

    fn directory_locked(&self, usage: &SourceUsage, relative: &Path) -> Result<File, String> {
        relative_path(relative, self.budget.max_path_bytes, true)?;
        let mut file = self
            .root
            .file
            .try_clone()
            .map_err(|error| format!("staged directory descriptor clone failed: {error}"))?;
        let mut prefix = PathBuf::new();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err("staged directory component is not relative".into());
            };
            prefix.push(name);
            let expected = match usage.entries.get(&prefix) {
                Some(SourceEntry::Directory {
                    identity: Some(identity),
                }) => *identity,
                _ => return Err("staged directory is not admitted".into()),
            };
            check_deadline(self.deadline)?;
            file = open_no_follow(&descriptor_path(&file)?.join(name), true, false)?;
            if object_identity(&file_metadata(&file, true)?)? != expected {
                return Err("staged source directory identity changed".into());
            }
        }
        Ok(file)
    }

    fn regular_locked(&self, usage: &SourceUsage, relative: &Path) -> Result<File, String> {
        relative_path(relative, self.budget.max_path_bytes, false)?;
        let (identity, bytes) = match usage.entries.get(relative) {
            Some(SourceEntry::File {
                identity: Some(identity),
                bytes,
                finished: true,
                ..
            }) => (*identity, *bytes),
            _ => return Err("staged source file is not finished and admitted".into()),
        };
        let parent = relative
            .parent()
            .ok_or("staged source file has no parent")?;
        let directory = self.directory_locked(usage, parent)?;
        let name = relative
            .file_name()
            .ok_or("staged source file has no name")?;
        let file = open_no_follow(&descriptor_path(&directory)?.join(name), false, false)?;
        let metadata = file_metadata(&file, false)?;
        if object_identity(&metadata)? != identity || metadata.len() != bytes {
            return Err("staged source file identity or length changed".into());
        }
        Ok(file)
    }

    /// Creates only this exact directory; every ancestor must already be admitted.
    pub(crate) fn create_dir(&self, relative: &Path) -> Result<(), String> {
        let result = (|| {
            self.root.verify_current(self.deadline)?;
            let mut usage = self.lock_usage()?;
            self.admit(&mut usage, relative, 0)?;
            usage.entries.insert(
                relative.to_path_buf(),
                SourceEntry::Directory { identity: None },
            );
            let parent = relative.parent().ok_or("staged directory has no parent")?;
            let directory = self.directory_locked(&usage, parent)?;
            let name = relative.file_name().ok_or("staged directory has no name")?;
            let destination = descriptor_path(&directory)?.join(name);
            fs::create_dir(&destination)
                .map_err(|error| format!("staged directory creation failed: {error}"))?;
            let file = open_no_follow(&destination, true, false)?;
            let identity = object_identity(&file_metadata(&file, true)?)?;
            if let Some(SourceEntry::Directory { identity: slot }) = usage.entries.get_mut(relative)
            {
                *slot = Some(identity);
            } else {
                return Err("staged directory reservation disappeared".into());
            }
            check_deadline(self.deadline)
        })();
        self.remember(result)
    }

    /// No raw output File escapes the once-admitted source byte reservation.
    pub(crate) fn create_new_file(
        &self,
        relative: &Path,
        declared_bytes: u64,
    ) -> Result<BoundedSourceWriter<'_>, String> {
        let result = (|| {
            self.root.verify_current(self.deadline)?;
            let mut usage = self.lock_usage()?;
            let ordinal = self.admit(&mut usage, relative, declared_bytes)?;
            usage.entries.insert(
                relative.to_path_buf(),
                SourceEntry::File {
                    ordinal,
                    identity: None,
                    bytes: declared_bytes,
                    finished: false,
                },
            );
            let parent = relative.parent().ok_or("staged file has no parent")?;
            let directory = self.directory_locked(&usage, parent)?;
            let name = relative.file_name().ok_or("staged file has no name")?;
            let file = open_no_follow(&descriptor_path(&directory)?.join(name), false, true)?;
            let identity = object_identity(&file_metadata(&file, false)?)?;
            if let Some(SourceEntry::File { identity: slot, .. }) = usage.entries.get_mut(relative)
            {
                *slot = Some(identity);
            } else {
                return Err("staged file reservation disappeared".into());
            }
            check_deadline(self.deadline)?;
            Ok(BoundedSourceWriter {
                file,
                owner: self,
                ordinal,
                declared_bytes,
                written: 0,
                finished: false,
            })
        })();
        self.remember(result)
    }

    /// Checks already sealed source I/O without sealing or accepting saved DATA.
    pub(crate) fn verify_materialized(&self) -> Result<(), String> {
        let result = (|| {
            self.root.verify_current(self.deadline)?;
            let usage = self.lock_usage()?;
            if !usage.sealed {
                return Err("staged source materialization is not sealed".into());
            }
            for entry in usage.entries.values() {
                match entry {
                    SourceEntry::Directory { identity: Some(_) }
                    | SourceEntry::File {
                        identity: Some(_),
                        finished: true,
                        ..
                    } => {}
                    _ => return Err("staged source has an unfinished reservation".into()),
                }
            }
            check_deadline(self.deadline)
        })();
        self.remember(result)
    }

    /// Completion of source I/O only; this does not grant analyzer admission.
    pub(crate) fn finish_materialization(&self) -> Result<(), String> {
        let result = (|| {
            self.root.verify_current(self.deadline)?;
            let mut usage = self.lock_usage()?;
            for entry in usage.entries.values() {
                match entry {
                    SourceEntry::Directory { identity: Some(_) }
                    | SourceEntry::File {
                        identity: Some(_),
                        finished: true,
                        ..
                    } => {}
                    _ => return Err("staged source has an unfinished reservation".into()),
                }
            }
            usage.sealed = true;
            check_deadline(self.deadline)
        })();
        self.remember(result)
    }

    pub(crate) fn open_file(&self, relative: &Path) -> Result<File, String> {
        let result = (|| {
            self.root.verify_current(self.deadline)?;
            let usage = self.lock_usage()?;
            let file = self.regular_locked(&usage, relative)?;
            check_deadline(self.deadline)?;
            Ok(file)
        })();
        self.remember(result)
    }

    pub(crate) fn metadata(&self, relative: &Path) -> Result<Metadata, String> {
        let result = (|| {
            self.root.verify_current(self.deadline)?;
            let usage = self.lock_usage()?;
            relative_path(relative, self.budget.max_path_bytes, true)?;
            let file = match usage.entries.get(relative) {
                Some(SourceEntry::Directory { .. }) => self.directory_locked(&usage, relative)?,
                Some(SourceEntry::File { .. }) => self.regular_locked(&usage, relative)?,
                None => return Err("staged source metadata path is not admitted".into()),
            };
            let metadata = file
                .metadata()
                .map_err(|error| format!("staged source metadata failed: {error}"))?;
            check_deadline(self.deadline)?;
            Ok(metadata)
        })();
        self.remember(result)
    }

    /// Retains native names after per-call map-plus-one-list path admission.
    /// The caller separately budgets multiple simultaneously retained lists.
    /// The actual directory descriptor stays held through the whole iteration.
    pub(crate) fn read_directory(&self, relative: &Path) -> Result<Vec<OsString>, String> {
        let result = (|| {
            self.root.verify_current(self.deadline)?;
            let usage = self.lock_usage()?;
            let directory = self.directory_locked(&usage, relative)?;
            let mut names = Vec::new();
            let mut bytes = 0_u64;
            for entry in fs::read_dir(descriptor_path(&directory)?)
                .map_err(|error| format!("staged source listing failed: {error}"))?
            {
                check_deadline(self.deadline)?;
                let count = u64::try_from(names.len())
                    .map_err(|error| format!("staged source listing count overflow: {error}"))?
                    .checked_add(1)
                    .ok_or("staged source listing count overflow")?;
                if count > self.budget.max_entries {
                    return Err("staged source listing entry bound exceeded".into());
                }
                let name = entry
                    .map_err(|error| format!("staged source listing entry failed: {error}"))?
                    .file_name();
                let size = relative_path(Path::new(&name), self.budget.max_path_bytes, false)?;
                bytes = bytes
                    .checked_add(size)
                    .ok_or("staged source listing byte overflow")?;
                if usage
                    .path_bytes
                    .checked_add(bytes)
                    .ok_or("staged source listing retained byte overflow")?
                    > self.budget.max_path_bytes
                {
                    return Err("staged source listing path bound exceeded".into());
                }
                let parent_bytes = relative_path(relative, self.budget.max_path_bytes, true)?;
                let separator = u64::from(parent_bytes != 0);
                let child_bytes = parent_bytes
                    .checked_add(separator)
                    .and_then(|value| value.checked_add(size))
                    .ok_or("staged source listing child path overflow")?;
                if child_bytes > self.budget.max_path_bytes {
                    return Err("staged source listing child path bound exceeded".into());
                }
                let child = relative.join(&name);
                if !usage.entries.contains_key(&child) {
                    return Err("staged source contains an unadmitted entry".into());
                }
                names.push(name);
            }
            for child in usage
                .entries
                .keys()
                .filter(|path| path.parent() == Some(relative))
            {
                let name = child
                    .file_name()
                    .ok_or("admitted staged child has no name")?;
                if !names.iter().any(|actual| actual == name) {
                    return Err("staged source is missing an admitted child".into());
                }
            }
            self.root.verify_current(self.deadline)?;
            Ok(names)
        })();
        self.remember(result)
    }

    pub(crate) fn set_executable(&self, relative: &Path, executable: bool) -> Result<(), String> {
        let result = (|| {
            let file = self.open_file(relative)?;
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(if executable {
                    0o755
                } else {
                    0o644
                }))
                .map_err(|error| format!("staged source mode write failed: {error}"))?;
                check_deadline(self.deadline)
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (file, executable);
                Err("staged source modes require qualified Linux".into())
            }
        })();
        self.remember(result)
    }
}

/// A source blob lease. Every write is bounded before I/O; failure is sticky.
pub(crate) struct BoundedSourceWriter<'a> {
    file: File,
    owner: &'a SourceAnchor,
    ordinal: u64,
    declared_bytes: u64,
    written: u64,
    finished: bool,
}

impl BoundedSourceWriter<'_> {
    fn io_failure(&self, error: String) -> io::Error {
        self.owner.fault(&error);
        io::Error::other(error)
    }

    fn ensure_ready(&self) -> io::Result<()> {
        if let Err(error) = check_deadline(self.owner.deadline) {
            return Err(self.io_failure(error));
        }
        drop(
            self.owner
                .lock_usage()
                .map_err(|error| self.io_failure(error))?,
        );
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<(), String> {
        let result = (|| {
            drop(self.owner.lock_usage()?);
            check_deadline(self.owner.deadline)?;
            self.owner.root.verify_current(self.owner.deadline)?;
            if self.written != self.declared_bytes {
                return Err("staged source lease did not write its exact declared length".into());
            }
            let metadata = file_metadata(&self.file, false)?;
            if metadata.len() != self.declared_bytes {
                return Err("staged source lease file length differs from its declaration".into());
            }
            let held_identity = object_identity(&metadata)?;
            let mut usage = self.owner.lock_usage()?;
            let (path, entry) = usage
                .entries
                .iter()
                .find(|(_, entry)| {
                    matches!(entry, SourceEntry::File { ordinal, .. } if *ordinal == self.ordinal)
                })
                .ok_or("staged source lease reservation is missing")?;
            let expected = match entry {
                SourceEntry::File {
                    identity: Some(identity),
                    bytes,
                    finished: false,
                    ..
                } if *bytes == self.declared_bytes => *identity,
                _ => return Err("staged source lease reservation is inconsistent".into()),
            };
            let parent = path.parent().ok_or("staged lease path has no parent")?;
            let directory = self.owner.directory_locked(&usage, parent)?;
            let name = path.file_name().ok_or("staged lease path has no name")?;
            let current = open_no_follow(&descriptor_path(&directory)?.join(name), false, false)?;
            if object_identity(&file_metadata(&current, false)?)? != expected
                || held_identity != expected
            {
                return Err("staged source lease path no longer names its held file".into());
            }
            let entry = usage.entries.values_mut().find(|entry| {
                matches!(entry, SourceEntry::File { ordinal, .. } if *ordinal == self.ordinal)
            }).ok_or("staged source lease reservation disappeared")?;
            if let SourceEntry::File { finished, .. } = entry {
                *finished = true;
            }
            check_deadline(self.owner.deadline)
        })();
        let result = self.owner.remember(result);
        if result.is_ok() {
            self.finished = true;
        }
        result
    }
}

impl Write for BoundedSourceWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.ensure_ready()?;
        let count = u64::try_from(bytes.len()).map_err(|error| {
            self.io_failure(format!("staged source write length overflow: {error}"))
        })?;
        if count > self.declared_bytes.saturating_sub(self.written) {
            return Err(self.io_failure("staged source write exceeds its declared length".into()));
        }
        let written = self
            .file
            .write(bytes)
            .map_err(|error| self.io_failure(format!("staged source write failed: {error}")))?;
        self.written = self
            .written
            .checked_add(written as u64)
            .ok_or_else(|| self.io_failure("staged source written byte count overflow".into()))?;
        if let Err(error) = check_deadline(self.owner.deadline) {
            return Err(self.io_failure(error));
        }
        Ok(written)
    }

    fn write_all(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        // The trait default skips write for empty input; sticky failure must
        // remain observable even when no bytes are requested.
        self.ensure_ready()?;
        while !bytes.is_empty() {
            let written = self.write(bytes)?;
            if written == 0 {
                return Err(self.io_failure("staged source write made no progress".into()));
            }
            bytes = bytes.get(written..).ok_or_else(|| {
                self.io_failure("staged source write returned an invalid byte count".into())
            })?;
        }
        self.ensure_ready()
    }

    fn flush(&mut self) -> io::Result<()> {
        self.ensure_ready()?;
        self.file
            .flush()
            .map_err(|error| self.io_failure(format!("staged source flush failed: {error}")))?;
        if let Err(error) = check_deadline(self.owner.deadline) {
            return Err(self.io_failure(error));
        }
        Ok(())
    }
}

impl Drop for BoundedSourceWriter<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.owner
                .fault("staged source lease dropped without exact finish");
        }
    }
}

#[cfg(all(
    test,
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{MetadataExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        path: PathBuf,
        deadline: Instant,
    }

    impl Fixture {
        fn new() -> Result<Self, String> {
            let base = std::env::temp_dir()
                .canonicalize()
                .map_err(|error| format!("fixture temp root: {error}"))?;
            let path = base.join(format!(
                "ripr-staged-anchor-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).map_err(|error| format!("fixture directory: {error}"))?;
            Ok(Self {
                path,
                deadline: Instant::now() + Duration::from_secs(60),
            })
        }

        fn identity(&self, path: &Path) -> Result<DirectoryIdentity, String> {
            let metadata =
                fs::metadata(path).map_err(|error| format!("fixture metadata: {error}"))?;
            Ok(DirectoryIdentity {
                path: path
                    .to_str()
                    .ok_or("fixture path is not UTF-8")?
                    .to_string(),
                dev: metadata.dev(),
                ino: metadata.ino(),
            })
        }

        fn anchor(&self, bytes: u64, count: u64, paths: u64) -> Result<SourceAnchor, String> {
            let directory =
                RetainedDirectory::open_absolute(&self.identity(&self.path)?, 4096, self.deadline)?;
            SourceAnchor::new(
                directory,
                SourceBudget::new(bytes, count, paths, 16)?,
                self.deadline,
            )
        }

        fn cleanup(self) -> Result<(), String> {
            fs::remove_dir_all(&self.path).map_err(|error| format!("fixture cleanup: {error}"))
        }
    }

    fn error<T>(result: Result<T, String>, category: &str) -> Result<String, String> {
        match result {
            Err(error) if error.contains(category) => Ok(error),
            Err(error) => Err(format!(
                "wrong error category: {error}; expected {category}"
            )),
            Ok(_) => Err(format!("unexpected success; expected {category}")),
        }
    }

    #[test]
    fn source_exact_native_paths_write_mode_and_read_use_held_objects() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(32, 8, 256)?;
        anchor.create_dir(Path::new("src"))?;
        let native = PathBuf::from(OsString::from_vec(b"src/nonutf-\xff.rs".to_vec()));
        let mut writer = anchor.create_new_file(&native, 4)?;
        writer
            .write_all(b"data")
            .map_err(|error| format!("source write: {error}"))?;
        writer.finish()?;
        anchor.set_executable(&native, true)?;
        anchor.finish_materialization()?;
        anchor.verify_materialized()?;
        let mut actual = String::new();
        anchor
            .open_file(&native)?
            .read_to_string(&mut actual)
            .map_err(|error| format!("source read: {error}"))?;
        assert_eq!(actual, "data");
        assert_eq!(anchor.metadata(&native)?.mode() & 0o777, 0o755);
        assert_eq!(
            anchor.read_directory(Path::new("src"))?,
            vec![
                native
                    .file_name()
                    .ok_or("native file name missing")?
                    .to_os_string()
            ]
        );
        assert_eq!(
            anchor.path_identifier(),
            fixture.path.to_str().ok_or("fixture path invalid")?
        );
        drop(anchor);
        fixture.cleanup()
    }

    #[test]
    fn writer_refuses_growth_before_io_and_failure_stays_unqualified() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 4, 256)?;
        let mut writer = anchor.create_new_file(Path::new("blob"), 2)?;
        match writer.write(b"abc") {
            Err(error) => assert!(error.to_string().contains("exceeds its declared length")),
            Ok(bytes) => return Err(format!("oversized write unexpectedly wrote {bytes} bytes")),
        }
        assert_eq!(
            fs::metadata(fixture.path.join("blob"))
                .map_err(|error| format!("written file metadata: {error}"))?
                .len(),
            0
        );
        match writer.write_all(b"") {
            Err(error) => assert!(error.to_string().contains("unqualified")),
            Ok(()) => return Err("empty write_all ignored a real oversized-write fault".into()),
        }
        match writer.flush() {
            Err(error) => assert!(error.to_string().contains("unqualified")),
            Ok(()) => return Err("flush ignored a real oversized-write fault".into()),
        }
        error(writer.finish(), "unqualified")?;
        error(anchor.finish_materialization(), "unqualified")?;
        error(anchor.verify_materialized(), "unqualified")?;
        error(anchor.create_new_file(Path::new("next"), 1), "unqualified")?;
        drop(anchor);
        fixture.cleanup()
    }

    #[test]
    fn incomplete_or_dropped_lease_cannot_qualify_source() -> Result<(), String> {
        for finish in [true, false] {
            let fixture = Fixture::new()?;
            let anchor = fixture.anchor(4, 4, 256)?;
            let mut writer = anchor.create_new_file(Path::new("blob"), 2)?;
            writer
                .write_all(b"a")
                .map_err(|error| format!("short source write: {error}"))?;
            if finish {
                error(writer.finish(), "exact declared length")?;
            } else {
                drop(writer);
            }
            error(anchor.finish_materialization(), "unqualified")?;
            drop(anchor);
            fixture.cleanup()?;
        }
        Ok(())
    }

    #[test]
    fn budgets_duplicates_and_file_directory_overlap_refuse_before_creation() -> Result<(), String>
    {
        for (bytes, count, paths, path, declared, category) in [
            (1, 4, 256, "large", 2, "declared byte bound"),
            (32, 1, 256, "entry", 1, "entry bound"),
            (32, 4, 3, "long", 1, "path byte bound"),
            (32, 4, 256, "../outside", 1, "canonical relative"),
        ] {
            let fixture = Fixture::new()?;
            let anchor = fixture.anchor(bytes, count, paths)?;
            error(anchor.create_new_file(Path::new(path), declared), category)?;
            assert_eq!(
                fs::read_dir(&fixture.path)
                    .map_err(|error| format!("budget listing: {error}"))?
                    .count(),
                0
            );
            drop(anchor);
            fixture.cleanup()?;
        }
        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        let writer = anchor.create_new_file(Path::new("file"), 0)?;
        writer.finish()?;
        error(
            anchor.create_dir(Path::new("file")),
            "duplicate staged source path",
        )?;
        drop(anchor);
        fixture.cleanup()?;

        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        let writer = anchor.create_new_file(Path::new("file"), 0)?;
        writer.finish()?;
        error(
            anchor.create_new_file(Path::new("file/child"), 1),
            "parent is not an admitted directory",
        )?;
        drop(anchor);
        fixture.cleanup()
    }

    #[test]
    fn symlink_ancestor_wrong_type_and_identity_replacement_are_refused() -> Result<(), String> {
        let fixture = Fixture::new()?;
        fs::create_dir(fixture.path.join("real")).map_err(|error| format!("real dir: {error}"))?;
        symlink("real", fixture.path.join("alias")).map_err(|error| format!("link: {error}"))?;
        let mut link_identity = fixture.identity(&fixture.path.join("real"))?;
        link_identity.path = fixture
            .path
            .join("alias")
            .to_str()
            .ok_or("alias path invalid")?
            .into();
        error(
            RetainedDirectory::open_absolute(&link_identity, 4096, fixture.deadline),
            "no-follow open",
        )?;
        fs::create_dir(fixture.path.join("real/child"))
            .map_err(|error| format!("real child: {error}"))?;
        let mut nested_identity = fixture.identity(&fixture.path.join("real/child"))?;
        nested_identity.path = fixture
            .path
            .join("alias/child")
            .to_str()
            .ok_or("nested alias invalid")?
            .into();
        error(
            RetainedDirectory::open_absolute(&nested_identity, 4096, fixture.deadline),
            "no-follow open",
        )?;
        fs::write(fixture.path.join("regular"), b"")
            .map_err(|error| format!("regular file: {error}"))?;
        error(
            RetainedDirectory::open_absolute(
                &fixture.identity(&fixture.path.join("regular"))?,
                4096,
                fixture.deadline,
            ),
            "no-follow open",
        )?;
        fixture.cleanup()?;

        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        anchor.create_dir(Path::new("dir"))?;
        fs::rename(fixture.path.join("dir"), fixture.path.join("old"))
            .map_err(|error| format!("rename directory: {error}"))?;
        fs::create_dir(fixture.path.join("dir"))
            .map_err(|error| format!("replace dir: {error}"))?;
        error(
            anchor.metadata(Path::new("dir")),
            "directory identity changed",
        )?;
        drop(anchor);
        fixture.cleanup()
    }

    #[test]
    fn file_replacement_and_unlisted_directory_entries_are_refused() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        let mut writer = anchor.create_new_file(Path::new("file"), 1)?;
        writer
            .write_all(b"a")
            .map_err(|error| format!("file write: {error}"))?;
        writer.finish()?;
        fs::rename(fixture.path.join("file"), fixture.path.join("old"))
            .map_err(|error| format!("rename file: {error}"))?;
        fs::write(fixture.path.join("file"), b"a")
            .map_err(|error| format!("replace file: {error}"))?;
        error(
            anchor.open_file(Path::new("file")),
            "identity or length changed",
        )?;
        drop(anchor);
        fixture.cleanup()?;

        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        fs::write(fixture.path.join("unexpected"), b"")
            .map_err(|error| format!("unexpected file: {error}"))?;
        error(anchor.read_directory(Path::new("")), "unadmitted entry")?;
        drop(anchor);
        fixture.cleanup()
    }

    #[test]
    fn listing_paths_share_retained_budget_and_deadlines_are_not_reset() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 3)?;
        let writer = anchor.create_new_file(Path::new("abc"), 0)?;
        writer.finish()?;
        error(anchor.read_directory(Path::new("")), "listing path bound")?;
        drop(anchor);
        fixture.cleanup()?;

        let fixture = Fixture::new()?;
        error(
            RetainedDirectory::open_absolute(
                &fixture.identity(&fixture.path)?,
                4096,
                Instant::now(),
            ),
            "deadline expired",
        )?;
        fixture.cleanup()
    }

    #[test]
    fn stage_role_listing_is_closed_and_empty_roles_stop_at_first_entry() -> Result<(), String> {
        let fixture = Fixture::new()?;
        for name in ["source", "spool", "artifacts"] {
            fs::create_dir(fixture.path.join(name))
                .map_err(|error| format!("role mkdir: {error}"))?;
        }
        let root = RetainedDirectory::open_absolute(
            &fixture.identity(&fixture.path)?,
            4096,
            fixture.deadline,
        )?;
        root.require_role_entries(fixture.deadline)?;
        let source = root.open_child(
            "source",
            &fixture.identity(&fixture.path.join("source"))?,
            fixture.deadline,
        )?;
        source.require_empty(fixture.deadline)?;
        fs::write(fixture.path.join("source/entry"), b"")
            .map_err(|error| format!("role file: {error}"))?;
        error(source.require_empty(fixture.deadline), "not empty")?;
        fs::write(fixture.path.join("extra"), b"")
            .map_err(|error| format!("extra file: {error}"))?;
        error(root.require_role_entries(fixture.deadline), "unexpected")?;
        drop(source);
        drop(root);
        fixture.cleanup()
    }

    #[test]
    fn held_directory_refuses_same_path_replacement_and_does_not_delete_stage() -> Result<(), String>
    {
        let fixture = Fixture::new()?;
        fs::create_dir(fixture.path.join("held"))
            .map_err(|error| format!("held mkdir: {error}"))?;
        let identity = fixture.identity(&fixture.path.join("held"))?;
        let retained = RetainedDirectory::open_absolute(&identity, 4096, fixture.deadline)?;
        fs::rename(fixture.path.join("held"), fixture.path.join("old"))
            .map_err(|error| format!("held rename: {error}"))?;
        fs::create_dir(fixture.path.join("held"))
            .map_err(|error| format!("held replace: {error}"))?;
        error(
            retained.verify_current(fixture.deadline),
            "directory identity mismatch",
        )?;
        drop(retained);
        assert!(
            fs::metadata(fixture.path.join("old"))
                .map_err(|error| format!("retained stage after drop: {error}"))?
                .is_dir()
        );
        fixture.cleanup()
    }

    #[test]
    fn materialized_getter_requires_prior_sealing_and_current_held_root() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        let writer = anchor.create_new_file(Path::new("file"), 0)?;
        writer.finish()?;
        error(anchor.verify_materialized(), "not sealed")?;
        // The read-only observation refused rather than quietly sealing it.
        error(anchor.finish_materialization(), "unqualified")?;
        drop(anchor);
        fixture.cleanup()?;

        let fixture = Fixture::new()?;
        fs::create_dir(fixture.path.join("source"))
            .map_err(|error| format!("source mkdir: {error}"))?;
        let directory = RetainedDirectory::open_absolute(
            &fixture.identity(&fixture.path.join("source"))?,
            4096,
            fixture.deadline,
        )?;
        let anchor = SourceAnchor::new(
            directory,
            SourceBudget::new(4, 8, 256, 4)?,
            fixture.deadline,
        )?;
        anchor.finish_materialization()?;
        anchor.verify_materialized()?;
        fs::rename(fixture.path.join("source"), fixture.path.join("old"))
            .map_err(|error| format!("source rename: {error}"))?;
        fs::create_dir(fixture.path.join("source"))
            .map_err(|error| format!("source replace: {error}"))?;
        error(anchor.verify_materialized(), "directory identity mismatch")?;
        drop(anchor);
        fixture.cleanup()
    }

    #[test]
    fn fresh_getter_refuses_admitted_or_sealed_reuse_and_new_source_recovers() -> Result<(), String>
    {
        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        assert_eq!(anchor.deadline(), fixture.deadline);
        anchor.verify_fresh()?;
        anchor.verify_fresh()?;
        anchor.create_dir(Path::new("src"))?;
        error(anchor.verify_fresh(), "not fresh")?;
        error(anchor.finish_materialization(), "unqualified")?;
        drop(anchor);
        fixture.cleanup()?;

        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        let writer = anchor.create_new_file(Path::new("file"), 1)?;
        error(anchor.verify_fresh(), "not fresh")?;
        drop(writer);
        error(anchor.verify_fresh(), "unqualified")?;
        drop(anchor);
        fixture.cleanup()?;

        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        anchor.finish_materialization()?;
        error(anchor.verify_fresh(), "already sealed")?;
        error(anchor.verify_fresh(), "unqualified")?;
        drop(anchor);
        fixture.cleanup()?;

        // Recovery requires another actual fresh source; no state is reset.
        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        anchor.verify_fresh()?;
        let mut writer = anchor.create_new_file(Path::new("file"), 4)?;
        writer
            .write_all(b"head")
            .map_err(|error| format!("fresh recovery write: {error}"))?;
        writer.finish()?;
        anchor.finish_materialization()?;
        anchor.verify_materialized()?;
        assert_eq!(
            fs::read(fixture.path.join("file"))
                .map_err(|error| format!("fresh recovery read: {error}"))?,
            b"head"
        );
        drop(anchor);
        fixture.cleanup()
    }

    #[test]
    fn fresh_getter_refuses_unadmitted_entries_and_replaced_held_root() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let anchor = fixture.anchor(4, 8, 256)?;
        anchor.verify_fresh()?;
        let unexpected = fixture.path.join("unexpected");
        fs::write(&unexpected, b"entry")
            .map_err(|error| format!("fresh unexpected write: {error}"))?;
        error(anchor.verify_fresh(), "role directory is not empty")?;
        fs::remove_file(&unexpected)
            .map_err(|error| format!("fresh unexpected removal: {error}"))?;
        error(anchor.verify_fresh(), "unqualified")?;
        drop(anchor);
        fixture.cleanup()?;

        let fixture = Fixture::new()?;
        let path = fixture.path.join("source");
        fs::create_dir(&path).map_err(|error| format!("fresh source mkdir: {error}"))?;
        let directory =
            RetainedDirectory::open_absolute(&fixture.identity(&path)?, 4096, fixture.deadline)?;
        let anchor = SourceAnchor::new(
            directory,
            SourceBudget::new(4, 8, 256, 4)?,
            fixture.deadline,
        )?;
        anchor.verify_fresh()?;
        fs::rename(&path, fixture.path.join("old"))
            .map_err(|error| format!("fresh source rename: {error}"))?;
        fs::create_dir(&path).map_err(|error| format!("fresh source replacement: {error}"))?;
        error(anchor.verify_fresh(), "directory identity mismatch")?;
        drop(anchor);
        fixture.cleanup()
    }
}
