//! Requested-only Linux temporary source ownership.
//!
//! This protects an intact namespace from other unprivileged users. It does
//! not make check/unlink atomic against the same UID, root, or mount changes.
//! Cleanup has finite admitted membership; it has no hard syscall deadline.

use super::{
    MAX_REQUESTED_ENTRIES, MAX_REQUESTED_PATH_BYTES, REQUESTED_WRAPPER_DIRECTORIES,
    RequestedEntryKind, RequestedNamespace, requested_checkpoint,
};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(target_arch = "x86_64")]
const DIRECTORY: i32 = 0x10000;
#[cfg(target_arch = "x86_64")]
const NOFOLLOW: i32 = 0x20000;
#[cfg(target_arch = "aarch64")]
const DIRECTORY: i32 = 0x4000;
#[cfg(target_arch = "aarch64")]
const NOFOLLOW: i32 = 0x8000;
const NONBLOCK: i32 = 0x800;
const CREDENTIAL_PREFIX_BYTES: usize = 64 * 1024;

type Clock = Option<(Instant, Duration)>;

fn refusal(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn clock(check: Clock) -> io::Result<()> {
    match check {
        Some((started, budget)) => {
            requested_checkpoint(started, budget).map_err(|error| refusal(error.to_string()))
        }
        None => Ok(()),
    }
}

fn timed<T>(check: Clock, work: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    clock(check)?;
    let result = work()?;
    clock(check)?;
    Ok(result)
}

/// Read the current thread's trusted-kernel Uid prefix, not a supplied profile.
/// The unconsumed status suffix is not authenticated by this observation.
fn credentials(check: Clock, prefix: &mut [u8]) -> io::Result<u32> {
    let mut input = timed(check, || File::open("/proc/thread-self/status"))?;
    let mut used = 0;
    let mut line_start = 0;
    loop {
        if used == prefix.len() {
            return Err(refusal("Requested credential prefix exceeds its bound"));
        }
        let read = timed(check, || input.read(&mut prefix[used..]))?;
        if read == 0 {
            return Err(refusal(
                "Requested credential prefix has no complete Uid line",
            ));
        }
        used += read;
        while let Some(offset) = prefix[line_start..used]
            .iter()
            .position(|byte| *byte == b'\n')
        {
            let end = line_start + offset;
            let line = &prefix[line_start..end];
            if line.starts_with(b"Uid:") {
                let line = std::str::from_utf8(line)
                    .map_err(|error| refusal(format!("Requested Uid line: {error}")))?;
                let mut fields = line.split_ascii_whitespace();
                if fields.next() != Some("Uid:") {
                    return Err(refusal("Requested Uid label is malformed"));
                }
                let mut values = [0_u32; 4];
                for value in &mut values {
                    *value = fields
                        .next()
                        .ok_or_else(|| refusal("Requested Uid fields are missing"))?
                        .parse::<u32>()
                        .map_err(|error| refusal(format!("Requested Uid field: {error}")))?;
                }
                if fields.next().is_some() || values.iter().any(|value| *value != values[0]) {
                    return Err(refusal("Requested thread credentials are not one UID"));
                }
                clock(check)?;
                return Ok(values[0]);
            }
            line_start = end + 1;
        }
    }
}

#[derive(Clone, Copy)]
struct Identity {
    dev: u64,
    ino: u64,
    uid: u32,
    mode: u32,
    directory: bool,
    nlink: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

impl Identity {
    fn capture(metadata: &Metadata) -> io::Result<Self> {
        if !metadata.is_dir() && !metadata.is_file() {
            return Err(refusal(
                "Requested object is not a regular file or directory",
            ));
        }
        if metadata.is_file() && metadata.nlink() != 1 {
            return Err(refusal("Requested regular file does not have one link"));
        }
        Ok(Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            uid: metadata.uid(),
            mode: metadata.mode() & 0o7777,
            directory: metadata.is_dir(),
            nlink: metadata.nlink(),
            len: metadata.len(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        })
    }

    fn same_object(&self, metadata: &Metadata) -> bool {
        metadata.dev() == self.dev
            && metadata.ino() == self.ino
            && metadata.uid() == self.uid
            && metadata.mode() & 0o7777 == self.mode
            && metadata.is_dir() == self.directory
            && (metadata.is_dir() || metadata.is_file())
            && (self.directory || metadata.nlink() == 1)
    }

    fn matches(&self, metadata: &Metadata) -> bool {
        self.same_object(metadata)
            && (self.directory
                || (metadata.nlink() == self.nlink
                    && metadata.len() == self.len
                    && metadata.mtime() == self.mtime
                    && metadata.mtime_nsec() == self.mtime_nsec
                    && metadata.ctime() == self.ctime
                    && metadata.ctime_nsec() == self.ctime_nsec))
    }
}

struct Pinned {
    file: File,
    identity: Identity,
}

fn normal(name: &OsStr) -> io::Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(refusal("Requested child is not one Normal component"));
    }
    Ok(())
}

fn child(parent: &File, name: &OsStr) -> io::Result<PathBuf> {
    normal(name)?;
    Ok(PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd())).join(name))
}

fn opened(path: &Path, directory: bool, check: Clock) -> io::Result<Pinned> {
    let named = timed(check, || fs::symlink_metadata(path))?;
    if named.is_dir() != directory || (!directory && !named.is_file()) {
        return Err(refusal("Requested child type is not admitted"));
    }
    let file = timed(check, || {
        OpenOptions::new()
            .read(true)
            .custom_flags(NOFOLLOW | NONBLOCK | if directory { DIRECTORY } else { 0 })
            .open(path)
    })?;
    let observed = timed(check, || file.metadata())?;
    let identity = Identity::capture(&observed)?;
    if !identity.matches(&named) {
        return Err(refusal("Requested child changed during open"));
    }
    Ok(Pinned { file, identity })
}

fn checked_pinned(pinned: &Pinned, check: Clock) -> io::Result<()> {
    let observed = timed(check, || pinned.file.metadata())?;
    if !pinned.identity.matches(&observed) {
        return Err(refusal("Requested held directory changed"));
    }
    Ok(())
}

fn owned(metadata: &Identity, uid: u32, private: bool) -> io::Result<()> {
    if metadata.uid != uid
        || (private && metadata.mode != 0o700)
        || (!private && metadata.mode & 0o022 != 0)
    {
        return Err(refusal(
            "Requested directory owner or permissions are unsafe",
        ));
    }
    Ok(())
}

fn ancestor(metadata: &Identity, uid: u32) -> io::Result<()> {
    if metadata.uid != 0 && metadata.uid != uid {
        return Err(refusal("Requested temp ancestor has a foreign owner"));
    }
    if metadata.mode & 0o022 != 0 && metadata.mode & 0o1000 == 0 {
        return Err(refusal(
            "Requested temp ancestor is writable without sticky protection",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum NodeState {
    Uncreated,
    Creating,
    Created(Identity),
    Removed,
}

struct Key {
    relative: String,
    kind: RequestedEntryKind,
    parent: Option<usize>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CleanupPhase {
    Fresh,
    Attempting,
    Removed,
    Refused,
}

struct State {
    phase: CleanupPhase,
    shared: Option<Pinned>,
    base: Option<Pinned>,
    target: Option<Pinned>,
    target_state: NodeState,
    nodes: Vec<NodeState>,
    seen: Vec<bool>,
}

/// One owned source key per node. File-entry records borrow these by index;
/// they do not retain another String copy of every source path.
pub(super) struct CleanupOwner {
    temporary: PathBuf,
    ancestry: Vec<Identity>,
    temp: Pinned,
    base_name: OsString,
    target_name: OsString,
    uid: u32,
    keys: Vec<Key>,
    children: Vec<usize>,
    state: Mutex<State>,
    credential_prefix: Mutex<Vec<u8>>,
}

impl CleanupOwner {
    pub(super) fn prepare(
        temporary: &Path,
        shared: &Path,
        base: &Path,
        target: &Path,
        namespace: &RequestedNamespace<'_>,
        started: Instant,
        budget: Duration,
    ) -> io::Result<Box<Self>> {
        let check = Some((started, budget));
        clock(check)?;
        if shared.parent() != Some(temporary)
            || shared.file_name() != Some(OsStr::new("ripr-git-candidate"))
            || base.parent() != Some(shared)
            || target.parent() != Some(base)
        {
            return Err(refusal("Requested wrapper topology is not direct"));
        }
        if temporary.as_os_str().as_bytes().len() > MAX_REQUESTED_PATH_BYTES {
            return Err(refusal(
                "Requested temp ancestry exceeds its input byte bound",
            ));
        }
        let mut count = 1_usize;
        for component in temporary.components() {
            clock(check)?;
            match component {
                Component::RootDir => {}
                Component::Normal(_) => {
                    count = count
                        .checked_add(1)
                        .ok_or_else(|| refusal("Requested ancestry count overflow"))?;
                    if count > MAX_REQUESTED_ENTRIES {
                        return Err(refusal("Requested temp ancestry exceeds its count bound"));
                    }
                }
                _ => {
                    return Err(refusal(
                        "Requested temp ancestry is not absolute and Normal",
                    ));
                }
            }
        }
        if !temporary.is_absolute() || count > MAX_REQUESTED_ENTRIES {
            return Err(refusal("Requested temp ancestry exceeds its count bound"));
        }
        let base_name = base
            .file_name()
            .ok_or_else(|| refusal("Requested base name is missing"))?;
        let target_name = target
            .file_name()
            .ok_or_else(|| refusal("Requested tree name is missing"))?;
        normal(base_name)?;
        normal(target_name)?;
        // Charge each simultaneous fixed physical-path copy separately.
        // Source keys are accounted by the existing namespace, not this envelope.
        let mut physical_bytes = count
            .checked_mul(std::mem::size_of::<Identity>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .and_then(|bytes| bytes.checked_add(CREDENTIAL_PREFIX_BYTES))
            .ok_or_else(|| refusal("Requested physical envelope overflow"))?;
        for path in [temporary, shared, base, target, temporary, base] {
            physical_bytes = physical_bytes
                .checked_add(path.as_os_str().as_bytes().len())
                .ok_or_else(|| refusal("Requested physical envelope overflow"))?;
        }
        for name in [base_name, target_name] {
            physical_bytes = physical_bytes
                .checked_add(name.as_bytes().len())
                .ok_or_else(|| refusal("Requested physical envelope overflow"))?;
        }
        if physical_bytes > MAX_REQUESTED_PATH_BYTES {
            return Err(refusal(
                "Requested physical envelope exceeds its byte bound",
            ));
        }
        let nodes = namespace.entries.len();
        let admitted_count = nodes
            .checked_add(REQUESTED_WRAPPER_DIRECTORIES)
            .ok_or_else(|| refusal("Requested key count overflow"))?;
        if admitted_count != namespace.total_entries || admitted_count > namespace.limits.entries {
            return Err(refusal("Requested owned keys do not match admitted count"));
        }
        let representation = nodes
            .checked_mul(
                std::mem::size_of::<Key>()
                    + std::mem::size_of::<NodeState>()
                    + std::mem::size_of::<usize>()
                    + std::mem::size_of::<bool>(),
            )
            .ok_or_else(|| refusal("Requested ledger representation overflow"))?;
        if representation > MAX_REQUESTED_PATH_BYTES {
            return Err(refusal("Requested ledger representation exceeds its bound"));
        }
        let mut keys = Vec::new();
        clock(check)?;
        keys.try_reserve_exact(nodes)
            .map_err(|error| refusal(format!("Requested key allocation: {error}")))?;
        clock(check)?;
        let mut copied = 0_usize;
        for (relative, kind) in &namespace.entries {
            clock(check)?;
            copied = copied
                .checked_add(relative.len())
                .ok_or_else(|| refusal("Requested key bytes overflow"))?;
            if copied > namespace.limits.path_bytes {
                return Err(refusal(
                    "Requested copied keys exceed admitted namespace bytes",
                ));
            }
            let mut name = String::new();
            name.try_reserve_exact(relative.len())
                .map_err(|error| refusal(format!("Requested key allocation: {error}")))?;
            name.push_str(relative);
            keys.push(Key {
                relative: name,
                kind: *kind,
                parent: None,
            });
            clock(check)?;
        }
        if copied != namespace.path_bytes {
            return Err(refusal("Requested copied keys do not match admitted bytes"));
        }
        for index in 0..keys.len() {
            clock(check)?;
            if let Some((parent, _)) = keys[index].relative.rsplit_once('/') {
                let parent_index = keys
                    .binary_search_by(|key| key.relative.as_str().cmp(parent))
                    .map_err(|position| {
                        refusal(format!(
                            "Requested admitted parent is missing at {position}"
                        ))
                    })?;
                if keys[parent_index].kind == RequestedEntryKind::File {
                    return Err(refusal("Requested parent is a file"));
                }
                keys[index].parent = Some(parent_index);
            }
        }
        let mut children = Vec::new();
        clock(check)?;
        children
            .try_reserve_exact(nodes)
            .map_err(|error| refusal(format!("Requested child-index allocation: {error}")))?;
        clock(check)?;
        children.extend(0..nodes);
        clock(check)?;
        children.sort_unstable_by(|left, right| {
            keys[*left]
                .parent
                .cmp(&keys[*right].parent)
                .then_with(|| keys[*left].relative.cmp(&keys[*right].relative))
        });
        clock(check)?;
        let mut states = Vec::new();
        clock(check)?;
        states
            .try_reserve_exact(nodes)
            .map_err(|error| refusal(format!("Requested state allocation: {error}")))?;
        clock(check)?;
        states.resize(nodes, NodeState::Uncreated);
        let mut seen = Vec::new();
        clock(check)?;
        seen.try_reserve_exact(nodes)
            .map_err(|error| refusal(format!("Requested membership allocation: {error}")))?;
        clock(check)?;
        seen.resize(nodes, false);
        let mut ancestry = Vec::new();
        clock(check)?;
        ancestry
            .try_reserve_exact(count)
            .map_err(|error| refusal(format!("Requested ancestry allocation: {error}")))?;
        clock(check)?;
        clock(check)?;
        let mut credential_prefix = Vec::new();
        credential_prefix
            .try_reserve_exact(CREDENTIAL_PREFIX_BYTES)
            .map_err(|error| refusal(format!("Requested credential allocation: {error}")))?;
        credential_prefix.resize(CREDENTIAL_PREFIX_BYTES, 0);
        clock(check)?;
        let uid = credentials(check, &mut credential_prefix)?;
        let mut current = opened(Path::new("/"), true, check)?;
        ancestor(&current.identity, uid)?;
        ancestry.push(current.identity);
        for component in temporary.components() {
            if let Component::Normal(name) = component {
                let next = opened(&child(&current.file, name)?, true, check)?;
                ancestor(&next.identity, uid)?;
                ancestry.push(next.identity);
                current = next;
            }
        }
        if credentials(check, &mut credential_prefix)? != uid {
            return Err(refusal("Requested credentials changed during admission"));
        }
        let owner = Self {
            temporary: temporary.to_path_buf(),
            ancestry,
            temp: current,
            base_name: base_name.to_os_string(),
            target_name: target_name.to_os_string(),
            uid,
            keys,
            children,
            credential_prefix: Mutex::new(credential_prefix),
            state: Mutex::new(State {
                phase: CleanupPhase::Fresh,
                shared: None,
                base: None,
                target: None,
                target_state: NodeState::Uncreated,
                nodes: states,
                seen,
            }),
        };
        {
            let state = owner.lock()?;
            owner.current(&state, check)?;
        }
        clock(check)?;
        let owner = Box::new(owner);
        clock(check)?;
        Ok(owner)
    }

    fn lock(&self) -> io::Result<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|error| refusal(format!("Requested owner state is poisoned: {error}")))
    }

    pub(super) fn blob_chunk(&self) -> io::Result<MutexGuard<'_, Vec<u8>>> {
        let prefix = self
            .credential_prefix
            .lock()
            .map_err(|error| refusal(format!("Requested credential state is poisoned: {error}")))?;
        if prefix.len() != CREDENTIAL_PREFIX_BYTES {
            return Err(refusal("Requested shared scratch has the wrong size"));
        }
        Ok(prefix)
    }

    fn current(&self, state: &State, check: Clock) -> io::Result<()> {
        {
            let mut prefix = self.blob_chunk()?;
            if credentials(check, &mut prefix)? != self.uid {
                return Err(refusal("Requested thread credentials changed"));
            }
        }
        let mut current = opened(Path::new("/"), true, check)?;
        let mut index = 0;
        if !self.ancestry[index].matches(&timed(check, || current.file.metadata())?) {
            return Err(refusal("Requested root ancestor changed"));
        }
        for component in self.temporary.components() {
            if let Component::Normal(name) = component {
                current = opened(&child(&current.file, name)?, true, check)?;
                index += 1;
                if !self.ancestry[index].matches(&timed(check, || current.file.metadata())?) {
                    return Err(refusal("Requested temp ancestor changed"));
                }
                ancestor(&current.identity, self.uid)?;
            }
        }
        checked_pinned(&self.temp, check)?;
        if !self
            .temp
            .identity
            .matches(&timed(check, || current.file.metadata())?)
        {
            return Err(refusal(
                "Requested temp descriptor no longer names the admitted temp",
            ));
        }
        if let Some(shared) = &state.shared {
            let named = opened(
                &child(&self.temp.file, OsStr::new("ripr-git-candidate"))?,
                true,
                check,
            )?;
            checked_pinned(shared, check)?;
            owned(&named.identity, self.uid, false)?;
            if !shared
                .identity
                .matches(&timed(check, || named.file.metadata())?)
            {
                return Err(refusal("Requested shared directory changed"));
            }
            if let Some(base) = &state.base {
                let named = opened(&child(&shared.file, &self.base_name)?, true, check)?;
                checked_pinned(base, check)?;
                owned(&named.identity, self.uid, true)?;
                if !base
                    .identity
                    .matches(&timed(check, || named.file.metadata())?)
                {
                    return Err(refusal("Requested base directory changed"));
                }
                if let Some(target) = &state.target {
                    let named = opened(&child(&base.file, &self.target_name)?, true, check)?;
                    checked_pinned(target, check)?;
                    owned(&named.identity, self.uid, true)?;
                    if !target
                        .identity
                        .matches(&timed(check, || named.file.metadata())?)
                    {
                        return Err(refusal("Requested tree directory changed"));
                    }
                }
            }
        }
        clock(check)
    }

    pub(super) fn verify(&self, started: Instant, budget: Duration) -> io::Result<()> {
        let state = self.lock()?;
        self.current(&state, Some((started, budget)))
    }

    pub(super) fn create_shared(&self, started: Instant, budget: Duration) -> io::Result<()> {
        let check = Some((started, budget));
        let mut state = self.lock()?;
        self.current(&state, check)?;
        let path = child(&self.temp.file, OsStr::new("ripr-git-candidate"))?;
        match timed(check, || fs::DirBuilder::new().mode(0o755).create(&path)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let shared = opened(&path, true, check)?;
        owned(&shared.identity, self.uid, false)?;
        state.shared = Some(shared);
        self.current(&state, check)
    }

    /// Exclusive creation only. The caller installs its report-only marker
    /// immediately after Ok and before pin_base or any postcreation callback.
    pub(super) fn create_base_entry(&self, started: Instant, budget: Duration) -> io::Result<()> {
        let check = Some((started, budget));
        let state = self.lock()?;
        self.current(&state, check)?;
        let shared = state
            .shared
            .as_ref()
            .ok_or_else(|| refusal("Requested shared owner is missing"))?;
        let path = child(&shared.file, &self.base_name)?;
        // Do not put a fallible postcheck between actual mkdir success and
        // the caller's report-only marker.
        clock(check)?;
        fs::DirBuilder::new().mode(0o700).create(path)
    }

    pub(super) fn pin_base(&self, started: Instant, budget: Duration) -> io::Result<()> {
        let check = Some((started, budget));
        let mut state = self.lock()?;
        self.current(&state, check)?;
        let shared = state
            .shared
            .as_ref()
            .ok_or_else(|| refusal("Requested shared owner is missing"))?;
        let base = opened(&child(&shared.file, &self.base_name)?, true, check)?;
        owned(&base.identity, self.uid, true)?;
        state.base = Some(base);
        self.current(&state, check)
    }

    pub(super) fn create_target(&self, started: Instant, budget: Duration) -> io::Result<()> {
        let check = Some((started, budget));
        let mut state = self.lock()?;
        self.current(&state, check)?;
        let base = state
            .base
            .as_ref()
            .ok_or_else(|| refusal("Requested base owner is missing"))?;
        let path = child(&base.file, &self.target_name)?;
        state.target_state = NodeState::Creating;
        timed(check, || fs::DirBuilder::new().mode(0o700).create(&path))?;
        let target = opened(&path, true, check)?;
        owned(&target.identity, self.uid, true)?;
        state.target_state = NodeState::Created(target.identity);
        state.target = Some(target);
        self.current(&state, check)
    }

    pub(super) fn key(&self, index: usize) -> io::Result<&str> {
        self.keys
            .get(index)
            .map(|key| key.relative.as_str())
            .ok_or_else(|| refusal("Requested key index is absent"))
    }

    pub(super) fn is_directory(&self, index: usize) -> io::Result<bool> {
        self.keys
            .get(index)
            .map(|key| key.kind != RequestedEntryKind::File)
            .ok_or_else(|| refusal("Requested key index is absent"))
    }

    pub(super) fn key_index(&self, relative: &str) -> io::Result<usize> {
        self.keys
            .binary_search_by(|key| key.relative.as_str().cmp(relative))
            .map_err(|position| refusal(format!("Requested admitted key is absent at {position}")))
    }

    fn parent(&self, index: usize, state: &State, check: Clock) -> io::Result<Pinned> {
        let key = self
            .keys
            .get(index)
            .ok_or_else(|| refusal("Requested key index is absent"))?;
        let target = state
            .target
            .as_ref()
            .ok_or_else(|| refusal("Requested tree owner is missing"))?;
        checked_pinned(target, check)?;
        let mut current = Pinned {
            file: timed(check, || target.file.try_clone())?,
            identity: target.identity,
        };
        if let Some((relative, _)) = key.relative.rsplit_once('/') {
            let mut end = 0_usize;
            for name in relative.split('/') {
                normal(OsStr::new(name))?;
                end = end
                    .checked_add(name.len())
                    .ok_or_else(|| refusal("Requested parent prefix overflow"))?;
                let prefix = relative
                    .get(..end)
                    .ok_or_else(|| refusal("Requested parent prefix is not a string boundary"))?;
                let parent_index = self.key_index(prefix)?;
                let next = opened(&child(&current.file, OsStr::new(name))?, true, check)?;
                owned(&next.identity, self.uid, true)?;
                let observed = timed(check, || next.file.metadata())?;
                match state.nodes.get(parent_index) {
                    Some(NodeState::Created(identity)) if identity.matches(&observed) => {}
                    _ => return Err(refusal("Requested admitted parent changed or is uncreated")),
                }
                current = next;
                end = end
                    .checked_add(1)
                    .ok_or_else(|| refusal("Requested parent prefix overflow"))?;
            }
        }
        Ok(current)
    }

    fn leaf(&self, index: usize) -> io::Result<&OsStr> {
        let key = self.key(index)?;
        let name = key
            .rsplit('/')
            .next()
            .ok_or_else(|| refusal("Requested leaf is absent"))?;
        normal(OsStr::new(name))?;
        Ok(OsStr::new(name))
    }

    pub(super) fn create_directory(
        &self,
        index: usize,
        started: Instant,
        budget: Duration,
    ) -> io::Result<()> {
        let check = Some((started, budget));
        let mut state = self.lock()?;
        self.current(&state, check)?;
        let parent = self.parent(index, &state, check)?;
        let path = child(&parent.file, self.leaf(index)?)?;
        if !self.is_directory(index)? || !matches!(state.nodes[index], NodeState::Uncreated) {
            return Err(refusal(
                "Requested directory creation is not a fresh admitted node",
            ));
        }
        state.nodes[index] = NodeState::Creating;
        timed(check, || fs::DirBuilder::new().mode(0o700).create(&path))?;
        let created = opened(&path, true, check)?;
        owned(&created.identity, self.uid, true)?;
        state.nodes[index] = NodeState::Created(created.identity);
        self.current(&state, check)
    }

    pub(super) fn create_file(
        &self,
        index: usize,
        started: Instant,
        budget: Duration,
    ) -> io::Result<File> {
        let check = Some((started, budget));
        let mut state = self.lock()?;
        self.current(&state, check)?;
        let parent = self.parent(index, &state, check)?;
        let path = child(&parent.file, self.leaf(index)?)?;
        if self.is_directory(index)? || !matches!(state.nodes[index], NodeState::Uncreated) {
            return Err(refusal(
                "Requested file creation is not a fresh admitted node",
            ));
        }
        state.nodes[index] = NodeState::Creating;
        let file = timed(check, || {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(NOFOLLOW | NONBLOCK)
                .open(&path)
        })?;
        let identity = Identity::capture(&timed(check, || file.metadata())?)?;
        if identity.directory || identity.uid != self.uid || identity.mode != 0o600 {
            return Err(refusal(
                "Requested created file owner or permissions are unsafe",
            ));
        }
        let named = timed(check, || fs::symlink_metadata(&path))?;
        if !identity.matches(&named) {
            return Err(refusal("Requested file changed during creation"));
        }
        state.nodes[index] = NodeState::Created(identity);
        self.current(&state, check)?;
        Ok(file)
    }

    pub(super) fn seal_file(
        &self,
        index: usize,
        file: &File,
        size: u64,
        started: Instant,
        budget: Duration,
    ) -> io::Result<()> {
        let check = Some((started, budget));
        let mut state = self.lock()?;
        self.current(&state, check)?;
        let observed = timed(check, || file.metadata())?;
        match state.nodes[index] {
            NodeState::Created(identity)
                if identity.same_object(&observed) && observed.len() == size => {}
            _ => {
                return Err(refusal(
                    "Requested written file changed or has the wrong size",
                ));
            }
        }
        state.nodes[index] = NodeState::Created(Identity::capture(&observed)?);
        clock(check)
    }

    fn children(&self, parent: Option<usize>) -> &[usize] {
        let start = self
            .children
            .partition_point(|index| self.keys[*index].parent < parent);
        let end = self
            .children
            .partition_point(|index| self.keys[*index].parent <= parent);
        &self.children[start..end]
    }

    fn audit_directory(
        &self,
        directory: &File,
        parent: Option<usize>,
        state: &mut State,
    ) -> io::Result<()> {
        let expected = self.children(parent);
        let admitted = expected
            .iter()
            .filter(|index| matches!(state.nodes[**index], NodeState::Created(_)))
            .count();
        let limit = admitted
            .checked_add(1)
            .ok_or_else(|| refusal("Requested membership count overflow"))?;
        let mut count = 0_usize;
        for entry in fs::read_dir(PathBuf::from(format!(
            "/proc/self/fd/{}",
            directory.as_raw_fd()
        )))? {
            let entry = entry?;
            count = count
                .checked_add(1)
                .ok_or_else(|| refusal("Requested membership count overflow"))?;
            if count >= limit {
                return Err(refusal("Requested directory has excess membership"));
            }
            let name = entry.file_name();
            normal(&name)?;
            let name = name
                .to_str()
                .ok_or_else(|| refusal("Requested membership is not UTF-8"))?;
            let matched = expected
                .binary_search_by(|index| {
                    self.keys[*index]
                        .relative
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .cmp(name)
                })
                .map_err(|position| {
                    refusal(format!(
                        "Requested directory has unknown membership at {position}"
                    ))
                })?;
            let index = expected[matched];
            if state.seen[index] {
                return Err(refusal("Requested directory enumeration repeated a child"));
            }
            let NodeState::Created(identity) = state.nodes[index] else {
                return Err(refusal("Requested directory contains an unowned child"));
            };
            let actual = opened(
                &child(directory, OsStr::new(name))?,
                identity.directory,
                None,
            )?;
            if !identity.matches(&actual.file.metadata()?) {
                return Err(refusal("Requested directory member changed"));
            }
            state.seen[index] = true;
        }
        for index in expected {
            if matches!(state.nodes[*index], NodeState::Created(_)) && !state.seen[*index] {
                return Err(refusal("Requested directory is missing an admitted child"));
            }
        }
        Ok(())
    }

    fn cleanup_once(
        &self,
        state: &mut State,
        #[cfg(test)] observe: &mut impl FnMut(usize) -> io::Result<()>,
    ) -> io::Result<()> {
        self.current(state, None)?;
        if matches!(state.target_state, NodeState::Creating)
            || state
                .nodes
                .iter()
                .any(|node| matches!(node, NodeState::Creating))
        {
            return Err(refusal(
                "Requested creation is uncertain; retained without cleanup",
            ));
        }
        let base = state
            .base
            .as_ref()
            .ok_or_else(|| refusal("Requested base ownership is absent"))?;
        let mut base_count = 0_usize;
        for entry in fs::read_dir(PathBuf::from(format!(
            "/proc/self/fd/{}",
            base.file.as_raw_fd()
        )))? {
            let entry = entry?;
            base_count += 1;
            if base_count > 1
                || !matches!(state.target_state, NodeState::Created(_))
                || entry.file_name() != self.target_name
            {
                return Err(refusal("Requested base contains unknown membership"));
            }
            let target = state
                .target
                .as_ref()
                .ok_or_else(|| refusal("Requested target ownership is absent"))?;
            let actual = opened(&child(&base.file, &self.target_name)?, true, None)?;
            if !target.identity.matches(&actual.file.metadata()?) {
                return Err(refusal("Requested target membership changed"));
            }
        }
        if matches!(state.target_state, NodeState::Created(_)) && base_count != 1 {
            return Err(refusal("Requested base is missing its admitted target"));
        }
        state.seen.fill(false);
        if let Some(target) = &state.target {
            let file = target.file.try_clone()?;
            self.audit_directory(&file, None, state)?;
        }
        for index in 0..self.keys.len() {
            if let NodeState::Created(identity) = state.nodes[index]
                && identity.directory
            {
                let parent = self.parent(index, state, None)?;
                let directory = opened(&child(&parent.file, self.leaf(index)?)?, true, None)?;
                if !identity.matches(&directory.file.metadata()?) {
                    return Err(refusal("Requested admitted directory changed"));
                }
                self.audit_directory(&directory.file, Some(index), state)?;
            }
        }
        self.current(state, None)?;
        for index in (0..self.keys.len()).rev() {
            if let NodeState::Created(identity) = state.nodes[index] {
                self.current(state, None)?;
                let parent = self.parent(index, state, None)?;
                let path = child(&parent.file, self.leaf(index)?)?;
                let actual = opened(&path, identity.directory, None)?;
                if !identity.matches(&actual.file.metadata()?) {
                    return Err(refusal("Requested removal identity changed"));
                }
                if identity.directory {
                    fs::remove_dir(path)?;
                } else {
                    fs::remove_file(path)?;
                }
                state.nodes[index] = NodeState::Removed;
                #[cfg(test)]
                observe(index)?;
            }
        }
        self.current(state, None)?;
        if let Some(target) = &state.target {
            checked_pinned(target, None)?;
            let base = state
                .base
                .as_ref()
                .ok_or_else(|| refusal("Requested base ownership is absent"))?;
            let path = child(&base.file, &self.target_name)?;
            let actual = opened(&path, true, None)?;
            if !target.identity.matches(&actual.file.metadata()?) {
                return Err(refusal("Requested target removal identity changed"));
            }
            fs::remove_dir(path)?;
            state.target_state = NodeState::Removed;
            state.target = None;
        }
        self.current(state, None)?;
        let shared = state
            .shared
            .as_ref()
            .ok_or_else(|| refusal("Requested shared ownership is absent"))?;
        let base = state
            .base
            .as_ref()
            .ok_or_else(|| refusal("Requested base ownership is absent"))?;
        let path = child(&shared.file, &self.base_name)?;
        let actual = opened(&path, true, None)?;
        if !base.identity.matches(&actual.file.metadata()?) {
            return Err(refusal("Requested base removal identity changed"));
        }
        fs::remove_dir(path)?;
        state.base = None;
        self.current(state, None)
    }

    pub(super) fn checked_cleanup(&self) -> io::Result<()> {
        self.cleanup(
            #[cfg(test)]
            &mut |_| Ok(()),
        )
    }

    fn cleanup(
        &self,
        #[cfg(test)] observe: &mut impl FnMut(usize) -> io::Result<()>,
    ) -> io::Result<()> {
        let mut state = self.lock()?;
        match state.phase {
            CleanupPhase::Removed => return Ok(()),
            CleanupPhase::Attempting => {
                return Err(refusal(
                    "Requested cleanup was interrupted; retained without retry",
                ));
            }
            CleanupPhase::Refused => {
                return Err(refusal(
                    "Requested cleanup already refused; retained without retry",
                ));
            }
            CleanupPhase::Fresh => state.phase = CleanupPhase::Attempting,
        }
        let result = self.cleanup_once(
            &mut state,
            #[cfg(test)]
            observe,
        );
        state.phase = if result.is_ok() {
            CleanupPhase::Removed
        } else {
            CleanupPhase::Refused
        };
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::git_candidate_execution::{
        GuardStorage, RequestedTreeLimits, TempRootGuard,
    };
    use std::io::Write;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{FileTypeExt, PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        shared: PathBuf,
        base: PathBuf,
        target: PathBuf,
        started: Instant,
        budget: Duration,
    }

    impl Fixture {
        fn new(name: &str) -> TestResult<Self> {
            let root = std::env::temp_dir().join(format!(
                "ripr-owner-{name}-{}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos()
            ));
            fs::DirBuilder::new().mode(0o700).create(&root)?;
            let shared = root.join("ripr-git-candidate");
            let base = shared.join("owned-base");
            let target = base.join("tree");
            Ok(Self {
                root,
                shared,
                base,
                target,
                started: Instant::now(),
                budget: Duration::from_mins(1),
            })
        }

        fn plan(&self, files: &[&str], directories: &[&str]) -> TestResult<Box<CleanupOwner>> {
            let mut namespace = RequestedNamespace::new(RequestedTreeLimits::STANDARD)
                .map_err(|error| refusal(error.to_string()))?;
            for path in directories {
                namespace
                    .admit_path(&self.target, path, RequestedEntryKind::Directory)
                    .map_err(|error| refusal(error.to_string()))?;
            }
            for path in files {
                namespace
                    .admit_path(&self.target, path, RequestedEntryKind::File)
                    .map_err(|error| refusal(error.to_string()))?;
            }
            Ok(CleanupOwner::prepare(
                &self.root,
                &self.shared,
                &self.base,
                &self.target,
                &namespace,
                self.started,
                self.budget,
            )?)
        }

        fn acquire(&self, owner: &CleanupOwner) -> TestResult {
            owner.create_shared(self.started, self.budget)?;
            owner.create_base_entry(self.started, self.budget)?;
            owner.pin_base(self.started, self.budget)?;
            owner.create_target(self.started, self.budget)?;
            for index in 0..owner.keys.len() {
                if owner.is_directory(index)? {
                    owner.create_directory(index, self.started, self.budget)?;
                }
            }
            Ok(())
        }

        fn write(&self, owner: &CleanupOwner, name: &str, bytes: &[u8]) -> TestResult {
            let index = owner.key_index(name)?;
            let mut file = owner.create_file(index, self.started, self.budget)?;
            file.write_all(bytes)?;
            owner.seal_file(index, &file, bytes.len() as u64, self.started, self.budget)?;
            Ok(())
        }

        fn guard(&self, owner: Box<CleanupOwner>) -> TempRootGuard {
            TempRootGuard(self.base.clone(), GuardStorage::Requested(owner))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // Independently owned fixture disposal is not Requested cleanup proof.
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn requested_owner_accepts_shared_0755_and_creates_private_nodes() -> TestResult {
        let fixture = Fixture::new("permissions")?;
        fs::DirBuilder::new().mode(0o755).create(&fixture.shared)?;
        let owner = fixture.plan(&["src/foo..rs", "literal\\name.rs"], &["empty"])?;
        fixture.acquire(&owner)?;
        fixture.write(&owner, "src/foo..rs", b"source")?;
        fixture.write(&owner, "literal\\name.rs", b"literal")?;
        let uid = owner.uid;
        assert_eq!(fs::metadata(&fixture.shared)?.mode() & 0o777, 0o755);
        for path in [
            &fixture.base,
            &fixture.target,
            &fixture.target.join("src"),
            &fixture.target.join("empty"),
        ] {
            let metadata = fs::metadata(path)?;
            assert_eq!(metadata.uid(), uid);
            assert_eq!(metadata.mode() & 0o777, 0o700);
        }
        let metadata = fs::metadata(fixture.target.join("src/foo..rs"))?;
        assert_eq!(metadata.uid(), uid);
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(
            fs::read(fixture.target.join("literal\\name.rs"))?,
            b"literal"
        );
        let guard = fixture.guard(owner);
        guard.checked_cleanup()?;
        assert!(
            !fixture.base.exists(),
            "owned source root remained after successful cleanup"
        );
        assert!(fixture.shared.is_dir(), "shared wrapper was deleted");
        fs::DirBuilder::new().mode(0o700).create(&fixture.base)?;
        fs::write(fixture.base.join("replacement"), b"after cleanup")?;
        drop(guard);
        assert_eq!(
            fs::read(fixture.base.join("replacement"))?,
            b"after cleanup"
        );
        Ok(())
    }

    #[test]
    fn requested_owner_shared_link_refuses_without_touching_outside() -> TestResult {
        for link in [false, true] {
            let fixture = Fixture::new(if link { "shared-link" } else { "shared-file" })?;
            let outside = fixture.root.join("outside");
            fs::create_dir(&outside)?;
            fs::write(outside.join("sentinel"), b"outside")?;
            if link {
                symlink(&outside, &fixture.shared)?;
            } else {
                fs::write(&fixture.shared, b"occupied shared file")?;
            }
            let owner = fixture.plan(&[], &[])?;
            let error = owner
                .create_shared(fixture.started, fixture.budget)
                .err()
                .ok_or("shared link or file was admitted")?;
            assert!(
                error.to_string().contains("type is not admitted"),
                "{error}"
            );
            assert_eq!(fs::read(outside.join("sentinel"))?, b"outside");
            assert!(
                !outside.join("owned-base").exists(),
                "wrote through shared link"
            );
            if link {
                assert!(
                    fs::symlink_metadata(&fixture.shared)?
                        .file_type()
                        .is_symlink()
                );
            } else {
                assert_eq!(fs::read(&fixture.shared)?, b"occupied shared file");
            }
        }
        Ok(())
    }

    #[test]
    fn requested_owner_shared_other_write_and_foreign_uid_decision_refuse() -> TestResult {
        let fixture = Fixture::new("unsafe-shared")?;
        fs::DirBuilder::new().mode(0o755).create(&fixture.shared)?;
        fs::set_permissions(&fixture.shared, fs::Permissions::from_mode(0o757))?;
        let owner = fixture.plan(&[], &[])?;
        let error = owner
            .create_shared(fixture.started, fixture.budget)
            .err()
            .ok_or("other-writable shared wrapper was admitted")?;
        assert!(
            error.to_string().contains("owner or permissions"),
            "{error}"
        );
        assert!(
            !fixture.base.exists(),
            "unsafe shared wrapper reached base creation"
        );
        fs::set_permissions(&fixture.shared, fs::Permissions::from_mode(0o755))?;
        let observed = Identity::capture(&fs::metadata(&fixture.shared)?)?;
        owned(&observed, observed.uid, false)?;
        let foreign = observed.uid.wrapping_add(1);
        let error = owned(&observed, foreign, false)
            .err()
            .ok_or("foreign UID decision was admitted")?;
        assert!(
            error.to_string().contains("owner or permissions"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn requested_owner_shared_replacement_refuses_before_base_creation() -> TestResult {
        let fixture = Fixture::new("shared-replaced")?;
        let owner = fixture.plan(&[], &[])?;
        owner.create_shared(fixture.started, fixture.budget)?;
        let moved = fixture.root.join("original-shared");
        fs::rename(&fixture.shared, &moved)?;
        fs::DirBuilder::new().mode(0o755).create(&fixture.shared)?;
        fs::write(fixture.shared.join("sentinel"), b"replacement")?;
        let error = owner
            .create_base_entry(fixture.started, fixture.budget)
            .err()
            .ok_or("replaced shared wrapper reached base creation")?;
        assert!(
            error.to_string().contains("shared directory changed"),
            "{error}"
        );
        assert!(!moved.join("owned-base").exists());
        assert!(!fixture.base.exists());
        assert_eq!(fs::read(fixture.shared.join("sentinel"))?, b"replacement");
        Ok(())
    }

    #[test]
    fn requested_owner_base_replacement_cleanup_refusal_is_sticky() -> TestResult {
        let fixture = Fixture::new("base-replaced")?;
        let owner = fixture.plan(&["source.rs"], &[])?;
        fixture.acquire(&owner)?;
        fixture.write(&owner, "source.rs", b"original")?;
        let guard = fixture.guard(owner);
        let moved = fixture.shared.join("original-base");
        fs::rename(&fixture.base, &moved)?;
        fs::DirBuilder::new().mode(0o700).create(&fixture.base)?;
        fs::write(fixture.base.join("sentinel"), b"replacement")?;
        let error = guard
            .checked_cleanup()
            .err()
            .ok_or("replaced base was deleted")?;
        assert!(
            error.to_string().contains("base directory changed"),
            "{error}"
        );
        let mut report = Vec::new();
        guard.clean_up_reporting_to(&mut report);
        assert!(String::from_utf8(report)?.contains("inspect retained identities"));
        drop(guard);
        assert_eq!(fs::read(moved.join("tree/source.rs"))?, b"original");
        assert_eq!(fs::read(fixture.base.join("sentinel"))?, b"replacement");
        Ok(())
    }

    #[test]
    fn requested_owner_ancestor_link_swap_refuses_before_file_write() -> TestResult {
        let fixture = Fixture::new("ancestor-link")?;
        let temporary = fixture.root.join("temp");
        fs::DirBuilder::new().mode(0o700).create(&temporary)?;
        let shared = temporary.join("ripr-git-candidate");
        let base = shared.join("base");
        let target = base.join("tree");
        let mut namespace = RequestedNamespace::new(RequestedTreeLimits::STANDARD)
            .map_err(|error| refusal(error.to_string()))?;
        namespace
            .admit_path(&target, "source.rs", RequestedEntryKind::File)
            .map_err(|error| refusal(error.to_string()))?;
        let owner = CleanupOwner::prepare(
            &temporary,
            &shared,
            &base,
            &target,
            &namespace,
            fixture.started,
            fixture.budget,
        )?;
        owner.create_shared(fixture.started, fixture.budget)?;
        owner.create_base_entry(fixture.started, fixture.budget)?;
        owner.pin_base(fixture.started, fixture.budget)?;
        owner.create_target(fixture.started, fixture.budget)?;
        let moved = fixture.root.join("original-temp");
        let outside = fixture.root.join("outside");
        fs::create_dir(&outside)?;
        fs::write(outside.join("sentinel"), b"outside")?;
        fs::rename(&temporary, &moved)?;
        symlink(&outside, &temporary)?;
        let index = owner.key_index("source.rs")?;
        let error = owner
            .create_file(index, fixture.started, fixture.budget)
            .err()
            .ok_or("ancestor link replacement reached file write")?;
        assert!(
            error.to_string().contains("type is not admitted"),
            "{error}"
        );
        assert!(
            !moved
                .join("ripr-git-candidate/base/tree/source.rs")
                .exists()
        );
        assert_eq!(fs::read(outside.join("sentinel"))?, b"outside");
        assert!(!outside.join("ripr-git-candidate").exists());
        let guard = TempRootGuard(base, GuardStorage::Requested(owner));
        guard
            .checked_cleanup()
            .err()
            .ok_or("ancestor drift cleanup was admitted")?;
        drop(guard);
        assert_eq!(fs::read(outside.join("sentinel"))?, b"outside");
        assert!(moved.join("ripr-git-candidate/base/tree").is_dir());
        Ok(())
    }

    #[test]
    fn requested_owner_symlink_and_hardlink_leaves_never_truncate() -> TestResult {
        for linked in [false, true] {
            let fixture = Fixture::new(if linked {
                "hardlink-leaf"
            } else {
                "symlink-leaf"
            })?;
            let owner = fixture.plan(&["source.rs"], &[])?;
            fixture.acquire(&owner)?;
            let outside = fixture.root.join("outside-file");
            fs::write(&outside, b"outside unchanged")?;
            let leaf = fixture.target.join("source.rs");
            if linked {
                fs::hard_link(&outside, &leaf)?;
            } else {
                symlink(&outside, &leaf)?;
            }
            let index = owner.key_index("source.rs")?;
            let error = owner
                .create_file(index, fixture.started, fixture.budget)
                .err()
                .ok_or("occupied leaf was opened for truncation")?;
            assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
            assert_eq!(fs::read(&outside)?, b"outside unchanged");
            let guard = fixture.guard(owner);
            guard
                .checked_cleanup()
                .err()
                .ok_or("uncertain occupied leaf cleanup was admitted")?;
            drop(guard);
            assert_eq!(fs::read(&outside)?, b"outside unchanged");
            fs::symlink_metadata(leaf)?;
        }
        Ok(())
    }

    #[test]
    fn requested_owner_hardlink_after_sealing_refuses_cleanup() -> TestResult {
        let fixture = Fixture::new("late-hardlink")?;
        let owner = fixture.plan(&["source.rs"], &[])?;
        fixture.acquire(&owner)?;
        fixture.write(&owner, "source.rs", b"source")?;
        let link = fixture.root.join("extra-link");
        fs::hard_link(fixture.target.join("source.rs"), &link)?;
        let guard = fixture.guard(owner);
        let error = guard
            .checked_cleanup()
            .err()
            .ok_or("multiply linked file was removed")?;
        assert!(error.to_string().contains("one link"), "{error}");
        drop(guard);
        assert_eq!(fs::read(&link)?, b"source");
        assert_eq!(fs::read(fixture.target.join("source.rs"))?, b"source");
        Ok(())
    }

    #[test]
    fn requested_owner_unknown_and_missing_membership_retains_without_retry() -> TestResult {
        for missing in [false, true] {
            let fixture = Fixture::new(if missing {
                "missing-member"
            } else {
                "unknown-member"
            })?;
            let owner = fixture.plan(&["source.rs"], &["empty"])?;
            fixture.acquire(&owner)?;
            fixture.write(&owner, "source.rs", b"source")?;
            let unknown = fixture.target.join("unknown");
            if missing {
                fs::remove_file(fixture.target.join("source.rs"))?;
            } else {
                fs::write(&unknown, b"unadmitted")?;
            }
            let guard = fixture.guard(owner);
            let error = guard
                .checked_cleanup()
                .err()
                .ok_or("changed membership was accepted")?;
            assert!(
                error.to_string().contains("membership")
                    || error.to_string().contains("missing an admitted"),
                "{error}"
            );
            if !missing {
                fs::remove_file(&unknown)?;
            }
            drop(guard);
            assert!(
                fixture.target.join("empty").is_dir(),
                "Drop retried failed checked cleanup"
            );
            if !missing {
                assert_eq!(fs::read(fixture.target.join("source.rs"))?, b"source");
            }
        }
        Ok(())
    }

    #[test]
    fn requested_owner_non_utf8_and_special_children_refuse() -> TestResult {
        let fixture = Fixture::new("unknown-types")?;
        let owner = fixture.plan(&["source.rs"], &[])?;
        fixture.acquire(&owner)?;
        fixture.write(&owner, "source.rs", b"source")?;
        let non_utf8 = fixture.target.join(OsString::from_vec(vec![0xff]));
        fs::write(&non_utf8, b"unknown")?;
        let socket = fixture.target.join("socket");
        let anchored_socket = {
            let state = owner.lock()?;
            let target = state
                .target
                .as_ref()
                .ok_or("target descriptor is missing")?;
            child(&target.file, OsStr::new("socket"))?
        };
        let listener = std::os::unix::net::UnixListener::bind(&anchored_socket)?;
        let guard = fixture.guard(owner);
        guard
            .checked_cleanup()
            .err()
            .ok_or("non-UTF8/special membership was admitted")?;
        drop(guard);
        assert_eq!(fs::read(non_utf8)?, b"unknown");
        assert_eq!(fs::read(fixture.target.join("source.rs"))?, b"source");
        assert!(fs::symlink_metadata(socket)?.file_type().is_socket());
        drop(listener);
        Ok(())
    }

    #[test]
    fn requested_owner_partial_cleanup_refusal_never_retries_from_drop() -> TestResult {
        let fixture = Fixture::new("partial-cleanup")?;
        let owner = fixture.plan(&["a.rs", "b.rs"], &[])?;
        fixture.acquire(&owner)?;
        fixture.write(&owner, "a.rs", b"a")?;
        fixture.write(&owner, "b.rs", b"b")?;
        let mut removed = None;
        let error = owner
            .cleanup(&mut |index| {
                removed = Some(index);
                Err(refusal("actual partial cleanup refused"))
            })
            .err()
            .ok_or("partial cleanup callback was ignored")?;
        assert!(error.to_string().contains("actual partial"), "{error}");
        let removed = removed.ok_or("cleanup did not perform a real removal")?;
        assert_eq!(owner.key(removed)?, "b.rs");
        assert!(
            !fixture.target.join("b.rs").exists(),
            "callback ran before native removal"
        );
        assert_eq!(fs::read(fixture.target.join("a.rs"))?, b"a");
        let guard = fixture.guard(owner);
        drop(guard);
        assert_eq!(fs::read(fixture.target.join("a.rs"))?, b"a");
        assert!(fixture.base.is_dir());
        Ok(())
    }

    #[test]
    fn requested_owner_actual_cleanup_unwind_poison_retains_remaining_nodes() -> TestResult {
        let fixture = Fixture::new("cleanup-unwind")?;
        let owner = fixture.plan(&["a.rs", "b.rs"], &[])?;
        fixture.acquire(&owner)?;
        fixture.write(&owner, "a.rs", b"a")?;
        fixture.write(&owner, "b.rs", b"b")?;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.cleanup(&mut |_| std::panic::resume_unwind(Box::new("after real removal")))
        }));
        let _panic = caught.err().ok_or("cleanup did not unwind")?;
        assert!(!fixture.target.join("b.rs").exists());
        assert_eq!(fs::read(fixture.target.join("a.rs"))?, b"a");
        let guard = fixture.guard(owner);
        let error = guard
            .checked_cleanup()
            .err()
            .ok_or("poisoned cleanup was retried")?;
        assert!(error.to_string().contains("poisoned"), "{error}");
        drop(guard);
        assert_eq!(fs::read(fixture.target.join("a.rs"))?, b"a");
        assert!(fixture.base.is_dir());
        Ok(())
    }

    #[test]
    fn requested_owner_checks_every_intermediate_parent_identity() -> TestResult {
        let fixture = Fixture::new("intermediate")?;
        let owner = fixture.plan(&["a/b/source.rs"], &[])?;
        fixture.acquire(&owner)?;
        let old = fixture.target.join("old-a");
        fs::rename(fixture.target.join("a"), &old)?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(fixture.target.join("a"))?;
        // The final parent b keeps its real inode. Only a changed, so a
        // final-parent-only comparison would admit this write incorrectly.
        fs::rename(old.join("b"), fixture.target.join("a/b"))?;
        fs::write(fixture.target.join("a/b/sentinel"), b"replacement parent")?;
        let index = owner.key_index("a/b/source.rs")?;
        let error = owner
            .create_file(index, fixture.started, fixture.budget)
            .err()
            .ok_or("changed intermediate parent was admitted")?;
        assert!(
            error.to_string().contains("admitted parent changed"),
            "{error}"
        );
        assert!(!fixture.target.join("a/b/source.rs").exists());
        assert_eq!(
            fs::read(fixture.target.join("a/b/sentinel"))?,
            b"replacement parent"
        );
        let guard = fixture.guard(owner);
        guard
            .checked_cleanup()
            .err()
            .ok_or("changed intermediate parent cleanup was admitted")?;
        drop(guard);
        assert_eq!(
            fs::read(fixture.target.join("a/b/sentinel"))?,
            b"replacement parent"
        );
        assert!(old.is_dir());
        Ok(())
    }

    #[test]
    fn requested_owner_ancestor_mode_drift_refusal_survives_restoration() -> TestResult {
        let fixture = Fixture::new("ancestor-mode")?;
        let owner = fixture.plan(&["source.rs"], &[])?;
        fixture.acquire(&owner)?;
        fixture.write(&owner, "source.rs", b"source")?;
        let guard = fixture.guard(owner);
        fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o705))?;
        let error = guard
            .checked_cleanup()
            .err()
            .ok_or("ancestor mode drift was ignored")?;
        assert!(
            error.to_string().contains("temp ancestor changed"),
            "{error}"
        );
        fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o700))?;
        drop(guard);
        assert_eq!(fs::read(fixture.target.join("source.rs"))?, b"source");
        Ok(())
    }

    #[test]
    fn requested_owner_expired_admission_creates_no_shared_wrapper() -> TestResult {
        let fixture = Fixture::new("expired")?;
        let namespace = RequestedNamespace::new(RequestedTreeLimits::STANDARD)
            .map_err(|error| refusal(error.to_string()))?;
        let error = CleanupOwner::prepare(
            &fixture.root,
            &fixture.shared,
            &fixture.base,
            &fixture.target,
            &namespace,
            fixture.started,
            Duration::ZERO,
        )
        .err()
        .ok_or("expired owner admission was accepted")?;
        assert!(
            error
                .to_string()
                .contains("named-tree materialization deadline exceeded"),
            "{error}"
        );
        assert!(
            !fixture.shared.exists(),
            "expired admission created a shared wrapper"
        );
        Ok(())
    }
}
