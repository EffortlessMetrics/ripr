//! Name-admitted dependent-package scope for Draft/Fast diff indexing (#5320).
//!
//! With unchanged tests, Draft/Fast selection indexes the changed packages
//! plus every package that reaches them through path dependencies (#2970
//! slice C). On a large workspace that reverse closure is most of the
//! repository: a two-file change in `bevy_reflect` selected 1,738 of 1,925
//! files and tripped the `diff_scope_oversized` guard. By default the
//! narrowing below runs only when that closure would exceed the narrowing
//! threshold (`RIPR_DIFF_NARROW_INDEX_FILES`, see
//! [`DependentScopeMode::Auto`]). That threshold bounds time; the separate,
//! higher `RIPR_MAX_DIFF_INDEX_FILES` guard bounds memory, so a narrowed
//! selection still over the threshold runs instead of refusing.
//!
//! The extra packages cannot contribute related tests here. The selection
//! is never workspace-complete, so the related-test package guard drops
//! every cross-package test that is not reached through the owner-uniqueness
//! or dependency-edge admits, and both of those admits require a
//! workspace-complete index. What a dependent file can still change is a
//! whole-index scan:
//!
//! - name-keyed scans of the changed owner: the "may be reached unseen" and
//!   non-test-caller scans, same-named definitions (receiver identity,
//!   competing definitions), receiver type declarations, `impl <trait> for`
//!   receivers and the one-inherent-constructor count;
//! - a caller chain to the owner (the transitive and macro reach witnesses);
//! - a Cargo binary invocation (the subprocess reach limitation);
//! - module composition through `#[path]` and `include!` (file roles);
//! - a package nested under a changed one, which passes the package guard;
//! - a registered test-harness target;
//! - macro bindings that make `assert_eq!` and local empty macros ambiguous
//!   for the owner pin, unioned over every indexed file.
//!
//! [`AdmissionQuery`] evaluates the name-keyed scans over the changed
//! packages' own index first. Each is monotone in the files it sees, so a
//! scan those packages already decide needs no dependent file; only the
//! names left open become admission tests. Nested packages, harness targets
//! and composition or subprocess markers are admitted outright. The macro
//! bindings of withheld files are folded into the owner-pin unions without
//! indexing them. The witness walks run only for `no_static_path` findings;
//! [`NarrowedScope::reach_index`] widens to the owner's caller levels on
//! demand, bounded by the walks' own depth, and names an unsearched reach
//! when that widening would exceed the narrowing threshold.
//!
//! Every admission test is a lexical superset of the parser fact it stands
//! for: a call, definition or mention the index records is spelled in the
//! file's bytes. Two cases keep the full selection instead: a changed probe
//! file with no derivable package prefix (the related-test package guard does
//! not apply to it, so any indexed test may relate), and a selection that
//! already spans the workspace (narrowing would turn off the
//! workspace-complete admits).

use crate::analysis::cancellation;
use crate::analysis::classify;
use crate::analysis::consumed_source::ConsumedRustSources;
use crate::analysis::facts::{FunctionSummary, RustIndex};
use rayon::prelude::*;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

/// Dependent files read and scanned per parallel admission batch, bounding
/// how many withheld files' bytes are held at once.
const ADMISSION_BATCH_FILES: usize = 64;

/// Source bytes after which an admission batch stops reading, so a run of
/// large generated files holds at most this plus one file at once.
const ADMISSION_BATCH_BYTES: usize = 8 * 1024 * 1024;

/// Env override for the dependent scope. `auto` (also empty or unset)
/// admits by name only when the whole reverse closure would exceed the
/// narrowing threshold (`RIPR_DIFF_NARROW_INDEX_FILES`); `named` always
/// admits by name; `full` never does (the pre-#5320 selection). Anything else fails. `full` and `named` are the
/// operator escape hatches and the parity-check switches.
pub(crate) const DEPENDENT_SCOPE_ENV: &str = "RIPR_DIFF_DEPENDENT_SCOPE";

/// Which dependent-package selection a diff run uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DependentScopeMode {
    /// Admit dependent files by name when the full selection would exceed
    /// the narrowing threshold (the default). Under it the full selection
    /// already runs, and narrowing costs more than it saves whenever the
    /// witness closure ends up admitting most dependent files (measured on
    /// rust-analyzer: 17.0s narrowed against 14.0s full, identical output).
    Auto,
    /// Admit dependent files by name.
    NameAdmitted,
    /// Index every file of every dependent package.
    Full,
    /// Withhold every dependent file and never widen: the falsifying
    /// control that shows each admission rule is load-bearing.
    #[cfg(test)]
    CoreOnly,
}

impl DependentScopeMode {
    /// Whether a run whose full selection holds `selected` files, against a
    /// narrowing threshold of `limit`, narrows.
    pub(crate) fn narrows(self, selected: usize, limit: usize) -> bool {
        match self {
            Self::Auto => selected > limit,
            Self::NameAdmitted => true,
            Self::Full => false,
            #[cfg(test)]
            Self::CoreOnly => true,
        }
    }

    pub(crate) fn from_env() -> Result<Self, String> {
        #[cfg(test)]
        if let Some(mode) = FORCED_MODE.with(std::cell::Cell::get) {
            return Ok(mode);
        }
        Self::from_env_value(std::env::var(DEPENDENT_SCOPE_ENV))
    }

    pub(super) fn from_env_value(value: Result<String, std::env::VarError>) -> Result<Self, String> {
        match value {
            Err(std::env::VarError::NotPresent) => Ok(Self::Auto),
            Err(std::env::VarError::NotUnicode(_)) => {
                Err(format!("{DEPENDENT_SCOPE_ENV} must be valid UTF-8"))
            }
            Ok(raw) => match raw.trim() {
                "" | "auto" => Ok(Self::Auto),
                "named" => Ok(Self::NameAdmitted),
                "full" => Ok(Self::Full),
                other => Err(format!(
                    "{DEPENDENT_SCOPE_ENV} must be `auto`, `named` or `full`, got `{other}`"
                )),
            },
        }
    }
}

#[cfg(test)]
std::thread_local! {
    // Thread-owned so parallel tests never see each other's mode or files.
    static FORCED_MODE: std::cell::Cell<Option<DependentScopeMode>> =
        const { std::cell::Cell::new(None) };
    static FORCED_REACH_LIMIT: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
    static OBSERVED_REACH_FILES: std::cell::RefCell<Vec<PathBuf>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static OBSERVED_REACH_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OBSERVED_MAIN_FILES: std::cell::RefCell<Option<Vec<PathBuf>>> =
        const { std::cell::RefCell::new(None) };
}

/// Run `work` with the dependent scope forced to `mode` on this thread.
#[cfg(test)]
pub(super) fn with_forced_mode<T>(mode: DependentScopeMode, work: impl FnOnce() -> T) -> T {
    FORCED_MODE.with(|forced| forced.set(Some(mode)));
    OBSERVED_REACH_FILES.with(|files| files.borrow_mut().clear());
    OBSERVED_REACH_PARSES.with(|parses| parses.set(0));
    OBSERVED_MAIN_FILES.with(|files| *files.borrow_mut() = None);
    let result = work();
    FORCED_MODE.with(|forced| forced.set(None));
    result
}

/// Run `work` with the reach-widening limit forced to `limit` on this
/// thread, so a small fixture can exercise the over-limit path.
#[cfg(test)]
pub(super) fn with_forced_reach_limit<T>(limit: usize, work: impl FnOnce() -> T) -> T {
    FORCED_REACH_LIMIT.with(|forced| forced.set(Some(limit)));
    let result = work();
    FORCED_REACH_LIMIT.with(|forced| forced.set(None));
    result
}

/// The main-index files of the last narrowed run on this thread; `None`
/// when no run narrowed.
#[cfg(test)]
pub(super) fn observed_main_files() -> Option<Vec<PathBuf>> {
    OBSERVED_MAIN_FILES.with(|files| files.borrow().clone())
}

/// How many withheld files the reach closures on this thread parsed since
/// the last forced-mode run began.
#[cfg(test)]
pub(super) fn observed_reach_parses() -> usize {
    OBSERVED_REACH_PARSES.with(std::cell::Cell::get)
}

/// The withheld files the last reach widening on this thread admitted.
#[cfg(test)]
pub(super) fn observed_reach_files() -> Vec<PathBuf> {
    OBSERVED_REACH_FILES.with(|files| files.borrow().clone())
}

/// What a dependent file must spell to enter the main index. Each set
/// stands for one name-keyed scan the changed packages have not decided;
/// every such scan is monotone in the files it sees, so once the changed
/// packages decide it, more files cannot change it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct AdmissionQuery {
    /// Owners the "may be reached unseen" scan or the non-test-caller scan
    /// left open: any file spelling the name can decide them.
    mentioned: BTreeSet<String>,
    /// Owners with fewer than two indexed definitions of their name, or no
    /// competing definition for the return pin: another `fn <name>` can
    /// change receiver identity or the pin.
    defined: BTreeSet<String>,
    /// Receiver types no indexed `struct`/`enum`/`union` declares yet.
    declared_types: BTreeSet<String>,
    /// Traits whose default-method owners take receivers from every
    /// `impl <trait> for` block.
    implemented_traits: BTreeSet<String>,
    /// Receiver types whose one inherent constructor pins a test binding:
    /// another `impl <type>` block changes the count.
    impl_types: BTreeSet<String>,
    /// Self types of the `impl <trait> for` blocks seen so far for
    /// `implemented_traits`. Each is a default-method receiver once
    /// declared, and its inherent constructors pin bindings, so a file that
    /// declares one or opens an `impl` block for it joins the index.
    trait_receivers: BTreeSet<String>,
    /// Local empty-macro names the changed packages declare: a file that
    /// spells one can make that macro ambiguous for the owner pin.
    empty_macro_names: BTreeSet<String>,
    /// Changed packages' path prefixes. A package nested under one passes
    /// the related-test package guard, so its files relate like the
    /// owner's own.
    package_prefixes: BTreeSet<String>,
    /// A changed file is a binary root, so a dependent test that runs a
    /// Cargo binary is the subprocess-reach witness.
    binary_owner: bool,
    /// The workspace package names macro-binding scans treat as own.
    package_names: BTreeSet<String>,
}

impl AdmissionQuery {
    /// Evaluate the scans over the changed packages' own index for every
    /// function whose span holds a changed line. `changed_lines` holds
    /// new-side line numbers per changed probe file.
    pub(super) fn from_core_index(
        index: &RustIndex,
        changed_lines: &[(PathBuf, BTreeSet<usize>)],
    ) -> Self {
        let mut query = Self {
            package_names: index.package_names.clone(),
            ..Self::default()
        };
        for facts in index.files().values() {
            if facts.source.contains("macro_rules") {
                query
                    .empty_macro_names
                    .extend(crate::analysis::syntax::local_empty_macro_names(
                        &facts.source,
                    ));
            }
        }
        for (file, lines) in changed_lines {
            query
                .package_prefixes
                .extend(classify::package_prefix(file));
            query.binary_owner |= super::is_binary_source_path(file);
            for owner in index.functions().iter().filter(|function| {
                function.file == *file
                    && lines
                        .iter()
                        .any(|line| function.start_line <= *line && *line <= function.end_line)
            }) {
                query.insert_owner(owner, index);
            }
        }
        for facts in index.files().values() {
            query.note_trait_receivers(&facts.source);
        }
        query
    }

    /// Record the self types `source` implements an owner's trait for.
    fn note_trait_receivers(&mut self, source: &str) {
        for trait_name in &self.implemented_traits {
            self.trait_receivers
                .extend(classify::trait_impl_self_type_names(source, trait_name));
        }
    }

    /// The admission test for files that declare or open an `impl` block
    /// for a trait receiver.
    fn trait_receiver_query(&self) -> Option<Self> {
        (!self.trait_receivers.is_empty()).then(|| Self {
            declared_types: self.trait_receivers.clone(),
            impl_types: self.trait_receivers.clone(),
            ..Self::default()
        })
    }

    fn insert_owner(&mut self, owner: &FunctionSummary, index: &RustIndex) {
        // Probes name their owner by the id's last segment.
        let names = std::iter::once(owner.name.clone())
            .chain(owner.id.0.rsplit("::").next().map(str::to_string))
            .filter(|name| !name.is_empty())
            .collect::<BTreeSet<_>>();
        if !classify::owner_may_be_reached_unseen(owner, index)
            || classify::is_assertion_shaped_owner(owner, index)
        {
            self.mentioned.extend(names.iter().cloned());
        }
        let same_named = index
            .functions()
            .iter()
            .filter(|function| function.name == owner.name)
            .count();
        let pin = classify::pin_scope_needs(owner, index);
        if same_named < 2 || pin.competing_definitions {
            self.defined.extend(names);
        }
        self.declared_types.extend(pin.receiver_type);
        self.implemented_traits.extend(pin.trait_impls);
        self.impl_types.extend(pin.constructor_type);
    }

    /// Whether `file` sits in a package nested under a changed package.
    fn nests_under_changed_package(&self, file: &Path) -> bool {
        let path = file.to_string_lossy().replace('\\', "/");
        let path = path.trim_start_matches("./");
        self.package_prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix.as_str()))
    }

    /// Whether a dependent file with these bytes can change a scan the
    /// changed packages left open. Each test is a lexical superset of the
    /// parser fact the scan reads.
    fn admits(&self, bytes: &[u8]) -> bool {
        let tokens = identifier_runs(bytes).collect::<Vec<_>>();
        let spells = |names: &BTreeSet<String>| {
            !names.is_empty()
                && (tokens
                    .iter()
                    .any(|token| names.iter().any(|name| name.as_bytes() == *token))
                    || names
                        .iter()
                        .filter(|name| !name.is_ascii())
                        .any(|name| spells_at_ascii_boundary(bytes, name.as_bytes())))
        };
        let declares = |keywords: &[&[u8]], names: &BTreeSet<String>| {
            !names.is_empty()
                && tokens.windows(3).any(|window| {
                    keywords.contains(&window[0])
                        && names.iter().any(|name| {
                            let name = name.as_bytes();
                            // `fn r#match`: the raw prefix is its own token.
                            window[1] == name || (window[1] == b"r" && window[2] == name)
                        })
                })
                || tokens.last_chunk::<2>().is_some_and(|[keyword, name]| {
                    keywords.contains(keyword)
                        && names.iter().any(|known| known.as_bytes() == *name)
                })
        };
        spells(&self.mentioned)
            || (!self.mentioned.is_empty() && contains(bytes, b"include_str!"))
            || declares(&[b"fn"], &self.defined)
            || declares(&[b"struct", b"enum", b"union"], &self.declared_types)
            || spells(&self.implemented_traits)
            || (self.binary_owner
                && (contains(bytes, b"CARGO_BIN_EXE_") || contains(bytes, b"cargo_bin")))
            || composes_modules(bytes)
            || spells(&self.empty_macro_names)
            || (!self.impl_types.is_empty() && impl_header_names(bytes, &self.impl_types))
    }

    #[cfg(test)]
    pub(super) fn for_names(mentioned: &[&str], defined: &[&str]) -> Self {
        Self {
            mentioned: mentioned.iter().map(|name| (*name).to_string()).collect(),
            defined: defined.iter().map(|name| (*name).to_string()).collect(),
            ..Self::default()
        }
    }
}

/// The admission query for one diff run: the changed packages' index
/// (`core_files`, which holds every changed probe file) evaluated at the
/// changed lines. Reads are recorded in `consumed`.
pub(super) fn admission_query(
    root: &Path,
    core_files: &[PathBuf],
    changed_files: &[&crate::analysis::diff::ChangedFile],
    test_harnesses: &[crate::config::TestHarnessRegistration],
    consumed: &mut ConsumedRustSources,
) -> Result<AdmissionQuery, String> {
    let changed_lines = changed_files
        .iter()
        .map(|changed| {
            // A removed line contributes its new-side anchor and the line
            // above it: the deletion sits between the two.
            let lines =
                changed
                    .added_lines
                    .iter()
                    .map(|line| line.new_side_line)
                    .chain(changed.removed_lines.iter().flat_map(|line| {
                        [line.new_side_line.saturating_sub(1), line.new_side_line]
                    }))
                    .filter(|line| *line > 0)
                    .collect::<BTreeSet<_>>();
            (changed.path.clone(), lines)
        })
        .collect::<Vec<_>>();
    let index = build_index(root, core_files, test_harnesses, consumed)?;
    Ok(AdmissionQuery::from_core_index(&index, &changed_lines))
}

/// The witness search for this finding's owner would need more files than
/// the narrowing threshold allows. Today's full selection refused the whole run in
/// this case; this keeps the run and names the unsearched reach instead of
/// letting `no_static_path` read as a searched-and-empty result.
pub(super) fn apply_reach_search_over_limit(
    finding: &mut crate::domain::Finding,
    probe: &crate::domain::Probe,
    files: usize,
    limit: usize,
) {
    let owner = super::owner_name_from_id(&probe.owner, &probe.location.file)
        .unwrap_or_else(|| "the changed owner".to_string());
    finding.static_limit_kind = Some(crate::domain::StaticLimitKind::RustTransitiveReachUnresolved);
    finding
        .stop_reasons
        .push(crate::domain::StopReason::TransitiveReachUnresolved);
    finding.evidence.push(format!(
        "ripr did not search dependent packages for a test that reaches `{owner}`: its callers \
         span at least {files} Rust files, over the {limit}-file narrowing threshold. A test there \
         may still observe this change. Raise RIPR_DIFF_NARROW_INDEX_FILES to search them \
         (it is capped at RIPR_MAX_DIFF_INDEX_FILES)."
    ));
    finding.evidence.extend([
        format!(
            "{}changed owner `{owner}` -> callers in dependent packages",
            crate::domain::LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX
        ),
        format!(
            "{}callers of `{owner}` -> tests beyond the {limit}-file narrowing threshold",
            crate::domain::LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX
        ),
        format!(
            "{}analysis/rust-public-api-transitive-reach",
            crate::domain::LIMITATION_ANALYZER_ROUTE_PREFIX
        ),
        format!(
            "{}named limitation only; ripr did not search these files and cannot confirm or deny that a test observes the change",
            crate::domain::LIMITATION_NON_CLAIM_PREFIX
        ),
    ]);
}

/// The dependent files withheld from the main index, and the reach index
/// built from them on demand for the no-static-path witnesses.
pub(super) struct NarrowedScope {
    root: PathBuf,
    all_files: Vec<PathBuf>,
    main_files: Vec<PathBuf>,
    /// Dependent files not in the main index.
    withheld: Vec<PathBuf>,
    /// The identifiers each withheld file spells, read once at admission.
    tokens: WithheldTokens,
    /// Each searched owner's caller closure: the withheld files it admits,
    /// or where it passed the limit. The limit is fixed for the scope's
    /// run, so a cached `Over` stays valid.
    closures: std::collections::BTreeMap<String, Closure>,
    /// The last widened index and the files it holds; owners whose closure
    /// selects the same files reuse it.
    reach_index: Option<(Vec<PathBuf>, RustIndex)>,
    test_harnesses: Vec<crate::config::TestHarnessRegistration>,
    /// Bytes the reach widening read, for the run's consumed sources.
    consumed: ConsumedRustSources,
    /// `false` only for the test-only core-only control.
    widen: bool,
}

/// Which withheld files spell each identifier, and the `macro_rules!`
/// names each defines, so the reach closure never rereads a file. Ids are
/// positions in [`NarrowedScope::withheld`].
#[derive(Default)]
struct WithheldTokens {
    postings: std::collections::HashMap<Box<[u8]>, Vec<u32>>,
    /// Files with any non-ASCII byte. A non-ASCII name can sit at an ASCII
    /// word boundary the identifier runs do not split at (or inside a run
    /// they drop for its leading digit), so every one of these files may
    /// spell it.
    unicode: Vec<u32>,
    macros: Vec<Vec<String>>,
}

/// One withheld file's lexical facts, scanned off the main thread.
struct FileTokens {
    unicode: bool,
    unique: HashSet<Box<[u8]>>,
    macros: Vec<String>,
}

impl FileTokens {
    fn scan(bytes: &[u8]) -> Self {
        Self {
            unicode: !bytes.is_ascii(),
            unique: identifier_runs(bytes).map(Box::from).collect(),
            macros: macro_rules_names(bytes),
        }
    }
}

impl WithheldTokens {
    #[cfg(test)]
    fn insert(&mut self, id: u32, bytes: &[u8]) {
        self.insert_scanned(id, FileTokens::scan(bytes));
    }

    fn insert_scanned(&mut self, id: u32, file: FileTokens) {
        if file.unicode {
            self.unicode.push(id);
        }
        for token in file.unique {
            self.postings.entry(token).or_default().push(id);
        }
        self.macros.push(file.macros);
    }

    /// Ids of the files that may spell one of `names`, ascending.
    fn spelling(&self, names: &BTreeSet<String>) -> BTreeSet<u32> {
        let unicode = names.iter().any(|name| !name.is_ascii());
        names
            .iter()
            .filter_map(|name| self.postings.get(name.as_bytes()))
            .flatten()
            .chain(self.unicode.iter().filter(|_| unicode))
            .copied()
            .collect()
    }

    /// Keep only the ids `keep` maps, renumbered.
    fn retain(&mut self, keep: &[Option<u32>]) {
        for ids in self.postings.values_mut() {
            ids.retain_mut(|id| match keep.get(*id as usize).copied().flatten() {
                Some(new) => {
                    *id = new;
                    true
                }
                None => false,
            });
        }
        self.postings.retain(|_, ids| !ids.is_empty());
        self.unicode = self
            .unicode
            .iter()
            .filter_map(|id| keep.get(*id as usize).copied().flatten())
            .collect();
        let macros = std::mem::take(&mut self.macros);
        self.macros = macros
            .into_iter()
            .zip(keep)
            .filter(|(_, kept)| kept.is_some())
            .map(|(names, _)| names)
            .collect();
    }
}

/// The outcome of dependent admission for one diff run.
pub(super) struct DependentAdmission {
    /// The files the main index loads.
    pub(super) main_files: Vec<PathBuf>,
    /// `Some` when dependent files were withheld and the witness search must
    /// widen on demand.
    pub(super) narrowed: Option<NarrowedScope>,
    /// The withheld files' macro bindings, for the owner-pin unions.
    pub(super) withheld_macro_bindings: classify::WithheldMacroBindings,
}

/// Admit the dependent files of `full_files` that `query` can reach.
/// `core_files` is the selection without the dependent packages; it always
/// stays whole. Every withheld file is read once.
pub(super) fn admit_dependents(
    root: &Path,
    all_files: &[PathBuf],
    core_files: Vec<PathBuf>,
    full_files: &[PathBuf],
    query: &AdmissionQuery,
    test_harnesses: &[crate::config::TestHarnessRegistration],
    consumed: &mut ConsumedRustSources,
) -> Result<DependentAdmission, String> {
    let core = core_files.iter().cloned().collect::<BTreeSet<_>>();
    #[cfg(test)]
    let core_only = DependentScopeMode::from_env()? == DependentScopeMode::CoreOnly;
    #[cfg(not(test))]
    let core_only = false;
    let mut admitted = core_files;
    let mut withheld = Vec::new();
    let mut tokens = WithheldTokens::default();
    // The owner-pin unions span every indexed file; the withheld share is
    // folded in as each file is withheld, until one file may shadow any
    // name. A withheld file that module context later pulls back into the
    // main index then counts twice, which a union absorbs.
    let mut withheld_macro_bindings = classify::WithheldMacroBindings::default();
    let mut bindings_saturated = false;
    // A package nested under a changed one relates like the owner's own, so
    // it is read first: its local empty macros join the query.
    let mut query = query.clone();
    let (nested, dependent): (Vec<_>, Vec<_>) = full_files
        .iter()
        .filter(|file| !core.contains(*file))
        .partition(|file| !core_only && query.nests_under_changed_package(file));
    for file in nested {
        cancellation::checkpoint()?;
        let bytes = read_source(root, file)?;
        consumed.record(file, bytes.as_deref());
        if let Some(bytes) = bytes {
            let source = String::from_utf8_lossy(&bytes);
            if source.contains("macro_rules") {
                query
                    .empty_macro_names
                    .extend(crate::analysis::syntax::local_empty_macro_names(&source));
            }
            query.note_trait_receivers(&source);
            admitted.push(file.clone());
        }
    }
    // The admission test reads only fields the loop below never changes,
    // so each chunk's lexical scans run on the rayon workers; reads,
    // recording and every order-dependent fold stay sequential, in input
    // order.
    let token = cancellation::current_token();
    let mut next = 0;
    while next < dependent.len() {
        let mut loaded = Vec::with_capacity(ADMISSION_BATCH_FILES);
        let batch_start = next;
        let mut batch_bytes = 0usize;
        while next < dependent.len()
            && next - batch_start < ADMISSION_BATCH_FILES
            && batch_bytes < ADMISSION_BATCH_BYTES
        {
            let file = dependent[next];
            next += 1;
            cancellation::checkpoint()?;
            // The bytes decide admission, so they bind the result exactly as
            // an indexed file's bytes do.
            let bytes = read_source(root, file)?;
            consumed.record(file, bytes.as_deref());
            // No content to index: the full selection skips it too.
            if let Some(bytes) = bytes {
                batch_bytes = batch_bytes.saturating_add(bytes.len());
                loaded.push((file, bytes));
            }
        }
        let scanned: Vec<Result<Option<FileTokens>, String>> = loaded
            .par_iter()
            .map(|(file, bytes)| {
                cancellation::with_optional_token(token.as_ref(), || {
                    cancellation::checkpoint()?;
                    let harness_target = test_harnesses
                        .iter()
                        .any(|registration| registration.target == **file);
                    let admit = !core_only && (harness_target || query.admits(bytes));
                    Ok((!admit).then(|| FileTokens::scan(bytes)))
                })
            })
            .collect();
        for ((file, bytes), scan) in loaded.iter().zip(scanned) {
            cancellation::checkpoint()?;
            match scan? {
                None => {
                    if !query.implemented_traits.is_empty() {
                        query.note_trait_receivers(&String::from_utf8_lossy(bytes));
                    }
                    admitted.push((*file).clone());
                }
                Some(file_tokens) => {
                    let id = u32::try_from(withheld.len()).map_err(|err| {
                        format!("dependent scope: too many withheld files: {err}")
                    })?;
                    tokens.insert_scanned(id, file_tokens);
                    withheld.push((*file).clone());
                    if !bindings_saturated {
                        bindings_saturated = withheld_macro_bindings
                            .absorb(&String::from_utf8_lossy(bytes), &query.package_names);
                    }
                }
            }
        }
    }
    // A trait receiver's declaration or inherent constructor can sit in a
    // file that never spells the trait. Every file that implements the
    // trait spells it and is admitted above, so the receivers are complete
    // here and one pass over the withheld files that spell them suffices.
    if let Some(receivers) = query.trait_receiver_query().filter(|_| !core_only) {
        for id in tokens.spelling(&receivers.declared_types) {
            cancellation::checkpoint()?;
            let Some(file) = withheld.get(id as usize) else {
                continue;
            };
            if read_source(root, file)?.is_some_and(|bytes| receivers.admits(&bytes)) {
                admitted.push(file.clone());
            }
        }
    }
    admitted.sort();
    admitted.dedup();
    let main_files = crate::analysis::workspace::with_module_context_files(all_files, admitted);
    let main_set = main_files.iter().collect::<BTreeSet<_>>();
    let mut kept = 0u32;
    let keep = withheld
        .iter()
        .map(|file| {
            (!main_set.contains(file)).then(|| {
                kept += 1;
                kept - 1
            })
        })
        .collect::<Vec<_>>();
    withheld.retain(|file| !main_set.contains(file));
    tokens.retain(&keep);
    #[cfg(test)]
    OBSERVED_MAIN_FILES.with(|files| *files.borrow_mut() = Some(main_files.clone()));
    if withheld.is_empty() {
        return Ok(DependentAdmission {
            main_files,
            narrowed: None,
            withheld_macro_bindings,
        });
    }
    Ok(DependentAdmission {
        main_files: main_files.clone(),
        narrowed: Some(NarrowedScope {
            root: root.to_path_buf(),
            all_files: all_files.to_vec(),
            main_files,
            withheld,
            tokens,
            closures: std::collections::BTreeMap::new(),
            reach_index: None,
            test_harnesses: test_harnesses.to_vec(),
            consumed: ConsumedRustSources::default(),
            widen: !core_only,
        }),
        withheld_macro_bindings,
    })
}

/// One caller level's admitted files, parsed (`None` when no withheld file
/// spells a name), and the `macro_rules!` names they define.
type LevelAdmission = (Option<RustIndex>, Vec<String>);

/// One owner's caller closure.
enum Closure {
    /// The withheld files the closure admits.
    Files(BTreeSet<PathBuf>),
    /// The closure stopped once the main files plus the files it had
    /// reached, with their module parents, exceeded the limit; at least
    /// this many files take part.
    Over(usize),
}

/// The witness index outcome for one owner.
pub(super) enum ReachIndex<'a> {
    /// The main index already holds every file the owner's reach can touch.
    Main,
    /// A wider index holding the owner's caller closure.
    Widened(&'a RustIndex),
    /// The caller closure would exceed the index file limit; the witness
    /// search cannot run over every file that can take part in it.
    OverLimit { files: usize, limit: usize },
}

impl NarrowedScope {
    /// The index the no-static-path witnesses must search for `owner`.
    ///
    /// Both witness walks run forward from a test's callee at most
    /// [`classify::MAX_TRANSITIVE_DEPTH`] hops, resolving every hop by bare
    /// name, and end at a call to the owner or at an invocation of a macro
    /// whose definition mentions it. So every function on a witness path
    /// sits within that many caller levels of those seeds, and every
    /// witnessing test spells a name on one of those levels. The closure
    /// admits, level by level, the withheld files that spell a seed or a
    /// caller name; a file on a path spells the name of its next hop, so it
    /// is admitted by the level below it.
    pub(super) fn reach_index(
        &mut self,
        owner: &str,
        main_index: &RustIndex,
        limit: usize,
    ) -> Result<ReachIndex<'_>, String> {
        if !self.widen {
            return Ok(ReachIndex::Main);
        }
        #[cfg(test)]
        let limit = FORCED_REACH_LIMIT
            .with(std::cell::Cell::get)
            .unwrap_or(limit);
        if !self.closures.contains_key(owner) {
            let closure = self.extend_reach(owner, main_index, limit)?;
            self.closures.insert(owner.to_string(), closure);
            #[cfg(test)]
            OBSERVED_REACH_FILES.with(|files| {
                *files.borrow_mut() = self
                    .closures
                    .values()
                    .filter_map(|closure| match closure {
                        Closure::Files(files) => Some(files),
                        Closure::Over(_) => None,
                    })
                    .flatten()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
            });
        }
        let closure = match self.closures.get(owner) {
            Some(Closure::Over(files)) => {
                return Ok(ReachIndex::OverLimit {
                    files: *files,
                    limit,
                });
            }
            Some(Closure::Files(files)) if !files.is_empty() => files,
            _ => return Ok(ReachIndex::Main),
        };
        // This owner's closure only: another owner's wider closure must not
        // push this one over the limit, and the count includes the module
        // parents the widened index loads.
        let selected = self
            .main_files
            .iter()
            .cloned()
            .chain(closure.iter().cloned())
            .collect::<Vec<_>>();
        let files =
            crate::analysis::workspace::with_module_context_files(&self.all_files, selected);
        if files.len() > limit {
            return Ok(ReachIndex::OverLimit {
                files: files.len(),
                limit,
            });
        }
        if self
            .reach_index
            .as_ref()
            .is_none_or(|(indexed, _)| *indexed != files)
        {
            let index = build_index(&self.root, &files, &self.test_harnesses, &mut self.consumed)?;
            self.reach_index = Some((files, index));
        }
        Ok(self
            .reach_index
            .as_ref()
            .map_or(ReachIndex::Main, |(_, index)| ReachIndex::Widened(index)))
    }

    /// The bytes the reach widening read.
    pub(super) fn into_consumed(self) -> ConsumedRustSources {
        self.consumed
    }

    /// The withheld files `owner`'s caller closure admits, or the count at
    /// which it passed `limit`. The count only grows level by level, and
    /// the module parents the widened index adds only add to it, so a
    /// closure stopped here would be over the limit at its end too.
    fn extend_reach(
        &mut self,
        owner: &str,
        main_index: &RustIndex,
        limit: usize,
    ) -> Result<Closure, String> {
        let mut admitted = Vec::new();
        let mut admitted_now = BTreeSet::new();
        let mut names = HashSet::from([owner.to_string()]);
        // Seeds: the owner, then every macro defined in a file that spells
        // it (a superset of the macros whose definition mentions it).
        let owner_bytes = byte_names(&BTreeSet::from([owner.to_string()]));
        let mut seed_macros = BTreeSet::new();
        for facts in main_index.files().values() {
            if spells_any(facts.source.as_bytes(), &owner_bytes) {
                seed_macros.extend(macro_rules_names(facts.source.as_bytes()));
            }
        }
        let Some((index, macros)) = self.admit_spelling(
            &BTreeSet::from([owner.to_string()]),
            &mut admitted_now,
            limit,
        )?
        else {
            return Ok(self.over(&admitted_now));
        };
        seed_macros.extend(macros);
        admitted.extend(index);
        seed_macros.retain(|name| !names.contains(name));
        let mut frontier = BTreeSet::new();
        if !seed_macros.is_empty() {
            names.extend(seed_macros.iter().cloned());
            let Some((index, _)) = self.admit_spelling(&seed_macros, &mut admitted_now, limit)?
            else {
                return Ok(self.over(&admitted_now));
            };
            admitted.extend(index);
            // Every invoker spells a seed macro, so it sits in the main
            // index or a file admitted just now: the first caller level.
            let seed_macro_bytes = byte_names(&seed_macros);
            frontier = std::iter::once(main_index)
                .chain(admitted.iter())
                .flat_map(|index| macro_body_callers(index, &seed_macro_bytes))
                .filter(|name| !names.contains(name))
                .collect::<BTreeSet<_>>();
        }
        for level in 0..classify::MAX_TRANSITIVE_DEPTH {
            cancellation::checkpoint()?;
            let callers = std::iter::once(main_index)
                .chain(admitted.iter())
                .flat_map(|index| caller_names(index, &names))
                .filter(|name| !names.contains(name))
                .chain(if level == 0 {
                    std::mem::take(&mut frontier)
                } else {
                    BTreeSet::new()
                })
                .collect::<BTreeSet<_>>();
            if callers.is_empty() {
                break;
            }
            names.extend(callers.iter().cloned());
            let Some((index, _)) = self.admit_spelling(&callers, &mut admitted_now, limit)? else {
                return Ok(self.over(&admitted_now));
            };
            admitted.extend(index);
        }
        Ok(Closure::Files(admitted_now))
    }

    fn over(&self, reached: &BTreeSet<PathBuf>) -> Closure {
        Closure::Over(self.selected_count(reached.iter()))
    }

    /// Files the widened index would load for the main files plus
    /// `reached`, counting the module parents it adds (#5450).
    fn selected_count<'a>(&self, reached: impl Iterator<Item = &'a PathBuf>) -> usize {
        let selected = self
            .main_files
            .iter()
            .cloned()
            .chain(reached.cloned())
            .collect::<Vec<_>>();
        crate::analysis::workspace::with_module_context_files(&self.all_files, selected).len()
    }

    /// Index the withheld files not yet admitted that spell one of `names`,
    /// with the `macro_rules!` names they define. `None` when they would
    /// take the main files plus the closure, with the module parents the
    /// widened index loads, past `limit`: they are counted
    /// into `admitted_now` but never parsed (#5450).
    fn admit_spelling(
        &mut self,
        names: &BTreeSet<String>,
        admitted_now: &mut BTreeSet<PathBuf>,
        limit: usize,
    ) -> Result<Option<LevelAdmission>, String> {
        let mut new_files = Vec::new();
        let mut macros = Vec::new();
        for id in self.tokens.spelling(names) {
            let Some(file) = self.withheld.get(id as usize) else {
                continue;
            };
            // A file an earlier owner's closure admitted is parsed again
            // here (a cache hit): its callers belong to this closure too.
            if admitted_now.contains(file) {
                continue;
            }
            if let Some(defined) = self.tokens.macros.get(id as usize) {
                macros.extend(defined.iter().cloned());
            }
            new_files.push(file.clone());
        }
        if new_files.is_empty() {
            return Ok(Some((None, macros)));
        }
        if self.selected_count(admitted_now.iter().chain(&new_files)) > limit {
            admitted_now.extend(new_files);
            return Ok(None);
        }
        // The level scan reads only call and body facts, so the parse is
        // enough; the reach index runs the full role pipeline later.
        let index = parse_index(&self.root, &new_files, &mut self.consumed)?;
        #[cfg(test)]
        OBSERVED_REACH_PARSES.with(|parses| parses.set(parses.get() + new_files.len()));
        admitted_now.extend(new_files);
        Ok(Some((Some(index), macros)))
    }
}

/// Names of functions and tests in `index` that call one of `names`.
fn caller_names(index: &RustIndex, names: &HashSet<String>) -> Vec<String> {
    let calls_any = |calls: &[crate::analysis::facts::CallFact]| {
        calls
            .iter()
            .any(|call| names.contains(call.name.trim_end_matches('!')))
    };
    index
        .functions()
        .iter()
        .filter(|function| calls_any(&function.calls))
        .map(|function| function.name.clone())
        .chain(
            index
                .tests()
                .iter()
                .filter(|test| calls_any(&test.calls))
                .map(|test| test.name.clone()),
        )
        .collect()
}

/// Names of functions and tests in `index` whose body spells one of
/// `macros`: an invocation the call facts may not record.
fn macro_body_callers(index: &RustIndex, macros: &HashSet<Vec<u8>>) -> Vec<String> {
    if macros.is_empty() {
        return Vec::new();
    }
    index
        .functions()
        .iter()
        .filter(|function| spells_any(function.body.as_bytes(), macros))
        .map(|function| function.name.clone())
        .chain(
            index
                .tests()
                .iter()
                .filter(|test| spells_any(test.body.as_bytes(), macros))
                .map(|test| test.name.clone()),
        )
        .collect()
}

fn load(
    root: &Path,
    files: &[PathBuf],
    consumed: &mut ConsumedRustSources,
) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let mut loaded = Vec::with_capacity(files.len());
    for file in files {
        cancellation::checkpoint()?;
        let bytes = read_source(root, file)?;
        consumed.record(file, bytes.as_deref());
        if let Some(bytes) = bytes {
            loaded.push((file.clone(), bytes));
        }
    }
    Ok(loaded)
}

fn parse_index(
    root: &Path,
    files: &[PathBuf],
    consumed: &mut ConsumedRustSources,
) -> Result<RustIndex, String> {
    crate::analysis::facts::parse_loaded_files_with_cache(root, &load(root, files, consumed)?)
}

fn build_index(
    root: &Path,
    files: &[PathBuf],
    test_harnesses: &[crate::config::TestHarnessRegistration],
    consumed: &mut ConsumedRustSources,
) -> Result<RustIndex, String> {
    let loaded = load(root, files, consumed)?;
    let cached = crate::analysis::rust_index::build_analysis_index_from_loaded_files(
        root,
        &loaded,
        test_harnesses,
    )?;
    Ok(cached.index)
}

/// The same bytes the index loads: committed content for a committed-history
/// diff, the worktree otherwise.
fn read_source(root: &Path, file: &Path) -> Result<Option<Vec<u8>>, String> {
    crate::analysis::committed_source::read_source_bytes(root, file)
        .map_err(|err| format!("failed to read {}: {err}", root.join(file).display()))
}

fn byte_names(names: &BTreeSet<String>) -> HashSet<Vec<u8>> {
    names
        .iter()
        .filter(|name| !name.is_empty())
        .map(|name| name.as_bytes().to_vec())
        .collect()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Identifier-shaped byte runs. Non-ASCII bytes count as identifier bytes,
/// so a Unicode identifier stays one token and a match is never split. A
/// run with non-ASCII bytes also yields its ASCII parts: the whole-index
/// name scans take ASCII word boundaries, so `“owner”` in a doc comment
/// names `owner` for them and must spell it here too.
fn identifier_runs(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .split(|byte| !is_identifier_byte(*byte))
        .flat_map(|run| {
            let unicode = !run.is_ascii();
            std::iter::once(run).chain(run.split(|byte| !byte.is_ascii()).filter(move |_| unicode))
        })
        .filter(|run| run.first().is_some_and(|first| !first.is_ascii_digit()))
}

/// Whether `bytes` spell any of `names` as a whole identifier, comments and
/// strings included (a superset of every parser fact). A non-ASCII name is
/// also matched at ASCII word boundaries, which the runs do not split at.
fn spells_any(bytes: &[u8], names: &HashSet<Vec<u8>>) -> bool {
    !names.is_empty()
        && (identifier_runs(bytes).any(|run| names.contains(run))
            || names
                .iter()
                .filter(|name| !name.is_ascii())
                .any(|name| spells_at_ascii_boundary(bytes, name)))
}

fn spells_at_ascii_boundary(bytes: &[u8], name: &[u8]) -> bool {
    let ascii_word = |byte: &u8| byte.is_ascii_alphanumeric() || *byte == b'_';
    let mut rest = bytes;
    while let Some(at) = find(rest, name) {
        let end = at + name.len();
        if !rest[..at].last().is_some_and(ascii_word)
            && !rest[end..].first().is_some_and(ascii_word)
        {
            return true;
        }
        rest = &rest[at + 1..];
    }
    false
}

/// Whether some `impl ... {` header in `bytes` spells one of `types` as a
/// whole identifier: a superset of every inherent `impl <type>` block.
fn impl_header_names(bytes: &[u8], types: &BTreeSet<String>) -> bool {
    let names = types
        .iter()
        .map(|name| name.as_bytes().to_vec())
        .collect::<HashSet<_>>();
    let mut rest = bytes;
    while let Some(at) = find(rest, b"impl") {
        let before_ok = at == 0 || !is_identifier_byte(rest[at - 1]);
        let after = &rest[at + b"impl".len()..];
        let after_ok = after.first().is_none_or(|byte| !is_identifier_byte(*byte));
        if before_ok && after_ok {
            let end = after
                .iter()
                .position(|byte| matches!(byte, b'{' | b';'))
                .unwrap_or(after.len());
            if spells_any(&after[..end], &names) {
                return true;
            }
        }
        rest = after;
    }
    false
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn is_identifier_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric() || byte >= 0x80
}

/// `include!` and `#[path]` compose a file out of other files, so the
/// composing file decides those files' module roles.
fn composes_modules(bytes: &[u8]) -> bool {
    contains(bytes, b"include!") || contains(bytes, b"#[path")
}

fn identifier_tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    identifier_runs(text.as_bytes()).map(|run| String::from_utf8_lossy(run).into_owned())
}

/// Names declared by `macro_rules! NAME` in `bytes`.
fn macro_rules_names(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut names = Vec::new();
    let mut rest = text.as_ref();
    while let Some(at) = rest.find("macro_rules!") {
        rest = &rest[at + "macro_rules!".len()..];
        if let Some(name) = identifier_tokens(rest.trim_start()).next() {
            names.push(name);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> HashSet<Vec<u8>> {
        list.iter().map(|name| name.as_bytes().to_vec()).collect()
    }

    #[test]
    fn spelling_matches_whole_identifiers_only() {
        let set = names(&["parse"]);
        assert!(spells_any(b"let x = s.parse::<u8>();", &set));
        assert!(spells_any(b"// see parse\n", &set));
        assert!(spells_any(b"use crate::parse as p;", &set));
        assert!(!spells_any(b"fn parser() {}", &set));
        assert!(!spells_any(b"fn reparse() {}", &set));
        assert!(!spells_any(b"let parse_x = 1;", &set));
    }

    #[test]
    fn spelling_keeps_unicode_identifiers_whole() {
        let set = names(&["größe"]);
        assert!(spells_any("fn größe() {}".as_bytes(), &set));
        assert!(!spells_any("fn größer() {}".as_bytes(), &set));
    }

    #[test]
    fn spelling_takes_ascii_boundaries_next_to_unicode() {
        // The reach scan treats a non-ASCII byte as a word boundary.
        let set = names(&["quarble_gauge"]);
        assert!(spells_any(
            "/// Wraps “quarble_gauge” here".as_bytes(),
            &set
        ));
        assert!(spells_any("let s = \"→quarble_gauge\";".as_bytes(), &set));
        assert!(!spells_any("/// “quarble_gauges”".as_bytes(), &set));
        let set = names(&["größe"]);
        assert!(spells_any("/// “größe”".as_bytes(), &set));
        assert!(spells_any("let s = \"→größe\";".as_bytes(), &set));
        assert!(!spells_any("/// “größe_x”".as_bytes(), &set));
        assert!(spells_any("/// 1→größe".as_bytes(), &set));
        assert!(spells_any("/// xgröße “größe”".as_bytes(), &set));
    }

    #[test]
    fn withheld_tokens_offer_unicode_files_for_a_unicode_name() {
        let mut tokens = WithheldTokens::default();
        tokens.insert(0, "/// “größe”".as_bytes());
        tokens.insert(1, b"fn plain() {}");
        tokens.insert(2, "/// 1→größe".as_bytes());
        let wanted = |list: &[&str]| list.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(tokens.spelling(&wanted(&["größe"])), BTreeSet::from([0, 2]));
        assert!(tokens.spelling(&wanted(&["plain_x"])).is_empty());
        tokens.retain(&[None, Some(0), Some(1)]);
        assert_eq!(tokens.spelling(&wanted(&["größe"])), BTreeSet::from([1]));
    }

    #[test]
    fn admission_takes_composition_and_subprocess_markers() {
        let none = AdmissionQuery::default();
        assert!(none.admits(b"include!(\"x.rs\");"));
        assert!(none.admits(b"#[path = \"a.rs\"] mod a;"));
        assert!(!none.admits(b"#![doc = include_str!(\"../README.md\")]"));
        let cargo_bin = b"let tool = env!(\"CARGO_BIN_EXE_tool\");";
        assert!(!none.admits(cargo_bin));
        let binary = AdmissionQuery {
            binary_owner: true,
            ..AdmissionQuery::default()
        };
        assert!(binary.admits(cargo_bin));
        assert!(!binary.admits(b"fn unrelated() {}"));
    }

    #[test]
    fn admission_reads_mentions_and_definitions_separately() {
        let mention = AdmissionQuery::for_names(&["parse"], &[]);
        assert!(mention.admits(b"let x = parse(1);"));
        assert!(mention.admits(b"#![doc = include_str!(\"../README.md\")]"));
        assert!(!mention.admits(b"fn parser() {}"));
        let definition = AdmissionQuery::for_names(&[], &["parse"]);
        assert!(definition.admits(b"pub fn parse() {}"));
        assert!(definition.admits(b"fn r#parse() {}"));
        assert!(definition.admits(b"fn parse"));
        assert!(!definition.admits(b"let x = parse(1);"));
        assert!(!definition.admits(b"fn parser() {}"));
    }

    #[test]
    fn admission_reads_a_unicode_mention_at_ascii_boundaries() {
        let mention = AdmissionQuery::for_names(&["größe"], &[]);
        assert!(mention.admits("/// see “größe”".as_bytes()));
        assert!(mention.admits("/// 1→größe".as_bytes()));
        assert!(!mention.admits("/// “größer”".as_bytes()));
    }

    #[test]
    fn admission_reads_declared_types_and_implemented_traits() {
        let query = AdmissionQuery {
            declared_types: BTreeSet::from(["Widget".to_string()]),
            implemented_traits: BTreeSet::from(["Render".to_string()]),
            ..AdmissionQuery::default()
        };
        assert!(query.admits(b"pub struct Widget;"));
        assert!(query.admits(b"enum Widget { A }"));
        assert!(!query.admits(b"let w: Widget = make();"));
        assert!(query.admits(b"impl Render for Thing {}"));
        assert!(!query.admits(b"fn unrelated() {}"));
    }

    #[test]
    fn impl_headers_name_the_constructor_type() {
        let types = BTreeSet::from(["Widget".to_string()]);
        assert!(impl_header_names(
            b"impl Widget { fn new() -> Self { Widget } }",
            &types
        ));
        assert!(impl_header_names(b"impl<T> Widget<T> {}", &types));
        assert!(impl_header_names(b"impl Default for Widget {}", &types));
        assert!(!impl_header_names(
            b"impl Gadget { fn widget() {} }\nlet w = Widget;",
            &types
        ));
        assert!(!impl_header_names(b"fn simple() {} // Widget", &types));
        assert!(!impl_header_names(b"implementation Widget {", &types));
    }

    #[test]
    fn nested_packages_follow_the_changed_package_prefix() {
        let query = AdmissionQuery {
            package_prefixes: BTreeSet::from(["crates/core/".to_string()]),
            ..AdmissionQuery::default()
        };
        assert!(query.nests_under_changed_package(Path::new("crates/core/fuzz/src/lib.rs")));
        assert!(!query.nests_under_changed_package(Path::new("crates/core_extra/src/lib.rs")));
        assert!(!query.nests_under_changed_package(Path::new("crates/app/src/lib.rs")));
    }

    #[test]
    fn admission_takes_empty_macro_names_and_constructor_impls() {
        let query = AdmissionQuery {
            empty_macro_names: BTreeSet::from(["noop".to_string()]),
            impl_types: BTreeSet::from(["Widget".to_string()]),
            ..AdmissionQuery::default()
        };
        assert!(query.admits(b"macro_rules! noop { ($($t:tt)*) => { assert!(true) } }"));
        assert!(query.admits(b"impl Widget { pub fn new() -> Self { todo!() } }"));
        assert!(!query.admits(b"fn build() -> Widget { Widget::new() }"));
    }

    #[test]
    fn withheld_macro_bindings_saturate_on_a_foreign_glob() {
        let packages = BTreeSet::from(["core".to_string()]);
        let mut bindings = classify::WithheldMacroBindings::default();
        assert!(!bindings.absorb("fn plain() {}", &packages));
        assert!(!bindings.absorb("use core::prelude::*;", &packages));
        assert!(bindings.absorb("use proptest::prelude::*;", &packages));
        assert!(bindings.absorb("fn plain() {}", &packages));
    }

    #[test]
    fn auto_narrows_only_over_the_limit() {
        assert!(!DependentScopeMode::Auto.narrows(1200, 1200));
        assert!(DependentScopeMode::Auto.narrows(1201, 1200));
        assert!(DependentScopeMode::NameAdmitted.narrows(10, 1200));
        assert!(!DependentScopeMode::Full.narrows(5000, 1200));
    }

    #[test]
    fn macro_rules_names_are_collected() {
        assert_eq!(
            macro_rules_names(b"macro_rules! a { () => {} }\nmacro_rules!   b_2 {}"),
            ["a", "b_2"]
        );
    }

    #[test]
    fn scope_mode_reads_the_override() {
        assert_eq!(
            DependentScopeMode::from_env_value(Err(std::env::VarError::NotPresent)),
            Ok(DependentScopeMode::Auto)
        );
        assert_eq!(
            DependentScopeMode::from_env_value(Ok("auto".to_string())),
            Ok(DependentScopeMode::Auto)
        );
        assert_eq!(
            DependentScopeMode::from_env_value(Ok("full".to_string())),
            Ok(DependentScopeMode::Full)
        );
        assert_eq!(
            DependentScopeMode::from_env_value(Ok(" named ".to_string())),
            Ok(DependentScopeMode::NameAdmitted)
        );
        assert_eq!(
            DependentScopeMode::from_env_value(Ok("all".to_string())),
            Err(format!(
                "{DEPENDENT_SCOPE_ENV} must be `auto`, `named` or `full`, got `all`"
            ))
        );
    }
}
