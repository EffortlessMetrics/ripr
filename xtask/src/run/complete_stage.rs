//! Parent-owned staging identifiers and bounded filesystem observations (#1627).
//! No saved data, inventory, native exit or Drop grants cleanup authority.
//! Qualified settlement and the semantic owner's exact manifest are still required.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StageRoleBudget {
    pub(crate) max_files: u64,
    pub(crate) max_directories: u64,
    pub(crate) max_name_bytes: u64,
    pub(crate) max_depth: u32,
    pub(crate) max_logical_bytes: u64,
    pub(crate) max_allocated_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StageBudget {
    pub(crate) max_files: u64,
    pub(crate) max_directories: u64,
    pub(crate) max_name_bytes: u64,
    pub(crate) max_depth: u32,
    pub(crate) max_logical_bytes: u64,
    pub(crate) max_allocated_bytes: u64,
    pub(crate) source: StageRoleBudget,
    pub(crate) spool: StageRoleBudget,
    pub(crate) artifacts: StageRoleBudget,
    /// Bounds checked traversal progress; cannot preempt a blocked kernel syscall.
    pub(crate) inventory_timeout: Duration,
}

impl StageBudget {
    fn aggregate(self) -> StageRoleBudget {
        StageRoleBudget {
            max_files: self.max_files,
            max_directories: self.max_directories,
            max_name_bytes: self.max_name_bytes,
            max_depth: self.max_depth,
            max_logical_bytes: self.max_logical_bytes,
            max_allocated_bytes: self.max_allocated_bytes,
        }
    }
    fn validate(self) -> Result<(), String> {
        let aggregate = self.aggregate();
        for limits in [aggregate, self.source, self.spool, self.artifacts] {
            if [
                limits.max_files,
                limits.max_directories,
                limits.max_name_bytes,
                limits.max_logical_bytes,
                limits.max_allocated_bytes,
            ]
            .contains(&u64::MAX)
                || limits.max_depth == u32::MAX
            {
                return Err("stage budget contains an unlimited sentinel".to_string());
            }
        }
        if self.max_directories < 4
            || self.max_name_bytes < 142 + 6 + 5 + 9
            || self.max_depth < 1
            || self.inventory_timeout.is_zero()
            || std::time::Instant::now()
                .checked_add(self.inventory_timeout)
                .is_none()
        {
            return Err(
                "stage budget lacks root/role capacity or a finite inventory deadline".to_string(),
            );
        }
        for (name, role) in [
            ("source", self.source),
            ("spool", self.spool),
            ("artifacts", self.artifacts),
        ] {
            if role.max_directories < 1
                || role.max_name_bytes < name.len() as u64
                || role.max_files > self.max_files
                || role.max_directories > self.max_directories
                || role.max_name_bytes > self.max_name_bytes
                || role.max_depth >= self.max_depth
                || role.max_logical_bytes > self.max_logical_bytes
                || role.max_allocated_bytes > self.max_allocated_bytes
            {
                return Err(format!(
                    "stage {name} role budget is inconsistent with aggregate bounds"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageRole {
    Source,
    Spool,
    Artifacts,
}
impl StageRole {
    fn name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Spool => "spool",
            Self::Artifacts => "artifacts",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Source => 0,
            Self::Spool => 1,
            Self::Artifacts => 2,
        }
    }
    fn budget(self, budget: StageBudget) -> StageRoleBudget {
        match self {
            Self::Source => budget.source,
            Self::Spool => budget.spool,
            Self::Artifacts => budget.artifacts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageDirectoryBinding {
    pub(crate) path: String,
    pub(crate) dev: u64,
    pub(crate) ino: u64,
}

/// Data only: workers must independently open/recheck these roots before writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageRootBinding {
    pub(crate) nonce: String,
    pub(crate) stage: StageDirectoryBinding,
    pub(crate) source: StageDirectoryBinding,
    pub(crate) spool: StageDirectoryBinding,
    pub(crate) artifacts: StageDirectoryBinding,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct StageUsage {
    pub(crate) files: u64,
    pub(crate) directories: u64,
    /// Sum of raw UTF-8 basename lengths, including the stage and role roots.
    pub(crate) name_bytes: u64,
    pub(crate) max_depth: u32,
    /// Regular-file lengths, including sparse holes; directories contribute zero.
    pub(crate) logical_bytes: u64,
    /// All files AND directories, using checked Linux st_blocks * 512.
    pub(crate) allocated_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageEntryKind {
    File,
    Directory,
}
#[derive(Debug)]
pub(crate) struct StageEntry {
    pub(crate) relative_path: String,
    pub(crate) role: Option<StageRole>,
    pub(crate) kind: StageEntryKind,
    pub(crate) dev: u64,
    pub(crate) ino: u64,
    pub(crate) logical_bytes: u64,
    pub(crate) allocated_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageInventoryClosure {
    RequiresQualifiedSettlementAndExpectedManifest,
}

/// Bounded observed data, never evidence of quiescence or expected file membership.
#[derive(Debug)]
pub(crate) struct StageInventory {
    pub(crate) entries: Vec<StageEntry>,
    pub(crate) aggregate: StageUsage,
    pub(crate) source: StageUsage,
    pub(crate) spool: StageUsage,
    pub(crate) artifacts: StageUsage,
    pub(crate) closure: StageInventoryClosure,
}

/// Not Clone/Deserialize. Directory descriptors stay with their creating parent.
/// Dropping this owner ONLY closes descriptors; even unknown stages remain on disk.
pub(crate) struct ParentStage {
    root: PathBuf,
    source: PathBuf,
    spool: PathBuf,
    artifacts: PathBuf,
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    owned: native::PinnedStage,
}

impl ParentStage {
    pub(crate) fn create(parent: &Path, nonce: &str, budget: StageBudget) -> Result<Self, String> {
        validate_nonce(nonce)?;
        budget.validate()?;
        #[cfg(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            native::create(parent, nonce, budget)
        }
        #[cfg(not(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = parent;
            Err("parent staging requires reviewed Linux x86_64/aarch64 descriptor semantics; creation refused".to_string())
        }
    }
    /// Use the caller's held clock for every setup phase; never restart it here.
    /// Cooperative checks cannot preempt filesystem syscalls or scheduling.
    pub(crate) fn create_with_deadline(
        parent: &Path,
        nonce: &str,
        budget: StageBudget,
        held_deadline: Instant,
    ) -> Result<Self, String> {
        stage_time(held_deadline)?;
        validate_nonce(nonce)?;
        budget.validate()?;
        stage_time(held_deadline)?;
        #[cfg(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            native::create_with_deadline(parent, nonce, budget, Some(held_deadline))
        }
        #[cfg(not(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = parent;
            Err("parent staging requires reviewed Linux x86_64/aarch64 descriptor semantics; creation refused".to_string())
        }
    }
    pub(crate) fn stage_root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn source_root(&self) -> &Path {
        &self.source
    }
    pub(crate) fn spool_root(&self) -> &Path {
        &self.spool
    }
    pub(crate) fn artifacts_root(&self) -> &Path {
        &self.artifacts
    }
    pub(crate) fn worker_binding(&self) -> Result<StageRootBinding, String> {
        #[cfg(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            self.owned.binding().map_err(|error| self.retained(error))
        }
        #[cfg(not(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            Err("parent staging is unavailable on this target".to_string())
        }
    }
    /// Checks namespaces and bounded observed entries. The name alone cannot
    /// certify closure: no real guard-issued settlement witness is available yet.
    pub(crate) fn audit_closed_inventory(&self) -> Result<StageInventory, String> {
        #[cfg(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            self.owned.inventory().map_err(|error| self.retained(error))
        }
        #[cfg(not(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            Err("parent staging is unavailable on this target".to_string())
        }
    }
    /// The supplied absolute clock bounds observed inventory, not closure authority.
    pub(crate) fn audit_closed_inventory_with_deadline(
        &self,
        held_deadline: Instant,
    ) -> Result<StageInventory, String> {
        stage_time(held_deadline).map_err(|error| {
            format!("{error}; parent stage retained at {}", self.root.display())
        })?;
        #[cfg(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            self.owned
                .inventory_with_deadline(held_deadline)
                .map_err(|error| self.retained(error))
        }
        #[cfg(not(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            Err("parent staging is unavailable on this target".to_string())
        }
    }
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    fn retained(&self, error: String) -> String {
        format!("{error}; parent stage retained at {}", self.root.display())
    }
}

fn stage_time(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("stage operation deadline exceeded".to_string())
    } else {
        Ok(())
    }
}

fn inventory_deadline(
    held_deadline: Instant,
    inventory_timeout: Duration,
) -> Result<Instant, String> {
    stage_time(held_deadline)?;
    let phase_deadline = Instant::now()
        .checked_add(inventory_timeout)
        .ok_or("stage inventory deadline overflow")?;
    let deadline = held_deadline.min(phase_deadline);
    stage_time(deadline)?;
    Ok(deadline)
}

fn with_time<T>(
    deadline: Option<Instant>,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if let Some(deadline) = deadline {
        stage_time(deadline)?;
    }
    let result = operation()?;
    if let Some(deadline) = deadline {
        stage_time(deadline)?;
    }
    Ok(result)
}

fn validate_nonce(nonce: &str) -> Result<(), String> {
    if nonce.len() != 128 || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("stage nonce must be exactly 128 ASCII hexadecimal bytes".to_string());
    }
    Ok(())
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod native {
    use super::*;
    use std::collections::HashSet;
    use std::ffi::OsStr;
    use std::fs::{self, File, Metadata, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
    use std::path::Component;
    use std::time::Instant;

    // Safe std APIs with explicit reviewed kernel ABI flags; no unsafe/dependency.
    // include/uapi/asm-generic/fcntl.h; arm64 overrides in
    // arch/arm64/include/uapi/asm/fcntl.h (Linux kernel syscall interface).
    #[cfg(target_arch = "x86_64")]
    const DIRECTORY: i32 = 0x10000;
    #[cfg(target_arch = "x86_64")]
    const NOFOLLOW: i32 = 0x20000;
    #[cfg(target_arch = "aarch64")]
    const DIRECTORY: i32 = 0x4000;
    #[cfg(target_arch = "aarch64")]
    const NOFOLLOW: i32 = 0x8000;
    const NONBLOCK: i32 = 0x800;
    const ROLES: [StageRole; 3] = [StageRole::Source, StageRole::Spool, StageRole::Artifacts];

    struct PinnedDir {
        path: PathBuf,
        file: File,
        dev: u64,
        ino: u64,
        private: bool,
    }
    impl PinnedDir {
        fn open_until(
            path: PathBuf,
            private: bool,
            deadline: Option<Instant>,
        ) -> Result<Self, String> {
            let expected = metadata_until(&path, deadline)?;
            let file = open_entry_until(&path, true, &expected, deadline)?;
            let owned = Self {
                path,
                file,
                dev: expected.dev(),
                ino: expected.ino(),
                private,
            };
            owned.check_until(deadline)?;
            Ok(owned)
        }
        fn fd_path(&self) -> PathBuf {
            PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
        }
        fn child(&self, name: &OsStr) -> Result<PathBuf, String> {
            basename(name)?;
            Ok(self.fd_path().join(name))
        }
        fn open_at_until(
            parent: &Self,
            name: &OsStr,
            path: PathBuf,
            private: bool,
            deadline: Option<Instant>,
        ) -> Result<Self, String> {
            let anchored = parent.child(name)?;
            let expected = metadata_until(&anchored, deadline)?;
            let file = open_entry_until(&anchored, true, &expected, deadline)?;
            let owned = Self {
                path,
                file,
                dev: expected.dev(),
                ino: expected.ino(),
                private,
            };
            owned.check_until(deadline)?;
            Ok(owned)
        }
        fn check(&self) -> Result<Metadata, String> {
            self.check_until(None)
        }
        fn check_until(&self, deadline: Option<Instant>) -> Result<Metadata, String> {
            let opened = with_time(deadline, || {
                self.file
                    .metadata()
                    .map_err(|error| format!("retained stage descriptor: {error}"))
            })?;
            let named = metadata_until(&self.path, deadline)?;
            if !opened.is_dir()
                || !named.is_dir()
                || opened.dev() != self.dev
                || opened.ino() != self.ino
                || named.dev() != self.dev
                || named.ino() != self.ino
                || (self.private
                    && (opened.mode() & 0o777 != 0o700 || named.mode() & 0o777 != 0o700))
            {
                return Err(format!(
                    "stage root identity/permissions changed: {}",
                    self.path.display()
                ));
            }
            Ok(opened)
        }
        fn binding(&self) -> Result<StageDirectoryBinding, String> {
            self.check()?;
            let path = self.path.to_str().ok_or("stage root is not UTF-8")?;
            Ok(StageDirectoryBinding {
                path: copy_text(path)?,
                dev: self.dev,
                ino: self.ino,
            })
        }
    }
    pub(super) struct PinnedStage {
        parent: PinnedDir,
        root: PinnedDir,
        roles: [PinnedDir; 3],
        nonce: String,
        budget: StageBudget,
    }
    impl PinnedStage {
        fn check(&self) -> Result<(), String> {
            self.check_until(None)
        }
        fn check_until(&self, deadline: Option<Instant>) -> Result<(), String> {
            self.parent.check_until(deadline)?;
            self.root.check_until(deadline)?;
            for role in &self.roles {
                role.check_until(deadline)?;
            }
            Ok(())
        }
        pub(super) fn binding(&self) -> Result<StageRootBinding, String> {
            self.check()?;
            let result = StageRootBinding {
                nonce: copy_text(&self.nonce)?,
                stage: self.root.binding()?,
                source: self.roles[0].binding()?,
                spool: self.roles[1].binding()?,
                artifacts: self.roles[2].binding()?,
            };
            self.check()?;
            Ok(result)
        }
        pub(super) fn inventory(&self) -> Result<StageInventory, String> {
            let deadline = Instant::now()
                .checked_add(self.budget.inventory_timeout)
                .ok_or("stage inventory deadline overflow")?;
            self.inventory_with_deadline(deadline)
        }
        pub(super) fn inventory_with_deadline(
            &self,
            deadline: Instant,
        ) -> Result<StageInventory, String> {
            let deadline = inventory_deadline(deadline, self.budget.inventory_timeout)?;
            self.check_until(Some(deadline))?;
            stage_time(deadline)?;
            let mut scan = Scan {
                budget: self.budget,
                aggregate: StageUsage::default(),
                roles: [StageUsage::default(); 3],
                entries: Vec::new(),
                seen: HashSet::new(),
                deadline,
            };
            let root_meta = self.root.check_until(Some(deadline))?;
            let root_name = self.root.path.file_name().ok_or("stage basename missing")?;
            let admitted =
                scan.admit(None, &root_meta, basename(root_name)?.len() as u64, (0, 0))?;
            scan.record(admitted, String::new())?;
            let mut present = [false; 3];
            let mut root_entries = with_time(Some(deadline), || {
                fs::read_dir(self.root.fd_path()).map_err(|error| format!("stage roles: {error}"))
            })?;
            loop {
                scan.time()?;
                let entry = root_entries.next();
                scan.time()?;
                let Some(entry) = entry else {
                    break;
                };
                let entry = entry.map_err(|error| format!("stage role entry: {error}"))?;
                let name = entry.file_name();
                let text = basename(&name)?;
                let role = ROLES
                    .into_iter()
                    .find(|role| role.name() == text)
                    .ok_or_else(|| format!("unknown stage root entry {text:?}"))?;
                if present[role.index()] {
                    return Err("duplicate stage role entry".to_string());
                }
                present[role.index()] = true;
                let pinned = &self.roles[role.index()];
                let observed = metadata_until(&self.root.child(&name)?, Some(deadline))?;
                let opened = pinned.check_until(Some(deadline))?;
                same_entry(&observed, &opened)?;
            }
            if present != [true; 3] {
                return Err("stage role inventory is incomplete".to_string());
            }
            let mut pending = Vec::new();
            for role in ROLES {
                scan.time()?;
                let pinned = &self.roles[role.index()];
                let observed = pinned.check_until(Some(deadline))?;
                let admitted =
                    scan.admit(Some(role), &observed, role.name().len() as u64, (1, 0))?;
                scan.record(admitted, copy_text(role.name())?)?;
                // Open one role basename beneath the retained root, then recheck
                // its advertised identity; /proc's descriptor link is intentional.
                pending
                    .try_reserve(1)
                    .map_err(|error| format!("reserve stage directory walk: {error}"))?;
                pending.push(Pending {
                    directory: PinnedDir::open_at_until(
                        &self.root,
                        OsStr::new(role.name()),
                        pinned.path.clone(),
                        true,
                        Some(deadline),
                    )?,
                    relative: copy_text(role.name())?,
                    role,
                    depth: 1,
                    role_depth: 0,
                });
            }
            while let Some(item) = pending.pop() {
                scan.time()?;
                item.directory.check_until(Some(deadline))?;
                let mut directory_entries = with_time(Some(deadline), || {
                    fs::read_dir(item.directory.fd_path())
                        .map_err(|error| format!("stage directory entries: {error}"))
                })?;
                loop {
                    scan.time()?;
                    let entry = directory_entries.next();
                    scan.time()?;
                    let Some(entry) = entry else {
                        break;
                    };
                    let entry = entry.map_err(|error| format!("stage entry: {error}"))?;
                    let name = entry.file_name();
                    let text = basename(&name)?;
                    let depth = item.depth.checked_add(1).ok_or("stage depth overflow")?;
                    let role_depth = item
                        .role_depth
                        .checked_add(1)
                        .ok_or("stage role depth overflow")?;
                    scan.depth(item.role, depth, role_depth)?;
                    let path = item.directory.child(&name)?;
                    let before = metadata_until(&path, Some(deadline))?;
                    // Admit counts/bytes before path copies or retained walk growth.
                    let admitted = scan.admit(
                        Some(item.role),
                        &before,
                        text.len() as u64,
                        (depth, role_depth),
                    )?;
                    let is_dir = before.is_dir();
                    let opened = open_entry_until(&path, is_dir, &before, Some(deadline))?;
                    let observed = with_time(Some(deadline), || {
                        opened
                            .metadata()
                            .map_err(|error| format!("stage opened entry: {error}"))
                    })?;
                    same_entry(&before, &observed)?;
                    let relative = join_name(&item.relative, text)?;
                    scan.record(admitted, copy_text(&relative)?)?;
                    if is_dir {
                        pending
                            .try_reserve(1)
                            .map_err(|error| format!("reserve stage directory walk: {error}"))?;
                        pending.push(Pending {
                            directory: PinnedDir {
                                path: item.directory.path.join(&name),
                                file: opened,
                                dev: observed.dev(),
                                ino: observed.ino(),
                                private: false,
                            },
                            relative,
                            role: item.role,
                            depth,
                            role_depth,
                        });
                    }
                }
                item.directory.check_until(Some(deadline))?;
            }
            scan.time()?;
            self.check_until(Some(deadline))?;
            Ok(StageInventory {
                entries: scan.entries,
                aggregate: scan.aggregate,
                source: scan.roles[0],
                spool: scan.roles[1],
                artifacts: scan.roles[2],
                closure: StageInventoryClosure::RequiresQualifiedSettlementAndExpectedManifest,
            })
        }
    }
    struct Pending {
        directory: PinnedDir,
        relative: String,
        role: StageRole,
        depth: u32,
        role_depth: u32,
    }
    struct Scan {
        budget: StageBudget,
        aggregate: StageUsage,
        roles: [StageUsage; 3],
        entries: Vec<StageEntry>,
        seen: HashSet<(u64, u64)>,
        deadline: Instant,
    }
    struct Admitted {
        aggregate: StageUsage,
        role_usage: Option<StageUsage>,
        entry: StageEntry,
    }
    impl Scan {
        fn time(&self) -> Result<(), String> {
            if Instant::now() >= self.deadline {
                Err("stage inventory deadline exceeded".to_string())
            } else {
                Ok(())
            }
        }
        fn depth(&self, role: StageRole, depth: u32, role_depth: u32) -> Result<(), String> {
            if depth > self.budget.max_depth || role_depth > role.budget(self.budget).max_depth {
                return Err("stage depth budget exceeded".to_string());
            }
            Ok(())
        }
        fn admit(
            &self,
            role: Option<StageRole>,
            meta: &Metadata,
            name_bytes: u64,
            depths: (u32, u32),
        ) -> Result<Admitted, String> {
            self.time()?;
            let (depth, role_depth) = depths;
            let kind = if meta.is_dir() {
                StageEntryKind::Directory
            } else if meta.is_file() {
                if meta.nlink() != 1 {
                    return Err("stage regular file has hardlink aliases".to_string());
                }
                StageEntryKind::File
            } else {
                return Err("stage entry is a link or unsupported file type".to_string());
            };
            if self.seen.contains(&(meta.dev(), meta.ino())) {
                return Err("stage inventory contains duplicate inode identity".to_string());
            }
            let logical = if meta.is_file() { meta.len() } else { 0 };
            let allocated = meta
                .blocks()
                .checked_mul(512)
                .ok_or("stage allocated-byte overflow")?;
            let aggregate = add_usage(
                self.aggregate,
                kind,
                name_bytes,
                depth,
                logical,
                allocated,
                self.budget.aggregate(),
            )?;
            let role_usage = if let Some(role) = role {
                Some(add_usage(
                    self.roles[role.index()],
                    kind,
                    name_bytes,
                    role_depth,
                    logical,
                    allocated,
                    role.budget(self.budget),
                )?)
            } else {
                None
            };
            Ok(Admitted {
                aggregate,
                role_usage,
                entry: StageEntry {
                    relative_path: String::new(),
                    role,
                    kind,
                    dev: meta.dev(),
                    ino: meta.ino(),
                    logical_bytes: logical,
                    allocated_bytes: allocated,
                },
            })
        }
        fn record(&mut self, mut admitted: Admitted, relative_path: String) -> Result<(), String> {
            self.time()?;
            self.entries
                .try_reserve(1)
                .map_err(|error| format!("reserve stage inventory: {error}"))?;
            self.seen
                .try_reserve(1)
                .map_err(|error| format!("reserve stage inode inventory: {error}"))?;
            self.seen.insert((admitted.entry.dev, admitted.entry.ino));
            self.aggregate = admitted.aggregate;
            if let (Some(role), Some(usage)) = (admitted.entry.role, admitted.role_usage) {
                self.roles[role.index()] = usage;
            }
            admitted.entry.relative_path = relative_path;
            self.entries.push(admitted.entry);
            Ok(())
        }
    }
    fn add_usage(
        mut usage: StageUsage,
        kind: StageEntryKind,
        names: u64,
        depth: u32,
        logical: u64,
        allocated: u64,
        limits: StageRoleBudget,
    ) -> Result<StageUsage, String> {
        match kind {
            StageEntryKind::File => {
                usage.files = usage
                    .files
                    .checked_add(1)
                    .ok_or("stage file-count overflow")?
            }
            StageEntryKind::Directory => {
                usage.directories = usage
                    .directories
                    .checked_add(1)
                    .ok_or("stage directory-count overflow")?
            }
        }
        usage.name_bytes = usage
            .name_bytes
            .checked_add(names)
            .ok_or("stage name-byte overflow")?;
        usage.logical_bytes = usage
            .logical_bytes
            .checked_add(logical)
            .ok_or("stage logical-byte overflow")?;
        usage.allocated_bytes = usage
            .allocated_bytes
            .checked_add(allocated)
            .ok_or("stage allocated-byte overflow")?;
        usage.max_depth = usage.max_depth.max(depth);
        for (observed, maximum, label) in [
            (usage.files, limits.max_files, "files"),
            (usage.directories, limits.max_directories, "directories"),
            (usage.name_bytes, limits.max_name_bytes, "name bytes"),
            (
                usage.logical_bytes,
                limits.max_logical_bytes,
                "logical bytes",
            ),
            (
                usage.allocated_bytes,
                limits.max_allocated_bytes,
                "allocated bytes",
            ),
        ] {
            if observed > maximum {
                return Err(format!(
                    "stage {label} budget exceeded ({observed} > {maximum})"
                ));
            }
        }
        if usage.max_depth > limits.max_depth {
            return Err("stage depth budget exceeded".to_string());
        }
        Ok(usage)
    }
    fn basename(name: &OsStr) -> Result<&str, String> {
        let mut components = Path::new(name).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err("stage entry is not one normal basename".to_string());
        }
        name.to_str()
            .ok_or_else(|| "stage entry basename is not UTF-8".to_string())
    }
    #[cfg(test)]
    fn metadata(path: &Path) -> Result<Metadata, String> {
        metadata_until(path, None)
    }
    fn metadata_until(path: &Path, deadline: Option<Instant>) -> Result<Metadata, String> {
        with_time(deadline, || {
            fs::symlink_metadata(path)
                .map_err(|error| format!("stage metadata {}: {error}", path.display()))
        })
    }
    fn same_entry(expected: &Metadata, opened: &Metadata) -> Result<(), String> {
        if expected.dev() != opened.dev()
            || expected.ino() != opened.ino()
            || expected.file_type() != opened.file_type()
            || expected.len() != opened.len()
            || expected.blocks() != opened.blocks()
            || expected.nlink() != opened.nlink()
        {
            return Err("stage entry changed during descriptor observation".to_string());
        }
        Ok(())
    }
    fn open_entry_until(
        path: &Path,
        directory: bool,
        expected: &Metadata,
        deadline: Option<Instant>,
    ) -> Result<File, String> {
        if expected.file_type().is_symlink() || (!expected.is_dir() && !expected.is_file()) {
            return Err("stage entry is a link or unsupported file type".to_string());
        }
        let file = with_time(deadline, || {
            OpenOptions::new()
                .read(true)
                .custom_flags(NOFOLLOW | NONBLOCK | if directory { DIRECTORY } else { 0 })
                .open(path)
                .map_err(|error| format!("stage no-follow open {}: {error}", path.display()))
        })?;
        same_entry(
            expected,
            &with_time(deadline, || {
                file.metadata()
                    .map_err(|error| format!("stage descriptor metadata: {error}"))
            })?,
        )?;
        same_entry(expected, &metadata_until(path, deadline)?)?;
        Ok(file)
    }
    fn copy_text(text: &str) -> Result<String, String> {
        let mut result = String::new();
        result
            .try_reserve_exact(text.len())
            .map_err(|error| format!("reserve stage path: {error}"))?;
        result.push_str(text);
        Ok(result)
    }
    fn join_name(parent: &str, name: &str) -> Result<String, String> {
        let length = parent
            .len()
            .checked_add(1)
            .and_then(|length| length.checked_add(name.len()))
            .ok_or("stage relative-path length overflow")?;
        let mut result = String::new();
        result
            .try_reserve_exact(length)
            .map_err(|error| format!("reserve stage relative path: {error}"))?;
        result.push_str(parent);
        result.push('/');
        result.push_str(name);
        Ok(result)
    }
    #[cfg(test)]
    fn pin_observed_parent(parent: &Path, observed: &Metadata) -> Result<PinnedDir, String> {
        pin_observed_parent_until(parent, observed, None)
    }
    fn pin_observed_parent_until(
        parent: &Path,
        observed: &Metadata,
        deadline: Option<Instant>,
    ) -> Result<PinnedDir, String> {
        // Keep the original observation through canonicalization and descriptor
        // acquisition, then recheck the supplied spelling before any mkdir.
        if !observed.is_dir() {
            return Err("stage parent is not a regular directory".to_string());
        }
        let canonical = with_time(deadline, || {
            fs::canonicalize(parent).map_err(|error| format!("canonical stage parent: {error}"))
        })?;
        if canonical.to_str().is_none() {
            return Err("stage parent is not UTF-8".to_string());
        }
        let pinned = PinnedDir::open_until(canonical, false, deadline)?;
        same_entry(observed, &pinned.check_until(deadline)?)?;
        same_entry(observed, &metadata_until(parent, deadline)?)?;
        Ok(pinned)
    }
    pub(super) fn create(
        parent: &Path,
        nonce: &str,
        budget: StageBudget,
    ) -> Result<ParentStage, String> {
        create_with_deadline(parent, nonce, budget, None)
    }
    pub(super) fn create_with_deadline(
        parent: &Path,
        nonce: &str,
        budget: StageBudget,
        deadline: Option<Instant>,
    ) -> Result<ParentStage, String> {
        let time = || deadline.map(stage_time).transpose().map(|_| ());
        time()?;
        let observed_parent = metadata_until(parent, deadline)?;
        time()?;
        let parent = pin_observed_parent_until(parent, &observed_parent, deadline)?;
        time()?;
        let name = format!("ripr-complete-{nonce}");
        let root_path = parent.path.join(&name);
        if root_path.to_str().is_none() {
            return Err("stage root is not UTF-8".to_string());
        }
        let result = (|| {
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            time()?;
            builder
                .create(parent.child(OsStr::new(&name))?)
                .map_err(|error| format!("fresh private stage creation: {error}"))?;
            time()?;
            let root = PinnedDir::open_at_until(
                &parent,
                OsStr::new(&name),
                root_path.clone(),
                true,
                deadline,
            )?;
            time()?;
            for role in ROLES {
                time()?;
                builder
                    .create(root.child(OsStr::new(role.name()))?)
                    .map_err(|error| format!("fresh stage role {}: {error}", role.name()))?;
                time()?;
            }
            let source = root_path.join("source");
            let spool = root_path.join("spool");
            let artifacts = root_path.join("artifacts");
            let roles = [
                PinnedDir::open_at_until(
                    &root,
                    OsStr::new("source"),
                    source.clone(),
                    true,
                    deadline,
                )?,
                PinnedDir::open_at_until(
                    &root,
                    OsStr::new("spool"),
                    spool.clone(),
                    true,
                    deadline,
                )?,
                PinnedDir::open_at_until(
                    &root,
                    OsStr::new("artifacts"),
                    artifacts.clone(),
                    true,
                    deadline,
                )?,
            ];
            time()?;
            let owned = PinnedStage {
                parent,
                root,
                roles,
                nonce: copy_text(nonce)?,
                budget,
            };
            let stage = ParentStage {
                root: root_path.clone(),
                source,
                spool,
                artifacts,
                owned,
            };
            if let Some(deadline) = deadline {
                stage.audit_closed_inventory_with_deadline(deadline)?;
            } else {
                stage.audit_closed_inventory()?;
            }
            time()?;
            Ok(stage)
        })();
        result.map_err(|error| {
            format!(
                "{error}; partial/unknown stage retained at {}",
                root_path.display()
            )
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::run::{ByteCaptureBudget, capture_complete_bytes_in_dir_with_budget};
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;
        use std::sync::atomic::{AtomicU64, Ordering};

        use super::super::tests::{budget, refusal};

        static NEXT: AtomicU64 = AtomicU64::new(1);
        fn io<T>(result: std::io::Result<T>) -> Result<T, String> {
            result.map_err(|error| error.to_string())
        }
        struct Fixture {
            parent: PathBuf,
        }
        impl Fixture {
            fn new() -> Result<Self, String> {
                let unique = NEXT.fetch_add(1, Ordering::Relaxed);
                let parent = std::env::temp_dir().join(format!(
                    "ripr-parent-stage-control-{}-{unique}",
                    std::process::id()
                ));
                io(fs::DirBuilder::new().mode(0o700).create(&parent))?;
                Ok(Self { parent })
            }
            fn stage(&self) -> Result<ParentStage, String> {
                ParentStage::create(
                    &self.parent,
                    &format!("{:0128x}", NEXT.fetch_add(1, Ordering::Relaxed)),
                    budget(),
                )
            }
        }
        // No recursive Drop cleanup: a control failure preserves the exact stage.
        // Each test's bounded fixtures remain for the parent qualification closeout.

        #[test]
        fn fresh_private_binding_counts_roots_and_drop_retains() -> Result<(), String> {
            let fixture = Fixture::new()?;
            let stage = fixture.stage()?;
            let binding = stage.worker_binding()?;
            let inventory = stage.audit_closed_inventory()?;
            assert_eq!(inventory.aggregate.directories, 4);
            assert_eq!(inventory.aggregate.files, 0);
            assert_eq!(inventory.aggregate.name_bytes, 142 + 6 + 5 + 9);
            assert_eq!(inventory.aggregate.logical_bytes, 0);
            assert_eq!(inventory.aggregate.max_depth, 1);
            assert_eq!(inventory.source.directories, 1);
            assert_eq!(inventory.spool.directories, 1);
            assert_eq!(inventory.artifacts.directories, 1);
            assert_eq!(inventory.entries.len(), 4);
            assert_eq!(
                inventory.closure,
                StageInventoryClosure::RequiresQualifiedSettlementAndExpectedManifest
            );
            let mut allocated = 0u64;
            for directory in [
                &binding.stage,
                &binding.source,
                &binding.spool,
                &binding.artifacts,
            ] {
                let meta = metadata(Path::new(&directory.path))?;
                assert_eq!(meta.mode() & 0o777, 0o700);
                assert_eq!((meta.dev(), meta.ino()), (directory.dev, directory.ino));
                assert_eq!(
                    io(fs::canonicalize(&directory.path))?.to_str(),
                    Some(directory.path.as_str())
                );
                allocated = allocated
                    .checked_add(meta.blocks().checked_mul(512).ok_or("block overflow")?)
                    .ok_or("allocated sum overflow")?;
            }
            assert_eq!(inventory.aggregate.allocated_bytes, allocated);
            refusal(
                ParentStage::create(&fixture.parent, &binding.nonce, budget()),
                "fresh private stage creation",
            )?;
            let retained = stage.stage_root().to_path_buf();
            drop(stage);
            if !retained.is_dir() || !retained.join("source").is_dir() {
                return Err("Drop removed a stage without qualified settlement".to_string());
            }
            let fresh = fixture.stage()?;
            if fresh.stage_root() == retained {
                return Err("fresh recovery reused a retained stage".to_string());
            }
            fresh.audit_closed_inventory()?;
            Ok(())
        }

        #[test]
        fn links_non_utf8_and_extra_roles_refuse_and_recover() -> Result<(), String> {
            let fixture = Fixture::new()?;
            let stage = fixture.stage()?;
            let sentinel = fixture.parent.join("unrelated-sentinel");
            io(fs::write(&sentinel, b"sentinel"))?;
            let link = stage.source_root().join("alias");
            io(symlink(&sentinel, &link))?;
            refusal(stage.audit_closed_inventory(), "link or unsupported")?;
            assert_eq!(io(fs::read(&sentinel))?, b"sentinel");
            io(fs::remove_file(&link))?;
            stage.audit_closed_inventory()?;
            io(symlink(&fixture.parent, &link))?;
            refusal(stage.audit_closed_inventory(), "link or unsupported")?;
            io(fs::remove_file(&link))?;
            let socket = stage.source_root().join("special");
            let listener = io(UnixListener::bind(&socket))?;
            refusal(stage.audit_closed_inventory(), "link or unsupported")?;
            drop(listener);
            io(fs::remove_file(&socket))?;
            stage.audit_closed_inventory()?;
            io(fs::hard_link(&sentinel, &link))?;
            refusal(stage.audit_closed_inventory(), "hardlink aliases")?;
            assert_eq!(io(fs::read(&sentinel))?, b"sentinel");
            io(fs::remove_file(&link))?;
            stage.audit_closed_inventory()?;
            let invalid = stage
                .source_root()
                .join(OsString::from_vec(vec![b'f', 0xff]));
            io(fs::write(&invalid, b"nonempty"))?;
            refusal(stage.audit_closed_inventory(), "basename is not UTF-8")?;
            io(fs::remove_file(invalid))?;
            stage.audit_closed_inventory()?;
            let extra = stage.stage_root().join("unknown");
            io(fs::write(&extra, b"nonempty"))?;
            refusal(stage.audit_closed_inventory(), "unknown stage root entry")?;
            io(fs::remove_file(&extra))?;
            io(fs::create_dir(&extra))?;
            refusal(stage.audit_closed_inventory(), "unknown stage root entry")?;
            io(fs::remove_dir(extra))?;
            stage.audit_closed_inventory()?;
            assert_eq!(io(fs::read(sentinel))?, b"sentinel");
            Ok(())
        }

        #[test]
        fn retained_root_and_role_descriptors_refuse_replacement() -> Result<(), String> {
            let fixture = Fixture::new()?;
            let stage = fixture.stage()?;
            let original = stage.worker_binding()?;
            let held = fixture.parent.join("held-original-stage");
            io(fs::rename(stage.stage_root(), &held))?;
            io(fs::DirBuilder::new().mode(0o700).create(stage.stage_root()))?;
            refusal(stage.worker_binding(), "identity/permissions changed")?;
            refusal(
                stage.audit_closed_inventory(),
                "identity/permissions changed",
            )?;
            assert_eq!(
                stage
                    .owned
                    .root
                    .file
                    .metadata()
                    .map_err(|e| e.to_string())?
                    .ino(),
                original.stage.ino
            );
            io(fs::remove_dir(stage.stage_root()))?;
            io(fs::rename(&held, stage.stage_root()))?;
            assert_eq!(stage.worker_binding()?, original);
            let held_role = stage.stage_root().join("held-source");
            io(fs::rename(stage.source_root(), &held_role))?;
            io(fs::DirBuilder::new()
                .mode(0o700)
                .create(stage.source_root()))?;
            refusal(stage.worker_binding(), "identity/permissions changed")?;
            refusal(
                stage.audit_closed_inventory(),
                "identity/permissions changed",
            )?;
            io(fs::remove_dir(stage.source_root()))?;
            io(fs::rename(&held_role, stage.source_root()))?;
            assert_eq!(stage.worker_binding()?, original);
            stage.audit_closed_inventory()?;
            Ok(())
        }

        #[test]
        fn initial_parent_substitution_refuses_before_creation_and_recovers() -> Result<(), String>
        {
            let fixture = Fixture::new()?;
            let sentinel = fixture.parent.join("original-sentinel");
            io(fs::write(&sentinel, b"original"))?;
            let observed = metadata(&fixture.parent)?;
            let held = fixture.parent.with_extension("held-original");
            io(fs::rename(&fixture.parent, &held))?;
            io(fs::DirBuilder::new().mode(0o700).create(&fixture.parent))?;
            refusal(
                pin_observed_parent(&fixture.parent, &observed),
                "changed during descriptor observation",
            )?;
            assert_eq!(io(fs::read_dir(&fixture.parent))?.count(), 0);
            assert_eq!(io(fs::read(held.join("original-sentinel")))?, b"original");
            io(fs::remove_dir(&fixture.parent))?;
            io(symlink(&held, &fixture.parent))?;
            refusal(
                pin_observed_parent(&fixture.parent, &observed),
                "changed during descriptor observation",
            )?;
            assert_eq!(io(fs::read_dir(&held))?.count(), 1);
            io(fs::remove_file(&fixture.parent))?;
            io(fs::rename(&held, &fixture.parent))?;
            let pinned = pin_observed_parent(&fixture.parent, &observed)?;
            assert_eq!(pinned.ino, observed.ino());
            let stage = fixture.stage()?;
            stage.audit_closed_inventory()?;
            assert_eq!(io(fs::read(&sentinel))?, b"original");
            Ok(())
        }

        #[test]
        fn all_entry_caps_distinguish_aggregate_and_role_usage() -> Result<(), String> {
            let fixture = Fixture::new()?;
            let mut stage = fixture.stage()?;
            let source = stage.source_root().join("first");
            let spool = stage.spool_root().join("second");
            io(fs::write(&source, b"source"))?;
            io(fs::write(&spool, b"spool"))?;
            let admitted = stage.audit_closed_inventory()?;
            assert_eq!(admitted.aggregate.files, 2);
            assert_eq!((admitted.source.files, admitted.spool.files), (1, 1));
            assert_eq!(admitted.aggregate.logical_bytes, 11);
            stage.owned.budget.max_files = 1;
            refusal(
                stage.audit_closed_inventory(),
                "files budget exceeded (2 > 1)",
            )?;
            stage.owned.budget = budget();
            let another = stage.source_root().join("third");
            io(fs::write(&another, b"third"))?;
            stage.owned.budget.source.max_files = 1;
            refusal(
                stage.audit_closed_inventory(),
                "files budget exceeded (2 > 1)",
            )?;
            stage.owned.budget = budget();
            let baseline_names = stage.audit_closed_inventory()?.aggregate.name_bytes;
            stage.owned.budget.max_name_bytes = baseline_names - 1;
            refusal(stage.audit_closed_inventory(), "name bytes budget exceeded")?;
            stage.owned.budget = budget();
            stage.owned.budget.source.max_name_bytes = "source".len() as u64;
            refusal(stage.audit_closed_inventory(), "name bytes budget exceeded")?;
            stage.owned.budget = budget();
            let directory = stage.artifacts_root().join("nested");
            io(fs::create_dir(&directory))?;
            stage.owned.budget.max_directories = 4;
            refusal(
                stage.audit_closed_inventory(),
                "directories budget exceeded",
            )?;
            stage.owned.budget = budget();
            stage.owned.budget.artifacts.max_directories = 1;
            refusal(
                stage.audit_closed_inventory(),
                "directories budget exceeded",
            )?;
            stage.owned.budget = budget();
            stage.owned.budget.max_depth = 1;
            refusal(stage.audit_closed_inventory(), "depth budget exceeded")?;
            stage.owned.budget = budget();
            stage.owned.budget.artifacts.max_depth = 0;
            refusal(stage.audit_closed_inventory(), "depth budget exceeded")?;
            stage.owned.budget = budget();
            let recovered = stage.audit_closed_inventory()?;
            assert_eq!(recovered.aggregate.files, 3);
            assert_eq!(recovered.aggregate.directories, 5);
            Ok(())
        }

        #[test]
        fn sparse_logical_and_all_entry_allocation_caps_refuse() -> Result<(), String> {
            let fixture = Fixture::new()?;
            let mut stage = fixture.stage()?;
            let sparse = stage.source_root().join("sparse");
            io(io(File::create(&sparse))?.set_len(8192))?;
            assert_eq!(metadata(&sparse)?.len(), 8192);
            stage.owned.budget.source.max_logical_bytes = 8191;
            refusal(
                stage.audit_closed_inventory(),
                "logical bytes budget exceeded",
            )?;
            stage.owned.budget = budget();
            stage.owned.budget.max_logical_bytes = 8191;
            refusal(
                stage.audit_closed_inventory(),
                "logical bytes budget exceeded",
            )?;
            stage.owned.budget = budget();
            let baseline = stage.audit_closed_inventory()?;
            let allocated = stage.spool_root().join("allocated");
            io(fs::write(&allocated, vec![b'a'; 8192]))?;
            let actual = metadata(&allocated)?
                .blocks()
                .checked_mul(512)
                .ok_or("block overflow")?;
            if actual == 0 {
                return Err("allocation control did not produce allocated blocks".to_string());
            }
            stage.owned.budget.max_allocated_bytes = baseline.aggregate.allocated_bytes;
            refusal(
                stage.audit_closed_inventory(),
                "allocated bytes budget exceeded",
            )?;
            stage.owned.budget = budget();
            stage.owned.budget.spool.max_allocated_bytes = baseline.spool.allocated_bytes;
            refusal(
                stage.audit_closed_inventory(),
                "allocated bytes budget exceeded",
            )?;
            stage.owned.budget = budget();
            let recovered = stage.audit_closed_inventory()?;
            assert_eq!(recovered.aggregate.logical_bytes, 16384);
            if recovered.aggregate.allocated_bytes < baseline.aggregate.allocated_bytes + actual {
                return Err("allocation inventory omitted the written file".to_string());
            }
            Ok(())
        }

        #[test]
        fn inventory_deadline_and_checked_overflow_refuse() -> Result<(), String> {
            let scan = Scan {
                budget: budget(),
                aggregate: StageUsage::default(),
                roles: [StageUsage::default(); 3],
                entries: Vec::new(),
                seen: HashSet::new(),
                deadline: Instant::now(),
            };
            refusal(scan.time(), "deadline exceeded")?;
            refusal(
                add_usage(
                    StageUsage {
                        allocated_bytes: u64::MAX,
                        ..StageUsage::default()
                    },
                    StageEntryKind::File,
                    1,
                    1,
                    1,
                    1,
                    budget().source,
                ),
                "allocated-byte overflow",
            )
        }

        #[test]
        fn explicit_stage_clock_retains_on_expiry_and_recovers() -> Result<(), String> {
            let fixture = Fixture::new()?;
            let nonce = format!("{:0128x}", NEXT.fetch_add(1, Ordering::Relaxed));
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(3))
                .ok_or("test deadline overflow")?;
            let stage =
                ParentStage::create_with_deadline(&fixture.parent, &nonce, budget(), deadline)?;
            let partial = stage.source_root().join("authenticated-observation-only");
            io(fs::write(&partial, b"retained"))?;
            let binding = stage.worker_binding()?;
            refusal(
                stage.audit_closed_inventory_with_deadline(Instant::now()),
                "stage operation deadline exceeded",
            )?;
            assert_eq!(io(fs::read(&partial))?, b"retained");
            assert_eq!(stage.worker_binding()?, binding);
            let recovered = stage.audit_closed_inventory_with_deadline(deadline)?;
            assert_eq!(recovered.aggregate.files, 1);
            assert_eq!(recovered.aggregate.logical_bytes, 8);
            assert_eq!(
                recovered.closure,
                StageInventoryClosure::RequiresQualifiedSettlementAndExpectedManifest
            );
            // No fresh clock is manufactured on the explicit entrypoint; passing
            // the same expired Instant remains a refusal even after recovery.
            let expired = Instant::now();
            for _ in 0..2 {
                refusal(
                    stage.audit_closed_inventory_with_deadline(expired),
                    "stage operation deadline exceeded",
                )?;
            }
            assert_eq!(stage.audit_closed_inventory()?.aggregate.files, 1);
            Ok(())
        }

        #[test]
        fn worker_success_failure_kill_overflow_timeout_retain_and_recover() -> Result<(), String> {
            let fixture = Fixture::new()?;
            // Existing qualified capture is the sole process owner. No status,
            // including exit0, is converted into stage cleanup authority here.
            for mode in ["success", "failure", "kill", "overflow", "timeout"] {
                let stage = fixture.stage()?;
                let root = stage.stage_root().to_path_buf();
                let partial = stage.source_root().join("partial");
                let args = vec![
                    "-c".to_string(),
                    "printf staged > \"$1/partial\"; case \"$2\" in success) exit 0;; failure) exit 7;; kill) kill -KILL $$;; overflow) printf four;; timeout) sleep 30 & wait;; esac".to_string(),
                    "stage-worker".to_string(),
                    stage.source_root().to_str().ok_or("source UTF-8")?.to_string(),
                    mode.to_string(),
                ];
                let started = Instant::now();
                let result = capture_complete_bytes_in_dir_with_budget(
                    Path::new("/bin/sh"),
                    &args,
                    (stage.stage_root(), None),
                    &[],
                    ByteCaptureBudget {
                        timeout: if mode == "timeout" {
                            Duration::from_millis(500)
                        } else {
                            Duration::from_secs(3)
                        },
                        stdout_bytes: if mode == "overflow" { 3 } else { 4096 },
                        stderr_bytes: 4096,
                    },
                    "parent staging native control",
                );
                assert_eq!(io(fs::read(&partial))?, b"staged");
                match (mode, result) {
                    ("overflow", Err(error))
                        if error.contains("stdout exceeds its 3-byte output budget") => {}
                    ("success", Ok(output))
                        if !output.timed_out && output.status.and_then(|s| s.code()) == Some(0) => {
                    }
                    ("failure", Ok(output))
                        if !output.timed_out && output.status.and_then(|s| s.code()) == Some(7) => {
                    }
                    ("kill", Ok(output))
                        if !output.timed_out && output.status.is_some_and(|s| !s.success()) => {}
                    ("timeout", Ok(output))
                        if output.timed_out && !output.status.is_some_and(|s| s.success()) => {}
                    (_, Err(error)) => {
                        return Err(format!("wrong {mode} capture refusal: {error}"));
                    }
                    (_, Ok(_)) => return Err(format!("wrong {mode} capture outcome")),
                }
                if started.elapsed() > Duration::from_secs(9) {
                    return Err(format!("{mode} exceeded capture settlement/drain bound"));
                }
                let inventory = stage.audit_closed_inventory()?;
                assert_eq!(inventory.aggregate.files, 1);
                assert_eq!(inventory.aggregate.logical_bytes, 6);
                assert_eq!(
                    inventory.closure,
                    StageInventoryClosure::RequiresQualifiedSettlementAndExpectedManifest
                );
                drop(stage);
                if !root.is_dir() || !partial.is_file() {
                    return Err(format!("{mode} removed stage without guard-issued witness"));
                }
                let fresh = fixture.stage()?;
                fresh.audit_closed_inventory()?;
                if fresh.stage_root() == root {
                    return Err(format!("{mode} recovery reused unknown custody"));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn budget() -> StageBudget {
        let role = StageRoleBudget {
            max_files: 64,
            max_directories: 64,
            max_name_bytes: 8192,
            max_depth: 7,
            max_logical_bytes: 1024 * 1024,
            max_allocated_bytes: 1024 * 1024,
        };
        StageBudget {
            max_files: 64,
            max_directories: 64,
            max_name_bytes: 8192,
            max_depth: 8,
            max_logical_bytes: 1024 * 1024,
            max_allocated_bytes: 1024 * 1024,
            source: role,
            spool: role,
            artifacts: role,
            inventory_timeout: Duration::from_secs(5),
        }
    }
    pub(super) fn refusal<T>(result: Result<T, String>, needle: &str) -> Result<(), String> {
        match result {
            Err(error) if error.contains(needle) => Ok(()),
            Err(error) => Err(format!("expected {needle:?}, observed {error:?}")),
            Ok(_) => Err(format!("expected refusal containing {needle:?}")),
        }
    }
    #[test]
    fn nonce_and_finite_budgets_refuse_before_creation() -> Result<(), String> {
        validate_nonce(&"A".repeat(128))?;
        for nonce in [
            "",
            "0",
            &"a".repeat(127),
            &"b".repeat(129),
            &"g".repeat(128),
            &"é".repeat(64),
        ] {
            refusal(
                ParentStage::create(Path::new("missing-stage-parent"), nonce, budget()),
                "128 ASCII hexadecimal",
            )?;
        }
        let nonce = "a".repeat(128);
        let mut limits = budget();
        limits.max_files = u64::MAX;
        refusal(
            ParentStage::create(Path::new("missing-stage-parent"), &nonce, limits),
            "unlimited sentinel",
        )?;
        let mut limits = budget();
        limits.max_name_bytes = 161;
        refusal(
            ParentStage::create(Path::new("missing-stage-parent"), &nonce, limits),
            "root/role capacity",
        )?;
        let mut limits = budget();
        limits.source.max_depth = limits.max_depth;
        refusal(
            ParentStage::create(Path::new("missing-stage-parent"), &nonce, limits),
            "role budget",
        )?;
        let mut limits = budget();
        limits.inventory_timeout = Duration::ZERO;
        refusal(
            ParentStage::create(Path::new("missing-stage-parent"), &nonce, limits),
            "finite inventory deadline",
        )
    }
    #[test]
    fn expired_stage_setup_refuses_before_parent_lookup() -> Result<(), String> {
        refusal(
            ParentStage::create_with_deadline(
                Path::new("missing-stage-parent"),
                &"a".repeat(128),
                budget(),
                Instant::now(),
            ),
            "stage operation deadline exceeded",
        )
    }
    #[test]
    fn held_clock_refuses_after_an_admitted_filesystem_operation() -> Result<(), String> {
        let witness =
            std::env::current_exe().map_err(|error| format!("test executable: {error}"))?;
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(100))
            .ok_or("test deadline overflow")?;
        let mut entered = false;
        refusal(
            with_time(Some(deadline), || {
                entered = true;
                let observed = std::fs::metadata(&witness)
                    .map_err(|error| format!("actual filesystem witness: {error}"))?;
                if !observed.is_file() || observed.len() == 0 {
                    return Err("filesystem witness is not a nonempty regular file".to_string());
                }
                // Cross the original Instant inside the same helper that wraps
                // native stage syscalls. A reset/removal of postflight must fail.
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
                Ok(observed)
            }),
            "stage operation deadline exceeded",
        )?;
        if !entered {
            return Err("held-clock control never admitted its filesystem operation".to_string());
        }
        // Refusal precedes operations on subsequent entries; it grants no fresh
        // clock merely because a previous observed result was otherwise valid.
        refusal(
            with_time(Some(deadline), || {
                Err::<(), _>("expired clock reached another operation".to_string())
            }),
            "stage operation deadline exceeded",
        )
    }
    #[test]
    fn inventory_phase_and_held_ceilings_both_refuse_and_preserve() -> Result<(), String> {
        let before = Instant::now();
        let held = before
            .checked_add(Duration::from_secs(5))
            .ok_or("test held deadline overflow")?;
        let phase_timeout = Duration::from_millis(100);
        let phase = inventory_deadline(held, phase_timeout)?;
        let after = Instant::now();
        if phase < before + phase_timeout || phase > after + phase_timeout || phase >= held {
            return Err("inventory phase ceiling was not retained".to_string());
        }
        let shorter_held = Instant::now()
            .checked_add(Duration::from_millis(100))
            .ok_or("test short held deadline overflow")?;
        if inventory_deadline(shorter_held, Duration::from_secs(5))? != shorter_held {
            return Err("inventory replaced the earlier held deadline".to_string());
        }
        refusal(
            inventory_deadline(Instant::now(), Duration::from_secs(5)),
            "stage operation deadline exceeded",
        )?;
        refusal(
            inventory_deadline(held, Duration::MAX),
            "stage inventory deadline overflow",
        )
    }
    #[test]
    fn inventory_phase_refuses_work_while_long_held_clock_remains_live() -> Result<(), String> {
        let witness =
            std::env::current_exe().map_err(|error| format!("test executable: {error}"))?;
        let held = Instant::now()
            .checked_add(Duration::from_secs(5))
            .ok_or("test held deadline overflow")?;
        let deadline = inventory_deadline(held, Duration::from_millis(100))?;
        let mut entered = false;
        refusal(
            with_time(Some(deadline), || {
                entered = true;
                let observed = std::fs::metadata(&witness)
                    .map_err(|error| format!("actual inventory witness: {error}"))?;
                if !observed.is_file() || observed.len() == 0 {
                    return Err("inventory witness is not a nonempty regular file".to_string());
                }
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
                Ok(observed)
            }),
            "stage operation deadline exceeded",
        )?;
        if !entered {
            return Err("inventory control never admitted actual work".to_string());
        }
        stage_time(held)?;
        refusal(
            with_time(Some(deadline), || {
                Err::<(), _>("expired inventory phase admitted another entry".to_string())
            }),
            "stage operation deadline exceeded",
        )
    }
    #[cfg(not(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    #[test]
    fn unsupported_stage_refuses_before_filesystem_access() -> Result<(), String> {
        refusal(
            ParentStage::create(
                Path::new("missing-stage-parent"),
                &"a".repeat(128),
                budget(),
            ),
            "creation refused",
        )
    }
}
