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
pub(crate) fn prepare_staged_named_tree(
    repository: &Path,
    tree: GitObjectId,
    owner: Arc<SourceAnchor>,
    deadline: Instant,
    budget: CompleteTreeBudget,
) -> Result<PreparedStagedTree, SubjectError> {
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
    let records = records(&listing.stdout, &owner, budget)?;
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

}
