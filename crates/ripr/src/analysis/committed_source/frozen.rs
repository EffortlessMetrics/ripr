//! Private, inactive complete-generation source authority.
//!
//! Inventory bytes are authenticated during named-tree materialization. Reads
//! keep logical repository paths, never fall back to the live tree, and poison
//! the authority on an unexpected physical file or a request outside that tree.
//! This is a source boundary, not complete coverage or aggregate resource proof.

use super::staged::SourceAnchor;
use crate::domain::GitObjectId;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrozenFileMode {
    Regular,
    Executable,
}

impl FrozenFileMode {
    pub(crate) const fn git_mode(self) -> &'static str {
        match self {
            Self::Regular => "100644",
            Self::Executable => "100755",
        }
    }
}

#[derive(Clone)]
pub(crate) struct FrozenFile {
    pub(crate) mode: FrozenFileMode,
    pub(crate) blob_oid: GitObjectId,
    pub(crate) size: u64,
    pub(crate) sha256: [u8; 32],
}

pub(crate) struct FrozenInventory {
    files: BTreeMap<PathBuf, FrozenFile>,
    directories: BTreeSet<PathBuf>,
}

impl Default for FrozenInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl FrozenInventory {
    pub(crate) fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            directories: BTreeSet::from([PathBuf::new()]),
        }
    }

    /// Authenticated root-relative records in deterministic path order.
    pub(crate) fn files(
        &self,
    ) -> impl ExactSizeIterator<Item = (&Path, &FrozenFile)> + DoubleEndedIterator {
        self.files.iter().map(|(path, file)| (path.as_path(), file))
    }

    /// Directory presence includes the root and authenticated empty trees.
    pub(crate) fn directories(&self) -> impl ExactSizeIterator<Item = &Path> + DoubleEndedIterator {
        self.directories.iter().map(PathBuf::as_path)
    }

    pub(crate) fn insert_directory(&mut self, path: &Path) {
        let mut current = Some(path);
        while let Some(directory) = current {
            self.directories.insert(directory.to_path_buf());
            current = directory.parent();
        }
    }

    pub(crate) fn insert_file(&mut self, path: PathBuf, file: FrozenFile) {
        if let Some(parent) = path.parent() {
            self.insert_directory(parent);
        }
        self.files.insert(path, file);
    }
}

enum FrozenOwner {
    Temporary(Arc<super::super::git_candidate_execution::TempRootGuard>),
    ParentStaged(Arc<SourceAnchor>),
}

struct Fault {
    kind: io::ErrorKind,
    message: String,
}

pub(crate) struct FrozenSourceAuthority {
    logical_root: PathBuf,
    physical_root: PathBuf,
    tree: GitObjectId,
    configuration: super::super::git_candidate_execution::CapturedConfiguration,
    inventory: Arc<FrozenInventory>,
    // Temporary ownership retains its original cleanup. A staged lease never
    // deletes the parent-owned source namespace.
    _owner: FrozenOwner,
    fault: Mutex<Option<Fault>>,
    #[cfg(test)]
    worker_contexts: std::sync::atomic::AtomicU8,
    #[cfg(test)]
    opened_reads: std::sync::atomic::AtomicUsize,
}

fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        metadata.file_attributes() & 0x0000_0400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn normalized_absolute(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "source path climbs above its filesystem root",
                    ));
                }
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

impl FrozenSourceAuthority {
    pub(crate) fn new(
        logical_root: &Path,
        physical_root: &Path,
        tree: GitObjectId,
        configuration: super::super::git_candidate_execution::CapturedConfiguration,
        inventory: Arc<FrozenInventory>,
        owner: Arc<super::super::git_candidate_execution::TempRootGuard>,
    ) -> io::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            logical_root: normalized_absolute(logical_root)?,
            physical_root: normalized_absolute(physical_root)?,
            tree,
            configuration,
            inventory,
            _owner: FrozenOwner::Temporary(owner),
            fault: Mutex::new(None),
            #[cfg(test)]
            worker_contexts: std::sync::atomic::AtomicU8::new(0),
            #[cfg(test)]
            opened_reads: std::sync::atomic::AtomicUsize::new(0),
        }))
    }

    /// Wraps actual already sealed source I/O. The caller still supplies the
    /// authenticated Git inventory/configuration/tree and their admitted bounds.
    pub(crate) fn new_staged(
        logical_root: &Path,
        tree: GitObjectId,
        configuration: super::super::git_candidate_execution::CapturedConfiguration,
        inventory: Arc<FrozenInventory>,
        owner: Arc<SourceAnchor>,
    ) -> io::Result<Arc<Self>> {
        owner.verify_materialized().map_err(io::Error::other)?;
        let authority = Arc::new(Self {
            logical_root: normalized_absolute(logical_root)?,
            // Identifier DATA only. Every staged I/O branch uses owner handles.
            physical_root: PathBuf::from(owner.path_identifier()),
            tree,
            configuration,
            inventory,
            _owner: FrozenOwner::ParentStaged(owner),
            fault: Mutex::new(None),
            #[cfg(test)]
            worker_contexts: std::sync::atomic::AtomicU8::new(0),
            #[cfg(test)]
            opened_reads: std::sync::atomic::AtomicUsize::new(0),
        });
        // Reconcile both namespaces before lending any frozen source context.
        // Each list is dropped before the next; its aggregate budget remains
        // separate from the root caller's full inventory/other retained buffers.
        for directory in &authority.inventory.directories {
            authority.inspect(directory)?;
            let FrozenOwner::ParentStaged(owner) = &authority._owner else {
                return Err(io::Error::other("staged source owner is missing"));
            };
            let names = owner
                .read_directory(directory)
                .map_err(|error| authority.changed(directory, &error))?;
            for name in names {
                let child = directory.join(name);
                if !authority.known(&child) {
                    return Err(authority.changed(&child, "unexpected staged snapshot entry"));
                }
            }
        }
        for file in authority.inventory.files.keys() {
            authority.inspect(file)?;
        }
        authority.verify_staged_current()?;
        Ok(authority)
    }

    fn verify_staged_current(&self) -> io::Result<()> {
        if let FrozenOwner::ParentStaged(owner) = &self._owner {
            self.ensure_clean()?;
            owner
                .verify_materialized()
                .map_err(|error| self.changed(Path::new(""), &error))?;
        }
        Ok(())
    }

    pub(crate) fn logical_root(&self) -> &Path {
        &self.logical_root
    }

    pub(crate) fn inventory(&self) -> &FrozenInventory {
        &self.inventory
    }

    pub(crate) fn captured_configuration(
        &self,
    ) -> &super::super::git_candidate_execution::CapturedConfiguration {
        &self.configuration
    }

    pub(crate) fn head_tree(&self) -> &GitObjectId {
        &self.tree
    }

    fn refuse(&self, kind: io::ErrorKind, detail: String) -> io::Error {
        match self.fault.lock() {
            Ok(mut fault) => {
                let first = fault.get_or_insert(Fault {
                    kind,
                    message: detail,
                });
                io::Error::new(first.kind, first.message.clone())
            }
            Err(_) => io::Error::other("frozen source fault state is poisoned"),
        }
    }

    pub(crate) fn refuse_external_effect(&self, detail: &str) -> io::Error {
        self.refuse(
            io::ErrorKind::PermissionDenied,
            format!("frozen source refuses unbound external effect: {detail}"),
        )
    }

    pub(crate) fn ensure_clean(&self) -> io::Result<()> {
        let fault = self.fault.lock().map_err(|error| {
            io::Error::other(format!("frozen source fault state is poisoned: {error}"))
        })?;
        match &*fault {
            Some(fault) => Err(io::Error::new(fault.kind, fault.message.clone())),
            None => Ok(()),
        }
    }

    /// Checked generation closeout. A detached worker or another scoped lease
    /// prevents completion; ordinary Drop remains the fallback on every error.
    pub(crate) fn finalize(self: Arc<Self>) -> io::Result<()> {
        let authority = match Arc::try_unwrap(self) {
            Ok(authority) => authority,
            Err(authority) => {
                return Err(authority.refuse(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "frozen snapshot cleanup has {} outstanding source-context leases",
                        Arc::strong_count(&authority).saturating_sub(1)
                    ),
                ));
            }
        };
        let primary = authority.ensure_clean().err();
        let owner = match authority._owner {
            FrozenOwner::Temporary(owner) => owner,
            FrozenOwner::ParentStaged(owner) => {
                // NativeStartup intentionally retains its own source Arc.
                // Release this lease without unwrapping or deleting the stage.
                let verification = owner.verify_materialized().map_err(io::Error::other);
                return match (primary, verification) {
                    (None, Ok(())) => Ok(()),
                    (Some(primary), Ok(())) => Err(primary),
                    (primary, Err(verification)) => Err(io::Error::new(
                        primary
                            .as_ref()
                            .map_or(verification.kind(), io::Error::kind),
                        match primary {
                            Some(primary) => format!(
                                "{primary}; frozen staged source verification failed: {verification}"
                            ),
                            None => {
                                format!("frozen staged source verification failed: {verification}")
                            }
                        },
                    )),
                };
            }
        };
        let owner = Arc::try_unwrap(owner).map_err(|owner| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "frozen snapshot cleanup has {} outstanding materialization leases",
                    Arc::strong_count(&owner).saturating_sub(1)
                ),
            )
        })?;
        let cleanup = owner.checked_cleanup();
        match (primary, cleanup) {
            (None, Ok(())) => Ok(()),
            (Some(primary), Ok(())) => Err(primary),
            (primary, Err(cleanup)) => Err(io::Error::new(
                primary.as_ref().map_or(cleanup.kind(), io::Error::kind),
                match primary {
                    Some(primary) => {
                        format!("{primary}; frozen snapshot checked cleanup failed: {cleanup}")
                    }
                    None => format!("frozen snapshot checked cleanup failed: {cleanup}"),
                },
            )),
        }
    }

    fn relative(&self, requested: &Path) -> io::Result<PathBuf> {
        self.ensure_clean()?;
        let logical = normalized_absolute(requested).map_err(|error| {
            self.refuse(error.kind(), format!("invalid frozen source path: {error}"))
        })?;
        logical
            .strip_prefix(&self.logical_root)
            .map(Path::to_path_buf)
            .map_err(|error| {
                self.refuse(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "frozen source request is outside repository: {} ({error})",
                        logical.display()
                    ),
                )
            })
    }

    fn known(&self, relative: &Path) -> bool {
        self.inventory.files.contains_key(relative) || self.inventory.directories.contains(relative)
    }

    fn absent(relative: &Path) -> io::Error {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "path is absent from frozen named tree: {}",
                relative.display()
            ),
        )
    }

    fn inspect(&self, relative: &Path) -> io::Result<std::fs::Metadata> {
        self.ensure_clean()?;
        if !self.known(relative) {
            return Err(Self::absent(relative));
        }
        if let FrozenOwner::ParentStaged(owner) = &self._owner {
            owner
                .verify_materialized()
                .map_err(|error| self.changed(relative, &error))?;
            let metadata = owner
                .metadata(relative)
                .map_err(|error| self.changed(relative, &error))?;
            if let Some(file) = self.inventory.files.get(relative) {
                if !metadata.file_type().is_file() || metadata.len() != file.size {
                    return Err(self.changed(relative, "admitted file type or size changed"));
                }
            } else if !metadata.file_type().is_dir() {
                return Err(self.changed(relative, "admitted directory type changed"));
            }
            owner
                .verify_materialized()
                .map_err(|error| self.changed(relative, &error))?;
            return Ok(metadata);
        }
        // Validate all physical parents before touching the requested file.
        // The snapshot is privately owned; concurrent external mutation is not
        // a supported writer contract. File opens also reject final-component
        // swaps and validate the actual opened handle.
        let mut physical = self.physical_root.clone();
        let root_metadata = std::fs::symlink_metadata(&physical)
            .map_err(|error| self.physical_error(relative, error))?;
        if is_link_like(&root_metadata) || !root_metadata.file_type().is_dir() {
            return Err(self.changed(relative, "snapshot root is not a directory"));
        }
        let mut metadata = root_metadata;
        for component in relative.components() {
            physical.push(component.as_os_str());
            metadata = std::fs::symlink_metadata(&physical)
                .map_err(|error| self.physical_error(relative, error))?;
            if is_link_like(&metadata) {
                return Err(self.changed(relative, "snapshot contains an unexpected symlink"));
            }
            if physical != self.physical_root.join(relative) && !metadata.file_type().is_dir() {
                return Err(self.changed(relative, "snapshot parent is not a directory"));
            }
        }
        if let Some(file) = self.inventory.files.get(relative) {
            if !metadata.file_type().is_file() || metadata.len() != file.size {
                return Err(self.changed(relative, "admitted file type or size changed"));
            }
        } else if !metadata.file_type().is_dir() {
            return Err(self.changed(relative, "admitted directory type changed"));
        }
        Ok(metadata)
    }

    fn changed(&self, relative: &Path, detail: &str) -> io::Error {
        self.refuse(
            io::ErrorKind::InvalidData,
            format!(
                "frozen source mismatch at {}: {detail}",
                self.logical_root.join(relative).display()
            ),
        )
    }

    fn physical_error(&self, relative: &Path, error: io::Error) -> io::Error {
        self.refuse(
            error.kind(),
            format!(
                "frozen source inaccessible at {}: {error}",
                self.logical_root.join(relative).display()
            ),
        )
    }

    pub(crate) fn verify_loaded(&self, requested: &Path, bytes: &[u8]) -> io::Result<()> {
        let relative = self.relative(requested)?;
        let Some(expected) = self.inventory.files.get(&relative) else {
            return Err(self.changed(&relative, "loaded input is not an admitted file"));
        };
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if bytes.len() as u64 != expected.size || digest != expected.sha256 {
            return Err(self.changed(&relative, "loaded bytes differ from admitted blob"));
        }
        self.ensure_clean()
    }

    fn read_verified(&self, requested: &Path, retain: u64) -> io::Result<Vec<u8>> {
        let relative = self.relative(requested)?;
        self.inspect(&relative)?;
        let Some(expected) = self.inventory.files.get(&relative) else {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                "frozen source is a directory",
            ));
        };
        #[cfg(test)]
        self.opened_reads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut file = match &self._owner {
            FrozenOwner::Temporary(_) => {
                let physical = self.physical_root.join(&relative);
                open_source_no_follow(&physical)
                    .map_err(|error| self.physical_error(&relative, error))?
            }
            FrozenOwner::ParentStaged(owner) => {
                owner
                    .verify_materialized()
                    .map_err(|error| self.changed(&relative, &error))?;
                owner
                    .open_file(&relative)
                    .map_err(|error| self.changed(&relative, &error))?
            }
        };
        let opened = file
            .metadata()
            .map_err(|error| self.physical_error(&relative, error))?;
        if is_link_like(&opened) || !opened.file_type().is_file() || opened.len() != expected.size {
            return Err(self.changed(&relative, "opened file type or size changed"));
        }
        let mut bytes = Vec::new();
        let mut hasher = Sha256::new();
        let mut consumed = 0_u64;
        let mut scratch = [0_u8; 64 * 1024];
        loop {
            let count = file
                .read(&mut scratch)
                .map_err(|error| self.physical_error(&relative, error))?;
            if count == 0 {
                break;
            }
            consumed = consumed
                .checked_add(count as u64)
                .ok_or_else(|| self.changed(&relative, "source length overflow"))?;
            if consumed > expected.size {
                return Err(self.changed(&relative, "source exceeds admitted size"));
            }
            hasher.update(&scratch[..count]);
            let keep = retain.saturating_sub(bytes.len() as u64).min(count as u64) as usize;
            bytes.try_reserve_exact(keep).map_err(|error| {
                self.changed(
                    &relative,
                    &format!("source buffer reservation failed: {error}"),
                )
            })?;
            bytes.extend_from_slice(&scratch[..keep]);
        }
        let digest: [u8; 32] = hasher.finalize().into();
        if consumed != expected.size || digest != expected.sha256 {
            return Err(self.changed(
                &relative,
                &format!(
                    "source bytes differ from admitted blob {}",
                    expected.blob_oid.as_str()
                ),
            ));
        }
        self.ensure_clean()?;
        self.verify_staged_current()?;
        Ok(bytes)
    }

    #[cfg(test)]
    pub(crate) fn record_worker_context(&self, phase: u8) {
        self.worker_contexts
            .fetch_or(phase, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn worker_contexts(&self) -> u8 {
        self.worker_contexts
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

fn open_source_no_follow(path: &Path) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(0x0002_0000 | 0x0000_0800);
    }
    #[cfg(all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(0x0000_0100 | 0x0000_0004);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(
        windows,
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(
            target_os = "macos",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    )))]
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "safe frozen no-follow open unsupported on this target",
        ));
    }
    options.open(path)
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<FrozenSourceAuthority>>> = const { RefCell::new(None) };
}

pub(crate) fn current() -> Option<Arc<FrozenSourceAuthority>> {
    CURRENT.with(|slot| slot.borrow().clone())
}

pub(crate) fn with_context<T>(
    authority: Option<Arc<FrozenSourceAuthority>>,
    work: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<Arc<FrozenSourceAuthority>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|slot| {
                *slot.borrow_mut() = self.0.take();
            });
        }
    }
    let previous = CURRENT.with(|slot| slot.replace(authority));
    let _restore = Restore(previous);
    work()
}

pub(crate) mod fs {
    use super::*;

    pub(crate) fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        match current() {
            Some(authority) => authority.read_verified(path.as_ref(), u64::MAX),
            None => std::fs::read(path),
        }
    }

    pub(crate) fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
        match current() {
            Some(authority) => {
                let bytes = authority.read_verified(path.as_ref(), u64::MAX)?;
                String::from_utf8(bytes)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.utf8_error()))
            }
            None => std::fs::read_to_string(path),
        }
    }

    /// Retain the caller's existing limit-plus-one sentinel while verifying
    /// every admitted byte. Callers retain their original oversize decision.
    pub(crate) fn read_with_limit(path: impl AsRef<Path>, limit: u64) -> io::Result<Vec<u8>> {
        match current() {
            Some(authority) => authority.read_verified(path.as_ref(), limit.saturating_add(1)),
            None => {
                let mut bytes = Vec::new();
                std::fs::File::open(path)?
                    .take(limit.saturating_add(1))
                    .read_to_end(&mut bytes)?;
                Ok(bytes)
            }
        }
    }

    pub(crate) fn read_prefix(path: impl AsRef<Path>, limit: u64) -> io::Result<Vec<u8>> {
        match current() {
            Some(authority) => authority.read_verified(path.as_ref(), limit),
            None => {
                let mut bytes = Vec::new();
                std::fs::File::open(path)?
                    .take(limit)
                    .read_to_end(&mut bytes)?;
                Ok(bytes)
            }
        }
    }

    pub(crate) fn metadata(path: impl AsRef<Path>) -> io::Result<std::fs::Metadata> {
        match current() {
            Some(authority) => authority.inspect(&authority.relative(path.as_ref())?),
            None => std::fs::metadata(path),
        }
    }

    pub(crate) fn symlink_metadata(path: impl AsRef<Path>) -> io::Result<std::fs::Metadata> {
        match current() {
            Some(authority) => authority.inspect(&authority.relative(path.as_ref())?),
            None => std::fs::symlink_metadata(path),
        }
    }

    pub(crate) fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
        match current() {
            Some(authority) => {
                let relative = authority.relative(path.as_ref())?;
                authority.inspect(&relative)?;
                Ok(authority.logical_root.join(relative))
            }
            None => std::fs::canonicalize(path),
        }
    }

    pub(crate) fn exists(path: impl AsRef<Path>) -> bool {
        match current() {
            Some(_) => metadata(path).is_ok(),
            None => path.as_ref().exists(),
        }
    }

    pub(crate) fn is_file(path: impl AsRef<Path>) -> bool {
        match current() {
            Some(_) => metadata(path).is_ok_and(|metadata| metadata.is_file()),
            None => path.as_ref().is_file(),
        }
    }

    pub(crate) fn is_dir(path: impl AsRef<Path>) -> bool {
        match current() {
            Some(_) => metadata(path).is_ok_and(|metadata| metadata.is_dir()),
            None => path.as_ref().is_dir(),
        }
    }

    pub(crate) enum FrozenReadDir {
        Ordinary(std::fs::ReadDir),
        ParentStaged {
            inner: std::vec::IntoIter<OsString>,
            authority: Arc<FrozenSourceAuthority>,
            relative: PathBuf,
            failed: bool,
            seen: BTreeSet<PathBuf>,
        },
        NamedTree {
            inner: std::fs::ReadDir,
            authority: Arc<FrozenSourceAuthority>,
            relative: PathBuf,
            failed: bool,
            seen: BTreeSet<PathBuf>,
        },
    }

    pub(crate) enum FrozenDirEntry {
        Ordinary(std::fs::DirEntry),
        ParentStaged {
            authority: Arc<FrozenSourceAuthority>,
            relative: PathBuf,
            name: OsString,
        },
        NamedTree {
            inner: std::fs::DirEntry,
            authority: Arc<FrozenSourceAuthority>,
            relative: PathBuf,
        },
    }

    pub(crate) fn read_dir(path: impl AsRef<Path>) -> io::Result<FrozenReadDir> {
        match current() {
            Some(authority) => {
                let relative = authority.relative(path.as_ref())?;
                if !authority.inspect(&relative)?.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotADirectory,
                        "frozen source is a file",
                    ));
                }
                if let FrozenOwner::ParentStaged(owner) = &authority._owner {
                    owner
                        .verify_materialized()
                        .map_err(|error| authority.changed(&relative, &error))?;
                    let names = owner
                        .read_directory(&relative)
                        .map_err(|error| authority.changed(&relative, &error))?;
                    return Ok(FrozenReadDir::ParentStaged {
                        inner: names.into_iter(),
                        authority,
                        relative,
                        failed: false,
                        seen: BTreeSet::new(),
                    });
                }
                let inner = std::fs::read_dir(authority.physical_root.join(&relative))
                    .map_err(|error| authority.physical_error(&relative, error))?;
                Ok(FrozenReadDir::NamedTree {
                    inner,
                    authority,
                    relative,
                    failed: false,
                    seen: BTreeSet::new(),
                })
            }
            None => std::fs::read_dir(path).map(FrozenReadDir::Ordinary),
        }
    }

    impl Iterator for FrozenReadDir {
        type Item = io::Result<FrozenDirEntry>;

        fn next(&mut self) -> Option<Self::Item> {
            match self {
                Self::Ordinary(inner) => inner
                    .next()
                    .map(|entry| entry.map(FrozenDirEntry::Ordinary)),
                Self::ParentStaged {
                    inner,
                    authority,
                    relative,
                    failed,
                    seen,
                } => {
                    if *failed {
                        return None;
                    }
                    if let Err(error) = authority.verify_staged_current() {
                        *failed = true;
                        return Some(Err(error));
                    }
                    let Some(name) = inner.next() else {
                        // Release the exhausted Vec allocation before another
                        // bounded actual list; do not retain two list buffers.
                        drop(std::mem::replace(inner, Vec::new().into_iter()));
                        let closeout = (|| {
                            let FrozenOwner::ParentStaged(owner) = &authority._owner else {
                                return Err(io::Error::other("staged iterator owner is missing"));
                            };
                            let actual = owner
                                .read_directory(relative)
                                .map_err(|error| authority.changed(relative, &error))?;
                            for name in actual {
                                let child = relative.join(name);
                                if !authority.known(&child) || !seen.contains(&child) {
                                    return Err(authority
                                        .changed(&child, "staged snapshot membership changed"));
                                }
                                authority.inspect(&child)?;
                            }
                            if let Some(missing) = authority
                                .inventory
                                .files
                                .keys()
                                .chain(authority.inventory.directories.iter())
                                .find(|path| {
                                    path.parent() == Some(relative.as_path())
                                        && !seen.contains(*path)
                                })
                            {
                                return Err(authority.changed(
                                    missing,
                                    "admitted staged snapshot child is missing",
                                ));
                            }
                            authority.verify_staged_current()
                        })();
                        *failed = true;
                        return match closeout {
                            Ok(()) => None,
                            Err(error) => Some(Err(error)),
                        };
                    };
                    let result = (|| {
                        let child = relative.join(&name);
                        if !authority.known(&child) {
                            return Err(
                                authority.changed(&child, "unexpected staged snapshot entry")
                            );
                        }
                        authority.inspect(&child)?;
                        seen.insert(child.clone());
                        Ok(FrozenDirEntry::ParentStaged {
                            authority: authority.clone(),
                            relative: child,
                            name,
                        })
                    })();
                    if result.is_err() {
                        *failed = true;
                    }
                    Some(result)
                }
                Self::NamedTree {
                    inner,
                    authority,
                    relative,
                    failed,
                    seen,
                } => {
                    if *failed {
                        return None;
                    }
                    if let Err(error) = authority.ensure_clean() {
                        *failed = true;
                        return Some(Err(error));
                    }
                    let Some(entry) = inner.next() else {
                        let missing = authority
                            .inventory
                            .files
                            .keys()
                            .chain(authority.inventory.directories.iter())
                            .find(|path| {
                                path.parent() == Some(relative.as_path()) && !seen.contains(*path)
                            });
                        *failed = true;
                        return missing.map(|path| {
                            Err(authority.changed(path, "admitted snapshot child is missing"))
                        });
                    };
                    let result = (|| {
                        let inner =
                            entry.map_err(|error| authority.physical_error(relative, error))?;
                        let child = relative.join(inner.file_name());
                        if !authority.known(&child) {
                            return Err(authority.changed(&child, "unexpected snapshot entry"));
                        }
                        seen.insert(child.clone());
                        Ok(FrozenDirEntry::NamedTree {
                            inner,
                            authority: authority.clone(),
                            relative: child,
                        })
                    })();
                    if result.is_err() {
                        *failed = true;
                    }
                    Some(result)
                }
            }
        }
    }

    impl FrozenDirEntry {
        pub(crate) fn path(&self) -> PathBuf {
            match self {
                Self::Ordinary(inner) => inner.path(),
                Self::NamedTree {
                    authority,
                    relative,
                    ..
                }
                | Self::ParentStaged {
                    authority,
                    relative,
                    ..
                } => authority.logical_root.join(relative),
            }
        }

        pub(crate) fn file_name(&self) -> OsString {
            match self {
                Self::Ordinary(inner) | Self::NamedTree { inner, .. } => inner.file_name(),
                Self::ParentStaged { name, .. } => name.clone(),
            }
        }

        pub(crate) fn file_type(&self) -> io::Result<std::fs::FileType> {
            match self {
                Self::Ordinary(inner) => inner.file_type(),
                Self::NamedTree {
                    authority,
                    relative,
                    ..
                }
                | Self::ParentStaged {
                    authority,
                    relative,
                    ..
                } => authority
                    .inspect(relative)
                    .map(|metadata| metadata.file_type()),
            }
        }

        #[cfg(test)]
        pub(crate) fn metadata(&self) -> io::Result<std::fs::Metadata> {
            match self {
                Self::Ordinary(inner) => inner.metadata(),
                Self::NamedTree {
                    authority,
                    relative,
                    ..
                }
                | Self::ParentStaged {
                    authority,
                    relative,
                    ..
                } => authority.inspect(relative),
            }
        }
    }
}

thread_local! {
    static CANONICAL_DIFF: RefCell<Option<Arc<str>>> = const { RefCell::new(None) };
}

pub(crate) fn canonical_diff() -> Option<Arc<str>> {
    CANONICAL_DIFF.with(|slot| slot.borrow().clone())
}

pub(crate) fn with_canonical_diff<T>(diff: Arc<str>, work: impl FnOnce() -> T) -> T {
    struct Restore(Option<Arc<str>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CANONICAL_DIFF.with(|slot| {
                *slot.borrow_mut() = self.0.take();
            });
        }
    }
    let previous = CANONICAL_DIFF.with(|slot| slot.replace(Some(diff)));
    let _restore = Restore(previous);
    work()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::error::Error;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    /// A core-only owned byte fixture. Production authentication is tested
    /// separately through prepare_named_tree and its actual Git blob stream.
    pub(crate) struct Fixture {
        pub(crate) logical: PathBuf,
        pub(crate) physical: PathBuf,
        pub(crate) authority: Arc<FrozenSourceAuthority>,
    }

    impl Fixture {
        pub(crate) fn new(files: &[(&str, &[u8])]) -> Result<Self, Box<dyn Error>> {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "ripr-frozen-source-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst),
            ));
            let owner = Arc::new(
                super::super::super::git_candidate_execution::TempRootGuard::for_test(root.clone()),
            );
            let logical = root.join("logical");
            let physical = root.join("physical");
            std::fs::create_dir_all(&logical)?;
            std::fs::create_dir_all(&physical)?;
            let mut inventory = FrozenInventory::new();
            for (path, bytes) in files {
                let relative = PathBuf::from(path);
                let destination = physical.join(&relative);
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(destination, bytes)?;
                inventory.insert_file(
                    relative,
                    FrozenFile {
                        mode: FrozenFileMode::Regular,
                        blob_oid: GitObjectId::parse("1111111111111111111111111111111111111111")
                            .map_err(|error| io::Error::other(error.to_string()))?,
                        size: bytes.len() as u64,
                        sha256: Sha256::digest(bytes).into(),
                    },
                );
            }
            let authority = FrozenSourceAuthority::new(
                &logical,
                &physical,
                GitObjectId::parse("1111111111111111111111111111111111111111")
                    .map_err(|error| io::Error::other(error.to_string()))?,
                super::super::super::git_candidate_execution::CapturedConfiguration::Absent,
                Arc::new(inventory),
                owner,
            )?;
            Ok(Self {
                logical,
                physical,
                authority,
            })
        }
    }

    #[test]
    fn active_frozen_source_precedes_legacy_overlay_and_preserves_ordinary_reads()
    -> Result<(), Box<dyn Error>> {
        let fixture = Fixture::new(&[("a.rs", b"named head")])?;
        std::fs::write(fixture.logical.join("a.rs"), b"live")?;
        let overlay = Arc::new(super::super::CommittedSourceOverlay {
            root: fixture.logical.clone(),
            entries: BTreeMap::from([("a.rs".into(), Some(b"legacy overlay".to_vec()))]),
            ..Default::default()
        });
        super::super::with_overlay(Some(overlay), || -> io::Result<()> {
            assert_eq!(
                super::super::read_source_bytes(&fixture.logical, Path::new("a.rs"))?,
                Some(b"legacy overlay".to_vec())
            );
            assert_eq!(fs::read(fixture.logical.join("a.rs"))?, b"live");
            with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
                assert_eq!(
                    super::super::read_source_bytes(&fixture.logical, Path::new("a.rs"))?,
                    Some(b"named head".to_vec())
                );
                assert_eq!(
                    super::super::read_source_bytes(&fixture.logical, Path::new("absent.rs"))?,
                    None
                );
                fixture.authority.ensure_clean()
            })
        })?;
        Ok(())
    }

    #[test]
    fn frozen_paths_allow_internal_siblings_and_refuse_escape_before_open()
    -> Result<(), Box<dyn Error>> {
        let fixture = Fixture::new(&[("one/a.rs", b"one"), ("two/b.rs", b"two")])?;
        std::fs::write(fixture.logical.join("live-only.rs"), b"live")?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            assert_eq!(fs::read(fixture.logical.join("one/../two/b.rs"))?, b"two");
            assert!(!fs::exists(fixture.logical.join("live-only.rs")));
            fixture.authority.ensure_clean()?;
            let before = fixture.authority.opened_reads.load(Ordering::SeqCst);
            let refused = fs::read(fixture.logical.join("../outside.rs"));
            let refused = refused
                .err()
                .ok_or_else(|| io::Error::other("outside source requests must fail"))?;
            assert_eq!(refused.kind(), io::ErrorKind::PermissionDenied);
            assert_eq!(
                fixture.authority.opened_reads.load(Ordering::SeqCst),
                before,
                "outside requests must refuse before the actual open boundary"
            );
            let failure = fixture
                .authority
                .ensure_clean()
                .err()
                .ok_or_else(|| io::Error::other("authority must retain source refusal"))?;
            assert!(!failure.to_string().is_empty());
            let failure = fs::read(fixture.logical.join("one/a.rs"))
                .err()
                .ok_or_else(|| io::Error::other("poisoned authority cannot resume source reads"))?;
            assert_eq!(failure.kind(), io::ErrorKind::PermissionDenied);
            Ok(())
        })?;
        assert!(current().is_none());
        Ok(())
    }

    #[test]
    fn frozen_missing_admitted_file_and_same_size_tamper_poison_but_absence_does_not()
    -> Result<(), Box<dyn Error>> {
        let missing = Fixture::new(&[("src/a.rs", b"head")])?;
        with_context(Some(missing.authority.clone()), || -> io::Result<()> {
            assert!(!fs::exists(missing.logical.join("absent.rs")));
            missing.authority.ensure_clean()?;
            std::fs::remove_file(missing.physical.join("src/a.rs"))?;
            assert!(!fs::is_file(missing.logical.join("src/a.rs")));
            let failure =
                missing.authority.ensure_clean().err().ok_or_else(|| {
                    io::Error::other("missing admitted source must poison authority")
                })?;
            assert_eq!(failure.kind(), io::ErrorKind::NotFound);
            let failure = super::super::read_source_bytes(&missing.logical, Path::new("src/a.rs"))
                .err()
                .ok_or_else(|| {
                    io::Error::other("missing admitted files must not become successful absence")
                })?;
            assert_eq!(failure.kind(), io::ErrorKind::NotFound);
            Ok(())
        })?;
        let changed = Fixture::new(&[("src/a.rs", b"head")])?;
        std::fs::write(changed.physical.join("src/a.rs"), b"evil")?;
        with_context(Some(changed.authority.clone()), || -> io::Result<()> {
            let failure = fs::read(changed.logical.join("src/a.rs"))
                .err()
                .ok_or_else(|| io::Error::other("same-size source replacement must fail"))?;
            assert_eq!(failure.kind(), io::ErrorKind::InvalidData);
            Ok(())
        })?;
        let failure = changed
            .authority
            .ensure_clean()
            .err()
            .ok_or_else(|| io::Error::other("same-size tamper must remain fatal"))?;
        assert_eq!(failure.kind(), io::ErrorKind::InvalidData);
        Ok(())
    }

    #[test]
    fn bounded_prefix_reads_verify_tail_and_keep_original_limit_sentinel()
    -> Result<(), Box<dyn Error>> {
        let fixture = Fixture::new(&[("large.rs", b"abcdefgh")])?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            let path = fixture.logical.join("large.rs");
            assert_eq!(fs::read_prefix(&path, 2)?, b"ab");
            assert_eq!(fs::read_with_limit(&path, 2)?, b"abc");
            assert_eq!(fs::read_with_limit(&path, 8)?, b"abcdefgh");
            std::fs::write(fixture.physical.join("large.rs"), b"abcdefgX")?;
            let failure = fs::read_prefix(&path, 2)
                .err()
                .ok_or_else(|| io::Error::other("prefix read must authenticate changed tail"))?;
            assert_eq!(failure.kind(), io::ErrorKind::InvalidData);
            let failure = fixture
                .authority
                .ensure_clean()
                .err()
                .ok_or_else(|| io::Error::other("changed prefix tail refusal must stick"))?;
            assert_eq!(failure.kind(), io::ErrorKind::InvalidData);
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn frozen_directory_entries_and_canonical_paths_keep_logical_identity()
    -> Result<(), Box<dyn Error>> {
        let fixture = Fixture::new(&[("src/a.rs", b"a"), ("src/b.rs", b"b")])?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            let mut entries: Vec<_> = fs::read_dir(fixture.logical.join("src"))?
                .map(|entry| {
                    let entry = entry?;
                    assert!(entry.file_type()?.is_file());
                    assert_eq!(entry.metadata()?.len(), 1);
                    Ok(entry.path())
                })
                .collect::<io::Result<_>>()?;
            entries.sort();
            assert_eq!(
                entries,
                vec![
                    fixture.logical.join("src/a.rs"),
                    fixture.logical.join("src/b.rs")
                ]
            );
            assert_eq!(
                fs::canonicalize(fixture.logical.join("src/../src/a.rs"))?,
                fixture.logical.join("src/a.rs")
            );
            fixture.authority.ensure_clean()
        })?;
        Ok(())
    }

    #[test]
    fn checked_cleanup_requires_last_lease_and_reports_actual_removal_failure()
    -> Result<(), Box<dyn Error>> {
        let normal = Fixture::new(&[("a.rs", b"head")])?;
        let physical = normal.physical.clone();
        normal.authority.finalize()?;
        assert!(
            !physical.exists(),
            "checked cleanup must remove the snapshot"
        );

        let busy = Fixture::new(&[("a.rs", b"head")])?;
        let lease = busy.authority.clone();
        let failure = busy.authority.finalize().err().ok_or_else(|| {
            io::Error::other("an outstanding source lease must prevent checked completion")
        })?;
        assert_eq!(failure.kind(), io::ErrorKind::WouldBlock);
        let retained = lease.ensure_clean().err().ok_or_else(|| {
            io::Error::other("early cleanup refusal must remain fatal to retained workers")
        })?;
        assert_eq!(retained.kind(), io::ErrorKind::WouldBlock);
        drop(lease);

        let blocked = Fixture::new(&[("a.rs", b"head")])?;
        let owned_root = blocked
            .logical
            .parent()
            .ok_or("fixture has no parent")?
            .to_path_buf();
        std::fs::remove_dir_all(&owned_root)?;
        std::fs::write(&owned_root, b"owned cleanup failure fixture")?;
        let failure = blocked.authority.finalize().err().ok_or_else(|| {
            io::Error::other("a root replaced by a file must fail actual checked cleanup")
        })?;
        assert!(failure.to_string().contains("checked cleanup failed"));
        std::fs::remove_file(&owned_root)?;
        Ok(())
    }

    #[test]
    fn directory_eof_reconciles_deleted_admitted_children() -> Result<(), Box<dyn Error>> {
        let fixture = Fixture::new(&[("src/a.rs", b"a"), ("src/b.rs", b"b")])?;
        std::fs::remove_file(fixture.physical.join("src/a.rs"))?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            let failure = fs::read_dir(fixture.logical.join("src"))?
                .collect::<io::Result<Vec<_>>>()
                .err()
                .ok_or_else(|| io::Error::other("missing admitted child was silently omitted"))?;
            assert_eq!(failure.kind(), io::ErrorKind::InvalidData);
            assert!(
                failure
                    .to_string()
                    .contains("admitted snapshot child is missing")
            );
            let retained = fixture.authority.ensure_clean().err().ok_or_else(|| {
                io::Error::other("directory EOF omission must poison the authority")
            })?;
            assert_eq!(retained.kind(), io::ErrorKind::InvalidData);
            Ok(())
        })?;
        Ok(())
    }

    #[test]
    fn frozen_context_and_owned_diff_restore_after_early_failure() -> Result<(), Box<dyn Error>> {
        let outer = Fixture::new(&[("a.rs", b"outer")])?;
        let inner = Fixture::new(&[("a.rs", b"inner")])?;
        with_context(Some(outer.authority.clone()), || -> io::Result<()> {
            with_canonical_diff(Arc::from("outer diff"), || -> io::Result<()> {
                let refused: io::Result<()> = with_context(Some(inner.authority.clone()), || {
                    with_canonical_diff(Arc::from("inner diff"), || {
                        assert_eq!(canonical_diff().as_deref(), Some("inner diff"));
                        Err(inner.authority.refuse_external_effect("fixture helper"))
                    })
                });
                let refused = refused
                    .err()
                    .ok_or_else(|| io::Error::other("unbound helper must fail"))?;
                assert_eq!(refused.kind(), io::ErrorKind::PermissionDenied);
                assert!(current().is_some_and(|current| Arc::ptr_eq(&current, &outer.authority)));
                assert_eq!(canonical_diff().as_deref(), Some("outer diff"));
                outer.authority.ensure_clean()?;
                let failure = inner
                    .authority
                    .ensure_clean()
                    .err()
                    .ok_or_else(|| io::Error::other("inner helper refusal must stick"))?;
                assert_eq!(failure.kind(), io::ErrorKind::PermissionDenied);
                Ok(())
            })
        })?;
        assert!(current().is_none());
        assert!(canonical_diff().is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn frozen_replacement_link_refuses_before_source_open() -> Result<(), Box<dyn Error>> {
        let fixture = Fixture::new(&[("a.rs", b"head")])?;
        let outside = fixture.logical.join("outside.rs");
        std::fs::write(&outside, b"evil")?;
        std::fs::remove_file(fixture.physical.join("a.rs"))?;
        std::os::unix::fs::symlink(&outside, fixture.physical.join("a.rs"))?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            let failure = fs::read(fixture.logical.join("a.rs"))
                .err()
                .ok_or_else(|| io::Error::other("snapshot replacement link must fail"))?;
            assert_eq!(failure.kind(), io::ErrorKind::InvalidData);
            Ok(())
        })?;
        assert_eq!(
            fixture.authority.opened_reads.load(Ordering::SeqCst),
            0,
            "replacement links must refuse before the source open"
        );
        let failure = fixture
            .authority
            .ensure_clean()
            .err()
            .ok_or_else(|| io::Error::other("snapshot refusal must remain fatal"))?;
        assert_eq!(failure.kind(), io::ErrorKind::InvalidData);
        Ok(())
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    struct StagedFixture {
        root: PathBuf,
        logical: PathBuf,
        physical: PathBuf,
        anchor: Arc<super::super::staged::SourceAnchor>,
        authority: Arc<FrozenSourceAuthority>,
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    impl StagedFixture {
        // Owned byte I/O only; synthetic object IDs never claim real Git,
        // native resource custody, analyzer admission or whole coverage.
        fn new(files: &[(PathBuf, &[u8])]) -> Result<Self, Box<dyn Error>> {
            use super::super::staged::{
                DirectoryIdentity, RetainedDirectory, SourceAnchor, SourceBudget,
            };
            use std::io::Write;
            use std::os::unix::fs::MetadataExt;
            let base = std::env::temp_dir().canonicalize()?;
            let root = base.join(format!(
                "ripr-frozen-staged-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst),
            ));
            std::fs::create_dir(&root)?;
            let logical = root.join("logical");
            let physical = root.join("source");
            std::fs::create_dir(&logical)?;
            std::fs::create_dir(&physical)?;
            let metadata = std::fs::metadata(&physical)?;
            let identity = DirectoryIdentity {
                path: physical
                    .to_str()
                    .ok_or("staged fixture path is not UTF-8")?
                    .into(),
                dev: metadata.dev(),
                ino: metadata.ino(),
            };
            let deadline = std::time::Instant::now() + std::time::Duration::from_mins(1);
            let anchor = Arc::new(SourceAnchor::new(
                RetainedDirectory::open_absolute(&identity, 4096, deadline)?,
                SourceBudget::new(1024 * 1024, 100, 32 * 1024, 1024 * 1024)?,
                deadline,
            )?);
            let mut inventory = FrozenInventory::new();
            for (path, bytes) in files {
                if let Some(parent) = path.parent() {
                    let mut prefix = PathBuf::new();
                    for component in parent.components() {
                        prefix.push(component.as_os_str());
                        if !inventory.directories.contains(&prefix) {
                            anchor.create_dir(&prefix)?;
                            inventory.insert_directory(&prefix);
                        }
                    }
                }
                let mut writer = anchor.create_new_file(path, bytes.len() as u64)?;
                writer.write_all(bytes)?;
                writer.finish()?;
                inventory.insert_file(
                    path.clone(),
                    FrozenFile {
                        mode: FrozenFileMode::Regular,
                        blob_oid: GitObjectId::parse("1111111111111111111111111111111111111111")
                            .map_err(|error| io::Error::other(error.to_string()))?,
                        size: bytes.len() as u64,
                        sha256: Sha256::digest(bytes).into(),
                    },
                );
            }
            anchor.finish_materialization()?;
            let authority = FrozenSourceAuthority::new_staged(
                &logical,
                GitObjectId::parse("1111111111111111111111111111111111111111")
                    .map_err(|error| io::Error::other(error.to_string()))?,
                super::super::super::git_candidate_execution::CapturedConfiguration::Absent,
                Arc::new(inventory),
                anchor.clone(),
            )?;
            Ok(Self {
                root,
                logical,
                physical,
                anchor,
                authority,
            })
        }

        fn cleanup(self) -> Result<(), Box<dyn Error>> {
            let Self {
                root,
                anchor,
                authority,
                ..
            } = self;
            drop(authority);
            drop(anchor);
            std::fs::remove_dir_all(root)?;
            Ok(())
        }
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    fn staged_error<T>(result: io::Result<T>, category: &str) -> Result<io::Error, Box<dyn Error>> {
        match result {
            Err(error) if error.to_string().contains(category) => Ok(error),
            Err(error) => Err(format!("wrong staged error: {error}; expected {category}").into()),
            Ok(_) => Err(format!("unexpected staged success; expected {category}").into()),
        }
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn staged_context_reads_named_bytes_and_preserves_real_entry_metadata()
    -> Result<(), Box<dyn Error>> {
        use std::os::unix::ffi::OsStringExt;
        let native = PathBuf::from(OsString::from_vec(b"src/native-\xff.rs".to_vec()));
        let fixture = StagedFixture::new(&[
            ("src/a.rs".into(), b"named head"),
            (native.clone(), b"native"),
        ])?;
        std::fs::create_dir(fixture.logical.join("src"))?;
        std::fs::write(fixture.logical.join("src/a.rs"), b"live")?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            assert_eq!(fs::read(fixture.logical.join("src/a.rs"))?, b"named head");
            std::fs::remove_file(fixture.logical.join("src/a.rs"))?;
            assert_eq!(fs::read(fixture.logical.join("src/a.rs"))?, b"named head");
            assert_eq!(fs::read(fixture.logical.join(&native))?, b"native");
            let mut names = Vec::new();
            for entry in fs::read_dir(fixture.logical.join("src"))? {
                let entry = entry?;
                assert!(entry.file_type()?.is_file());
                assert!(entry.metadata()?.is_file());
                assert_eq!(
                    entry.path().parent(),
                    Some(fixture.logical.join("src").as_path())
                );
                names.push(entry.file_name());
            }
            names.sort();
            let mut expected = vec![
                OsString::from("a.rs"),
                native
                    .file_name()
                    .ok_or_else(|| io::Error::other("native name missing"))?
                    .to_os_string(),
            ];
            expected.sort();
            assert_eq!(names, expected);
            fixture.authority.ensure_clean()
        })?;
        fixture.cleanup()
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn staged_prefix_authenticates_full_tail_and_escape_refuses_before_open()
    -> Result<(), Box<dyn Error>> {
        let fixture = StagedFixture::new(&[("large.rs".into(), b"abcdefgh")])?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            assert_eq!(fs::read_prefix(fixture.logical.join("large.rs"), 2)?, b"ab");
            assert_eq!(
                fs::read_with_limit(fixture.logical.join("large.rs"), 2)?,
                b"abc"
            );
            std::fs::write(fixture.physical.join("large.rs"), b"abcdefgX")?;
            let error = staged_error(
                fs::read_prefix(fixture.logical.join("large.rs"), 2),
                "source bytes differ",
            )
            .map_err(|error| io::Error::other(error.to_string()))?;
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            Ok(())
        })?;
        fixture.cleanup()?;

        let fixture = StagedFixture::new(&[("a.rs".into(), b"head")])?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            let before = fixture.authority.opened_reads.load(Ordering::SeqCst);
            let error = staged_error(
                fs::read(fixture.logical.join("../outside.rs")),
                "outside repository",
            )
            .map_err(|error| io::Error::other(error.to_string()))?;
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert_eq!(
                fixture.authority.opened_reads.load(Ordering::SeqCst),
                before
            );
            Ok(())
        })?;
        fixture.cleanup()
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn staged_listing_rechecks_actual_membership_at_eof_and_refuses_replacement()
    -> Result<(), Box<dyn Error>> {
        let fixture = StagedFixture::new(&[("a.rs".into(), b"head")])?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            let mut entries = fs::read_dir(&fixture.logical)?;
            entries
                .next()
                .ok_or_else(|| io::Error::other("actual staged entry missing"))??;
            std::fs::write(fixture.physical.join("extra.rs"), b"extra")?;
            let closeout = entries
                .next()
                .ok_or_else(|| io::Error::other("EOF missed actual extra"))?;
            let error = staged_error(closeout, "unadmitted entry")
                .map_err(|error| io::Error::other(error.to_string()))?;
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            Ok(())
        })?;
        fixture.cleanup()?;

        let fixture = StagedFixture::new(&[("a.rs".into(), b"head")])?;
        std::fs::rename(fixture.physical.join("a.rs"), fixture.physical.join("old"))?;
        std::fs::write(fixture.physical.join("a.rs"), b"head")?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            staged_error(
                fs::read(fixture.logical.join("a.rs")),
                "identity or length changed",
            )
            .map_err(|error| io::Error::other(error.to_string()))?;
            Ok(())
        })?;
        fixture.cleanup()?;

        let fixture = StagedFixture::new(&[("a.rs".into(), b"head")])?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            let mut entries = fs::read_dir(&fixture.logical)?;
            entries
                .next()
                .ok_or_else(|| io::Error::other("actual staged entry missing"))??;
            std::fs::remove_file(fixture.physical.join("a.rs"))?;
            let closeout = entries
                .next()
                .ok_or_else(|| io::Error::other("EOF missed actual missing file"))?;
            staged_error(closeout, "missing an admitted child")
                .map_err(|error| io::Error::other(error.to_string()))?;
            Ok(())
        })?;
        fixture.cleanup()
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn staged_constructor_reconciles_inventory_and_refuses_unsealed_source()
    -> Result<(), Box<dyn Error>> {
        use super::super::staged::{
            DirectoryIdentity, RetainedDirectory, SourceAnchor, SourceBudget,
        };
        use std::os::unix::fs::MetadataExt;
        let fixture = StagedFixture::new(&[("a.rs".into(), b"head")])?;
        let empty = FrozenInventory::new();
        staged_error(
            FrozenSourceAuthority::new_staged(
                &fixture.logical,
                GitObjectId::parse("1111111111111111111111111111111111111111")
                    .map_err(|error| io::Error::other(error.to_string()))?,
                super::super::super::git_candidate_execution::CapturedConfiguration::Absent,
                Arc::new(empty),
                fixture.anchor.clone(),
            ),
            "unexpected staged snapshot entry",
        )?;
        fixture.cleanup()?;

        let fixture = StagedFixture::new(&[])?;
        let path = fixture.root.join("unsealed");
        std::fs::create_dir(&path)?;
        let metadata = std::fs::metadata(&path)?;
        let identity = DirectoryIdentity {
            path: path.to_str().ok_or("unsealed path invalid")?.into(),
            dev: metadata.dev(),
            ino: metadata.ino(),
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_mins(1);
        let unsealed = Arc::new(SourceAnchor::new(
            RetainedDirectory::open_absolute(&identity, 4096, deadline)?,
            SourceBudget::new(1024, 10, 1024, 1024)?,
            deadline,
        )?);
        staged_error(
            FrozenSourceAuthority::new_staged(
                &fixture.logical,
                GitObjectId::parse("1111111111111111111111111111111111111111")
                    .map_err(|error| io::Error::other(error.to_string()))?,
                super::super::super::git_candidate_execution::CapturedConfiguration::Absent,
                Arc::new(FrozenInventory::new()),
                unsealed.clone(),
            ),
            "not sealed",
        )?;
        drop(unsealed);
        fixture.cleanup()
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn staged_finalize_refuses_detached_context_and_never_deletes_parent_stage()
    -> Result<(), Box<dyn Error>> {
        let fixture = StagedFixture::new(&[("a.rs".into(), b"head")])?;
        // The SourceAnchor Arc is intentionally still held, like NativeStartup.
        let StagedFixture {
            root,
            logical: _,
            physical,
            anchor,
            authority,
        } = fixture;
        authority.finalize()?;
        assert_eq!(std::fs::read(physical.join("a.rs"))?, b"head");
        anchor.verify_materialized()?;
        drop(anchor);
        std::fs::remove_dir_all(root)?;

        let fixture = StagedFixture::new(&[("a.rs".into(), b"head")])?;
        let detached = fixture.authority.clone();
        staged_error(detached.finalize(), "outstanding source-context leases")?;
        assert_eq!(std::fs::read(fixture.physical.join("a.rs"))?, b"head");
        staged_error(
            fixture.authority.ensure_clean(),
            "outstanding source-context leases",
        )?;
        fixture.cleanup()?;

        let fixture = StagedFixture::new(&[("a.rs".into(), b"head")])?;
        std::fs::write(fixture.physical.join("a.rs"), b"evil")?;
        with_context(Some(fixture.authority.clone()), || -> io::Result<()> {
            staged_error(
                fs::read(fixture.logical.join("a.rs")),
                "source bytes differ",
            )
            .map_err(|error| io::Error::other(error.to_string()))?;
            Ok(())
        })?;
        let StagedFixture {
            root,
            logical: _,
            physical,
            anchor,
            authority,
        } = fixture;
        staged_error(authority.finalize(), "source bytes differ")?;
        assert_eq!(std::fs::read(physical.join("a.rs"))?, b"evil");
        drop(anchor);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
