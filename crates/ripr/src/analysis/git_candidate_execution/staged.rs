//! Inactive, worker-contained named-tree materialization into retained source.
//!
//! This is object/source capture, not analyzer admission. A caller must hold
//! actual NativeStartup and the externally enforced worker deadline/resources.
//! Payload counters below do not establish exact heap/RSS or disk authority.

use super::{CapturedConfiguration, SubjectError, append_captured_configuration, failed};
use crate::analysis::committed_source::frozen::{
    FrozenFile, FrozenFileMode, FrozenInventory, FrozenSourceAuthority,
};
use crate::analysis::committed_source::staged::SourceAnchor;
use crate::domain::GitObjectId;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const STREAM_MAX: usize = 256 * 1024 * 1024;
const DRAIN_RESERVE: Duration = Duration::from_secs(5);
const SOURCE_MAX: u64 = 512 * 1024 * 1024;
const FILE_MAX: u64 = 256 * 1024 * 1024;

/// Caller-admitted phase DATA, derived from the authenticated native profile.
/// No environment defaults, deserialization or execution capability exists.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CompleteTreeBudget {
    pub(crate) max_listing_bytes: usize,
    pub(crate) max_entries: usize,
    pub(crate) max_path_bytes: u64,
    pub(crate) max_source_bytes: u64,
    pub(crate) max_file_bytes: u64,
    pub(crate) max_configuration_bytes: u64,
    pub(crate) max_buffered_bytes: u64,
}

impl CompleteTreeBudget {
    fn validate(self) -> Result<(), SubjectError> {
        if self.max_listing_bytes == 0 || self.max_listing_bytes > STREAM_MAX
            || self.max_entries == 0 || self.max_path_bytes == 0
            || self.max_source_bytes == 0 || self.max_source_bytes > SOURCE_MAX
            || self.max_file_bytes == 0 || self.max_file_bytes > FILE_MAX
            || self.max_configuration_bytes == 0
            || self.max_configuration_bytes > crate::bounded_input::MAX_CLI_INPUT_BYTES
            || self.max_buffered_bytes == 0
        {
            return Err(failed("staged tree finite profile admission is invalid".into()));
        }
        // Bound retained representations before listing capture and clones.
        // Native allocator/node/stack costs are additionally enforced by the
        // caller's actual finite AS profile, never inferred from this sum.
        let records = (self.max_entries as u64)
            .checked_mul(
                (std::mem::size_of::<TreeRecord<'_>>()
                    + std::mem::size_of::<FrozenFile>() + 64) as u64,
            );
        let representation = (self.max_listing_bytes as u64).checked_mul(2)
            .and_then(|n| self.max_path_bytes.checked_mul(4).and_then(|paths| n.checked_add(paths)))
            .and_then(|n| records.and_then(|records| n.checked_add(records)))
            .and_then(|n| self.max_configuration_bytes.checked_mul(2).and_then(|cfg| n.checked_add(cfg)))
            // Eight queued chunks + consumer + blocked sender + reader scratch;
            // stderr, header and file-copy scratch are separate retained data.
            .and_then(|n| n.checked_add(11 * 64 * 1024 + 8 * 1024 + 4096 + 64 * 1024));
        if representation.is_none_or(|bytes| bytes > self.max_buffered_bytes) {
            return Err(failed("staged tree buffered representation admission exceeded".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct TreeRecord<'a> {
    path: &'a str,
    object: &'a str,
    mode: Option<FrozenFileMode>,
}

/// A borrowed view of the entire validated original tree listing.
/// This is DATA only; no saved view or callback creates execution authority.
pub(crate) struct ValidatedNamedTreePlan<'a> {
    tree: &'a GitObjectId,
    listing: &'a [u8],
    records: &'a [TreeRecord<'a>],
}

impl<'a> ValidatedNamedTreePlan<'a> {
    pub(crate) fn tree(&self) -> &GitObjectId {
        self.tree
    }

    pub(crate) fn original_listing(&self) -> &[u8] {
        self.listing
    }

    /// Original order, including directories and every supported file.
    pub(crate) fn observations(
        &self,
    ) -> impl ExactSizeIterator<Item = (&'a str, Option<FrozenFileMode>, &'a str)> + '_ {
        self.records.iter().map(|record| (record.path, record.mode, record.object))
    }
}

fn records<'a>(
    listing: &'a [u8],
    owner: &SourceAnchor,
    budget: CompleteTreeBudget,
) -> Result<Vec<TreeRecord<'a>>, SubjectError> {
    let mut records = Vec::new();
    let mut entries = 1_usize; // the admitted root
    let mut paths = 0_u64;
    super::validate_configuration_inventory_with(listing, |mode, kind, object, path| {
        entries = entries.checked_add(1)
            .filter(|count| *count <= budget.max_entries)
            .ok_or_else(|| failed("staged tree inventory entry admission exceeded".into()))?;
        paths = paths.checked_add(path.len() as u64)
            .filter(|bytes| *bytes <= budget.max_path_bytes)
            .ok_or_else(|| failed("staged tree retained path admission exceeded".into()))?;
        // The original path refusal is shared, including its existing rule
        // for double dots and platform separators. No lossy inverse is used.
        super::safe_join(Path::new(owner.path_identifier()), path)?;
        let mode = match (mode, kind) {
            ("040000", "tree") => None,
            ("100644", "blob") => Some(FrozenFileMode::Regular),
            ("100755", "blob") => Some(FrozenFileMode::Executable),
            _ => return Err(failed(format!(
                "unsupported tree entry mode \x60{mode}\x60 (\x60{kind}\x60) for \x60{path}\x60: the candidate tree contains a non-file object ripr cannot faithfully materialize"
            ))),
        };
        records.try_reserve_exact(1)
            .map_err(|error| failed(format!("staged tree record allocation failed: {error}")))?;
        records.push(TreeRecord { path, object, mode });
        Ok(())
    })?;
    Ok(records)
}

// Validate structure without decoding paths again or retaining copied names.
// The temporary usize index fits the existing per-record representation budget.
fn validated_records<'a>(
    listing: &'a [u8],
    owner: &SourceAnchor,
    deadline: Instant,
    budget: CompleteTreeBudget,
) -> Result<Vec<TreeRecord<'a>>, SubjectError> {
    budget.validate()?;
    if listing.len() > budget.max_listing_bytes {
        return Err(failed("staged tree plan listing admission exceeded".into()));
    }
    let records = records(listing, owner, budget)?;
    // Match the actual SourceAnchor raw component predicate without creating
    // another path decoder or normalizing an alias before observation.
    for record in &records {
        checkpoint(deadline)?;
        let bytes = record.path.as_bytes();
        if bytes.is_empty() || bytes.contains(&0)
            || bytes.split(|byte| *byte == b'/')
                .any(|part| part.is_empty() || part == b"." || part == b"..")
        {
            return Err(failed(format!(
                "staged tree plan path is not canonical: {}", record.path
            )));
        }
    }
    let index_bytes = records.len().checked_mul(std::mem::size_of::<usize>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .filter(|bytes| *bytes <= budget.max_buffered_bytes)
        .ok_or_else(|| failed("staged tree plan index admission exceeded".into()))?;
    let index_allowance = records.len().checked_mul(64)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| failed("staged tree plan index allowance overflowed".into()))?;
    if records.len().checked_add(1).is_none_or(|count| count > budget.max_entries)
        || index_bytes > index_allowance
    {
        return Err(failed("staged tree plan index admission exceeded".into()));
    }
    let mut order = Vec::new();
    order.try_reserve_exact(records.len())
        .map_err(|error| failed(format!("staged tree plan index allocation failed: {error}")))?;
    order.extend(0..records.len());
    order.sort_unstable_by(|left, right| records[*left].path.cmp(records[*right].path));
    for record in &records {
        checkpoint(deadline)?;
        for (end, _) in record.path.match_indices('/') {
            let ancestor = &record.path[..end];
            let position = order.binary_search_by(|index| records[*index].path.cmp(ancestor))
                .map_err(|position| failed(format!(
                    "staged tree inventory parent directory is missing: {ancestor} (insertion position {position})"
                )))?;
            if records[order[position]].mode.is_some() {
                return Err(failed(format!(
                    "staged tree inventory path overlaps a file: {ancestor}"
                )));
            }
        }
    }
    drop(order);
    Ok(records)
}

fn observe_plan<T>(
    tree: &GitObjectId,
    listing: &[u8],
    records: &[TreeRecord<'_>],
    owner: &SourceAnchor,
    deadline: Instant,
    callback: impl FnOnce(&ValidatedNamedTreePlan<'_>) -> Result<T, SubjectError>,
) -> Result<T, SubjectError> {
    checkpoint(deadline)?;
    owner.verify_fresh()
        .map_err(|error| failed(format!("staged tree fresh source refused: {error}")))?;
    let outcome = callback(&ValidatedNamedTreePlan { tree, listing, records });
    // Even a rejected callback must not hide a source mutation or reset the
    // actual clock. No callback result can seal or qualify the source.
    let postflight = checkpoint(deadline).and_then(|()| owner.verify_fresh()
        .map_err(|error| failed(format!("staged tree fresh source refused: {error}"))));
    match (outcome, postflight) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(_), Err(error)) => Err(error),
        (Err(primary), Err(error)) => Err(failed(format!(
            "{primary}; staged tree plan callback postflight failed: {error}"
        ))),
    }
}

/// Authenticated capture DATA and an actual retained source lease. Construction
/// is private; conversion still installs no analyzer context or guard exception.
pub(crate) struct PreparedStagedTree {
    pub(crate) tree: GitObjectId,
    pub(crate) configuration: CapturedConfiguration,
    inventory: Arc<FrozenInventory>,
    owner: Arc<SourceAnchor>,
}

impl PreparedStagedTree {
    pub(crate) fn frozen_source_authority(
        self,
        logical_root: &Path,
    ) -> Result<Arc<FrozenSourceAuthority>, SubjectError> {
        FrozenSourceAuthority::new_staged(
            logical_root, self.tree, self.configuration, self.inventory, self.owner,
        ).map_err(|error| failed(format!("staged frozen source closeout failed: {error}")))
    }
}

fn checkpoint(deadline: Instant) -> Result<(), SubjectError> {
    crate::analysis::cancellation::checkpoint_typed()
        .map_err(|error| failed(error.to_string()))?;
    if Instant::now() >= deadline {
        return Err(failed("staged tree held worker deadline exhausted".into()));
    }
    Ok(())
}

/// Capture the entire exact tree through the same inventory and batch grammar.
/// Only an actual fresh NativeStartup source lease is a supported caller.
/// Errors never seal a partial source or remove any parent-owned directory.
fn capture_staged_listing(
    repository: &Path,
    tree: &GitObjectId,
    owner: &SourceAnchor,
    deadline: Instant,
    budget: CompleteTreeBudget,
) -> Result<Vec<u8>, SubjectError> {
    budget.validate()?;
    if deadline != owner.deadline() {
        return Err(failed("staged tree deadline differs from its actual source anchor".into()));
    }
    owner.verify_fresh().map_err(|error| failed(format!("staged tree fresh source refused: {error}")))?;
    checkpoint(deadline)?;
    let timeout = deadline.saturating_duration_since(Instant::now())
        .checked_sub(DRAIN_RESERVE).filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| failed("staged tree held deadline lacks its existing Git drain reserve".into()))?;
    let listing = crate::git::run_git_complete_output_with_deadline_and_limit(
        repository,
        &["ls-tree", "-r", "-t", "-z", "--full-tree", tree.as_str()],
        timeout,
        budget.max_listing_bytes,
        crate::git::CompleteGitEnvironment::WholeInput,
    ).map_err(|error| failed(format!("git ls-tree failed: {error}")))?;
    checkpoint(deadline)?;
    if !listing.status.success() {
        return Err(failed("git ls-tree of the candidate tree failed".into()));
    }
    Ok(listing.stdout)
}

/// Observe a full validated original listing without creating or sealing source
/// entries. The callback may retain only DATA subject to its own admitted budget.
pub(crate) fn with_staged_named_tree_plan<T>(
    repository: &Path,
    tree: GitObjectId,
    owner: Arc<SourceAnchor>,
    deadline: Instant,
    budget: CompleteTreeBudget,
    callback: impl FnOnce(&ValidatedNamedTreePlan<'_>) -> Result<T, SubjectError>,
) -> Result<T, SubjectError> {
    let listing = capture_staged_listing(repository, &tree, &owner, deadline, budget)?;
    let records = validated_records(&listing, &owner, deadline, budget)?;
    observe_plan(&tree, &listing, &records, &owner, deadline, callback)
}

/// Preserve the existing staged capture route with an empty plan observer.
pub(crate) fn prepare_staged_named_tree(
    repository: &Path,
    tree: GitObjectId,
    owner: Arc<SourceAnchor>,
    deadline: Instant,
    budget: CompleteTreeBudget,
) -> Result<PreparedStagedTree, SubjectError> {
    prepare_staged_named_tree_with_plan(repository, tree, owner, deadline, budget, |_| Ok(()))
}

/// Validate the complete original listing and invoke the fallible observer
/// before ANY materialization write. Parent-plan authentication belongs to the
/// caller; this callback and its DATA view grant no analyzer admission.
pub(crate) fn prepare_staged_named_tree_with_plan(
    repository: &Path,
    tree: GitObjectId,
    owner: Arc<SourceAnchor>,
    deadline: Instant,
    budget: CompleteTreeBudget,
    callback: impl FnOnce(&ValidatedNamedTreePlan<'_>) -> Result<(), SubjectError>,
) -> Result<PreparedStagedTree, SubjectError> {
    let listing = capture_staged_listing(repository, &tree, &owner, deadline, budget)?;
    let records = validated_records(&listing, &owner, deadline, budget)?;
    observe_plan(&tree, &listing, &records, &owner, deadline, callback)?;
    let mut inventory = FrozenInventory::new();
    for record in records.iter().filter(|record| record.mode.is_none()) {
        checkpoint(deadline)?;
        owner.create_dir(Path::new(record.path))
            .map_err(|error| failed(format!("materialization mkdir failed: {error}")))?;
        inventory.insert_directory(Path::new(record.path));
    }
    let mut configuration = CapturedConfiguration::Absent;
    if records.iter().any(|record| record.mode.is_some()) {
        let mut session = crate::git::CatFileBatch::spawn_complete(repository, deadline)
            .map_err(|error| failed(format!("git cat-file --batch failed: {error}")))?;
        let mut total_bytes = 0_u64;
        let mut chunk = [0_u8; 64 * 1024];
        for record in records.iter().filter(|record| record.mode.is_some()) {
            checkpoint(deadline)?;
            let object = GitObjectId::parse(record.object)
                .map_err(|error| failed(error.to_string()))?;
            let mode = record.mode.ok_or_else(|| failed("staged tree file mode is missing".into()))?;
            let size = session.request_blob(record.object)
                .map_err(|error| failed(format!("git cat-file blob {} failed: {error}", record.object)))?
                .ok_or_else(|| failed(format!("git cat-file blob {} is missing", record.object)))?;
            total_bytes = total_bytes.checked_add(size)
                .filter(|bytes| *bytes <= budget.max_source_bytes)
                .ok_or_else(|| failed("candidate tree materialization exceeded its unchanged source total limit".into()))?;
            if size > budget.max_file_bytes {
                return Err(failed("staged tree file exceeds its unchanged admitted file limit".into()));
            }
            let config = record.path == "ripr.toml";
            if config && size > budget.max_configuration_bytes {
                return Err(failed(format!(
                    "configuration capture exceeds the {}-byte input limit",
                    budget.max_configuration_bytes,
                )));
            }
            let mut config_bytes = Vec::new();
            let relative = Path::new(record.path);
            let mut file = owner.create_new_file(relative, size)
                .map_err(|error| failed(format!("materialization write failed: {error}")))?;
            let mut hash = Sha256::new();
            let mut remaining = size;
            while remaining > 0 {
                checkpoint(deadline)?;
                let take = remaining.min(chunk.len() as u64) as usize;
                session.read_blob_bytes(&mut chunk[..take])
                    .map_err(|error| failed(format!("git cat-file blob {} failed: {error}", record.object)))?;
                hash.update(&chunk[..take]);
                if config {
                    append_captured_configuration(&mut config_bytes, &chunk[..take], budget.max_configuration_bytes)?;
                }
                file.write_all(&chunk[..take])
                    .map_err(|error| failed(format!("materialization write failed: {error}")))?;
                remaining -= take as u64;
            }
            session.end_blob()
                .map_err(|error| failed(format!("git cat-file blob {} failed: {error}", record.object)))?;
            file.finish().map_err(|error| failed(format!("staged file closeout failed: {error}")))?;
            owner.set_executable(relative, mode == FrozenFileMode::Executable)
                .map_err(|error| failed(format!("staged source mode failed: {error}")))?;
            if config {
                let text = String::from_utf8(config_bytes)
                    .map_err(|error| failed(format!("captured configuration is not UTF-8: {error}")))?;
                configuration = CapturedConfiguration::Present { blob_oid: object.clone(), text };
            }
            inventory.insert_file(relative.to_path_buf(), FrozenFile {
                mode, blob_oid: object, size, sha256: hash.finalize().into(),
            });
        }
        // Every requested blob and trailer, actual stdout/stderr EOF and
        // native reader joins must finish before source sealing.
        session.finish().map_err(|error| failed(format!("git cat-file --batch failed: {error}")))?;
    }
    checkpoint(deadline)?;
    owner.finish_materialization()
        .map_err(|error| failed(format!("staged materialization closeout failed: {error}")))?;
    owner.verify_materialized().map_err(failed)?;
    Ok(PreparedStagedTree { tree, configuration, inventory: Arc::new(inventory), owner })
}


#[cfg(all(test, target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
mod tests {
    use super::*;
    use crate::analysis::committed_source::frozen::{self, fs as frozen_fs};
    use crate::analysis::committed_source::staged::{
        DirectoryIdentity, RetainedDirectory, SourceBudget,
    };
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        repository: PathBuf,
        deadline: Instant,
    }

    impl Fixture {
        fn new() -> Result<Self, String> {
            let root = std::env::temp_dir().canonicalize()
                .map_err(|error| error.to_string())?
                .join(format!("ripr-staged-tree-{}-{}", std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)));
            let repository = root.join("repository");
            fs::create_dir_all(&repository).map_err(|error| error.to_string())?;
            let fixture = Self {
                root, repository, deadline: Instant::now() + Duration::from_secs(60),
            };
            fixture.git(&["init", "--initial-branch=main", "--object-format=sha1"])?;
            Ok(fixture)
        }

        fn git(&self, args: &[&str]) -> Result<String, String> {
            let output = crate::git::run_git_output_with_deadline_and_limit_isolated(
                &self.repository, args, Duration::from_secs(30), 4 * 1024 * 1024,
            ).map_err(|error| error.to_string())?;
            if !output.status.success() {
                return Err(format!("fixture git {args:?} failed: {}",
                    String::from_utf8_lossy(&output.stderr)));
            }
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }

        fn write(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
            let destination = self.repository.join(path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(destination, bytes).map_err(|error| error.to_string())
        }

        fn tree(&self) -> Result<GitObjectId, String> {
            self.git(&["add", "."])?;
            GitObjectId::parse(self.git(&["write-tree"])?.trim())
                .map_err(|error| error.to_string())
        }

        fn anchor(&self, name: &str) -> Result<Arc<SourceAnchor>, String> {
            let path = self.root.join(name);
            fs::create_dir(&path).map_err(|error| error.to_string())?;
            let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
            let identity = DirectoryIdentity {
                path: path.to_str().ok_or("fixture path is not UTF-8")?.to_string(),
                dev: metadata.dev(), ino: metadata.ino(),
            };
            Ok(Arc::new(SourceAnchor::new(
                RetainedDirectory::open_absolute(&identity, 4096, self.deadline)?,
                SourceBudget::new(2 * 1024 * 1024, 64, 16 * 1024, 1024 * 1024)?,
                self.deadline,
            )?))
        }

        fn capture(&self, tree: &GitObjectId, owner: Arc<SourceAnchor>,
            budget: CompleteTreeBudget) -> Result<PreparedStagedTree, SubjectError> {
            prepare_staged_named_tree(&self.repository, tree.clone(), owner,
                self.deadline, budget)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // Test-owned fixture cleanup is separate from production stage custody.
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn budget() -> CompleteTreeBudget {
        CompleteTreeBudget {
            max_listing_bytes: 64 * 1024, max_entries: 64,
            max_path_bytes: 16 * 1024, max_source_bytes: 2 * 1024 * 1024,
            max_file_bytes: 1024 * 1024, max_configuration_bytes: 4096,
            max_buffered_bytes: 2 * 1024 * 1024,
        }
    }

    fn refused<T, E: std::fmt::Display>(result: Result<T, E>,
        needle: &str) -> Result<(), String> {
        match result {
            Err(error) if error.to_string().contains(needle) => Ok(()),
            Err(error) => Err(format!("wrong refusal: {error}; expected {needle}")),
            Ok(_) => Err(format!("unexpected success; expected {needle}")),
        }
    }

    fn inventory(authority: &FrozenSourceAuthority)
        -> (Vec<(PathBuf, FrozenFileMode, String, u64, [u8; 32])>, Vec<PathBuf>) {
        (
            authority.inventory().files().map(|(path, file)| (
                path.to_path_buf(), file.mode, file.blob_oid.as_str().to_string(),
                file.size, file.sha256,
            )).collect(),
            authority.inventory().directories().map(Path::to_path_buf).collect(),
        )
    }

    #[test]
    fn actual_staged_tree_matches_ordinary_full_inventory_bytes_and_context()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        let binary: Vec<u8> = (0_u8..=255).cycle().take(131_073).collect();
        fixture.write("src/lib.rs", b"mod sibling; pub fn entry() -> u8 { sibling::value() }\n")?;
        fixture.write("src/sibling.rs", b"pub fn value() -> u8 { 7 }\n")?;
        fixture.write("tests/context.rs", b"use sample::entry; #[test] fn calls_entry() { assert_eq!(entry(),7); }\n")?;
        fixture.write("binary.bin", &binary)?;
        fixture.write("empty.rs", b"")?;
        fixture.write("executable.rs", b"pub fn executable() {}\n")?;
        fixture.write("ripr.toml", b"[analysis]\nmode = \"draft\"\n")?;
        fixture.git(&["add", "."])?;
        fixture.git(&["update-index", "--chmod=+x", "executable.rs"])?;
        let tree = GitObjectId::parse(&fixture.git(&["write-tree"])?)
            .map_err(|error| error.to_string())?;
        let ordinary = super::super::prepare_named_tree(&fixture.repository,
            tree.as_str(), None).map_err(|error| error.to_string())?
            .frozen_source_authority(&fixture.repository).map_err(|error| error.to_string())?;
        let owner = fixture.anchor("source")?;
        // Dirty and staged decoys must never replace any named-tree bytes.
        fixture.write("src/sibling.rs", b"pub fn poison() {}\n")?;
        fixture.write("ripr.toml", b"poison ambient configuration")?;
        fixture.git(&["add", "."])?;
        let captured = fixture.capture(&tree, owner.clone(), budget())
            .map_err(|error| error.to_string())?;
        assert_eq!(captured.configuration, *ordinary.captured_configuration());
        let staged = captured.frozen_source_authority(&fixture.repository)
            .map_err(|error| error.to_string())?;
        assert_eq!(inventory(&staged), inventory(&ordinary));
        assert_eq!(staged.head_tree(), ordinary.head_tree());
        assert_ne!(owner.metadata(Path::new("executable.rs"))?.mode() & 0o111, 0);
        assert_eq!(owner.metadata(Path::new("src/lib.rs"))?.mode() & 0o111, 0);
        for (path, _, _, _, _) in inventory(&ordinary).0 {
            let expected = frozen::with_context(Some(ordinary.clone()), ||
                frozen_fs::read(fixture.repository.join(&path)))
                .map_err(|error| error.to_string())?;
            let actual = frozen::with_context(Some(staged.clone()), ||
                frozen_fs::read(fixture.repository.join(&path)))
                .map_err(|error| error.to_string())?;
            assert_eq!(actual, expected, "named-tree parity for {}", path.display());
        }
        // An existing nested cwd must retain all siblings and original paths.
        let nested_owner = fixture.anchor("nested-cwd-source")?;
        let nested = prepare_staged_named_tree(&fixture.repository.join("src"),
            tree.clone(), nested_owner.clone(), fixture.deadline, budget())
            .map_err(|error| error.to_string())?
            .frozen_source_authority(&fixture.repository).map_err(|error| error.to_string())?;
        assert_eq!(inventory(&nested), inventory(&ordinary),
            "nested cwd narrowed the named whole tree");
        assert_eq!(frozen::with_context(Some(nested.clone()), ||
            frozen_fs::read(fixture.repository.join("tests/context.rs")))
            .map_err(|error| error.to_string())?,
            b"use sample::entry; #[test] fn calls_entry() { assert_eq!(entry(),7); }\n");
        nested.finalize().map_err(|error| error.to_string())?;
        nested_owner.verify_materialized()?;
        fs::remove_dir_all(fixture.repository.join("src")).map_err(|error| error.to_string())?;
        assert_eq!(frozen::with_context(Some(staged.clone()), ||
            frozen_fs::read(fixture.repository.join("src/sibling.rs")))
            .map_err(|error| error.to_string())?, b"pub fn value() -> u8 { 7 }\n");
        staged.finalize().map_err(|error| error.to_string())?;
        ordinary.finalize().map_err(|error| error.to_string())?;
        owner.verify_materialized()?;
        assert!(Path::new(owner.path_identifier()).exists(),
            "frozen closeout must retain the parent's source stage");
        Ok(())
    }

    #[test]
    fn actual_empty_tree_and_authentic_empty_directory_are_exhaustive()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        let empty = GitObjectId::parse(&fixture.git(&["write-tree"])?)
            .map_err(|error| error.to_string())?;
        let mut bytes = b"40000 vacant\0".to_vec();
        for pair in empty.as_str().as_bytes().chunks_exact(2) {
            let hex = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
            bytes.push(u8::from_str_radix(hex, 16).map_err(|error| error.to_string())?);
        }
        let input = fixture.root.join("raw-tree");
        fs::write(&input, bytes).map_err(|error| error.to_string())?;
        let nested = GitObjectId::parse(&fixture.git(&["hash-object", "-t", "tree", "-w",
            input.to_str().ok_or("fixture object path invalid")?])?)
            .map_err(|error| error.to_string())?;
        for (name, tree, expected) in [
            ("empty", empty, vec![PathBuf::new()]),
            ("directory", nested, vec![PathBuf::new(), PathBuf::from("vacant")]),
        ] {
            let owner = fixture.anchor(name)?;
            let captured = fixture.capture(&tree, owner.clone(), budget())
                .map_err(|error| error.to_string())?;
            assert_eq!(captured.configuration, CapturedConfiguration::Absent);
            let authority = captured.frozen_source_authority(&fixture.repository)
                .map_err(|error| error.to_string())?;
            assert_eq!(authority.inventory().files().len(), 0);
            assert_eq!(inventory(&authority).1, expected);
            authority.finalize().map_err(|error| error.to_string())?;
            owner.verify_materialized()?;
        }
        Ok(())
    }

    #[test]
    fn inventory_observation_refuses_every_malformed_or_unsupported_record_before_io()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        let owner = fixture.anchor("inventory")?;
        let oid = "1111111111111111111111111111111111111111";
        let regular = format!("100644 blob {oid}\ta.rs\0");
        let cases: Vec<(Vec<u8>, &str)> = vec![
            (regular.trim_end_matches('\0').as_bytes().to_vec(), "NUL-terminated"),
            (format!("{regular}{regular}").into_bytes(), "duplicates path"),
            (b"\0".to_vec(), "empty record"),
            (format!("100644 blob extra {oid}\ta.rs\0").into_bytes(), "metadata is malformed"),
            (format!("100644 blob {oid} a.rs\0").into_bytes(), "no TAB"),
            (format!("100644 blob {oid}\t../outside.rs\0").into_bytes(), "path is malformed"),
            (format!("100644 blob {oid}\ta..b.rs\0").into_bytes(), "escapes"),
            (format!("120000 blob {oid}\tlink.rs\0").into_bytes(), "unsupported tree entry"),
            (format!("160000 commit {oid}\tmodule\0").into_bytes(), "unsupported tree entry"),
            (format!("040000 tree {oid}\tripr.toml\0").into_bytes(), "is a directory"),
            ([format!("100644 blob {oid}\t").as_bytes(), b"bad-\xff.rs\0"].concat(), "not UTF-8"),
        ];
        for (listing, expected) in cases {
            refused(records(&listing, &owner, budget()), expected)?;
            owner.verify_fresh()?;
            assert_eq!(fs::read_dir(owner.path_identifier())
                .map_err(|error| error.to_string())?.count(), 0);
        }
        let mut limited = budget();
        limited.max_entries = 1;
        refused(records(regular.as_bytes(), &owner, limited), "entry admission")?;
        limited = budget();
        limited.max_path_bytes = 1;
        refused(records(regular.as_bytes(), &owner, limited), "path admission")?;
        owner.verify_fresh()
    }

    #[test]
    fn actual_capture_caps_refuse_and_separate_fresh_source_recovers()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        fixture.write("ripr.toml", b"[analysis]\nmode = \"draft\"\n")?;
        fixture.write("src/lib.rs", b"pub fn sample() {}\n")?;
        let tree = fixture.tree()?;
        let mut variants = Vec::new();
        let mut value = budget(); value.max_listing_bytes = 1; variants.push((value, "git ls-tree"));
        value = budget(); value.max_entries = 1; variants.push((value, "entry admission"));
        value = budget(); value.max_path_bytes = 1; variants.push((value, "path admission"));
        value = budget(); value.max_source_bytes = 1; variants.push((value, "source total limit"));
        value = budget(); value.max_file_bytes = 1; variants.push((value, "file limit"));
        value = budget(); value.max_configuration_bytes = 1; variants.push((value, "configuration capture"));
        value = budget(); value.max_buffered_bytes = 1; variants.push((value, "buffered representation"));
        for (index, (limit, category)) in variants.into_iter().enumerate() {
            let failed_owner = fixture.anchor(&format!("failure-{index}"))?;
            refused(fixture.capture(&tree, failed_owner.clone(), limit), category)?;
            refused(failed_owner.verify_materialized(), "not sealed")?;
            assert!(Path::new(failed_owner.path_identifier()).exists());
            let fresh = fixture.anchor(&format!("recovery-{index}"))?;
            let captured = fixture.capture(&tree, fresh.clone(), budget())
                .map_err(|error| error.to_string())?;
            let authority = captured.frozen_source_authority(&fixture.repository)
                .map_err(|error| error.to_string())?;
            assert_eq!(authority.inventory().files().len(), 2);
            authority.finalize().map_err(|error| error.to_string())?;
            fresh.verify_materialized()?;
        }
        Ok(())
    }

    #[test]
    fn actual_invalid_configuration_missing_tree_and_reused_source_never_seal()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        fixture.write("src/lib.rs", b"pub fn sample() {}\n")?;
        let good = fixture.tree()?;
        fixture.write("ripr.toml", b"invalid-\xff")?;
        let bad = fixture.tree()?;
        let owner = fixture.anchor("invalid-config")?;
        refused(fixture.capture(&bad, owner.clone(), budget()), "not UTF-8")?;
        refused(owner.verify_materialized(), "not sealed")?;
        fs::remove_file(fixture.repository.join("ripr.toml"))
            .map_err(|error| error.to_string())?;
        fixture.write("ripr.toml/child", b"configuration directory")?;
        let configuration_directory = fixture.tree()?;
        let directory_owner = fixture.anchor("configuration-directory")?;
        refused(fixture.capture(&configuration_directory, directory_owner.clone(), budget()),
            "is a directory")?;
        directory_owner.verify_fresh()?;
        let absent = GitObjectId::parse("1111111111111111111111111111111111111111")
            .map_err(|error| error.to_string())?;
        let missing = fixture.anchor("missing-tree")?;
        refused(fixture.capture(&absent, missing.clone(), budget()), "candidate tree failed")?;
        refused(missing.verify_materialized(), "not sealed")?;
        let reused = fixture.anchor("reused")?;
        let captured = fixture.capture(&good, reused.clone(), budget())
            .map_err(|error| error.to_string())?;
        let authority = captured.frozen_source_authority(&fixture.repository)
            .map_err(|error| error.to_string())?;
        authority.finalize().map_err(|error| error.to_string())?;
        reused.verify_materialized()?;
        refused(prepare_staged_named_tree(Path::new("/no/repository"), good.clone(),
            reused.clone(), fixture.deadline, budget()), "already sealed")?;
        refused(reused.verify_materialized(), "unqualified")?;
        let fresh = fixture.anchor("fresh-recovery")?;
        refused(prepare_staged_named_tree(Path::new("/no/repository"), good.clone(),
            fresh.clone(), fixture.deadline + Duration::from_nanos(1), budget()), "deadline differs")?;
        fresh.verify_fresh()?;
        fixture.capture(&good, fresh.clone(), budget())
            .map_err(|error| error.to_string())?.frozen_source_authority(&fixture.repository)
            .map_err(|error| error.to_string())?.finalize().map_err(|error| error.to_string())?;
        fresh.verify_materialized()
    }
    #[test]
    fn same_inventory_grammar_preserves_ordinary_errors_and_synchronous_observation()
        -> Result<(), String> {
        let oid = "1111111111111111111111111111111111111111";
        let row = format!("100644 blob {oid}\ta.rs\0");
        // Literal old-grammar outcomes are independent of the new wrapper.
        let cases = [
            (Vec::new(), None),
            (row.as_bytes().to_vec(), None),
            (row.trim_end_matches('\0').as_bytes().to_vec(),
                Some("configuration inventory is not NUL-terminated")),
            (format!("{row}{row}").into_bytes(),
                Some("configuration inventory duplicates path a.rs")),
            (b"\0".to_vec(), Some("configuration inventory contains an empty record")),
            (format!("100644 blob {oid} extra\ta.rs\0").into_bytes(),
                Some("configuration inventory metadata is malformed")),
            (format!("100644 blob {oid}\t./a.rs\0").into_bytes(),
                Some("configuration inventory path is malformed")),
            (format!("040000 tree {oid}\tripr.toml\0").into_bytes(),
                Some("configuration inventory ripr.toml is a directory")),
            (format!("100644 blob {oid}\tripr.toml/a.rs\0").into_bytes(),
                Some("configuration inventory ripr.toml is a directory")),
        ];
        for (listing, expected) in cases {
            for result in [
                super::super::validate_configuration_inventory(&listing),
                super::super::validate_configuration_inventory_with(
                    &listing, |_, _, _, _| Ok(()),
                ),
            ] {
                match (result, expected) {
                    (Ok(()), None) => {}
                    (Err(SubjectError::ExecutionFailed { detail }), Some(expected)) =>
                        assert_eq!(detail, expected, "literal old grammar refusal changed"),
                    (other, expected) => return Err(format!(
                        "wrong literal inventory outcome {other:?}; expected {expected:?}")),
                }
            }
        }
        let mut observed = Vec::new();
        super::super::validate_configuration_inventory_with(row.as_bytes(),
            |mode, kind, object, path| {
                observed.push((mode, kind, object, path));
                Ok(())
            }).map_err(|error| error.to_string())?;
        assert_eq!(observed, vec![("100644", "blob", oid, "a.rs")]);
        let mut calls = 0;
        refused(super::super::validate_configuration_inventory_with(
            format!("{row}{row}").as_bytes(), |_, _, _, _| {
                calls += 1;
                Err(failed("real observer admission refused".into()))
            }), "real observer admission refused")?;
        assert_eq!(calls, 1, "fallible observation must stop immediately");
        Ok(())
    }

    #[test]
    fn original_plan_is_whole_and_lossless_from_nested_cwd_without_source_writes()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        fixture.write("src/lib.rs", b"pub fn entry() {}\n")?;
        fixture.write("src/tab\tname.rs", b"pub fn tabbed() {}\n")?;
        fixture.write("tests/test.py", b"def test_entry():\n    pass\n")?;
        fixture.write("web/index.ts", b"export const entry = 1;\n")?;
        let tree = fixture.tree()?;
        let expected = fixture.git(&[
            "ls-tree", "-r", "-t", "-z", "--full-tree", tree.as_str(),
        ])?.into_bytes();
        let mut views = Vec::new();
        for (name, repository) in [
            ("whole-plan", fixture.repository.clone()),
            ("nested-plan", fixture.repository.join("src")),
        ] {
            let owner = fixture.anchor(name)?;
            let view = with_staged_named_tree_plan(
                &repository, tree.clone(), owner.clone(), fixture.deadline, budget(),
                |plan| {
                    assert_eq!(plan.tree(), &tree);
                    Ok((
                        plan.original_listing().to_vec(),
                        plan.observations().map(|(path, mode, object)| (
                            path.to_string(), mode, object.to_string(),
                        )).collect::<Vec<_>>(),
                    ))
                },
            ).map_err(|error| error.to_string())?;
            assert_eq!(view.0, expected, "original NUL listing changed");
            assert_eq!(view.1.iter().map(|row| row.0.as_str()).collect::<Vec<_>>(),
                vec!["src", "src/lib.rs", "src/tab\tname.rs", "tests",
                    "tests/test.py", "web", "web/index.ts"]);
            owner.verify_fresh()?;
            assert_eq!(fs::read_dir(owner.path_identifier())
                .map_err(|error| error.to_string())?.count(), 0);
            views.push(view);
        }
        assert_eq!(views[0], views[1], "nested cwd narrowed or rewrote the plan");
        let empty = Fixture::new()?;
        let empty_tree = empty.tree()?;
        let owner = empty.anchor("empty-plan")?;
        with_staged_named_tree_plan(
            &empty.repository, empty_tree.clone(), owner.clone(), empty.deadline, budget(),
            |plan| {
                assert_eq!(plan.tree(), &empty_tree);
                assert_eq!(plan.original_listing(), b"");
                assert_eq!(plan.observations().len(), 0);
                Ok(())
            },
        ).map_err(|error| error.to_string())?;
        owner.verify_fresh()?;
        refused(owner.verify_materialized(), "not sealed")
    }

    #[test]
    fn real_prewrite_rejection_is_empty_and_noop_capture_retains_full_parity()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        fixture.write("src/lib.rs", b"pub fn entry() {}\n")?;
        fixture.write("src/helper.py", b"def helper():\n    return 1\n")?;
        fixture.write("web/index.ts", b"export const entry = 1;\n")?;
        fixture.write("ripr.toml", b"[analysis]\nmode = \"fast\"\n")?;
        let tree = fixture.tree()?;
        let rejected = fixture.anchor("rejected-plan")?;
        let mut calls = 0;
        refused(prepare_staged_named_tree_with_plan(
            &fixture.repository, tree.clone(), rejected.clone(), fixture.deadline, budget(),
            |plan| {
                calls += 1;
                assert_eq!(plan.tree(), &tree);
                assert_eq!(plan.observations().filter(|(_, mode, _)| mode.is_some()).count(), 4);
                Err(failed("authenticated parent plan differs".into()))
            },
        ), "authenticated parent plan differs")?;
        assert_eq!(calls, 1);
        rejected.verify_fresh()?;
        assert_eq!(fs::read_dir(rejected.path_identifier())
            .map_err(|error| error.to_string())?.count(), 0);
        let ordinary = super::super::prepare_named_tree(
            &fixture.repository, tree.as_str(), None,
        ).map_err(|error| error.to_string())?
            .frozen_source_authority(&fixture.repository)
            .map_err(|error| error.to_string())?;
        let old_owner = fixture.anchor("old-noop")?;
        let old = fixture.capture(&tree, old_owner.clone(), budget())
            .map_err(|error| error.to_string())?
            .frozen_source_authority(&fixture.repository)
            .map_err(|error| error.to_string())?;
        let new_owner = fixture.anchor("explicit-noop")?;
        let new = prepare_staged_named_tree_with_plan(
            &fixture.repository, tree.clone(), new_owner.clone(), fixture.deadline, budget(),
            |plan| {
                assert_eq!(plan.tree(), &tree);
                assert_eq!(plan.observations().filter(|(_, mode, _)| mode.is_some()).count(), 4);
                Ok(())
            },
        ).map_err(|error| error.to_string())?
            .frozen_source_authority(&fixture.repository)
            .map_err(|error| error.to_string())?;
        assert_eq!(inventory(&old), inventory(&new));
        assert_eq!(inventory(&ordinary), inventory(&new));
        assert_eq!(old.captured_configuration(), new.captured_configuration());
        assert_eq!(ordinary.captured_configuration(), new.captured_configuration());
        for (path, _, _, _, _) in inventory(&old).0 {
            let before = frozen::with_context(Some(ordinary.clone()), ||
                frozen_fs::read(fixture.repository.join(&path)))
                .map_err(|error| error.to_string())?;
            let after = frozen::with_context(Some(new.clone()), ||
                frozen_fs::read(fixture.repository.join(&path)))
                .map_err(|error| error.to_string())?;
            assert_eq!(before, after);
        }
        ordinary.finalize().map_err(|error| error.to_string())?;
        old.finalize().map_err(|error| error.to_string())?;
        new.finalize().map_err(|error| error.to_string())?;
        old_owner.verify_materialized()?;
        new_owner.verify_materialized()
    }

    #[test]
    fn full_plan_parser_rejects_corrupt_original_records_before_callback()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        fixture.write("src/lib.rs", b"pub fn entry() {}\n")?;
        let tree = fixture.tree()?;
        let captured = fixture.anchor("original-listing")?;
        let (raw, object) = with_staged_named_tree_plan(
            &fixture.repository, tree.clone(), captured.clone(), fixture.deadline, budget(),
            |plan| {
                let object = plan.observations().find(|(_, mode, _)| mode.is_some())
                    .ok_or_else(|| failed("fixture has no blob observation".into()))?.2;
                Ok((plan.original_listing().to_vec(), object.to_string()))
            },
        ).map_err(|error| error.to_string())?;
        captured.verify_fresh()?;
        let mut truncated = raw.clone();
        let _ = truncated.pop();
        let mut duplicate = raw.clone();
        duplicate.extend_from_slice(&raw);
        let mut extra_empty = raw.clone();
        extra_empty.push(0);
        let cases = [
            (truncated, "NUL-terminated"),
            (duplicate, "duplicates path"),
            (extra_empty, "empty record"),
            (format!("100644 blob {object}\tparent.rs\0\
                100644 blob {object}\tparent.rs/child.rs\0").into_bytes(), "overlaps a file"),
            (format!("100644 blob {object}\tmissing/child.rs\0").into_bytes(),
                "parent directory is missing"),
            (format!("120000 blob {object}\tlink.rs\0").into_bytes(), "unsupported tree entry"),
            (format!("040000 tree {}\ta\0\
                040000 tree {}\ta/\0", tree.as_str(), tree.as_str()).into_bytes(),
                "plan path is not canonical"),
            (format!("040000 tree {}\ta\0\
                100644 blob {object}\ta/.\0", tree.as_str()).into_bytes(),
                "plan path is not canonical"),
            (format!("040000 tree {}\ta\0\
                100644 blob {object}\ta/./child.rs\0", tree.as_str()).into_bytes(),
                "plan path is not canonical"),
            (format!("040000 tree {}\ta\0\
                100644 blob {object}\ta//child.rs\0", tree.as_str()).into_bytes(),
                "plan path is not canonical"),
            (format!("100644 blob {object}\ttrailing.rs/\0").into_bytes(),
                "plan path is not canonical"),
        ];
        for (index, (listing, category)) in cases.into_iter().enumerate() {
            let owner = fixture.anchor(&format!("corrupt-plan-{index}"))?;
            let mut calls = 0;
            let result = validated_records(&listing, &owner, fixture.deadline, budget())
                .and_then(|records| observe_plan(
                    &tree, &listing, &records, &owner, fixture.deadline, |_| {
                        calls += 1;
                        Ok(())
                    },
                ));
            refused(result, category)?;
            assert_eq!(calls, 0, "partial or corrupt plan reached callback");
            owner.verify_fresh()?;
            assert_eq!(fs::read_dir(owner.path_identifier())
                .map_err(|error| error.to_string())?.count(), 0);
        }
        // An actual Git tree with a symlink-mode entry must also refuse
        // before the external observer; live worktree file type is irrelevant.
        let cache = format!("120000,{object},link.rs");
        fixture.git(&["update-index", "--add", "--cacheinfo", &cache])?;
        let unsupported = GitObjectId::parse(&fixture.git(&["write-tree"])?)
            .map_err(|error| error.to_string())?;
        let owner = fixture.anchor("actual-unsupported-plan")?;
        let mut calls = 0;
        refused(with_staged_named_tree_plan(
            &fixture.repository, unsupported, owner.clone(), fixture.deadline, budget(),
            |_| { calls += 1; Ok(()) },
        ), "unsupported tree entry")?;
        assert_eq!(calls, 0);
        owner.verify_fresh()?;
        assert_eq!(fs::read_dir(owner.path_identifier())
            .map_err(|error| error.to_string())?.count(), 0);
        Ok(())
    }

    #[test]
    fn real_plan_ingress_caps_refuse_before_callback_or_source_write()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        fixture.write("src/lib.rs", b"pub fn entry() {}\n")?;
        let tree = fixture.tree()?;
        let mut variants = Vec::new();
        let mut value = budget();
        value.max_listing_bytes = 1; variants.push((value, "git ls-tree"));
        value = budget();
        value.max_entries = 1; variants.push((value, "entry admission"));
        value = budget();
        value.max_path_bytes = 1; variants.push((value, "path admission"));
        value = budget();
        value.max_buffered_bytes = 1; variants.push((value, "buffered representation"));
        for (index, (limit, category)) in variants.into_iter().enumerate() {
            let owner = fixture.anchor(&format!("plan-cap-{index}"))?;
            let mut calls = 0;
            refused(with_staged_named_tree_plan(
                &fixture.repository, tree.clone(), owner.clone(), fixture.deadline, limit,
                |_| { calls += 1; Ok(()) },
            ), category)?;
            assert_eq!(calls, 0);
            owner.verify_fresh()?;
            assert_eq!(fs::read_dir(owner.path_identifier())
                .map_err(|error| error.to_string())?.count(), 0);
        }
        Ok(())
    }

    #[test]
    fn listing_plan_never_substitutes_for_source_file_or_configuration_byte_limits()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        fixture.write("src/lib.rs", b"pub fn entry() {}\n")?;
        fixture.write("ripr.toml", b"[analysis]\nmode = \"draft\"\n")?;
        let tree = fixture.tree()?;
        let mut variants = Vec::new();
        let mut value = budget();
        value.max_source_bytes = 1; variants.push((value, "source total limit"));
        value = budget();
        value.max_file_bytes = 1; variants.push((value, "file limit"));
        value = budget();
        value.max_configuration_bytes = 1; variants.push((value, "configuration capture"));
        for (index, (limit, category)) in variants.into_iter().enumerate() {
            let plan_owner = fixture.anchor(&format!("metadata-only-{index}"))?;
            with_staged_named_tree_plan(
                &fixture.repository, tree.clone(), plan_owner.clone(), fixture.deadline, limit,
                |plan| {
                    assert_eq!(plan.observations().filter(|(_, mode, _)| mode.is_some()).count(), 2);
                    Ok(())
                },
            ).map_err(|error| error.to_string())?;
            plan_owner.verify_fresh()?;
            let write_owner = fixture.anchor(&format!("byte-refusal-{index}"))?;
            refused(prepare_staged_named_tree_with_plan(
                &fixture.repository, tree.clone(), write_owner.clone(), fixture.deadline, limit,
                |_| Ok(()),
            ), category)?;
            refused(write_owner.verify_materialized(), "not sealed")?;
        }
        Ok(())
    }

    #[test]
    fn callback_source_mutation_and_failure_never_reach_materialization_or_sealing()
        -> Result<(), String> {
        let fixture = Fixture::new()?;
        fixture.write("src/lib.rs", b"pub fn entry() {}\n")?;
        let tree = fixture.tree()?;
        for (index, callback_fails) in [false, true].into_iter().enumerate() {
            let owner = fixture.anchor(&format!("callback-mutation-{index}"))?;
            let result = prepare_staged_named_tree_with_plan(
                &fixture.repository, tree.clone(), owner.clone(), fixture.deadline, budget(),
                |_| {
                    owner.create_dir(Path::new("callback-created")).map_err(failed)?;
                    if callback_fails {
                        Err(failed("callback primary refusal".into()))
                    } else {
                        Ok(())
                    }
                },
            );
            match result {
                Err(error) => {
                    let detail = error.to_string();
                    if callback_fails && !detail.contains("callback primary refusal") {
                        return Err(format!("primary callback error disappeared: {detail}"));
                    }
                    if !detail.contains("staged tree fresh source refused") {
                        return Err(format!("source mutation postflight disappeared: {detail}"));
                    }
                }
                Ok(_) => return Err("callback source mutation reached materialization".into()),
            }
            assert_eq!(fs::read_dir(owner.path_identifier())
                .map_err(|error| error.to_string())?.count(), 1);
            assert!(fs::metadata(Path::new(owner.path_identifier()).join("callback-created"))
                .map_err(|error| error.to_string())?.is_dir(),
                "callback-created fixture directory disappeared");
            refused(owner.verify_materialized(), "unqualified")?;
        }
        Ok(())
    }

    #[test]
    fn actual_clock_expiry_during_callback_cannot_reset_the_materialization_deadline()
        -> Result<(), String> {
        let mut fixture = Fixture::new()?;
        fixture.write("src/lib.rs", b"pub fn entry() {}\n")?;
        let tree = fixture.tree()?;
        let original = fixture.anchor("deadline-listing")?;
        let raw = with_staged_named_tree_plan(
            &fixture.repository, tree.clone(), original.clone(), fixture.deadline, budget(),
            |plan| Ok(plan.original_listing().to_vec()),
        ).map_err(|error| error.to_string())?;
        original.verify_fresh()?;
        fixture.deadline = Instant::now() + Duration::from_secs(1);
        let owner = fixture.anchor("deadline-callback")?;
        let records = validated_records(&raw, &owner, fixture.deadline, budget())
            .map_err(|error| error.to_string())?;
        let mut calls = 0;
        refused(observe_plan(
            &tree, &raw, &records, &owner, fixture.deadline, |_| {
                calls += 1;
                std::thread::sleep(fixture.deadline.saturating_duration_since(Instant::now())
                    + Duration::from_millis(1));
                Ok(())
            },
        ), "held worker deadline exhausted")?;
        assert_eq!(calls, 1, "actual callback clock control did not run");
        assert_eq!(fs::read_dir(owner.path_identifier())
            .map_err(|error| error.to_string())?.count(), 0);
        Ok(())
    }

}
