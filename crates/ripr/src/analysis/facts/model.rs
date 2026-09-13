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
    pub(crate) fn from_index(root: &Path, files: &BTreeMap<PathBuf, FileFacts>) -> Self {
        let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let mut authorities = BTreeMap::new();
        for (relative, facts) in files {
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
                    source_digest: source_digest(facts.source.as_bytes()),
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
        test_current && seam_current && source_digest(source.as_bytes()) == file.source_digest
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
                full.starts_with(&self.root)
                    && std::fs::read(&full)
                        .map(|bytes| source_digest(&bytes) == authority.source_digest)
                        .unwrap_or(false)
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
    let mut cursor = source.parent().map(Path::to_path_buf);
    while let Some(directory) = cursor {
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
                "{}:{}:{:?};",
                path.display(),
                metadata.len(),
                modified.map(|time| (time.as_secs(), time.subsec_nanos()))
            ));
        }
        Err(error) => output.push_str(&format!("{}:{:?};", path.display(), error.kind())),
    }
}

fn source_digest(bytes: &[u8]) -> String {
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
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RustIndex {
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
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

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    #[serde(default)]
    pub module_declarations: Vec<ModuleDeclarationFact>,
    /// Source-role provenance for this file occurrence (#3533): the ordered
    /// edge chain from the compilation unit whose declarations and include
    /// edges composed this file's roles, plus the earliest edge in the chain
    /// that could not be resolved. Composer-owned and recomputed on every
    /// index build — `serde(skip)` keeps composed state out of the on-disk
    /// file-fact cache, which stores pre-composition parse facts only.
    #[serde(skip)]
    pub role_provenance: SourceRoleProvenance,
    /// Original file source text. Held so `analysis/value-extraction-v2`
    /// can scan for top-level `const`/`static` declarations without
    /// re-reading the file at evidence-build time. Not part of any
    /// cached envelope (the cache stores `ClassifiedSeam` only).
    pub source: String,
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionFact {
    pub id: SymbolId,
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
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
    /// Names of `fn` items nested inside this function's body (#3727 Slice
    /// A), sorted and deduplicated. A nested `fn <callee>` item is hoisted
    /// and defeats whole-body shadow decisions (see
    /// `analysis::extract::shadow`). Parser-backed only — the lexical
    /// fallback leaves this empty. Empty on a parser-backed file is a real
    /// "no nested fn" result.
    #[serde(default)]
    pub nested_fn_names: Vec<String>,
    /// Shadow-shaped binding facts from the `let` statements in this
    /// function's body (#3727 Slice A), one entry per whole-word pattern
    /// name, sorted by (line, name). Initializer-less declarations
    /// (`let flag;`) produce no entries, mirroring the lexical scanner's
    /// `;` bound. Parser-backed only — the lexical fallback leaves this
    /// empty.
    #[serde(default)]
    pub let_bindings: Vec<LetBindingFact>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TestFact {
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
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
    #[serde(default)]
    pub nested_fn_names: Vec<String>,
    /// Body-relative `let` binding facts, mirroring
    /// `FunctionFact.let_bindings` (#3727 Slice A).
    #[serde(default)]
    pub let_bindings: Vec<LetBindingFact>,
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProbeShapeFact {
    pub start_line: usize,
    pub end_line: usize,
    /// Byte offset of the shape's start within the source file. Populated
    /// by the parser-backed summarizer; the lexical fallback emits no
    /// probe shapes at all, so this stays accurate.
    pub start_byte: usize,
    pub kind: String,
    pub text: String,
}

pub type FunctionSummary = FunctionFact;
pub type TestSummary = TestFact;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    #[test]
    fn rust_index_default_has_empty_fact_sets() {
        let index = RustIndex::default();
        assert!(index.files.is_empty());
        assert!(index.tests.is_empty());
        assert!(index.functions.is_empty());
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
            kind: "predicate".to_string(),
            text: "x > 0".to_string(),
        };
        assert_eq!(shape.start_line, 10);
        assert_eq!(shape.end_line, 12);
        assert_eq!(shape.start_byte, 256);
        assert_eq!(shape.kind, "predicate");
        assert_eq!(shape.text, "x > 0");
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
                        source: (*source).to_string(),
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
                        source: (*source).to_string(),
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
                        source: (*source).to_string(),
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
}
