use crate::domain::{OracleKind, OracleStrength, SymbolId};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkspaceRootAuthority {
    pub(crate) root: PathBuf,
    pub(crate) workspace_identity: String,
    pub(crate) files: BTreeMap<PathBuf, WorkspaceFileAuthority>,
    #[serde(skip, default)]
    current_files: Arc<Mutex<BTreeMap<PathBuf, (String, bool)>>>,
}

impl Clone for WorkspaceRootAuthority {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            workspace_identity: self.workspace_identity.clone(),
            files: self.files.clone(),
            current_files: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

impl PartialEq for WorkspaceRootAuthority {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && self.workspace_identity == other.workspace_identity
            && self.files == other.files
    }
}

impl Eq for WorkspaceRootAuthority {}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkspaceFileAuthority {
    pub(crate) source_digest: String,
    pub(crate) package_identity: String,
    pub(crate) valid: bool,
}

impl WorkspaceRootAuthority {
    #[cfg(test)]
    pub(crate) fn from_index(root: &Path, files: &BTreeMap<PathBuf, FileFacts>) -> Self {
        Self::from_sources(
            root,
            files
                .iter()
                .map(|(path, facts)| (path, facts.source.as_ref())),
        )
    }
    pub(crate) fn from_sources<'a>(
        root: &Path,
        files: impl Iterator<Item = (&'a PathBuf, &'a str)>,
    ) -> Self {
        let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let mut authorities = BTreeMap::new();
        for (relative, source) in files {
            let path_valid = is_relative_without_parent(relative)
                && canonical_root
                    .join(relative)
                    .canonicalize()
                    .map(|path| path.starts_with(&canonical_root))
                    .unwrap_or(false);
            let (package_identity, package_valid) =
                match resolve_package_identity(&canonical_root, relative) {
                    PackageIdentity::Known(identity) => (identity, true),
                    PackageIdentity::UnreadableManifest { manifest } => (
                        format!(
                            "manifest-unreadable:{}:{}",
                            relative_path(&canonical_root, &manifest),
                            relative_path(&canonical_root, &canonical_root.join(relative)),
                        ),
                        false,
                    ),
                };
            authorities.insert(
                relative.clone(),
                WorkspaceFileAuthority {
                    source_digest: source_digest(source.as_bytes()),
                    package_identity,
                    valid: path_valid && package_valid,
                },
            );
        }
        let mut canonical = String::new();
        for (path, authority) in &authorities {
            canonical.push_str(&path.to_string_lossy().replace('\\', "/"));
            canonical.push('\0');
            canonical.push_str(&authority.source_digest);
            canonical.push('\0');
            canonical.push_str(&authority.package_identity);
            canonical.push('\n');
        }
        Self {
            root: canonical_root,
            workspace_identity: source_digest(canonical.as_bytes()),
            files: authorities,
            current_files: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub(crate) fn validates_target(
        &self,
        test_file: &Path,
        seam_file: &Path,
        source: &str,
    ) -> bool {
        self.validates_target_digest(test_file, seam_file, &source_digest(source.as_bytes()))
    }

    /// `validates_target` for a caller that already holds the SHA-256 of the
    /// test file's indexed source, so a hot loop can hash each file once.
    pub(crate) fn validates_target_digest(
        &self,
        test_file: &Path,
        seam_file: &Path,
        test_source_digest: &str,
    ) -> bool {
        let Some(file) = self.files.get(test_file) else {
            return false;
        };
        let Some(seam) = self.files.get(seam_file) else {
            return false;
        };
        if !file.valid || !seam.valid || file.package_identity != seam.package_identity {
            return false;
        }
        let test_current = self.current_file_is_current(test_file, file);
        let seam_current = self.current_file_is_current(seam_file, seam);
        test_current && seam_current && test_source_digest == file.source_digest
    }

    fn current_file_is_current(&self, path: &Path, authority: &WorkspaceFileAuthority) -> bool {
        let fingerprint = filesystem_fingerprint(&self.root, path);
        if let Ok(cache) = self.current_files.lock()
            && let Some((cached_fingerprint, valid)) = cache.get(path)
            && cached_fingerprint == &fingerprint
        {
            return *valid;
        }
        let valid = authority.valid
            && self.root.join(path).canonicalize().is_ok_and(|full| {
                // Read through the committed-source seam: in committed-history
                // mode the index holds `HEAD` bytes for dirty paths, so the
                // working-tree bytes would never match its digest.
                full.starts_with(&self.root)
                    && crate::analysis::committed_source::read_source_bytes(&self.root, path)
                        .ok()
                        .flatten()
                        .is_some_and(|bytes| {
                            // `source_digest` hashes the indexed text, so the
                            // re-read bytes go through the same decode (BOM
                            // dropped, non-UTF-8 lossy) before comparing.
                            let indexed = super::build::rust_source_text(&bytes);
                            source_digest(indexed.text.as_bytes()) == authority.source_digest
                        })
                    && matches!(
                        resolve_package_identity(&self.root, path),
                        PackageIdentity::Known(ref identity)
                            if identity == &authority.package_identity
                    )
            });
        if let Ok(mut cache) = self.current_files.lock() {
            cache.insert(path.to_path_buf(), (fingerprint, valid));
        }
        valid
    }
}

fn filesystem_fingerprint(root: &Path, relative: &Path) -> String {
    let mut fingerprint = String::new();
    let source = root.join(relative);
    append_metadata_fingerprint(&mut fingerprint, &source);
    append_entry_fingerprint(&mut fingerprint, &source);
    // Where the path resolves, through every symlink in the chain: a link
    // retargeted further along (`a -> b`, `b` moved outside the root) leaves
    // the source entry and the followed file unchanged (#5478).
    match source.canonicalize() {
        // `Debug`, not `display()`: display is lossy for non-UTF-8 names, so
        // two distinct resolved paths could render the same.
        Ok(resolved) => fingerprint.push_str(&format!("=>{resolved:?};")),
        Err(error) => fingerprint.push_str(&format!("=>{:?};", error.kind())),
    }
    let mut cursor = source.parent().map(Path::to_path_buf);
    while let Some(directory) = cursor {
        append_entry_fingerprint(&mut fingerprint, &directory);
        append_metadata_fingerprint(&mut fingerprint, &directory.join("Cargo.toml"));
        if directory == root {
            break;
        }
        cursor = directory.parent().map(Path::to_path_buf);
    }
    fingerprint
}

fn append_metadata_fingerprint(output: &mut String, path: &Path) {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            use std::time::UNIX_EPOCH;
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
            output.push_str(&format!(
                "{}:{}:{:?}",
                path.display(),
                metadata.len(),
                modified.map(|time| (time.as_secs(), time.subsec_nanos()))
            ));
            // The followed file's identity: two files that exist at once
            // never share it, whatever size and mtime they were given.
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                output.push_str(&format!(":{}:{}", metadata.dev(), metadata.ino()));
            }
            output.push(';');
        }
        Err(error) => output.push_str(&format!("{}:{:?};", path.display(), error.kind())),
    }
}

/// The directory entry itself, not what it points at: whether it is a symlink,
/// where a symlink points, and on Unix its device, inode and ctime. `metadata`
/// follows links, so swapping a file or directory for a symlink to same-sized
/// bytes written within one mtime tick left the followed fingerprint unchanged
/// and the cache kept admitting a target that now resolves outside the root
/// (#5478). The link target covers a symlink retargeted to another symlink
/// where no file identity is available (Windows); ctime cannot be set by a
/// user and changes when a freed inode number is reused.
fn append_entry_fingerprint(output: &mut String, path: &Path) {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            let is_symlink = metadata.file_type().is_symlink();
            output.push_str(&format!("{}:link={is_symlink}", path.display()));
            if is_symlink {
                match std::fs::read_link(path) {
                    Ok(target) => output.push_str(&format!(":->{target:?}")),
                    Err(error) => output.push_str(&format!(":->{:?}", error.kind())),
                }
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                output.push_str(&format!(
                    ":{}:{}:{}.{}",
                    metadata.dev(),
                    metadata.ino(),
                    metadata.ctime(),
                    metadata.ctime_nsec()
                ));
            }
            output.push(';');
        }
        Err(error) => output.push_str(&format!("{}:entry:{:?};", path.display(), error.kind())),
    }
}

pub(crate) fn source_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn is_relative_without_parent(path: &Path) -> bool {
    !path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

#[derive(Debug, PartialEq, Eq)]
enum PackageIdentity {
    Known(String),
    UnreadableManifest { manifest: PathBuf },
}

fn resolve_package_identity(root: &Path, relative: &Path) -> PackageIdentity {
    resolve_package_identity_with_reader(root, relative, |path| std::fs::read(path))
}

fn resolve_package_identity_with_reader<F>(
    root: &Path,
    relative: &Path,
    mut read_manifest: F,
) -> PackageIdentity
where
    F: FnMut(&Path) -> std::io::Result<Vec<u8>>,
{
    let mut cursor = root.join(relative).parent().map(Path::to_path_buf);
    while let Some(directory) = cursor {
        let manifest = directory.join("Cargo.toml");
        match read_manifest(&manifest) {
            Ok(bytes) => {
                let relative_manifest = relative_path(root, &manifest);
                return PackageIdentity::Known(format!(
                    "{relative_manifest}:{}",
                    source_digest(&bytes)
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return PackageIdentity::UnreadableManifest { manifest },
        }
        if directory == root {
            break;
        }
        cursor = directory.parent().map(Path::to_path_buf);
    }
    let containing = relative.parent().unwrap_or_else(|| Path::new("."));
    PackageIdentity::Known(format!(
        "directory:{}",
        containing.to_string_lossy().replace('\\', "/")
    ))
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
pub use super::index::RustIndex;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct OwnedRustIndex {
    pub files: BTreeMap<PathBuf, FileFacts>,
    pub tests: Vec<TestFact>,
    pub functions: Vec<FunctionFact>,
    /// #3731 review: crate names declared by the analyzed root manifest —
    /// the `[package] name` plus the `[lib] name` target when declared —
    /// each in raw and crate-identifier form (hyphens normalize to
    /// underscores, #3731 review F23). Used by the reveal-side
    /// same-name-import gate to tell an own-crate import (the normal
    /// integration-test binding of the changed owner, through the lib
    /// target) from a foreign same-name import. Workspace members'
    /// manifests are not resolved here, so a same-name import through a
    /// member crate's name counts as foreign (fail-closed under-credit);
    /// empty when the root manifest declares no package (a virtual
    /// workspace root).
    #[serde(default)]
    pub package_names: BTreeSet<String>,
    #[serde(default)]
    pub include_parents: BTreeMap<PathBuf, ResolvedIncludeParent>,
    #[serde(default)]
    pub include_limitations: Vec<RustIncludeLimitation>,
    /// Indexed files whose bytes are not UTF-8. They stay indexed from a
    /// lossy decode on lexical fallback and the fallback disclosure names
    /// them with `rust_source_not_utf8`.
    #[serde(default)]
    pub non_utf8_sources: BTreeSet<PathBuf>,
    /// Physical file-level include targets discovered before contextual
    /// ownership is reduced to one parent. This remains populated for
    /// ambiguous/conflicting include requirements so module resolution keeps
    /// the fragment's physical directory anchor without granting a role.
    #[serde(skip)]
    pub(crate) include_targets: BTreeSet<PathBuf>,
    /// Subjects established by registered harness adapters (#3532).
    /// Empty without registrations; every entry carries its registration
    /// provenance, harness kind, adapter generation, subject identity,
    /// and selector capability so consumers project them without
    /// reconstructing any of it. For libtest-mimic trial subjects,
    /// executable-test denominator admission is decided by the
    /// reachability authority (#3636): a subject excluded from the run
    /// argument keeps its fact and syntactic claim and is named by a
    /// `registration_unreachable` limitation.
    #[serde(default)]
    pub harness_subjects: Vec<HarnessSubjectFact>,
    /// Typed limitations recorded by registered harness adapters (#3532):
    /// shapes the registration saw but could not classify (dynamic names,
    /// loop-driven registration, ambiguous imports). Unregistered or
    /// ambiguous harnesses are limitations here, never production or
    /// executable-test optimism.
    #[serde(default)]
    pub harness_limitations: Vec<HarnessLimitationFact>,
    #[serde(default)]
    pub(crate) workspace_authority: Option<WorkspaceRootAuthority>,
}

/// One resolved file-level include edge (#3533): the fragment's compilation
/// unit parent plus the cfg-test requirement of the `include!` invocation
/// itself. A `#[cfg(test)] include!(...)` invocation only exists in test
/// builds, so the fragment's content is test-only regardless of the parent
/// file's own context.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedIncludeParent {
    /// The including file (physical path).
    pub parent: PathBuf,
    /// True when the `include!` invocation is structurally gated on a test
    /// build (`cfg(test)` or a `test` conjunct through `cfg_predicates`).
    pub requires_test: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RustIncludeLimitation {
    pub parent: PathBuf,
    pub line: usize,
    pub expression: String,
    pub reason_code: String,
}

/// One out-of-line `mod name;` declaration captured by the parser producer
/// (#3533). The cfg-test requirement is classified once, at the producer
/// boundary, through the shared `cfg_predicates` authority (#3530) — the same
/// closed classification the inline-module membership walk consumes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModuleDeclarationFact {
    /// Declared module name (`mod <name>;`).
    pub name: String,
    /// Line of the `mod` token (1-based), excluding the attribute lines.
    pub line: usize,
    /// `#[path]` target shape. Only an exact string-literal target resolves;
    /// everything else fails closed (see [`ModulePathTarget::Unknown`]).
    pub path_target: ModulePathTarget,
    /// True when the declaration's attributes structurally require a test
    /// build (`cfg(test)` or a `test` conjunct through `cfg_predicates`).
    /// A `cfg(any(test, ...))` alternative never requires test and never
    /// grants a composed role.
    pub requires_test: bool,
}

/// The `#[path]` target shape of one out-of-line module declaration (#3533).
///
/// Adjacently tagged: serde cannot serialize an internally tagged newtype
/// variant holding a string, so `tag` alone made every file with a literal
/// `#[path]` fail its file-fact cache store (#4171). Unit variants keep the
/// `{"kind": ...}` shape they always had, so existing entries still decode.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum ModulePathTarget {
    /// No `#[path]` attribute: default resolution relative to the declaring
    /// file's module directory (`<stem>/<name>.rs`, `<stem>/<name>/mod.rs`).
    Default,
    /// An exact `#[path = "..."]` string literal, resolved relative to the
    /// declaring file's containing directory (the Rust reference rule for
    /// non-inline `#[path]` targets).
    Literal(String),
    /// A `#[path]` attribute that is not a plain string literal (macro call,
    /// concatenation, `concat!(env!("OUT_DIR"), ...)`), or a `path` attribute
    /// introduced conditionally by `cfg_attr` — the effective target then
    /// depends on the active configuration and no static single file exists.
    /// Typed unknown — composition fails closed for this declaration instead
    /// of falling back to default name resolution, which would resolve a file
    /// Rust does not compile under the conditional configuration.
    Unknown,
}

/// Provenance of a composed source role (#3533).
///
/// Records the edge chain — module declarations, `#[path]` redirections,
/// literal repository-local include edges — from the compilation unit down to
/// this file occurrence, in order. The chain explains which context granted a
/// composed evidence role; it does not change any output contract (roles are
/// not surfaced in JSON output on this issue).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceRoleProvenance {
    /// Ordered chain from the compilation unit to this file (outermost edge
    /// first). Empty for files whose roles are fully standalone-parse derived.
    pub edges: Vec<SourceRoleProvenanceEdge>,
    /// Reason code of the earliest edge in the chain that could not be
    /// resolved (`rust_module_ambiguous_parent`,
    /// `rust_module_cycle_or_depth_limit`, `rust_module_context_conflict`).
    /// `None` means every recorded edge resolved exactly. An unresolved edge
    /// fails closed: no composed role is granted from it.
    pub earliest_unresolved_reason: Option<String>,
}

/// One edge in a composed source-role provenance chain (#3533).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceRoleProvenanceEdge {
    /// How this edge composes the child file into the parent context.
    pub kind: SourceRoleProvenanceEdgeKind,
    /// Including/declaring file (physical path).
    pub parent: PathBuf,
    /// Included/declared file (physical path).
    pub child: PathBuf,
    /// Source text naming the edge: `mod <name>;` for module edges, the
    /// include! expression for include edges.
    pub declaration: String,
    /// Line of the declaration in the parent file (1-based).
    pub line: usize,
    /// Whether this edge structurally requires a test build.
    pub requires_test: bool,
}

/// The composition edge kind of one provenance entry (#3533).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRoleProvenanceEdgeKind {
    /// Out-of-line `mod name;` declaration (exact `#[path]` included).
    Module,
    /// Literal repository-local file-level `include!` edge.
    Include,
}

/// An opaque property-macro invocation, retained only to name a limitation.
/// These lexical mentions never establish functions, tests, calls or oracles.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UnresolvedPropertyMacroFact {
    pub name: String,
    pub line: usize,
    pub mentioned_identifiers: Vec<String>,
}

/// Text that shares its file's `source` allocation instead of copying it
/// (#5415 step 2).
///
/// Function/test bodies and probe-shape snippets are usually exact
/// substrings of the indexed file source; storing each as its own `String`
/// kept ~84MB of duplicate bytes alive on the #5415 repro. `Shared` values
/// hold the file's `Arc<str>` plus a byte span, so cloning a fact into the
/// index arenas shares the allocation instead of duplicating it. `Owned`
/// covers producer output that is not a verbatim substring:
/// lexical-fallback bodies with normalized line endings, and
/// whitespace-collapsed or synthetic shape text.
///
/// Equality, debug, display and deref observe the resolved text, so the
/// representation never changes analysis results. Spans are `u32` to keep
/// the type `String`-sized; a body past 4GB falls back to `Owned`.
#[derive(Clone)]
pub struct SourceText {
    inner: SourceTextInner,
}

#[derive(Clone)]
enum SourceTextInner {
    Shared {
        source: Arc<str>,
        start: u32,
        len: u32,
    },
    Owned(Arc<str>),
}

impl SourceText {
    /// Share `source[start..start + text.len()]` when it reproduces `text`
    /// exactly; otherwise keep an owned copy. Producers pass the span they
    /// sliced from, so a wrong offset degrades to today's allocation
    /// instead of corrupting the text.
    pub fn shared_or_owned(source: &Arc<str>, start: usize, text: &str) -> Self {
        let end = start.saturating_add(text.len());
        let span = u32::try_from(start)
            .ok()
            .and_then(|start| u32::try_from(text.len()).ok().map(|len| (start, len)));
        if let (Some((start32, len32)), Some(window)) = (span, source.get(start..end))
            && window == text
        {
            return Self {
                inner: SourceTextInner::Shared {
                    source: Arc::clone(source),
                    start: start32,
                    len: len32,
                },
            };
        }
        Self::owned(text)
    }

    /// Keep an owned copy, for producer output that is not a verbatim
    /// substring of the file source.
    pub fn owned(text: impl AsRef<str>) -> Self {
        Self {
            inner: SourceTextInner::Owned(Arc::from(text.as_ref())),
        }
    }

    /// The resolved text. `Shared` spans are validated against the live
    /// allocation at construction, and `Arc<str>` contents are immutable,
    /// so the span always resolves.
    pub fn as_str(&self) -> &str {
        match &self.inner {
            SourceTextInner::Shared { source, start, len } => {
                let start = *start as usize;
                source
                    .get(start..start.saturating_add(*len as usize))
                    .unwrap_or("")
            }
            SourceTextInner::Owned(text) => text,
        }
    }

    /// The shared allocation, for mechanism pins that prove children reuse
    /// the file's `source` instead of copying it. `None` for `Owned` text.
    #[cfg(test)]
    pub(crate) fn shared_source(&self) -> Option<&Arc<str>> {
        match &self.inner {
            SourceTextInner::Shared { source, .. } => Some(source),
            SourceTextInner::Owned(_) => None,
        }
    }
}

impl std::ops::Deref for SourceText {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for SourceText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Debug for SourceText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl std::fmt::Display for SourceText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}

impl Default for SourceText {
    fn default() -> Self {
        Self::owned("")
    }
}

impl PartialEq for SourceText {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SourceText {}

impl PartialOrd for SourceText {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SourceText {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialEq<str> for SourceText {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SourceText {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<SourceText> for &str {
    fn eq(&self, other: &SourceText) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<String> for SourceText {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<SourceText> for String {
    fn eq(&self, other: &SourceText) -> bool {
        self.as_str() == other.as_str()
    }
}

impl From<String> for SourceText {
    fn from(text: String) -> Self {
        Self::owned(text)
    }
}

impl From<&str> for SourceText {
    fn from(text: &str) -> Self {
        Self::owned(text)
    }
}

impl From<SourceText> for String {
    fn from(text: SourceText) -> Self {
        text.as_str().to_string()
    }
}

impl From<&SourceText> for String {
    fn from(text: &SourceText) -> Self {
        text.as_str().to_string()
    }
}

impl serde::Serialize for SourceText {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Bare serialization has no parent allocation to resolve a span
        // against, so it always inlines (detached by definition).
        WireText::Inline {
            text: self.as_str().to_string(),
        }
        .serialize(serializer)
    }
}

/// Cache payload for [`SourceText`]: a span into the entry's `source`, or
/// inline text for `Owned` values. `SourceText` deliberately has no
/// `Deserialize`: linking a span needs the parent allocation, so only
/// [`FileFacts`] deserializes, through [`FileFactsWire`].
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(crate) enum WireText {
    Span { start: u32, len: u32 },
    Inline { text: String },
}

impl WireText {
    /// Attached encoding for one child of `parent`: a span is emitted
    /// only when the child provably shares the parent allocation. A
    /// child from a foreign allocation (reassigned source, arena-backed
    /// view child from another file) falls back to inline text, so a
    /// span can never silently resolve to different bytes at decode.
    pub(crate) fn attached(text: &SourceText, parent: &Arc<str>) -> Self {
        match &text.inner {
            SourceTextInner::Shared { source, start, len } if Arc::ptr_eq(source, parent) => {
                WireText::Span {
                    start: *start,
                    len: *len,
                }
            }
            _ => WireText::Inline {
                text: text.as_str().to_string(),
            },
        }
    }
}

/// Resolve one wire value against its entry's `source`. A span outside the
/// allocation or off a char boundary rejects the whole entry (the caller
/// treats it as a cache miss and re-extracts); spans never invent text.
fn link_wire_text(wire: WireText, source: &Arc<str>, what: &str) -> Result<SourceText, String> {
    match wire {
        WireText::Inline { text } => Ok(SourceText::owned(text)),
        WireText::Span { start, len } => {
            let (start_usize, len_usize) = (start as usize, len as usize);
            match source.get(start_usize..start_usize.saturating_add(len_usize)) {
                Some(_) => Ok(SourceText {
                    inner: SourceTextInner::Shared {
                        source: Arc::clone(source),
                        start,
                        len,
                    },
                }),
                None => Err(format!(
                    "{what} span {start}..{} falls outside its {} source bytes",
                    start_usize.saturating_add(len_usize),
                    source.len()
                )),
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFacts {
    pub path: PathBuf,
    pub functions: Vec<FunctionFact>,
    pub tests: Vec<TestFact>,
    pub calls: Vec<CallFact>,
    pub returns: Vec<ReturnFact>,
    pub literals: Vec<LiteralFact>,
    pub probe_shapes: Vec<ProbeShapeFact>,
    /// True when parser-backed syntax failed and lexical fallback produced these facts.
    /// Lexical fallback intentionally emits no probe shapes and may under-credit repo seams.
    pub used_lexical_fallback: bool,
    /// Out-of-line `mod name;` declarations observed by the parser producer
    /// (#3533). Top-level declarations only: an out-of-line module nested in
    /// an inline module keeps the typed fail-closed status quo (no composed
    /// role) because cross-file resolution through inline nesting is not a
    /// producer here yet. The lexical fallback emits no module declarations.
    pub module_declarations: Vec<ModuleDeclarationFact>,
    /// Opaque property blocks from the existing source parse. No expansion or
    /// executable-test authority is inferred from the macro's spelling.
    pub unresolved_property_macros: Vec<UnresolvedPropertyMacroFact>,
    /// Source-role provenance for this file occurrence (#3533): the ordered
    /// edge chain from the compilation unit whose declarations and include
    /// edges composed this file's roles, plus the earliest edge in the chain
    /// that could not be resolved. Composer-owned and recomputed on every
    /// index build — `serde(skip)` keeps composed state out of the on-disk
    /// file-fact cache, which stores pre-composition parse facts only.
    pub role_provenance: SourceRoleProvenance,
    /// Original file source text. Held so `analysis/value-extraction-v2`
    /// can scan for top-level `const`/`static` declarations without
    /// re-reading the file at evidence-build time. Serialized in the file-fact
    /// cache and bound by its semantic payload digest. Reference-counted so
    /// child [`SourceText`] spans share this allocation (#5415 step 2).
    pub source: Arc<str>,
}

/// Producer-owned source role for one indexed Rust function (#3531).
///
/// This replaces the historical `FunctionFact::is_test` boolean, which
/// compressed several different questions into a single bit. The variants
/// keep exactly the meanings the producers can distinguish apart, so
/// consumers compare roles instead of re-deriving them from paths,
/// attributes, or `cfg` strings:
///
/// - `TestAttribute` and `ParameterizedExpansion` are executable tests:
///   they carry (or were promoted from) an exact supported test-defining
///   attribute and register `TestFact`s — the test selector denominator.
/// - `RegisteredTestAttribute` is an executable test carrying an exact
///   repository-registered test-producing attribute (#3532 harness
///   registry); it registers a `TestFact` through this same role
///   authority.
/// - `CfgTestModule` is evidence-only helper role: a function inside a
///   test-required module (`#[cfg(test)]`, or a `test` conjunct in
///   `cfg(all(...))` through the shared `cfg_predicates` authority). It
///   stays evidence-capable without entering the executable-test
///   denominator.
/// - `HarnessHelper` is evidence-only helper role inside a registered
///   custom test-harness target (#3532, e.g. a `[[test]]`
///   `harness = false` libtest-mimic suite): the custom harness never
///   runs libtest collection, so an attribute alone never makes a member
///   an executable test. Executable subjects come only from the harness
///   registry's adapter.
/// - `Production` is ordinary non-evidence source and remains a
///   production-subject candidate. Production-subject *eligibility* is
///   still the consumer-side composition of this role with the file-level
///   `SourceRole` (`analysis/workspace/source_role.rs`): a `Production`-role
///   function under `tests/**` stays ineligible through that file role,
///   exactly as before this type existed.
///
/// Dimensions with no function-level producer today are deliberately absent
/// rather than fabricated: the explicit production-like opt-in
/// (`analysis.production_like_targets`) acts at the file `SourceRole`
/// layer. Extending this enum is a producer change; a renderer or
/// consumer may display the role but may not recalculate it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionSourceRole {
    /// Ordinary production function: no test-defining attribute and not a
    /// member of a test-required module. The historical `is_test == false`.
    Production,
    /// Carries an exact supported test-defining attribute (`#[test]`,
    /// `#[tokio::test]`, `#[async_std::test]`, `#[rstest]`, ...). Registers
    /// an executable `TestFact`. When the facts came from the lexical
    /// fallback, the parser-fallback provenance stays on
    /// `FileFacts::used_lexical_fallback`.
    TestAttribute,
    /// Member of a test-required module — classified by the parser through
    /// the `cfg_predicates` authority (#3530) or preserved by the facts
    /// normalizer's cfg-test walk. Evidence-role helper: no executable
    /// `TestFact` is registered for this variant alone.
    CfgTestModule,
    /// Promoted by the explicit test-case authority
    /// (`facts::parameterized_tests`) from an exact `#[test_case(...)]`
    /// family attribute. Registers an executable, promoted `TestFact`.
    ParameterizedExpansion,
    /// Carries an exact repository-registered test-producing attribute
    /// through the harness registry (#3532, `analysis.test_harnesses`).
    /// Classified through this same role authority; registers an
    /// executable `TestFact` whose provenance is the registration.
    RegisteredTestAttribute,
    /// Member of a registered custom test-harness target (#3532, e.g. a
    /// `[[test]]` `harness = false` libtest-mimic suite). Evidence-only:
    /// the custom harness does not run libtest collection, so no
    /// executable `TestFact` is registered for this variant alone.
    /// Executable subjects are the adapter-established harness subjects.
    HarnessHelper,
}

impl FunctionSourceRole {
    /// The exact projection of the historical `FunctionFact::is_test`
    /// boolean: does this function carry test/evidence role at all
    /// (executable test or evidence-only helper)?
    ///
    /// This is a named projection with one documented meaning. A consumer
    /// asking a different question (executable-test membership,
    /// production-subject eligibility, ...) must compare variants or compose
    /// the file-level `SourceRole` instead of widening this predicate.
    pub fn is_evidence_role(self) -> bool {
        !matches!(self, Self::Production)
    }

    /// Whether functions with this role register executable `TestFact`s —
    /// the test selector denominator. Evidence-only helper roles
    /// (`CfgTestModule`, `HarnessHelper`) never enter it on their own.
    pub fn registers_executable_test(self) -> bool {
        matches!(
            self,
            Self::TestAttribute | Self::ParameterizedExpansion | Self::RegisteredTestAttribute
        )
    }
}

/// One shadow-shaped binding name captured from a `let` pattern (#3727
/// Slice A). Produced by the parser-backed summarizer only: the lexical
/// fallback emits no binding facts at all, so an empty set on a
/// parser-backed file is a real "no binding" result, not missing data.
///
/// Scanner-equivalent granularity, deliberately (see
/// `analysis::extract::shadow`): `line` is the 0-based line of the `let`
/// token RELATIVE TO the function body start, and `name` is one whole-word
/// identifier token extracted from the binding's pattern text with the same
/// ASCII word class the shared lexical authority
/// (`extract::shadow::pattern_contains_word`) uses. No scope extents, no
/// columns: a binding defeats uses at and after its own body-relative line.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LetBindingFact {
    /// 0-based line of the `let` token relative to the enclosing function's
    /// body start (the same body-relative line math the lexical shadow
    /// scanners use).
    pub line: usize,
    /// One whole-word identifier from the binding's pattern (`x` for
    /// `let mut x = ..`, `a` and `b` for `let (a, b) = ..`). Extracted with
    /// the lexical authority's whole-word semantics so the fact-derived and
    /// lexical shadow decisions stay equivalent.
    pub name: String,
}

/// The item container a function is declared in, read from the parser
/// (#4478, #3727). It decides which call syntax can name the function: a
/// bare `name(..)` names a module-level function, never a method, and a
/// method call `recv.name(..)` names a function with a `self` receiver in an
/// `impl` or `trait` block.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FunctionContainer {
    /// Not established: the lexical fallback producer, or a cache entry
    /// written before the fact existed. Consumers fail closed on it.
    #[default]
    Unknown,
    /// A module-level `fn`.
    Free,
    /// A `fn` item nested in another function's body.
    Local,
    /// A `fn` in an inherent `impl <self_ty>` block.
    Inherent { self_ty: String },
    /// A `fn` in an `impl <trait_path> for <self_ty>` block.
    TraitImpl { trait_path: String, self_ty: String },
    /// A `fn` in a `trait <trait_name>` block: a default method when it has
    /// a body, a required-method declaration when it does not.
    Trait { trait_name: String },
}

/// Parser facts about a function item's declaration (#4478).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionItemFact {
    pub container: FunctionContainer,
    /// Whether the parameter list starts with a `self` receiver.
    pub has_self_param: bool,
    /// Whether the item has a body block (a trait's required method does
    /// not).
    pub has_body: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionFact {
    pub id: SymbolId,
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: SourceText,
    pub calls: Vec<CallFact>,
    pub returns: Vec<ReturnFact>,
    pub literals: Vec<LiteralFact>,
    pub source_role: FunctionSourceRole,
    /// Attribute syntax lines (e.g., `#[rstest]`, `#[case(100, 100)]`,
    /// `#[test]`) captured from the AST `attrs()` iterator. Used by
    /// `analysis/value-extraction-v2` to read rstest case parameters
    /// without re-reading the file. The lexical fallback path
    /// populates this as empty.
    pub attrs: Vec<String>,
    /// Attribute syntax lines on the `impl` block that encloses this
    /// function, when it is an associated function (`#[pymethods]`,
    /// `#[wasm_bindgen]`, `#[napi]`). Kept apart from `attrs` so test and
    /// harness detection still read only the function's own attributes.
    /// Parser-backed only — the lexical fallback leaves this empty.
    pub impl_attrs: Vec<String>,
    /// Names of `fn` items nested inside this function's body (#3727 Slice
    /// A), sorted and deduplicated. A nested `fn <callee>` item is hoisted
    /// and defeats whole-body shadow decisions (see
    /// `analysis::extract::shadow`). Parser-backed only — the lexical
    /// fallback leaves this empty. Empty on a parser-backed file is a real
    /// "no nested fn" result.
    pub nested_fn_names: Vec<String>,
    /// Shadow-shaped binding facts from the `let` statements in this
    /// function's body (#3727 Slice A), one entry per whole-word pattern
    /// name, sorted by (line, name). Initializer-less declarations
    /// (`let flag;`) produce no entries, mirroring the lexical scanner's
    /// `;` bound. Parser-backed only — the lexical fallback leaves this
    /// empty.
    pub let_bindings: Vec<LetBindingFact>,
    /// Where the item is declared (#4478). Parser-backed only; the lexical
    /// fallback leaves it `Unknown`.
    pub item: FunctionItemFact,
    /// Where the definition sits for a type-path call `T::name(` (#4558).
    /// Parser-backed only; the lexical fallback leaves it `Unknown`.
    pub impl_context: FunctionImplContext,
}

/// Which item a function is defined in, as far as a type-path call
/// `T::name(` can reach it (#4558).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FunctionImplContext {
    /// Not established: lexical fallback, a trait body (a default method
    /// is reachable as `T::name` for any implementor), or an impl whose
    /// self type is not a plain named path (generic parameter, reference,
    /// trait object, tuple).
    #[default]
    Unknown,
    /// A module-level or function-local `fn`: never the target of `T::name`.
    Free,
    /// A method of an inherent or trait impl whose self type is the named
    /// path ending in `self_type` (generic arguments dropped).
    Impl { self_type: String },
}

impl FunctionImplContext {
    /// Whether a call spelled `self_type::name(` could resolve to this
    /// definition. Fails open (true) for `Unknown`: the caller treats every
    /// such definition as a competing target.
    pub fn may_be_target_of_type_path(&self, self_type: &str) -> bool {
        match self {
            Self::Unknown => true,
            Self::Free => false,
            Self::Impl { self_type: own } => own == self_type,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestFact {
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: SourceText,
    pub calls: Vec<CallFact>,
    pub assertions: Vec<OracleFact>,
    pub literals: Vec<LiteralFact>,
    /// Attribute syntax lines on the test fn. Mirrors
    /// `FunctionFact.attrs`. Carries `#[rstest]` and `#[case(...)]` for
    /// case-driven tests so value resolution can map case literals to
    /// test parameters.
    pub attrs: Vec<String>,
    /// Shadow facts for the test body, mirroring `FunctionFact`
    /// (#3727 Slice A): nested `fn` item names and `let` binding facts,
    /// both body-relative. Parser-backed only; the lexical fallback leaves
    /// them empty.
    pub nested_fn_names: Vec<String>,
    /// Body-relative `let` binding facts, mirroring
    /// `FunctionFact.let_bindings` (#3727 Slice A).
    pub let_bindings: Vec<LetBindingFact>,
}

impl TestFact {
    /// Calls written in the test's own body. A credited same-file helper's
    /// calls (`facts::test_helpers`) sit on the helper's lines: their
    /// arguments name the helper's parameters, which the test's `let`
    /// bindings and case rows do not bind, so value resolution reads only
    /// these.
    pub(crate) fn body_calls(&self) -> impl Iterator<Item = &CallFact> {
        self.calls
            .iter()
            .filter(|call| (self.start_line..=self.end_line).contains(&call.line))
    }
}

/// Whether a selector route is known for one harness subject (#3532).
/// A registration can describe a selector adapter; passive analysis
/// never runs it, so every capability stays explicitly unexecuted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessSelectorCapability {
    /// No selector route is known to RIPR for this subject.
    None,
    /// A named selector candidate exists (e.g. the libtest-mimic trial
    /// name, or the registered attribute's test name). Represented as
    /// unexecuted: no passive analysis starts Cargo or the harness.
    NamedUnexecuted,
}

impl HarnessSelectorCapability {
    /// Stable wire string for projections.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::NamedUnexecuted => "named_unexecuted",
        }
    }
}

/// What the adapter claims one harness subject is (#3532). The claim is
/// stated per subject so consumers never assume expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessSubjectClaim {
    /// The source invocation itself is one source-level test subject
    /// (e.g. one `Trial::test("name", ...)` registration). Generated
    /// cases inside it are not enumerated. The subject's span and body
    /// stay the invocation, and its evidence is bounded two ways
    /// (#3603): a bare-identifier callback resolving to exactly one
    /// function in the registered target contributes that function's
    /// parsed body evidence (calls, oracles, literals) one level deep;
    /// method-position `.unwrap()`/`.expect()` calls inside the claimed
    /// span register smoke oracles. Closures, path callbacks, and
    /// unresolved or ambiguous names contribute nothing beyond the
    /// invocation span — the boundary is fail-closed.
    ///
    /// Reachability boundary (#3604, #3636): this is a syntactic claim
    /// bounded by the registered target — a named invocation exists in
    /// the registered target. It does not claim the harness registers or
    /// executes the trial. Whether the subject enters the
    /// executable-test denominator is decided by the bounded
    /// reachability authority: a construction provably excluded from
    /// every resolved run argument (or a target with no run entry call)
    /// keeps this claim but does not enter the denominator and is named
    /// by a `registration_unreachable` limitation; a construction the
    /// bounded resolver can neither connect nor exclude stays in the
    /// denominator under this claim and is disclosed by an aggregate
    /// `registration_reachability_unknown` limitation. There is no
    /// per-subject reachability field: the unknown bucket is exactly the
    /// case where per-subject attribution is not reliable.
    NamedInvocation,
    /// The function is one executable test (registered test-producing
    /// attribute).
    NamedFunction,
}

impl HarnessSubjectClaim {
    /// Stable wire string for projections.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NamedInvocation => "named_invocation",
            Self::NamedFunction => "named_function",
        }
    }
}

/// One executable test subject established by a registered harness
/// adapter (#3532). A matching adapter emits these typed subject facts
/// rather than mutating `FunctionFact` ad hoc. Subjects normally also
/// register an ordinary `TestFact` (same name/file/span) so the
/// executable-test denominator and every existing test consumer see it;
/// the reachability authority (#3636) is the one exception — a subject
/// whose construction provably cannot reach the harness run entry point
/// keeps its subject fact and claim while its `TestFact` is withheld
/// and a `registration_unreachable` limitation names it.
///
/// Evidence boundary for `HarnessSubjectClaim::NamedInvocation` (#3603):
/// `start_line`/`end_line`/`body` stay the registration invocation, while
/// `calls`/`assertions`/`literals` widen over exactly the code the
/// subject exercises — see the claim's docs for the fail-closed bounds.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HarnessSubjectFact {
    /// The registration that authorized this subject.
    pub registration_id: String,
    /// Harness family (e.g. `custom_harness`, `registered_attribute`).
    pub harness_kind: String,
    /// Adapter generation (e.g. `libtest_mimic_v1`).
    pub adapter: String,
    /// Exact source marker the adapter matched (crate path or attribute
    /// path). Prefix/suffix lookalikes never produce subjects.
    pub marker: String,
    /// Stable subject identity: the trial name or the test fn name.
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
    pub calls: Vec<CallFact>,
    pub assertions: Vec<OracleFact>,
    pub literals: Vec<LiteralFact>,
    pub selector: HarnessSelectorCapability,
    pub claim: HarnessSubjectClaim,
    /// Trust provenance of the authorizing registration (e.g.
    /// `ripr.toml [analysis.test_harnesses]`).
    pub provenance: String,
}

/// One typed limitation recorded by a registered harness adapter (#3532):
/// a shape the registration saw but could not classify statically.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HarnessLimitationFact {
    /// The registration that observed the limitation.
    pub registration_id: String,
    /// Stable limitation code, e.g. `dynamic_trial_name`,
    /// `dynamic_trial_registration`, `ambiguous_import`,
    /// `unanchored_trial_path`, `registration_unreachable` (a trial
    /// construction excluded from every resolved run entry argument, or
    /// a target with no run entry call — the syntactic subject claim is
    /// retained), or `registration_reachability_unknown` (the aggregate
    /// disclosure naming trials whose reachability the bounded resolver
    /// could neither connect nor exclude; they remain admitted).
    pub code: String,
    pub file: PathBuf,
    pub line: usize,
    /// Human-readable detail naming what could not be classified and why.
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OracleFact {
    pub line: usize,
    pub text: String,
    pub kind: OracleKind,
    pub strength: OracleStrength,
    pub observed_tokens: Vec<String>,
    /// #3731 observation authority: whether the guarded Result match's Ok
    /// arm(s) observe the unwrapped success value, decided over the Ok-arm
    /// bodies at extraction time (the synthesized oracle text keeps its
    /// `Ok(..) => ..` template, so this decision cannot be re-derived from
    /// the text). `None` when the fact is not a guarded Result match;
    /// `Some(false)` covers the guarded-routing form (no Ok arm — the
    /// success value flows into a trivial catch-all) and payload-ignoring
    /// Ok arms (`Ok(_) => {}`), so a return-value probe's confirmation is
    /// refused (fail closed, under-credit).
    pub ok_value_observed: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallFact {
    pub line: usize,
    pub name: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReturnFact {
    pub line: usize,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LiteralFact {
    pub line: usize,
    pub value: String,
}

/// Closed probe-shape vocabulary (#5415 step 1).
///
/// `kind` used to be a `String` per shape: about 0.5M small allocations on a
/// mid-sized workspace for 8 distinct values. The enum serializes as exactly
/// the same strings, so cache payloads, goldens and machine output are
/// byte-identical; unknown strings now fail at the decode boundary and take
/// the corrupt-entry quarantine path instead of reaching analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeShapeKind {
    Predicate,
    ReturnValue,
    ErrorPath,
    CallDeletion,
    FieldConstruction,
    SideEffect,
    MatchArm,
    UnsafeBoundary,
}

impl ProbeShapeKind {
    /// Wire spelling shared by the cache payload, goldens and machine output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Predicate => "predicate",
            Self::ReturnValue => "return_value",
            Self::ErrorPath => "error_path",
            Self::CallDeletion => "call_deletion",
            Self::FieldConstruction => "field_construction",
            Self::SideEffect => "side_effect",
            Self::MatchArm => "match_arm",
            Self::UnsafeBoundary => "unsafe_boundary",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeShapeFact {
    pub start_line: usize,
    pub end_line: usize,
    /// Byte offset of the shape's start within the source file. Populated
    /// by the parser-backed summarizer; the lexical fallback emits no
    /// probe shapes at all, so this stays accurate.
    pub start_byte: usize,
    pub kind: ProbeShapeKind,
    pub text: SourceText,
}

/// Cache payload mirrors of the fact structs. The wire carries [`WireText`]
/// spans instead of allocated strings; only [`FileFacts`] crosses the
/// decode boundary, linking every child to the entry's `source`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct FunctionFactWire {
    pub id: SymbolId,
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: WireText,
    pub calls: Vec<CallFact>,
    pub returns: Vec<ReturnFact>,
    pub literals: Vec<LiteralFact>,
    pub source_role: FunctionSourceRole,
    pub attrs: Vec<String>,
    #[serde(default)]
    pub impl_attrs: Vec<String>,
    #[serde(default)]
    pub nested_fn_names: Vec<String>,
    #[serde(default)]
    pub let_bindings: Vec<LetBindingFact>,
    #[serde(default)]
    pub item: FunctionItemFact,
    #[serde(default)]
    pub impl_context: FunctionImplContext,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TestFactWire {
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: WireText,
    pub calls: Vec<CallFact>,
    pub assertions: Vec<OracleFact>,
    pub literals: Vec<LiteralFact>,
    pub attrs: Vec<String>,
    #[serde(default)]
    pub nested_fn_names: Vec<String>,
    #[serde(default)]
    pub let_bindings: Vec<LetBindingFact>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ProbeShapeFactWire {
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub kind: ProbeShapeKind,
    pub text: WireText,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct FileFactsWire {
    pub path: PathBuf,
    pub functions: Vec<FunctionFactWire>,
    pub tests: Vec<TestFactWire>,
    pub calls: Vec<CallFact>,
    pub returns: Vec<ReturnFact>,
    pub literals: Vec<LiteralFact>,
    pub probe_shapes: Vec<ProbeShapeFactWire>,
    pub used_lexical_fallback: bool,
    pub module_declarations: Vec<ModuleDeclarationFact>,
    pub unresolved_property_macros: Vec<UnresolvedPropertyMacroFact>,
    pub source: String,
}

impl FunctionFactWire {
    /// Attached encoding: the body spans only when it shares `parent`
    /// (see [`WireText::attached`]).
    pub(crate) fn attached(fact: &FunctionFact, parent: &Arc<str>) -> Self {
        Self {
            id: fact.id.clone(),
            name: fact.name.clone(),
            file: fact.file.clone(),
            start_line: fact.start_line,
            end_line: fact.end_line,
            body: WireText::attached(&fact.body, parent),
            calls: fact.calls.clone(),
            returns: fact.returns.clone(),
            literals: fact.literals.clone(),
            source_role: fact.source_role,
            attrs: fact.attrs.clone(),
            impl_attrs: fact.impl_attrs.clone(),
            nested_fn_names: fact.nested_fn_names.clone(),
            let_bindings: fact.let_bindings.clone(),
            item: fact.item.clone(),
            impl_context: fact.impl_context.clone(),
        }
    }

    /// Self-contained encoding for detached snapshots (whole-index wire):
    /// the body resolves to inline text, so the payload decodes without
    /// its file allocation. Attached children instead use [`Self::attached`]
    /// through [`FileFacts`].
    fn detached(fact: &FunctionFact) -> Self {
        Self {
            id: fact.id.clone(),
            name: fact.name.clone(),
            file: fact.file.clone(),
            start_line: fact.start_line,
            end_line: fact.end_line,
            body: WireText::Inline {
                text: fact.body.as_str().to_string(),
            },
            calls: fact.calls.clone(),
            returns: fact.returns.clone(),
            literals: fact.literals.clone(),
            source_role: fact.source_role,
            attrs: fact.attrs.clone(),
            impl_attrs: fact.impl_attrs.clone(),
            nested_fn_names: fact.nested_fn_names.clone(),
            let_bindings: fact.let_bindings.clone(),
            item: fact.item.clone(),
            impl_context: fact.impl_context.clone(),
        }
    }
}

impl TestFactWire {
    /// Attached encoding; see [`FunctionFactWire::attached`].
    pub(crate) fn attached(fact: &TestFact, parent: &Arc<str>) -> Self {
        Self {
            name: fact.name.clone(),
            file: fact.file.clone(),
            start_line: fact.start_line,
            end_line: fact.end_line,
            body: WireText::attached(&fact.body, parent),
            calls: fact.calls.clone(),
            assertions: fact.assertions.clone(),
            literals: fact.literals.clone(),
            attrs: fact.attrs.clone(),
            nested_fn_names: fact.nested_fn_names.clone(),
            let_bindings: fact.let_bindings.clone(),
        }
    }

    /// Self-contained encoding for detached snapshots; see
    /// [`FunctionFactWire::detached`].
    fn detached(fact: &TestFact) -> Self {
        Self {
            name: fact.name.clone(),
            file: fact.file.clone(),
            start_line: fact.start_line,
            end_line: fact.end_line,
            body: WireText::Inline {
                text: fact.body.as_str().to_string(),
            },
            calls: fact.calls.clone(),
            assertions: fact.assertions.clone(),
            literals: fact.literals.clone(),
            attrs: fact.attrs.clone(),
            nested_fn_names: fact.nested_fn_names.clone(),
            let_bindings: fact.let_bindings.clone(),
        }
    }
}

impl ProbeShapeFactWire {
    /// Attached encoding; see [`FunctionFactWire::attached`].
    pub(crate) fn attached(fact: &ProbeShapeFact, parent: &Arc<str>) -> Self {
        Self {
            start_line: fact.start_line,
            end_line: fact.end_line,
            start_byte: fact.start_byte,
            kind: fact.kind,
            text: WireText::attached(&fact.text, parent),
        }
    }

    /// Self-contained encoding for detached snapshots; see
    /// [`FunctionFactWire::detached`].
    fn detached(fact: &ProbeShapeFact) -> Self {
        Self {
            start_line: fact.start_line,
            end_line: fact.end_line,
            start_byte: fact.start_byte,
            kind: fact.kind,
            text: WireText::Inline {
                text: fact.text.as_str().to_string(),
            },
        }
    }
}

impl From<&FileFacts> for FileFactsWire {
    fn from(facts: &FileFacts) -> Self {
        Self {
            path: facts.path.clone(),
            functions: facts
                .functions
                .iter()
                .map(|fact| FunctionFactWire::attached(fact, &facts.source))
                .collect(),
            tests: facts
                .tests
                .iter()
                .map(|fact| TestFactWire::attached(fact, &facts.source))
                .collect(),
            calls: facts.calls.clone(),
            returns: facts.returns.clone(),
            literals: facts.literals.clone(),
            probe_shapes: facts
                .probe_shapes
                .iter()
                .map(|fact| ProbeShapeFactWire::attached(fact, &facts.source))
                .collect(),
            used_lexical_fallback: facts.used_lexical_fallback,
            module_declarations: facts.module_declarations.clone(),
            unresolved_property_macros: facts.unresolved_property_macros.clone(),
            source: facts.source.to_string(),
        }
    }
}

impl FunctionFactWire {
    fn link(self, source: &Arc<str>) -> Result<FunctionFact, String> {
        Ok(FunctionFact {
            id: self.id,
            name: self.name,
            file: self.file,
            start_line: self.start_line,
            end_line: self.end_line,
            body: link_wire_text(self.body, source, "function body")?,
            calls: self.calls,
            returns: self.returns,
            literals: self.literals,
            source_role: self.source_role,
            attrs: self.attrs,
            impl_attrs: self.impl_attrs,
            nested_fn_names: self.nested_fn_names,
            let_bindings: self.let_bindings,
            item: self.item,
            impl_context: self.impl_context,
        })
    }
}

impl TestFactWire {
    fn link(self, source: &Arc<str>) -> Result<TestFact, String> {
        Ok(TestFact {
            name: self.name,
            file: self.file,
            start_line: self.start_line,
            end_line: self.end_line,
            body: link_wire_text(self.body, source, "test body")?,
            calls: self.calls,
            assertions: self.assertions,
            literals: self.literals,
            attrs: self.attrs,
            nested_fn_names: self.nested_fn_names,
            let_bindings: self.let_bindings,
        })
    }
}

impl ProbeShapeFactWire {
    fn link(self, source: &Arc<str>) -> Result<ProbeShapeFact, String> {
        Ok(ProbeShapeFact {
            start_line: self.start_line,
            end_line: self.end_line,
            start_byte: self.start_byte,
            kind: self.kind,
            text: link_wire_text(self.text, source, "probe shape text")?,
        })
    }
}

impl FileFactsWire {
    fn link(self) -> Result<FileFacts, String> {
        let source: Arc<str> = Arc::from(self.source.as_str());
        let mut functions = Vec::with_capacity(self.functions.len());
        for wire in self.functions {
            functions.push(wire.link(&source)?);
        }
        let mut tests = Vec::with_capacity(self.tests.len());
        for wire in self.tests {
            tests.push(wire.link(&source)?);
        }
        let mut probe_shapes = Vec::with_capacity(self.probe_shapes.len());
        for wire in self.probe_shapes {
            probe_shapes.push(wire.link(&source)?);
        }
        Ok(FileFacts {
            path: self.path,
            functions,
            tests,
            calls: self.calls,
            returns: self.returns,
            literals: self.literals,
            probe_shapes,
            used_lexical_fallback: self.used_lexical_fallback,
            module_declarations: self.module_declarations,
            unresolved_property_macros: self.unresolved_property_macros,
            role_provenance: SourceRoleProvenance::default(),
            source,
        })
    }
}

impl serde::Serialize for FunctionFact {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FunctionFactWire::detached(self).serialize(serializer)
    }
}

/// Detached decode for whole-index snapshots: inline bodies decode owned,
/// but a span without its file allocation is rejected instead of guessed.
/// Attached bodies always decode through [`FileFacts`].
impl<'de> serde::Deserialize<'de> for FunctionFact {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = FunctionFactWire::deserialize(deserializer)?;
        let body = match wire.body {
            WireText::Inline { text } => SourceText::owned(text),
            WireText::Span { .. } => {
                return Err(serde::de::Error::custom(
                    "function body span needs its file source; decode through FileFacts",
                ));
            }
        };
        Ok(FunctionFact {
            id: wire.id,
            name: wire.name,
            file: wire.file,
            start_line: wire.start_line,
            end_line: wire.end_line,
            body,
            calls: wire.calls,
            returns: wire.returns,
            literals: wire.literals,
            source_role: wire.source_role,
            attrs: wire.attrs,
            impl_attrs: wire.impl_attrs,
            nested_fn_names: wire.nested_fn_names,
            let_bindings: wire.let_bindings,
            item: wire.item,
            impl_context: wire.impl_context,
        })
    }
}

impl serde::Serialize for TestFact {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        TestFactWire::detached(self).serialize(serializer)
    }
}

/// Detached decode for whole-index snapshots; see [`FunctionFact`].
impl<'de> serde::Deserialize<'de> for TestFact {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = TestFactWire::deserialize(deserializer)?;
        let body = match wire.body {
            WireText::Inline { text } => SourceText::owned(text),
            WireText::Span { .. } => {
                return Err(serde::de::Error::custom(
                    "test body span needs its file source; decode through FileFacts",
                ));
            }
        };
        Ok(TestFact {
            name: wire.name,
            file: wire.file,
            start_line: wire.start_line,
            end_line: wire.end_line,
            body,
            calls: wire.calls,
            assertions: wire.assertions,
            literals: wire.literals,
            attrs: wire.attrs,
            nested_fn_names: wire.nested_fn_names,
            let_bindings: wire.let_bindings,
        })
    }
}

impl serde::Serialize for ProbeShapeFact {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ProbeShapeFactWire::detached(self).serialize(serializer)
    }
}

impl serde::Serialize for FileFacts {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FileFactsWire::from(self).serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for FileFacts {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        FileFactsWire::deserialize(deserializer)
            .and_then(|wire| wire.link().map_err(serde::de::Error::custom))
    }
}

pub type FunctionSummary = FunctionFact;
pub type TestSummary = TestFact;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use std::mem::size_of;

    #[test]
    fn rust_index_default_has_empty_fact_sets() {
        let index = RustIndex::default();
        assert!(index.files().is_empty());
        assert!(index.tests().is_empty());
        assert!(index.functions().is_empty());
    }

    #[test]
    fn module_path_target_encodes_literal_and_decodes_pre_4171_unit_entries()
    -> Result<(), serde_json::Error> {
        let literal = ModulePathTarget::Literal("other.rs".to_string());
        let encoded = serde_json::to_value(&literal)?;
        assert_eq!(
            encoded,
            serde_json::json!({"kind": "literal", "path": "other.rs"})
        );
        assert_eq!(
            serde_json::from_value::<ModulePathTarget>(encoded)?,
            literal
        );
        // Cache entries written before #4171 hold only unit variants in the
        // internally tagged shape; they must keep decoding.
        for (legacy, expected) in [
            (r#"{"kind":"default"}"#, ModulePathTarget::Default),
            (r#"{"kind":"unknown"}"#, ModulePathTarget::Unknown),
        ] {
            assert_eq!(serde_json::from_str::<ModulePathTarget>(legacy)?, expected);
            assert_eq!(serde_json::to_string(&expected)?, legacy);
        }
        Ok(())
    }

    #[test]
    fn file_facts_default_has_empty_collections() {
        let facts = FileFacts::default();
        assert!(facts.path.as_os_str().is_empty());
        assert!(facts.functions.is_empty());
        assert!(facts.tests.is_empty());
        assert!(facts.calls.is_empty());
        assert!(facts.returns.is_empty());
        assert!(facts.literals.is_empty());
        assert!(facts.probe_shapes.is_empty());
        assert!(!facts.used_lexical_fallback);
        // #3533: parser-produced module declarations start empty and the
        // composer-owned provenance starts unresolved-free standalone.
        assert!(facts.module_declarations.is_empty());
        assert!(facts.role_provenance.edges.is_empty());
        assert_eq!(facts.role_provenance.earliest_unresolved_reason, None);
    }

    #[test]
    fn fact_types_clone_and_compare_equal_for_simple_samples() {
        let call = CallFact {
            line: 1,
            name: "test_fn".to_string(),
            text: "test_fn()".to_string(),
        };
        let call_cloned = call.clone();
        assert_eq!(call, call_cloned);

        let ret = ReturnFact {
            line: 2,
            text: "return Ok(())".to_string(),
        };
        let ret_cloned = ret.clone();
        assert_eq!(ret, ret_cloned);

        let lit = LiteralFact {
            line: 3,
            value: "42".to_string(),
        };
        let lit_cloned = lit.clone();
        assert_eq!(lit, lit_cloned);
    }

    #[test]
    fn probe_shape_fact_preserves_span_kind_text_and_start_byte() {
        let shape = ProbeShapeFact {
            start_line: 10,
            end_line: 12,
            start_byte: 256,
            kind: ProbeShapeKind::Predicate,
            text: "x > 0".into(),
        };
        assert_eq!(shape.start_line, 10);
        assert_eq!(shape.end_line, 12);
        assert_eq!(shape.start_byte, 256);
        assert_eq!(shape.kind, ProbeShapeKind::Predicate);
        assert_eq!(shape.text, "x > 0");
    }

    #[test]
    fn probe_shape_kind_serde_keeps_the_historical_wire_strings() -> Result<(), serde_json::Error> {
        // #5415 step 1: the in-memory type changed, the bytes did not. Every
        // variant must round-trip through exactly its historical string, or
        // cache payloads and goldens drift.
        let cases = [
            (ProbeShapeKind::Predicate, "predicate"),
            (ProbeShapeKind::ReturnValue, "return_value"),
            (ProbeShapeKind::ErrorPath, "error_path"),
            (ProbeShapeKind::CallDeletion, "call_deletion"),
            (ProbeShapeKind::FieldConstruction, "field_construction"),
            (ProbeShapeKind::SideEffect, "side_effect"),
            (ProbeShapeKind::MatchArm, "match_arm"),
            (ProbeShapeKind::UnsafeBoundary, "unsafe_boundary"),
        ];
        for (kind, wire) in cases {
            assert_eq!(kind.as_str(), wire);
            let encoded = serde_json::to_value(kind)?;
            assert_eq!(encoded, serde_json::json!(wire));
            let decoded: ProbeShapeKind = serde_json::from_value(encoded)?;
            assert_eq!(decoded, kind);
        }
        Ok(())
    }

    #[test]
    fn source_text_is_string_sized() {
        // #5415 step 2: the sharing wrapper must not bloat the ~0.5M shapes
        // and facts that carry it; Arc + u32 span + tag fits in 24 bytes.
        assert_eq!(size_of::<SourceText>(), size_of::<String>());
    }

    #[test]
    fn source_text_shares_exact_spans_and_owns_the_rest() {
        let source: Arc<str> = Arc::from("fn a() {}\nfn b(x: i32) {}\n");
        // Exact substring at the sliced offset shares the allocation.
        let shared = SourceText::shared_or_owned(&source, 10, "fn b(x: i32) {}");
        assert_eq!(shared.as_str(), "fn b(x: i32) {}");
        assert!(
            shared
                .shared_source()
                .is_some_and(|arc| Arc::ptr_eq(arc, &source))
        );
        // A wrong offset degrades to an owned copy with identical text,
        // never to a corrupt slice.
        let owned = SourceText::shared_or_owned(&source, 11, "fn b(x: i32) {}");
        assert_eq!(owned.as_str(), "fn b(x: i32) {}");
        assert_eq!(owned.shared_source(), None);
        // Text that is not a substring at all stays owned.
        let synthetic = SourceText::shared_or_owned(&source, 0, "fn c() {}");
        assert_eq!(synthetic.as_str(), "fn c() {}");
        assert_eq!(synthetic.shared_source(), None);
        // Past-the-end spans stay owned instead of panicking.
        let past_end = SourceText::shared_or_owned(&source, usize::MAX, "fn b(x: i32) {}");
        assert_eq!(past_end.as_str(), "fn b(x: i32) {}");
        assert_eq!(past_end.shared_source(), None);
    }

    #[test]
    fn source_text_matches_string_observation() {
        let text = SourceText::from("x > 0".to_string());
        let plain = "x > 0".to_string();
        assert_eq!(format!("{text:?}"), format!("{plain:?}"));
        assert_eq!(text.to_string(), plain);
        assert_eq!(text, plain);
        assert_eq!(plain, text);
        assert_eq!(text, "x > 0");
        assert_eq!("x > 0", text);
        assert!(text.contains(">"));
        assert_eq!(text.len(), plain.len());
        assert_eq!(text.cmp(&SourceText::from("y")), std::cmp::Ordering::Less);
        let stub: &str = &text;
        assert_eq!(stub, "x > 0");
    }

    #[test]
    fn file_facts_wire_roundtrip_links_spans_to_one_allocation() -> Result<(), serde_json::Error> {
        let source: Arc<str> = Arc::from("fn a() {}\n#[test] fn b() { assert!(true); }\n");
        let facts = FileFacts {
            path: PathBuf::from("src/lib.rs"),
            functions: vec![FunctionFact {
                id: SymbolId("src/lib.rs::a".to_string()),
                name: "a".to_string(),
                file: PathBuf::from("src/lib.rs"),
                start_line: 1,
                end_line: 1,
                body: SourceText::shared_or_owned(&source, 0, "fn a() {}"),
                calls: Vec::new(),
                returns: Vec::new(),
                literals: Vec::new(),
                source_role: FunctionSourceRole::Production,
                attrs: Vec::new(),
                impl_attrs: Vec::new(),
                nested_fn_names: Vec::new(),
                let_bindings: Vec::new(),
                item: FunctionItemFact::default(),
                impl_context: FunctionImplContext::Unknown,
            }],
            tests: Vec::new(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            probe_shapes: vec![ProbeShapeFact {
                start_line: 2,
                end_line: 2,
                start_byte: 27,
                kind: ProbeShapeKind::Predicate,
                text: SourceText::shared_or_owned(&source, 27, "assert!(true)"),
            }],
            used_lexical_fallback: false,
            module_declarations: Vec::new(),
            unresolved_property_macros: Vec::new(),
            role_provenance: SourceRoleProvenance::default(),
            source: Arc::clone(&source),
        };
        // The wire carries spans, not copied bodies.
        let wire = serde_json::to_value(&facts)?;
        assert_eq!(
            wire["functions"][0]["body"],
            serde_json::json!({"start": 0, "len": 9})
        );
        assert_eq!(
            wire["probe_shapes"][0]["text"],
            serde_json::json!({"start": 27, "len": 13})
        );
        // Decode links every child to the single source allocation.
        let decoded: FileFacts = serde_json::from_value(wire)?;
        assert_eq!(decoded.functions[0].body.as_str(), "fn a() {}");
        assert_eq!(decoded.probe_shapes[0].text.as_str(), "assert!(true)");
        for child in [
            decoded.functions[0].body.shared_source(),
            decoded.probe_shapes[0].text.shared_source(),
        ] {
            assert!(child.is_some_and(|arc| Arc::ptr_eq(arc, &decoded.source)));
        }
        assert_eq!(decoded, facts);
        Ok(())
    }

    #[test]
    fn file_facts_wire_rejects_spans_outside_the_source() {
        let wire = serde_json::json!({
            "path": "src/lib.rs",
            "functions": [{
                "id": "src/lib.rs::a",
                "name": "a",
                "file": "src/lib.rs",
                "start_line": 1,
                "end_line": 1,
                "body": {"start": 4, "len": 99},
                "calls": [],
                "returns": [],
                "literals": [],
                "source_role": "production",
                "attrs": [],
            }],
            "tests": [],
            "calls": [],
            "returns": [],
            "literals": [],
            "probe_shapes": [],
            "used_lexical_fallback": false,
            "module_declarations": [],
            "unresolved_property_macros": [],
            "source": "fn a() {}",
        });
        let decoded: Result<FileFacts, _> = serde_json::from_value(wire);
        assert!(decoded.is_err(), "out-of-range span must reject the entry");
    }

    #[test]
    fn file_facts_wire_rejects_spans_splitting_a_char() {
        // "🦀" is 4 bytes; a span ending inside it is not a valid slice.
        let wire = serde_json::json!({
            "path": "src/lib.rs",
            "functions": [],
            "tests": [],
            "calls": [],
            "returns": [],
            "literals": [],
            "probe_shapes": [{
                "start_line": 1,
                "end_line": 1,
                "start_byte": 0,
                "kind": "predicate",
                "text": {"start": 0, "len": 2},
            }],
            "used_lexical_fallback": false,
            "module_declarations": [],
            "unresolved_property_macros": [],
            "source": "🦀();",
        });
        let decoded: Result<FileFacts, _> = serde_json::from_value(wire);
        assert!(decoded.is_err(), "split char must reject the entry");
    }

    #[test]
    fn file_facts_wire_rejects_legacy_bare_string_bodies() {
        // 1.20 payloads carry bodies as bare strings; the span wire must
        // fail the decode (cache miss + recompute), never misread them.
        let wire = serde_json::json!({
            "path": "src/lib.rs",
            "functions": [{
                "id": "src/lib.rs::a",
                "name": "a",
                "file": "src/lib.rs",
                "start_line": 1,
                "end_line": 1,
                "body": "fn a() {}",
                "calls": [],
                "returns": [],
                "literals": [],
                "source_role": "production",
                "attrs": [],
            }],
            "tests": [],
            "calls": [],
            "returns": [],
            "literals": [],
            "probe_shapes": [],
            "used_lexical_fallback": false,
            "module_declarations": [],
            "unresolved_property_macros": [],
            "source": "fn a() {}",
        });
        let decoded: Result<FileFacts, _> = serde_json::from_value(wire);
        assert!(decoded.is_err(), "legacy string body must not decode");
    }

    #[test]
    fn attached_wire_inlines_children_from_a_foreign_allocation() -> Result<(), serde_json::Error> {
        // A child built against another allocation must never encode a
        // span: the span would resolve against the wrong `source` at
        // decode. It inlines its text instead, and the round trip
        // preserves every byte.
        let home: Arc<str> = Arc::from("fn a() {}\n");
        let away: Arc<str> = Arc::from("fn b() {}\n");
        let mut facts = FileFacts {
            path: PathBuf::from("src/lib.rs"),
            functions: vec![FunctionFact {
                id: SymbolId("src/lib.rs::a".to_string()),
                name: "a".to_string(),
                file: PathBuf::from("src/lib.rs"),
                start_line: 1,
                end_line: 1,
                body: SourceText::shared_or_owned(&home, 0, "fn a() {}"),
                calls: Vec::new(),
                returns: Vec::new(),
                literals: Vec::new(),
                source_role: FunctionSourceRole::Production,
                attrs: Vec::new(),
                impl_attrs: Vec::new(),
                nested_fn_names: Vec::new(),
                let_bindings: Vec::new(),
                item: FunctionItemFact::default(),
                impl_context: FunctionImplContext::Unknown,
            }],
            tests: Vec::new(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            probe_shapes: Vec::new(),
            used_lexical_fallback: false,
            module_declarations: Vec::new(),
            unresolved_property_macros: Vec::new(),
            role_provenance: SourceRoleProvenance::default(),
            source: Arc::clone(&home),
        };
        // Paired children span.
        let wire = serde_json::to_value(&facts)?;
        assert_eq!(
            wire["functions"][0]["body"],
            serde_json::json!({"start": 0, "len": 9})
        );
        // Reassigned source: the foreign child inlines, and the text
        // survives the round trip instead of resolving to "fn b() {}".
        facts.source = Arc::clone(&away);
        let wire = serde_json::to_value(&facts)?;
        assert_eq!(
            wire["functions"][0]["body"],
            serde_json::json!({"text": "fn a() {}"})
        );
        let decoded: FileFacts = serde_json::from_value(wire)?;
        assert_eq!(decoded.functions[0].body.as_str(), "fn a() {}");
        assert_eq!(decoded.source.as_ref(), "fn b() {}\n");
        Ok(())
    }

    #[test]
    fn detached_fact_decode_accepts_inline_and_rejects_spans() -> Result<(), serde_json::Error> {
        let inline = serde_json::json!({
            "name": "b",
            "file": "src/lib.rs",
            "start_line": 2,
            "end_line": 2,
            "body": {"text": "fn b() {}"},
            "calls": [],
            "assertions": [],
            "literals": [],
            "attrs": [],
        });
        let decoded: TestFact = serde_json::from_value(inline)?;
        assert_eq!(decoded.body.as_str(), "fn b() {}");
        let span = serde_json::json!({
            "name": "b",
            "file": "src/lib.rs",
            "start_line": 2,
            "end_line": 2,
            "body": {"start": 11, "len": 9},
            "calls": [],
            "assertions": [],
            "literals": [],
            "attrs": [],
        });
        let detached: Result<TestFact, _> = serde_json::from_value(span);
        assert!(detached.is_err(), "detached span must not resolve");
        Ok(())
    }

    #[test]
    fn probe_shape_kind_rejects_unknown_wire_strings_at_decode() {
        // Unknown kinds used to decode into a String and map to None in
        // family_for_probe_shape. Now they fail at the decode boundary and
        // the cache entry takes the corrupt-entry quarantine path. Cases
        // carried over from the retired is_known_probe_shape exactness test.
        for unknown in [
            "",
            "opaque_shape",
            "not_return_value",
            "return_value_extra",
            "predicate ",
            "side-effect",
            "MATCH_ARM",
        ] {
            let decoded: Result<ProbeShapeKind, _> =
                serde_json::from_value(serde_json::Value::String(unknown.to_string()));
            assert!(decoded.is_err(), "expected `{unknown}` to stay unknown");
        }
    }

    #[test]
    fn probe_shape_fact_retains_no_per_shape_kind_allocation() {
        // #5415 step 1 pin: kind is one discriminant byte, not a String
        // plus heap. The struct must be strictly smaller than the old
        // String-kind layout (56 vs 72 bytes on 64-bit). Text stays owned;
        // that is step 2.
        assert_eq!(size_of::<ProbeShapeKind>(), 1);
        assert!(size_of::<ProbeShapeFact>() < size_of::<usize>() * 3 + size_of::<String>() * 2);
    }

    #[test]
    fn permission_denied_manifest_is_not_treated_as_absent() {
        let root = Path::new("workspace");
        let relative = Path::new("pkg/src/lib.rs");
        let identity = resolve_package_identity_with_reader(root, relative, |manifest| {
            if manifest.ends_with(Path::new("pkg/Cargo.toml")) {
                Err(std::io::Error::from(ErrorKind::PermissionDenied))
            } else {
                Err(std::io::Error::from(ErrorKind::NotFound))
            }
        });

        assert!(matches!(
            identity,
            PackageIdentity::UnreadableManifest { manifest }
                if manifest.ends_with(Path::new("pkg/Cargo.toml"))
        ));
    }

    #[test]
    fn unreadable_manifest_invalidates_authority_entries() -> Result<(), Box<dyn std::error::Error>>
    {
        struct FixtureCleanup(PathBuf);
        impl Drop for FixtureCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let root = std::env::temp_dir().join(format!(
            "ripr-authority-unreadable-manifest-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let _cleanup = FixtureCleanup(root.clone());
        std::fs::create_dir_all(root.join("pkg/src"))?;
        std::fs::create_dir_all(root.join("pkg/tests"))?;
        // Reading a directory as Cargo.toml is a deterministic non-NotFound
        // manifest error on every supported platform.
        std::fs::create_dir(root.join("pkg/Cargo.toml"))?;

        let sources = [
            (
                PathBuf::from("pkg/src/lib.rs"),
                "pub fn source() -> i32 { 1 }\n",
            ),
            (
                PathBuf::from("pkg/tests/lib.rs"),
                "#[test]\nfn source_test() { assert_eq!(1, 1); }\n",
            ),
        ];
        for (path, source) in &sources {
            std::fs::write(root.join(path), source)?;
        }
        let files = sources
            .iter()
            .map(|(path, source)| {
                (
                    path.clone(),
                    FileFacts {
                        path: path.clone(),
                        source: (*source).into(),
                        ..FileFacts::default()
                    },
                )
            })
            .collect();
        let authority = WorkspaceRootAuthority::from_index(&root, &files);
        let source = Path::new("pkg/src/lib.rs");
        let test = Path::new("pkg/tests/lib.rs");
        let source_authority = authority.files.get(source).ok_or("missing source")?;
        let test_authority = authority.files.get(test).ok_or("missing test")?;

        assert!(!source_authority.valid);
        assert!(!test_authority.valid);
        assert_ne!(
            source_authority.package_identity,
            test_authority.package_identity
        );
        assert!(!authority.validates_target(source, test, "pub fn source() -> i32 { 1 }\n"));
        Ok(())
    }
    #[test]
    fn manifest_added_after_index_invalidates_target() -> Result<(), Box<dyn std::error::Error>> {
        struct FixtureCleanup(PathBuf);
        impl Drop for FixtureCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let root = std::env::temp_dir().join(format!(
            "ripr-authority-manifest-race-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let _cleanup = FixtureCleanup(root.clone());
        std::fs::create_dir_all(root.join("pkg/src"))?;
        std::fs::create_dir_all(root.join("pkg/tests"))?;
        std::fs::write(root.join("pkg/Cargo.toml"), "[package]\nname = \"pkg\"\n")?;

        let sources = [
            (
                PathBuf::from("pkg/src/lib.rs"),
                "pub fn source() -> i32 { 1 }\n",
            ),
            (
                PathBuf::from("pkg/tests/lib.rs"),
                "#[test]\nfn source_test() { assert_eq!(1, 1); }\n",
            ),
        ];
        for (path, source) in &sources {
            std::fs::write(root.join(path), source)?;
        }
        let files = sources
            .iter()
            .map(|(path, source)| {
                (
                    path.clone(),
                    FileFacts {
                        path: path.clone(),
                        source: (*source).into(),
                        ..FileFacts::default()
                    },
                )
            })
            .collect();
        let authority = WorkspaceRootAuthority::from_index(&root, &files);
        std::fs::write(
            root.join("pkg/tests/Cargo.toml"),
            "[package]\nname = \"nested-tests\"\n",
        )?;

        assert!(!authority.validates_target(
            Path::new("pkg/tests/lib.rs"),
            Path::new("pkg/src/lib.rs"),
            "#[test]\nfn source_test() { assert_eq!(1, 1); }\n"
        ));
        Ok(())
    }

    #[test]
    fn source_change_invalidates_cached_currentness() -> Result<(), Box<dyn std::error::Error>> {
        struct FixtureCleanup(PathBuf);
        impl Drop for FixtureCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let root = std::env::temp_dir().join(format!(
            "ripr-authority-cache-invalidation-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let _cleanup = FixtureCleanup(root.clone());
        std::fs::create_dir_all(root.join("pkg/src"))?;
        std::fs::create_dir_all(root.join("pkg/tests"))?;
        std::fs::write(root.join("pkg/Cargo.toml"), "[package]\nname = \"pkg\"\n")?;
        let sources = [
            (
                PathBuf::from("pkg/src/lib.rs"),
                "pub fn source() -> i32 { 1 }\n",
            ),
            (
                PathBuf::from("pkg/tests/lib.rs"),
                "#[test]\nfn source_test() { assert_eq!(1, 1); }\n",
            ),
        ];
        for (path, source) in &sources {
            std::fs::write(root.join(path), source)?;
        }
        let files = sources
            .iter()
            .map(|(path, source)| {
                (
                    path.clone(),
                    FileFacts {
                        path: path.clone(),
                        source: (*source).into(),
                        ..FileFacts::default()
                    },
                )
            })
            .collect();
        let authority = WorkspaceRootAuthority::from_index(&root, &files);
        let test = Path::new("pkg/tests/lib.rs");
        let source = Path::new("pkg/src/lib.rs");
        assert!(authority.validates_target(test, source, sources[1].1));
        std::fs::write(root.join("pkg/tests/lib.rs"), "changed\n")?;
        assert!(!authority.validates_target(test, source, sources[1].1));
        Ok(())
    }

    /// #5478: swap the test file for a symlink to identical bytes outside the
    /// root, with the same size and mtime, after the cache saw it current.
    /// Under load the original write and the copy land in one mtime tick; the
    /// test pins that case by copying the mtime instead of racing for it.
    #[cfg(unix)]
    #[test]
    fn symlink_swap_with_same_size_and_mtime_invalidates_cached_currentness()
    -> Result<(), Box<dyn std::error::Error>> {
        struct FixtureCleanup(Vec<PathBuf>);
        impl Drop for FixtureCleanup {
            fn drop(&mut self) {
                for path in &self.0 {
                    let _ = std::fs::remove_dir_all(path);
                }
            }
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "ripr-authority-symlink-swap-{}-{stamp}",
            std::process::id()
        ));
        let root = base.join("root");
        let outside = base.join("outside");
        let _cleanup = FixtureCleanup(vec![base.clone()]);
        std::fs::create_dir_all(root.join("pkg/src"))?;
        std::fs::create_dir_all(root.join("pkg/tests"))?;
        std::fs::create_dir_all(&outside)?;
        std::fs::write(root.join("pkg/Cargo.toml"), "[package]\nname = \"pkg\"\n")?;
        let sources = [
            (
                PathBuf::from("pkg/src/lib.rs"),
                "pub fn source() -> i32 { 1 }\n",
            ),
            (
                PathBuf::from("pkg/tests/lib.rs"),
                "#[test]\nfn source_test() { assert_eq!(1, 1); }\n",
            ),
        ];
        for (path, source) in &sources {
            std::fs::write(root.join(path), source)?;
        }
        let files = sources
            .iter()
            .map(|(path, source)| {
                (
                    path.clone(),
                    FileFacts {
                        path: path.clone(),
                        source: (*source).into(),
                        ..FileFacts::default()
                    },
                )
            })
            .collect();
        let authority = WorkspaceRootAuthority::from_index(&root, &files);
        let test = Path::new("pkg/tests/lib.rs");
        let source = Path::new("pkg/src/lib.rs");
        assert!(authority.validates_target(test, source, sources[1].1));

        let original = root.join(test);
        let modified = std::fs::metadata(&original)?.modified()?;
        let escaped = outside.join("lib.rs");
        std::fs::write(&escaped, sources[1].1)?;
        std::fs::File::options()
            .write(true)
            .open(&escaped)?
            .set_modified(modified)?;
        std::fs::remove_file(&original)?;
        std::os::unix::fs::symlink(&escaped, &original)?;
        let followed = std::fs::metadata(&original)?;
        assert_eq!(
            followed.modified()?,
            modified,
            "fixture must keep the mtime"
        );
        assert_eq!(followed.len(), sources[1].1.len() as u64);

        assert!(!authority.validates_target(test, source, sources[1].1));
        Ok(())
    }

    /// #5478 review: a test file that is already a symlink inside the root,
    /// retargeted to an outside copy with the same size and mtime. Its
    /// `link` flag never changes, so the link target and identity must.
    #[cfg(unix)]
    #[test]
    fn symlink_retarget_with_same_size_and_mtime_invalidates_cached_currentness()
    -> Result<(), Box<dyn std::error::Error>> {
        struct FixtureCleanup(PathBuf);
        impl Drop for FixtureCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "ripr-authority-symlink-retarget-{}-{stamp}",
            std::process::id()
        ));
        let root = base.join("root");
        let outside = base.join("outside");
        let _cleanup = FixtureCleanup(base.clone());
        std::fs::create_dir_all(root.join("pkg/src"))?;
        std::fs::create_dir_all(root.join("pkg/tests"))?;
        std::fs::create_dir_all(&outside)?;
        std::fs::write(root.join("pkg/Cargo.toml"), "[package]\nname = \"pkg\"\n")?;
        let test_source = "#[test]\nfn source_test() { assert_eq!(1, 1); }\n";
        let sources = [
            (
                PathBuf::from("pkg/src/lib.rs"),
                "pub fn source() -> i32 { 1 }\n",
            ),
            (PathBuf::from("pkg/tests/lib.rs"), test_source),
        ];
        std::fs::write(root.join(&sources[0].0), sources[0].1)?;
        let inside = root.join("pkg/tests/real.rs");
        std::fs::write(&inside, test_source)?;
        let link = root.join(&sources[1].0);
        std::os::unix::fs::symlink("real.rs", &link)?;
        let files = sources
            .iter()
            .map(|(path, source)| {
                (
                    path.clone(),
                    FileFacts {
                        path: path.clone(),
                        source: (*source).into(),
                        ..FileFacts::default()
                    },
                )
            })
            .collect();
        let authority = WorkspaceRootAuthority::from_index(&root, &files);
        let test = Path::new("pkg/tests/lib.rs");
        let source = Path::new("pkg/src/lib.rs");
        assert!(authority.validates_target(test, source, test_source));

        let modified = std::fs::metadata(&inside)?.modified()?;
        let escaped = outside.join("lib.rs");
        std::fs::write(&escaped, test_source)?;
        std::fs::File::options()
            .write(true)
            .open(&escaped)?
            .set_modified(modified)?;
        std::fs::remove_file(&link)?;
        std::os::unix::fs::symlink(&escaped, &link)?;
        let followed = std::fs::metadata(&link)?;
        assert_eq!(
            followed.modified()?,
            modified,
            "fixture must keep the mtime"
        );
        assert_eq!(followed.len(), test_source.len() as u64);

        assert!(!authority.validates_target(test, source, test_source));
        Ok(())
    }

    /// #5478 review: the test file links to an intermediate link that points
    /// at an in-root file. Moving that file outside (same inode, size, mtime)
    /// and retargeting the intermediate link changes neither the test file's
    /// entry nor the followed file, only where the chain resolves.
    #[cfg(unix)]
    #[test]
    fn chained_symlink_retarget_invalidates_cached_currentness()
    -> Result<(), Box<dyn std::error::Error>> {
        struct FixtureCleanup(PathBuf);
        impl Drop for FixtureCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "ripr-authority-symlink-chain-{}-{stamp}",
            std::process::id()
        ));
        let root = base.join("root");
        let outside = base.join("outside");
        let _cleanup = FixtureCleanup(base.clone());
        std::fs::create_dir_all(root.join("pkg/src"))?;
        std::fs::create_dir_all(root.join("pkg/tests"))?;
        std::fs::create_dir_all(&outside)?;
        std::fs::write(root.join("pkg/Cargo.toml"), "[package]\nname = \"pkg\"\n")?;
        let test_source = "#[test]\nfn source_test() { assert_eq!(1, 1); }\n";
        let sources = [
            (
                PathBuf::from("pkg/src/lib.rs"),
                "pub fn source() -> i32 { 1 }\n",
            ),
            (PathBuf::from("pkg/tests/lib.rs"), test_source),
        ];
        std::fs::write(root.join(&sources[0].0), sources[0].1)?;
        let real = root.join("pkg/src/real.rs");
        std::fs::write(&real, test_source)?;
        let intermediate = root.join("pkg/src/intermediate.rs");
        std::os::unix::fs::symlink(&real, &intermediate)?;
        std::os::unix::fs::symlink(&intermediate, root.join(&sources[1].0))?;
        let files = sources
            .iter()
            .map(|(path, source)| {
                (
                    path.clone(),
                    FileFacts {
                        path: path.clone(),
                        source: (*source).into(),
                        ..FileFacts::default()
                    },
                )
            })
            .collect();
        let authority = WorkspaceRootAuthority::from_index(&root, &files);
        let test = Path::new("pkg/tests/lib.rs");
        let source = Path::new("pkg/src/lib.rs");
        assert!(authority.validates_target(test, source, test_source));

        let moved = outside.join("real.rs");
        std::fs::rename(&real, &moved)?;
        std::fs::remove_file(&intermediate)?;
        std::os::unix::fs::symlink(&moved, &intermediate)?;
        assert!(
            std::fs::canonicalize(root.join(test))?.starts_with(&outside),
            "fixture must resolve outside the root"
        );

        assert!(!authority.validates_target(test, source, test_source));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_lookalike_resolution_invalidates_cached_currentness()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::ffi::OsStrExt;

        struct FixtureCleanup(PathBuf);
        impl Drop for FixtureCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "ripr-authority-non-utf8-{}-{stamp}",
            std::process::id()
        ));
        // Two names that `display()` renders identically ("r\u{FFFD}").
        let root = base.join(std::ffi::OsStr::from_bytes(b"r\xff"));
        let outside = base.join(std::ffi::OsStr::from_bytes(b"r\xfe"));
        let _cleanup = FixtureCleanup(base.clone());
        std::fs::create_dir_all(root.join("pkg/src"))?;
        std::fs::create_dir_all(root.join("pkg/tests"))?;
        std::fs::create_dir_all(outside.join("pkg/src"))?;
        std::fs::write(root.join("pkg/Cargo.toml"), "[package]\nname = \"pkg\"\n")?;
        let test_source = "#[test]\nfn source_test() { assert_eq!(1, 1); }\n";
        let sources = [
            (
                PathBuf::from("pkg/src/lib.rs"),
                "pub fn source() -> i32 { 1 }\n",
            ),
            (PathBuf::from("pkg/tests/lib.rs"), test_source),
        ];
        std::fs::write(root.join(&sources[0].0), sources[0].1)?;
        let real = root.join("pkg/src/real.rs");
        std::fs::write(&real, test_source)?;
        let intermediate = root.join("pkg/src/intermediate.rs");
        std::os::unix::fs::symlink(&real, &intermediate)?;
        std::os::unix::fs::symlink(&intermediate, root.join(&sources[1].0))?;
        let files = sources
            .iter()
            .map(|(path, source)| {
                (
                    path.clone(),
                    FileFacts {
                        path: path.clone(),
                        source: (*source).into(),
                        ..FileFacts::default()
                    },
                )
            })
            .collect();
        let authority = WorkspaceRootAuthority::from_index(&root, &files);
        let test = Path::new("pkg/tests/lib.rs");
        let source = Path::new("pkg/src/lib.rs");
        assert!(authority.validates_target(test, source, test_source));

        // Same inode, size and mtime; the resolved path differs only in a
        // byte that `display()` hides.
        let moved = outside.join("pkg/src/real.rs");
        std::fs::rename(&real, &moved)?;
        std::fs::remove_file(&intermediate)?;
        std::os::unix::fs::symlink(&moved, &intermediate)?;
        let resolved = std::fs::canonicalize(root.join(test))?;
        assert!(
            resolved.starts_with(&outside),
            "fixture must resolve outside the root"
        );
        assert_eq!(
            resolved.display().to_string(),
            std::fs::canonicalize(&root)?
                .join("pkg/src/real.rs")
                .display()
                .to_string(),
            "fixture must render like the in-root path"
        );

        assert!(!authority.validates_target(test, source, test_source));
        Ok(())
    }
}
