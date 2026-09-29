//! Module-tree reachability evidence for changed Rust files (#4435).
//!
//! The layout rule alone decides too much: any file under `src/` (or below a
//! declared crate root outside `src/`) seeded diff probes even when no `mod`
//! names it, so rustc never compiles it; and the out-of-line modules of an
//! external crate root (`[lib] path = "../shared/lib.rs"`) never seeded,
//! because their nearest manifest is not the declaring package.
//!
//! This pass walks the module tree of the packages that could compile a
//! changed file, from every Cargo target root, following `mod`, `#[path]` and
//! literal `include!` edges, and records two facts on the source-role
//! context:
//!
//! - **orphans**: the owning package's walk is complete and does not reach
//!   the file, so no target compiles it. It seeds no probes.
//! - **external module sources**: a package whose `[lib]`/`[[bin]]` root sits
//!   outside its own directory reaches the file from that root. It seeds.
//!
//! Unknown never proves absence. A walk that meets an edge the scan cannot
//! resolve (dynamic `#[path]`, `cfg_if!`-wrapped declarations, a parse error,
//! the file bound) is incomplete, and the package keeps the layout rule. An
//! orphan verdict additionally needs every Rust file in the workspace to scan
//! completely, since another package can reach into this one through
//! `#[path]`, `include!` or a macro that expands to either.
//!
//! Not modeled, as in rust-analyzer's module discovery without expansion: an
//! attribute or derive macro, or a statement-position macro call, that emits
//! a `#[path]` module; a package outside the workspace root whose target
//! path points into it; and case-insensitive path matching. Each would need
//! a file-level edge no Rust source in the workspace spells.
//! Roots are over-collected on purpose (every autodiscovered and declared
//! target, whatever `autobins`/`autotests` say): an extra root can only
//! reach more files, which keeps a verdict of "unreached" conservative.
//!
//! The pass runs only over the files the caller names (the changed files
//! for the diff loop, the finding anchors for the LSP partition), so both
//! surfaces reach the same verdict for the same file and neither pays for
//! packages the diff never touched.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::cargo_targets::{
    collect_explicit_paths, declared_targets_from_manifest, lexical, nearest_manifest_dir,
    normalize, owning_package_dir,
};
use super::source_role::SourceRoleContext;
use crate::analysis::syntax::{RustModuleTreeEdge, RustModuleTreeScan, rust_module_tree_scan};

/// Files one package walk may visit before it stops and reports itself
/// incomplete.
const MAX_WALK_FILES: usize = 20_000;

/// Queue length up to which the walk picks the entry nearest its target.
const DIRECTED_QUEUE_LIMIT: usize = 512;

/// How a reached file anchors its own default `mod name;` children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ChildAnchor {
    /// Crate roots and `mod.rs` files: children live beside the file.
    Directory,
    /// Ordinary module files: children live under `<dir>/<stem>/`.
    Stem,
    /// Files loaded through `#[path]`: try both anchors, so the walk
    /// over-reaches rather than miss a child.
    Both,
    /// Files pasted in by `include!`: their `mod name;` declarations resolve
    /// against the including module, which this walk does not model, so any
    /// such declaration makes the walk incomplete. Their `#[path]` and
    /// `include!` edges are still followed.
    Included,
}

/// Where a search found a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    /// Reached from a `[lib]` or `[[bin]]` root.
    Production,
    /// Reached only from a test, bench, example or build-script root.
    Evidence,
}

/// One package's module-tree walk, explored lazily.
///
/// A declared file is usually a few `mod` hops below its crate root, so the
/// walk searches toward each asked-for file (the pending entry sharing the
/// longest path prefix with it goes first) and stops when it is found. Only
/// a file the walk cannot find costs a full traversal, and the explored
/// state carries over to the next file asked about. Production roots are
/// exhausted before evidence roots, so a file found in the evidence phase is
/// known to be unreachable from production.
#[derive(Debug)]
struct PackageWalk {
    /// Pending `(file, anchor)` entries for the current phase.
    queue: Vec<(PathBuf, ChildAnchor)>,
    /// The evidence roots, queued once the production phase is exhausted.
    evidence_roots: Option<Vec<PathBuf>>,
    phase: Origin,
    /// Entries already expanded in the current phase. The anchor is part of
    /// the key: a file reached both as `mod foo;` and through `#[path]`
    /// resolves its children under both directories.
    visited: BTreeSet<(PathBuf, ChildAnchor)>,
    /// Every reached file with the first phase that reached it.
    reached: BTreeMap<PathBuf, Origin>,
    production_roots: BTreeSet<PathBuf>,
    production_root_read: bool,
    /// Parsed module-tree scans, so the evidence phase re-parses nothing.
    scans: BTreeMap<PathBuf, Option<RustModuleTreeScan>>,
    /// False once any edge could not be resolved.
    complete: bool,
}

/// Records module-tree evidence for `candidates` (workspace-relative Rust
/// paths) on `context`. See the module docs for the two recorded facts.
///
/// Returns, for each candidate reached through an external crate root, the
/// root prefix of every declaring package (`/`-separated with a trailing
/// `/`, empty for the workspace root), so diff scoping keeps all of their
/// tests.
pub(crate) fn apply_module_graph_evidence<'a, I>(
    workspace_root: &Path,
    context: &mut SourceRoleContext,
    candidates: I,
) -> BTreeMap<PathBuf, BTreeSet<String>>
where
    I: IntoIterator<Item = &'a Path>,
{
    let mut external_packages = BTreeMap::new();
    let mut walks: BTreeMap<PathBuf, Option<PackageWalk>> = BTreeMap::new();
    let mut listing: Option<Option<WorkspaceListing>> = None;
    let mut member_packages: Option<BTreeSet<PathBuf>> = None;
    let mut external_declarers: Option<(BTreeSet<PathBuf>, bool)> = None;
    let mut escaping: Option<Option<EscapingReach>> = None;
    for candidate in candidates {
        if candidate.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let relative = normalize(candidate);
        let anchored = lexical(&normalize(&workspace_root.join(&relative)));
        // A symlinked module directory reaches the file under another
        // spelling, so the walk also matches the canonical path.
        let mut targets = vec![anchored.clone()];
        if let Ok(canonical) = std::fs::canonicalize(&anchored)
            && canonical != anchored
        {
            targets.push(canonical);
        }
        // The layout owner (nearest `src`/`tests`/`benches`/`examples`
        // parent) and the nearest manifest differ for a package nested in
        // such a directory (`tests/harness/Cargo.toml`); both are asked.
        // Every ancestor package is asked too: its root may reach the file
        // through plain `mod` edges across a nested manifest
        // (`mod inner;` beside `inner/Cargo.toml`).
        let absolute = workspace_root.join(&relative);
        let owners = [
            owning_package_dir(workspace_root, &absolute),
            nearest_manifest_dir(workspace_root, &absolute),
        ]
        .into_iter()
        .flatten()
        .chain(ancestor_manifest_dirs(workspace_root, &absolute))
        .map(|dir| lexical(&normalize(&dir)))
        .collect::<BTreeSet<_>>();
        let mut owner_proves_unreached = true;
        let mut owner_walked = false;
        let mut owner_reaches_production = false;
        for dir in &owners {
            let Some(walk) = walks
                .entry(dir.clone())
                .or_insert_with(|| PackageWalk::new(workspace_root, dir))
                .as_mut()
            else {
                // A workspace-only manifest compiles nothing; anything else
                // without a readable package is unknown.
                owner_proves_unreached &= is_workspace_only_manifest(workspace_root, dir);
                continue;
            };
            owner_walked = true;
            match walk.find(workspace_root, &targets) {
                Some(Origin::Production) => {
                    owner_reaches_production = true;
                    break;
                }
                // Compiled from a test root: never an orphan, but an external
                // root may still reach it as production.
                Some(Origin::Evidence) => owner_proves_unreached = false,
                None => owner_proves_unreached &= walk.proves_unreached(),
            }
        }
        if owner_reaches_production {
            continue;
        }
        // Only a package walk can prove a file unreached.
        owner_proves_unreached &= owner_walked;

        // A package whose production root sits outside its own directory
        // can reach a file no layout or owning manifest attributes to it:
        // below the root's directory, or anywhere through `#[path]` and
        // `include!`. Such packages are rare, so each one is asked.
        let listing = listing.get_or_insert_with(|| list_workspace(workspace_root));
        let (declarers, declarers_listed) = external_declarers
            .get_or_insert_with(|| external_root_declarers(workspace_root, listing.as_ref()))
            .clone();
        let mut declarers_prove_unreached = declarers_listed;
        let mut declaring_packages = BTreeSet::new();
        for package_dir in &declarers {
            let Some(walk) = walks
                .entry(package_dir.clone())
                .or_insert_with(|| PackageWalk::new(workspace_root, package_dir))
                .as_mut()
            else {
                declarers_prove_unreached = false;
                continue;
            };
            match walk.find(workspace_root, &targets) {
                // Only a package the seam-cache discovery also knows (a
                // workspace member outside skipped directories) is granted
                // the file; any other reach just blocks the verdict.
                Some(Origin::Production)
                    if member_packages
                        .get_or_insert_with(|| member_package_dirs(workspace_root))
                        .contains(package_dir) =>
                {
                    declaring_packages.insert(package_dir.clone());
                }
                Some(Origin::Production) => declarers_prove_unreached = false,
                // Reached only from a test, bench or example root: compiled,
                // just not as production. No grant, and no orphan.
                Some(Origin::Evidence) => declarers_prove_unreached = false,
                None => declarers_prove_unreached &= walk.proves_unreached(),
            }
        }
        if !declaring_packages.is_empty() {
            let prefixes = declaring_packages
                .iter()
                .filter_map(|package_dir| package_prefix(workspace_root, package_dir))
                .collect::<BTreeSet<_>>();
            if !prefixes.is_empty() {
                external_packages.insert(relative.clone(), prefixes);
            }
            context.declared_production_sources.insert(relative);
        } else if owner_proves_unreached
            && declarers_prove_unreached
            && escaping
                .get_or_insert_with(|| {
                    listing
                        .as_ref()
                        .and_then(|listing| EscapingReach::scan(workspace_root, listing))
                })
                .as_mut()
                .is_some_and(|reach| reach.proves_unreached(workspace_root, &targets))
        {
            context.module_graph_orphans.insert(relative);
        }
    }
    external_packages
}

/// Whether the escaping scan skips `dir`: VCS metadata, and Cargo's build
/// output (`target` beside a manifest). Anything else is scanned, so a
/// source module that happens to be named `node_modules` or sits in a
/// `target` directory no manifest owns is still seen.
fn escaping_scan_skips(dir: &Path) -> bool {
    match dir.file_name().and_then(|name| name.to_str()) {
        Some(".git") => true,
        Some("target") => dir
            .parent()
            .is_some_and(|parent| parent.join("Cargo.toml").is_file()),
        _ => false,
    }
}

/// Directory entries the escaping scan may visit before it gives up.
const MAX_ESCAPING_SCAN_ENTRIES: usize = 200_000;

/// Every Rust file, package manifest and symlink under the workspace root.
#[derive(Debug)]
struct WorkspaceListing {
    rust_files: Vec<PathBuf>,
    manifest_dirs: Vec<PathBuf>,
    /// Canonical targets of every symlink in the workspace.
    symlink_targets: Vec<PathBuf>,
}

/// Lists the workspace, or `None` when the listing is cut short or cannot
/// describe the tree the analysis reads: under a committed-source overlay,
/// a Rust file or manifest that exists only at `HEAD` is missing from the
/// working-tree listing.
fn list_workspace(workspace_root: &Path) -> Option<WorkspaceListing> {
    if crate::analysis::committed_source::committed_paths_missing_on_disk(workspace_root)
        .iter()
        .any(|path| path.ends_with(".rs") || path.ends_with("Cargo.toml"))
    {
        return None;
    }
    let mut listing = WorkspaceListing {
        rust_files: Vec::new(),
        manifest_dirs: Vec::new(),
        symlink_targets: Vec::new(),
    };
    let mut pending = vec![lexical(&normalize(workspace_root))];
    let mut visited_entries = 0usize;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).ok()? {
            crate::analysis::cancellation::checkpoint().ok()?;
            visited_entries += 1;
            if visited_entries > MAX_ESCAPING_SCAN_ENTRIES {
                return None;
            }
            let entry = entry.ok()?;
            let path = entry.path();
            let file_type = entry.file_type().ok()?;
            if file_type.is_symlink() {
                // A dangling link aliases nothing.
                if let Ok(target) = std::fs::canonicalize(&path) {
                    listing.symlink_targets.push(target);
                }
            } else if file_type.is_dir() {
                if !escaping_scan_skips(&path) {
                    pending.push(path);
                }
            } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
                listing.manifest_dirs.push(lexical(&normalize(&dir)));
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                listing.rust_files.push(lexical(&normalize(&path)));
            }
        }
    }
    Some(listing)
}

/// Directories between `file` and the workspace root (inclusive) that hold
/// a `Cargo.toml`, on disk or at `HEAD`.
fn ancestor_manifest_dirs(workspace_root: &Path, file: &Path) -> Vec<PathBuf> {
    let root = lexical(&normalize(workspace_root));
    let file = lexical(&normalize(file));
    file.ancestors()
        .skip(1)
        .take_while(|dir| dir.starts_with(&root))
        .filter(|dir| {
            !matches!(
                read_source(workspace_root, &dir.join("Cargo.toml")),
                SourceRead::Absent
            )
        })
        .map(Path::to_path_buf)
        .collect()
}

/// Package directories the seam-cache manifest discovery reports.
fn member_package_dirs(workspace_root: &Path) -> BTreeSet<PathBuf> {
    crate::analysis::seam_cache::workspace_manifest_dir_prefixes(workspace_root)
        .into_iter()
        .map(|prefix| lexical(&normalize(&workspace_root.join(&prefix))))
        .collect()
}

/// Whether `dir/Cargo.toml` is a virtual workspace manifest, which compiles
/// nothing. A missing, invalid or legacy `[project]` manifest is not.
fn is_workspace_only_manifest(workspace_root: &Path, dir: &Path) -> bool {
    let SourceRead::Text(text) = read_source(workspace_root, &dir.join("Cargo.toml")) else {
        return false;
    };
    // Cargo still builds a legacy `[project]` table, so only a manifest with
    // `[workspace]` and neither table is workspace-only.
    toml::from_str::<toml::Value>(&text).is_ok_and(|value| {
        value.get("workspace").is_some()
            && value.get("package").is_none()
            && value.get("project").is_none()
    })
}

/// What reaches files from outside the walks a verdict asks: every
/// `#[path]` and `include!` edge declared anywhere in the workspace, walked
/// with its own `mod` children, and every symlink, which gives a file a
/// second spelling some package may compile it under.
///
/// A package's walk never sees another package reaching into it
/// (`#[path = "../../b/src/proto/mod.rs"]` in crate `a`, a build script that
/// `include!`s a sibling's source, or `a/src/shared -> ../../common/src/shared`),
/// so an orphan verdict also requires that none of these reaches the file.
/// This is built once, and only when an orphan verdict is about to be
/// recorded.
#[derive(Debug)]
struct EscapingReach {
    /// A walk seeded with every `#[path]`/`include!` target.
    walk: PackageWalk,
    /// Canonical targets of every symlink in the workspace.
    symlink_targets: Vec<PathBuf>,
}

impl EscapingReach {
    /// Scans every listed Rust file, or `None` when any of them cannot be
    /// scanned completely. Files a symlinked directory reaches are not
    /// listed; the symlink alias covers them.
    fn scan(workspace_root: &Path, listing: &WorkspaceListing) -> Option<Self> {
        let mut roots = Vec::new();
        for file in &listing.rust_files {
            let source = match read_source(workspace_root, file) {
                SourceRead::Text(source) => source,
                SourceRead::Absent => continue,
                SourceRead::Unreadable => return None,
            };
            // Without a macro call, a `path` or an `include`, a file can
            // neither declare an escaping edge nor hide one.
            if !source.contains('!') && !source.contains("path") && !source.contains("include") {
                continue;
            }
            let scan = rust_module_tree_scan(&source);
            // Any unresolved construct (a dependency's item macro can expand
            // to `#[path = "../../b/src/x.rs"] mod x;` without spelling it)
            // leaves every verdict unknown.
            if !scan.complete {
                return None;
            }
            let directory = file.parent().map(Path::to_path_buf).unwrap_or_default();
            for edge in scan.edges {
                match edge {
                    RustModuleTreeEdge::Path(target) => {
                        roots.push((lexical(&directory.join(target)), ChildAnchor::Both));
                    }
                    RustModuleTreeEdge::Include(target) => {
                        roots.push((lexical(&directory.join(target)), ChildAnchor::Included));
                    }
                    RustModuleTreeEdge::Default { .. } => {}
                }
            }
        }
        Some(Self {
            walk: PackageWalk::from_loaded_files(roots),
            symlink_targets: listing.symlink_targets.clone(),
        })
    }

    /// Whether nothing outside the asked walks reaches any of `targets`.
    fn proves_unreached(&mut self, workspace_root: &Path, targets: &[PathBuf]) -> bool {
        let aliased = targets.iter().any(|target| {
            let canonical = std::fs::canonicalize(target).unwrap_or_else(|_| target.clone());
            self.symlink_targets
                .iter()
                .any(|link| canonical.starts_with(link))
        });
        !aliased && self.walk.find(workspace_root, targets).is_none() && self.walk.complete
    }
}

/// Workspace-relative package root prefix of an anchored package directory.
fn package_prefix(workspace_root: &Path, package_dir: &Path) -> Option<String> {
    let relative = package_dir
        .strip_prefix(lexical(&normalize(workspace_root)))
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    Some(if relative.is_empty() {
        relative
    } else {
        format!("{relative}/")
    })
}

/// Every package in the workspace with a target path (library, binary,
/// test, bench, example or build script) outside its own directory.
///
/// Packages come from the full workspace listing, so a package under a
/// directory other discovery skips (`fixtures/`) is still asked. Without a
/// listing the seam-cache manifest discovery supplies the grants, and the
/// second value is false: that set may be missing a declarer.
fn external_root_declarers(
    workspace_root: &Path,
    listing: Option<&WorkspaceListing>,
) -> (BTreeSet<PathBuf>, bool) {
    let package_dirs = match listing {
        Some(listing) => listing.manifest_dirs.clone(),
        None => member_package_dirs(workspace_root).into_iter().collect(),
    };
    let mut declarers = BTreeSet::new();
    for package_dir in package_dirs {
        let Some((manifest_text, manifest)) = read_manifest(workspace_root, &package_dir) else {
            continue;
        };
        if production_roots(&manifest, &package_dir)
            .iter()
            .chain(&evidence_roots(&manifest, &manifest_text, &package_dir))
            .any(|root| !root.starts_with(&package_dir))
        {
            declarers.insert(package_dir);
        }
    }
    (declarers, listing.is_some())
}

/// The package manifest text and its parsed value, or `None` for a missing,
/// invalid or virtual (workspace-only) manifest, which compiles nothing.
/// Read through the same committed-source view as the module files, so a
/// dirty manifest cannot describe a different crate than the sources walked.
fn read_manifest(workspace_root: &Path, package_dir: &Path) -> Option<(String, toml::Value)> {
    let SourceRead::Text(text) = read_source(workspace_root, &package_dir.join("Cargo.toml"))
    else {
        return None;
    };
    let value = toml::from_str::<toml::Value>(&text).ok()?;
    value.get("package")?;
    Some((text, value))
}

/// Explicit `[lib]`/`[[bin]]` paths plus the autodiscovered `src/lib.rs`
/// (unless `[lib] path` replaces it), `src/main.rs` and `src/bin/` roots.
fn production_roots(manifest: &toml::Value, package_dir: &Path) -> BTreeSet<PathBuf> {
    let mut roots = BTreeSet::new();
    collect_explicit_paths(manifest.get("bin"), package_dir, &mut roots);
    if let Some(lib) = manifest.get("lib") {
        collect_explicit_paths(
            Some(&toml::Value::Array(vec![lib.clone()])),
            package_dir,
            &mut roots,
        );
    }
    let src = package_dir.join("src");
    // A package has one library: a declared `[lib] path` replaces
    // `src/lib.rs` as the root. A `path` that is not a string keeps the
    // default, so an unreadable declaration never drops a real root.
    let custom_lib_path = manifest
        .get("lib")
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str)
        .is_some_and(|path| !path.trim().is_empty());
    if !custom_lib_path {
        roots.insert(src.join("lib.rs"));
    }
    roots.insert(src.join("main.rs"));
    roots.extend(autodiscovered(&src.join("bin")));
    roots
        .into_iter()
        .map(|root| lexical(&normalize(&root)))
        .collect()
}

/// Test, bench, example and build-script roots, declared and autodiscovered.
fn evidence_roots(
    manifest: &toml::Value,
    manifest_text: &str,
    package_dir: &Path,
) -> BTreeSet<PathBuf> {
    let declared = declared_targets_from_manifest(manifest_text, package_dir);
    let mut roots = BTreeSet::new();
    roots.extend(declared.tests);
    roots.extend(declared.benches);
    roots.extend(declared.build_script);
    collect_explicit_paths(manifest.get("example"), package_dir, &mut roots);
    for dir in ["tests", "benches", "examples"] {
        roots.extend(autodiscovered(&package_dir.join(dir)));
    }
    roots
        .into_iter()
        .map(|root| lexical(&normalize(&root)))
        .collect()
}

/// Cargo's autodiscovery shapes under one directory: `<dir>/<name>.rs` and
/// `<dir>/<name>/main.rs`.
fn autodiscovered(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            roots.push(path.join("main.rs"));
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            roots.push(path);
        }
    }
    roots
}

impl PackageWalk {
    /// A walk over the package in `package_dir`, or `None` when the
    /// directory holds no package manifest.
    fn new(workspace_root: &Path, package_dir: &Path) -> Option<Self> {
        let (manifest_text, manifest) = read_manifest(workspace_root, package_dir)?;
        let production_roots = production_roots(&manifest, package_dir);
        let evidence_roots = evidence_roots(&manifest, &manifest_text, package_dir);
        Some(Self {
            queue: production_roots
                .iter()
                .map(|root| (root.clone(), ChildAnchor::Directory))
                .collect(),
            evidence_roots: Some(evidence_roots.into_iter().collect()),
            phase: Origin::Production,
            visited: BTreeSet::new(),
            reached: BTreeMap::new(),
            production_roots,
            production_root_read: false,
            scans: BTreeMap::new(),
            complete: true,
        })
    }

    /// A walk from files loaded through `#[path]` or `include!`, outside any
    /// one package's target roots.
    fn from_loaded_files(files: Vec<(PathBuf, ChildAnchor)>) -> Self {
        Self {
            queue: files,
            evidence_roots: None,
            phase: Origin::Production,
            visited: BTreeSet::new(),
            reached: BTreeMap::new(),
            production_roots: BTreeSet::new(),
            production_root_read: false,
            scans: BTreeMap::new(),
            complete: true,
        }
    }

    /// Where the walk reaches any spelling of the asked-for file (lexical
    /// first, then canonical), exploring only as far as needed.
    fn find(&mut self, workspace_root: &Path, targets: &[PathBuf]) -> Option<Origin> {
        let direction = targets.first()?.clone();
        loop {
            if let Some(origin) = targets.iter().find_map(|target| self.reached.get(target)) {
                return Some(*origin);
            }
            if !self.step(workspace_root, &direction) {
                return None;
            }
        }
    }

    /// Whether a completed walk proves every unreached file unreachable. A
    /// package with no readable library or binary root is not a tree this
    /// walk can judge (a partial checkout, a manifest ahead of its sources).
    fn proves_unreached(&self) -> bool {
        self.complete && self.production_root_read
    }

    /// Expands one pending entry, preferring the one nearest `target`.
    /// Returns false once both phases are exhausted.
    fn step(&mut self, workspace_root: &Path, target: &Path) -> bool {
        // The directed pick scans the queue, so past a bound the walk falls
        // back to plain depth-first order and stays linear.
        let position = if self.queue.len() <= DIRECTED_QUEUE_LIMIT {
            self.queue
                .iter()
                .enumerate()
                .max_by_key(|(_, (file, _))| shared_prefix_len(file, target))
                .map(|(position, _)| position)
        } else {
            self.queue.len().checked_sub(1)
        };
        let Some(position) = position else {
            return match self.evidence_roots.take() {
                Some(roots) => {
                    self.phase = Origin::Evidence;
                    self.visited.clear();
                    self.queue = roots
                        .into_iter()
                        .map(|root| (root, ChildAnchor::Directory))
                        .collect();
                    true
                }
                None => false,
            };
        };
        let (file, anchor) = self.queue.swap_remove(position);
        if !self.visited.insert((file.clone(), anchor)) {
            return true;
        }
        if !self.scans.contains_key(&file) {
            if self.scans.len() >= MAX_WALK_FILES {
                // Stop exploring: the tree is too large to judge.
                self.complete = false;
                self.queue.clear();
                self.evidence_roots = None;
                return false;
            }
            let scan = match read_source(workspace_root, &file) {
                SourceRead::Text(source) => Some(rust_module_tree_scan(&source)),
                SourceRead::Absent => None,
                SourceRead::Unreadable => {
                    self.complete = false;
                    None
                }
            };
            self.scans.insert(file.clone(), scan);
        }
        let Some(Some(scan)) = self.scans.get(&file) else {
            return true;
        };
        let edges = scan.edges.clone();
        self.complete &= scan.complete;
        if anchor == ChildAnchor::Included
            && edges
                .iter()
                .any(|edge| matches!(edge, RustModuleTreeEdge::Default { .. }))
        {
            self.complete = false;
        }
        if self.phase == Origin::Production && self.production_roots.contains(&file) {
            self.production_root_read = true;
        }
        self.reached.entry(file.clone()).or_insert(self.phase);
        if let Ok(canonical) = std::fs::canonicalize(&file)
            && canonical != file
        {
            self.reached.entry(canonical).or_insert(self.phase);
        }
        let directory = file.parent().map(Path::to_path_buf).unwrap_or_default();
        for edge in edges {
            match edge {
                RustModuleTreeEdge::Default { inline, name } => {
                    for base in child_bases(&file, &directory, anchor) {
                        let base = inline.iter().fold(base, |base, segment| base.join(segment));
                        self.queue
                            .push((base.join(format!("{name}.rs")), ChildAnchor::Stem));
                        self.queue
                            .push((base.join(&name).join("mod.rs"), ChildAnchor::Directory));
                    }
                }
                RustModuleTreeEdge::Path(target) => {
                    self.queue
                        .push((lexical(&directory.join(target)), ChildAnchor::Both));
                }
                RustModuleTreeEdge::Include(target) => {
                    self.queue
                        .push((lexical(&directory.join(target)), ChildAnchor::Included));
                }
            }
        }
        true
    }
}

/// Number of leading path components two paths share.
fn shared_prefix_len(left: &Path, right: &Path) -> usize {
    left.components()
        .zip(right.components())
        .take_while(|(left, right)| left == right)
        .count()
}

/// The directories a file's default `mod name;` children resolve under.
fn child_bases(file: &Path, directory: &Path, anchor: ChildAnchor) -> Vec<PathBuf> {
    let stem_dir = file
        .file_stem()
        .map(|stem| directory.join(stem))
        .unwrap_or_else(|| directory.to_path_buf());
    let is_mod_rs = file.file_name().is_some_and(|name| name == "mod.rs");
    match anchor {
        ChildAnchor::Directory => vec![directory.to_path_buf()],
        ChildAnchor::Stem if is_mod_rs => vec![directory.to_path_buf()],
        ChildAnchor::Stem => vec![stem_dir],
        ChildAnchor::Both | ChildAnchor::Included => vec![directory.to_path_buf(), stem_dir],
    }
}

enum SourceRead {
    Text(String),
    Absent,
    Unreadable,
}

/// Reads a module file through the same committed-source view the analysis
/// reads, so a committed-history diff walks HEAD content.
fn read_source(workspace_root: &Path, file: &Path) -> SourceRead {
    let root = lexical(&normalize(workspace_root));
    let bytes = match file.strip_prefix(&root) {
        Ok(relative) => {
            match crate::analysis::committed_source::read_source_bytes(workspace_root, relative) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => return SourceRead::Absent,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return SourceRead::Absent;
                }
                Err(_) => return SourceRead::Unreadable,
            }
        }
        // A module outside the analyzed workspace still belongs to the tree.
        Err(_) => match std::fs::read(file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return SourceRead::Absent;
            }
            Err(_) => return SourceRead::Unreadable,
        },
    };
    match String::from_utf8(bytes) {
        Ok(text) => SourceRead::Text(text),
        Err(_) => SourceRead::Unreadable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str, files: &[(&str, &str)]) -> Result<PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("ripr-module-graph-{name}-{stamp}"));
        for (path, text) in files {
            let path = root.join(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            std::fs::write(&path, text).map_err(|error| error.to_string())?;
        }
        Ok(root)
    }

    fn evidence_for(root: &Path, candidates: &[&str]) -> SourceRoleContext {
        let mut context = SourceRoleContext::empty();
        apply_module_graph_evidence(root, &mut context, candidates.iter().map(Path::new));
        context
    }

    const MANIFEST: &str = "[package]\nname='tree'\nversion='0.1.0'\nedition='2021'\n";

    #[test]
    fn walk_follows_mod_rs_bin_roots_and_evidence_roots() -> Result<(), String> {
        let root = fixture(
            "anchors",
            &[
                ("Cargo.toml", MANIFEST),
                ("src/lib.rs", "pub mod a;\n"),
                ("src/a/mod.rs", "pub mod b;\n"),
                ("src/a/b.rs", ""),
                ("src/bin/tool.rs", "mod cli;\nfn main() {}\n"),
                ("src/bin/cli.rs", ""),
                (
                    "tests/it.rs",
                    "#[path = \"../src/shared_by_tests.rs\"]\nmod shared;\n",
                ),
                ("src/shared_by_tests.rs", ""),
                ("src/orphan.rs", ""),
                ("src/a/orphan.rs", ""),
            ],
        )?;
        let context = evidence_for(
            &root,
            &[
                "src/a/b.rs",
                "src/bin/cli.rs",
                "src/shared_by_tests.rs",
                "src/orphan.rs",
                "src/a/orphan.rs",
            ],
        );
        assert_eq!(
            context.module_graph_orphans,
            BTreeSet::from([
                PathBuf::from("src/a/orphan.rs"),
                PathBuf::from("src/orphan.rs")
            ])
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn walk_expands_a_file_under_every_anchor_that_reaches_it() -> Result<(), String> {
        // `foo.rs` is both `mod foo;` (children under `src/foo/`) and a
        // `#[path]` module (children beside it): rustc compiles both.
        let root = fixture(
            "two-anchors",
            &[
                ("Cargo.toml", MANIFEST),
                (
                    "src/lib.rs",
                    "mod foo;\n#[path = \"foo.rs\"]\nmod foo_again;\n",
                ),
                ("src/foo.rs", "mod bar;\n"),
                ("src/foo/bar.rs", ""),
                ("src/bar.rs", ""),
            ],
        )?;
        let context = evidence_for(&root, &["src/foo/bar.rs", "src/bar.rs"]);
        assert!(context.module_graph_orphans.is_empty(), "{context:?}");
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn another_packages_path_edge_and_unknown_macros_block_the_orphan_verdict() -> Result<(), String>
    {
        // Crate `a` compiles `b/src/proto.rs` through `#[path]`; `b` never
        // declares it. `c` calls a dependency's item macro, which may
        // expand to `mod generated;`. Neither file is an orphan; `b`'s
        // undeclared `stray.rs` still is.
        let root = fixture(
            "cross-package",
            &[
                ("a/Cargo.toml", MANIFEST),
                (
                    "a/src/lib.rs",
                    "#[path = \"../../b/src/proto.rs\"]\nmod proto;\n",
                ),
                ("b/Cargo.toml", MANIFEST),
                ("b/src/lib.rs", ""),
                ("b/src/proto.rs", ""),
                ("b/src/stray.rs", ""),
                ("c/Cargo.toml", MANIFEST),
                ("c/src/lib.rs", "decl::declare_mod!(generated);\n"),
                ("c/src/generated.rs", ""),
            ],
        )?;
        let context = evidence_for(
            &root,
            &["b/src/proto.rs", "b/src/stray.rs", "c/src/generated.rs"],
        );
        // `c`'s macro could equally expand to a `#[path]` into `b`, so it
        // leaves every verdict in the workspace unknown.
        assert!(
            context.module_graph_orphans.is_empty(),
            "{:?}",
            context.module_graph_orphans
        );
        std::fs::remove_file(root.join("c/src/lib.rs")).map_err(|error| error.to_string())?;
        let context = evidence_for(
            &root,
            &["b/src/proto.rs", "b/src/stray.rs", "c/src/generated.rs"],
        );
        assert_eq!(
            context.module_graph_orphans,
            BTreeSet::from([PathBuf::from("b/src/stray.rs")])
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn walk_proves_nothing_without_a_readable_production_root() -> Result<(), String> {
        // No `src/lib.rs`, `src/main.rs` or declared root: the tree is
        // unknown, so a loose `src/` file keeps the layout rule.
        let root = fixture(
            "no-root",
            &[("Cargo.toml", MANIFEST), ("src/a.rs", ""), ("src/b.rs", "")],
        )?;
        let context = evidence_for(&root, &["src/a.rs"]);
        assert!(context.module_graph_orphans.is_empty());
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn walk_proves_nothing_for_virtual_or_missing_manifests() -> Result<(), String> {
        let root = fixture(
            "virtual",
            &[
                ("Cargo.toml", "[workspace]\nmembers=[]\n"),
                ("src/lib.rs", ""),
                ("src/orphan.rs", ""),
                ("loose/src/lib.rs", ""),
                ("loose/src/orphan.rs", ""),
            ],
        )?;
        let context = evidence_for(&root, &["src/orphan.rs", "loose/src/orphan.rs"]);
        assert!(context.module_graph_orphans.is_empty());
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn another_packages_path_target_blocks_its_children_and_cfg_attr_paths() -> Result<(), String> {
        // Exact-head review on #4556: crate `a` compiles `b/src/proto/mod.rs`
        // through `#[path]`, so its child `inner.rs` is compiled too; and a
        // `cfg_attr` path cannot be resolved, so it blocks every verdict.
        // `b/src/stray.rs` sits outside the reached directory and stays an
        // orphan while only the plain `#[path]` exists.
        let root = fixture(
            "path-children",
            &[
                ("a/Cargo.toml", MANIFEST),
                (
                    "a/src/lib.rs",
                    "#[path = \"../../b/src/proto/mod.rs\"]\nmod proto;\n",
                ),
                ("b/Cargo.toml", MANIFEST),
                ("b/src/lib.rs", ""),
                ("b/src/proto/mod.rs", "pub mod inner;\n"),
                ("b/src/proto/inner.rs", ""),
                ("b/src/stray.rs", ""),
            ],
        )?;
        let context = evidence_for(&root, &["b/src/proto/inner.rs", "b/src/stray.rs"]);
        assert_eq!(
            context.module_graph_orphans,
            BTreeSet::from([PathBuf::from("b/src/stray.rs")])
        );
        std::fs::write(
            root.join("a/src/lib.rs"),
            "#[cfg_attr(unix, path = \"../../b/src/stray.rs\")]\nmod plat;\n",
        )
        .map_err(|error| error.to_string())?;
        let context = evidence_for(&root, &["b/src/stray.rs"]);
        assert!(
            context.module_graph_orphans.is_empty(),
            "an unresolved `cfg_attr` path must block the verdict: {:?}",
            context.module_graph_orphans
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn package_nested_in_a_tests_directory_is_asked_too() -> Result<(), String> {
        // Exact-head review on #4556: the layout owner of
        // `tests/harness/helpers.rs` is the root package, but the nearest
        // manifest's `[lib]` compiles it.
        let root = fixture(
            "nested-harness",
            &[
                (
                    "Cargo.toml",
                    "[package]\nname='root'\nversion='0.1.0'\nedition='2021'\n[workspace]\nmembers=['tests/harness']\n",
                ),
                ("src/lib.rs", ""),
                (
                    "tests/harness/Cargo.toml",
                    "[package]\nname='harness'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='lib.rs'\n",
                ),
                ("tests/harness/lib.rs", "pub mod helpers;\n"),
                ("tests/harness/helpers.rs", ""),
                ("tests/harness/stray.rs", ""),
            ],
        )?;
        let context = evidence_for(
            &root,
            &["tests/harness/helpers.rs", "tests/harness/stray.rs"],
        );
        assert_eq!(
            context.module_graph_orphans,
            BTreeSet::from([PathBuf::from("tests/harness/stray.rs")])
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn escaping_edges_from_target_dirs_test_targets_and_includes_block_verdicts()
    -> Result<(), String> {
        // Re-review on #4556. Each case compiles a file its owning package
        // never declares; a true orphan beside it stays an orphan.
        let root = fixture(
            "escape-shapes",
            &[
                ("Cargo.toml", "[workspace]\nmembers=['a','b','c']\n"),
                ("a/Cargo.toml", MANIFEST),
                // A source module named `target` (no manifest beside it).
                ("a/src/lib.rs", "mod target;\n"),
                (
                    "a/src/target/mod.rs",
                    "#[path = \"../../../b/src/x.rs\"]\nmod x;\n",
                ),
                (
                    "c/Cargo.toml",
                    "[package]\nname='c'\nversion='0.1.0'\nedition='2021'\n[[test]]\nname='helper'\npath='../b/src/helper.rs'\n",
                ),
                ("c/src/lib.rs", ""),
                ("b/Cargo.toml", MANIFEST),
                ("b/src/lib.rs", ""),
                ("b/src/x.rs", ""),
                ("b/src/helper.rs", ""),
                ("b/src/stray.rs", ""),
            ],
        )?;
        let context = evidence_for(&root, &["b/src/x.rs", "b/src/helper.rs", "b/src/stray.rs"]);
        assert_eq!(
            context.module_graph_orphans,
            BTreeSet::from([PathBuf::from("b/src/stray.rs")])
        );
        // `include!` pastes text into the including module, so a `mod`
        // declared by the included file resolves against the includer.
        std::fs::write(root.join("b/src/lib.rs"), "include!(\"../gen/frag.rs\");\n")
            .map_err(|error| error.to_string())?;
        std::fs::create_dir_all(root.join("b/gen")).map_err(|error| error.to_string())?;
        std::fs::write(root.join("b/gen/frag.rs"), "mod stray;\n")
            .map_err(|error| error.to_string())?;
        let context = evidence_for(&root, &["b/src/stray.rs"]);
        assert!(
            context.module_graph_orphans.is_empty(),
            "{:?}",
            context.module_graph_orphans
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[cfg(unix)]
    #[test]
    fn walk_matches_modules_reached_through_a_symlinked_directory() -> Result<(), String> {
        // Exact-head review on #4556: `a/src/shared` links to
        // `common/src/shared`, so `a` compiles `common/src/shared/x.rs`
        // under another spelling.
        let root = fixture(
            "symlinked",
            &[
                ("Cargo.toml", "[workspace]\nmembers=['a','common']\n"),
                ("a/Cargo.toml", MANIFEST),
                ("a/src/lib.rs", "mod shared;\n"),
                ("common/Cargo.toml", MANIFEST),
                ("common/src/lib.rs", ""),
                ("common/src/shared/mod.rs", "pub mod x;\n"),
                ("common/src/shared/x.rs", ""),
            ],
        )?;
        let unlinked = evidence_for(&root, &["common/src/shared/x.rs"]);
        assert!(
            unlinked
                .module_graph_orphans
                .contains(Path::new("common/src/shared/x.rs")),
            "fixture control: without the link no target compiles the file"
        );
        std::os::unix::fs::symlink(root.join("common/src/shared"), root.join("a/src/shared"))
            .map_err(|error| error.to_string())?;
        let context = evidence_for(&root, &["common/src/shared/x.rs"]);
        assert!(
            context.module_graph_orphans.is_empty(),
            "{:?}",
            context.module_graph_orphans
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn escaping_scan_reads_committed_files_missing_from_disk() -> Result<(), String> {
        // Re-review on #4556: at `HEAD`, `a/src/gen.rs` reaches `b`'s
        // undeclared file through `#[path]`; the working tree deleted it.
        let root = fixture(
            "committed-missing",
            &[
                ("Cargo.toml", "[workspace]\nmembers=['a','b']\n"),
                ("a/Cargo.toml", MANIFEST),
                ("a/src/lib.rs", "mod gen;\n"),
                ("b/Cargo.toml", MANIFEST),
                ("b/src/lib.rs", ""),
                ("b/src/reached.rs", ""),
            ],
        )?;
        let worktree = evidence_for(&root, &["b/src/reached.rs"]);
        assert!(
            worktree
                .module_graph_orphans
                .contains(Path::new("b/src/reached.rs")),
            "fixture control: the working tree has no edge to the file"
        );
        let overlay = crate::analysis::committed_source::CommittedSourceOverlay::from_entries(
            &root,
            [(
                "a/src/gen.rs",
                Some(b"#[path = \"../../b/src/reached.rs\"]\nmod reached;\n".as_slice()),
            )],
        );
        let committed = crate::analysis::committed_source::with_overlay(
            Some(std::sync::Arc::new(overlay)),
            || evidence_for(&root, &["b/src/reached.rs"]),
        );
        assert!(
            committed.module_graph_orphans.is_empty(),
            "{:?}",
            committed.module_graph_orphans
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn declarers_in_skipped_dirs_ancestors_and_head_only_manifests_are_asked() -> Result<(), String>
    {
        // Third review on #4556. `fixtures/x` compiles `b/src/shared.rs`
        // from its `[lib]`; the root package compiles `inner/mod.rs` across
        // `inner/Cargo.toml`. `stray.rs` and `b/src/stray.rs` stay orphans.
        let root = fixture(
            "declarer-shapes",
            &[
                (
                    "Cargo.toml",
                    "[package]\nname='root'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='lib.rs'\n[workspace]\nmembers=['b','fixtures/x','inner']\n",
                ),
                ("lib.rs", "mod inner;\n"),
                ("stray.rs", ""),
                (
                    "inner/Cargo.toml",
                    "[package]\nname='inner'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='own.rs'\n",
                ),
                ("inner/own.rs", ""),
                ("inner/mod.rs", ""),
                ("b/Cargo.toml", MANIFEST),
                ("b/src/lib.rs", ""),
                ("b/src/shared.rs", ""),
                ("b/src/stray.rs", ""),
                (
                    "fixtures/x/Cargo.toml",
                    "[package]\nname='x'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='../../b/src/shared.rs'\n",
                ),
            ],
        )?;
        let candidates = [
            "inner/mod.rs",
            "stray.rs",
            "b/src/shared.rs",
            "b/src/stray.rs",
        ];
        let context = evidence_for(&root, &candidates);
        assert_eq!(
            context.module_graph_orphans,
            BTreeSet::from([PathBuf::from("b/src/stray.rs"), PathBuf::from("stray.rs")])
        );
        // A manifest that exists only at `HEAD` is invisible to the listing,
        // so no verdict is issued.
        let overlay = crate::analysis::committed_source::CommittedSourceOverlay::from_entries(
            &root,
            [(
                "c/Cargo.toml",
                Some(b"[package]\nname='c'\nversion='0.1.0'\nedition='2021'\n".as_slice()),
            )],
        );
        let committed = crate::analysis::committed_source::with_overlay(
            Some(std::sync::Arc::new(overlay)),
            || evidence_for(&root, &candidates),
        );
        assert!(
            committed.module_graph_orphans.is_empty(),
            "{:?}",
            committed.module_graph_orphans
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn legacy_project_manifest_is_not_workspace_only() -> Result<(), String> {
        // Delta review on #4556: Cargo still builds `[project]` below
        // edition 2024, so a nested `legacy/Cargo.toml` using it is unknown,
        // not a manifest that compiles nothing.
        let root = fixture(
            "legacy-project",
            &[
                ("Cargo.toml", MANIFEST),
                ("src/lib.rs", ""),
                (
                    "legacy/Cargo.toml",
                    "[project]\nname='legacy'\nversion='0.1.0'\n",
                ),
                ("legacy/src/lib.rs", "mod stray;\n"),
                ("legacy/src/stray.rs", ""),
            ],
        )?;
        let context = evidence_for(&root, &["legacy/src/stray.rs"]);
        assert!(
            context.module_graph_orphans.is_empty(),
            "{:?}",
            context.module_graph_orphans
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }

    #[test]
    fn walk_reads_the_committed_manifest_under_the_overlay() -> Result<(), String> {
        // Devin review on #4556: a committed-history run reads `HEAD` source,
        // so the root inventory must come from the `HEAD` manifest too. On
        // disk the lib root moved to `src/new.rs`; at `HEAD` it is
        // `src/old.rs`, which therefore is compiled and must not be an orphan.
        let root = fixture(
            "dirty-manifest",
            &[
                (
                    "Cargo.toml",
                    "[package]\nname='tree'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='src/new.rs'\n",
                ),
                ("src/new.rs", ""),
                ("src/old.rs", ""),
            ],
        )?;
        let worktree = evidence_for(&root, &["src/old.rs"]);
        assert!(
            worktree
                .module_graph_orphans
                .contains(Path::new("src/old.rs")),
            "fixture control: the working-tree manifest leaves `src/old.rs` unreached"
        );
        let overlay = crate::analysis::committed_source::CommittedSourceOverlay::from_entries(
            &root,
            [
                (
                    "Cargo.toml",
                    Some(
                        b"[package]\nname='tree'\nversion='0.1.0'\nedition='2021'\n[lib]\npath='src/old.rs'\n"
                            .as_slice(),
                    ),
                ),
                ("src/new.rs", None),
            ],
        );
        let committed = crate::analysis::committed_source::with_overlay(
            Some(std::sync::Arc::new(overlay)),
            || evidence_for(&root, &["src/old.rs"]),
        );
        assert!(
            committed.module_graph_orphans.is_empty(),
            "the `HEAD` manifest compiles `src/old.rs`: {:?}",
            committed.module_graph_orphans
        );
        std::fs::remove_dir_all(root).map_err(|error| error.to_string())
    }
}
