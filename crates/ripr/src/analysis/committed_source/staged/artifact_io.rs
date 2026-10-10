//! Inactive descriptor-backed writes for a closed complete-artifact directory.
//!
//! Receipts describe observed file bytes and identities, not analyzer, role,
//! cleanup or publication authority. Root must lend the genuine artifact role,
//! retain its invocation owner, admit upstream serialization/buffers and whole
//! stage allocation, and enforce one consuming emitter across instances.
//! Checks around filesystem calls are cooperative, not syscall preemption.

use super::{RetainedDirectory, check_deadline, descriptor_path, file_metadata, object_identity};
use sha2::{Digest, Sha256};
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use std::fs::OpenOptions;
use std::fs::{self, File, Metadata};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

const FILE_MAX: u64 = 256 * 1024 * 1024;
const CHUNK: usize = 64 * 1024;
const MANIFEST: &str = "complete-manifest.json";
const MANIFEST_SLOT: usize = 9;
const SLOTS: usize = 10;

/// Closed payload names. Root separately checks correspondence with its
/// private ArtifactRole contract; this enum does not grant an artifact role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArtifactSlot {
    OriginalRaw,
    CheckDiff,
    PresentationDiff,
    RawLedger,
    FullCheck,
    FindingIndex,
    ReviewInput,
    PrJson,
    PrMarkdown,
}

impl ArtifactSlot {
    pub(crate) const ALL: [Self; 9] = [
        Self::OriginalRaw,
        Self::CheckDiff,
        Self::PresentationDiff,
        Self::RawLedger,
        Self::FullCheck,
        Self::FindingIndex,
        Self::ReviewInput,
        Self::PrJson,
        Self::PrMarkdown,
    ];

    pub(crate) const fn path(self) -> &'static str {
        match self {
            Self::OriginalRaw => "canonical-u0.raw",
            Self::CheckDiff => "check.diff",
            Self::PresentationDiff => "pr.diff",
            Self::RawLedger => "raw-ledger.jsonl",
            Self::FullCheck => "check.json",
            Self::FindingIndex => "finding-index.json",
            Self::ReviewInput => "review-input.json",
            Self::PrJson => "repo-exposure.json",
            Self::PrMarkdown => "repo-exposure.md",
        }
    }

    pub(crate) const fn ordinal(self) -> usize {
        match self {
            Self::OriginalRaw => 0,
            Self::CheckDiff => 1,
            Self::PresentationDiff => 2,
            Self::RawLedger => 3,
            Self::FullCheck => 4,
            Self::FindingIndex => 5,
            Self::ReviewInput => 6,
            Self::PrJson => 7,
            Self::PrMarkdown => 8,
        }
    }
}

fn name(index: usize) -> &'static str {
    if index == MANIFEST_SLOT {
        MANIFEST
    } else {
        ArtifactSlot::ALL[index].path()
    }
}

/// Caller-admitted scalar DATA. The aggregate applies to nine payloads only;
/// the manifest has a separate cap. No defaults, environment or policy parser.
/// Root must validate the SAME profile, usize conversions and consumer caps
/// (including index/review), plus all upstream serialization/buffer admissions.
pub(crate) struct ArtifactBudget {
    caps: [u64; 9],
    payload_total: u64,
    manifest: u64,
    file_size: u64,
}

impl ArtifactBudget {
    pub(crate) fn new(
        caps: [u64; 9],
        payload_total: u64,
        manifest: u64,
        file_size: u64,
    ) -> Result<Self, String> {
        if file_size == 0
            || file_size > FILE_MAX
            || payload_total == 0
            || manifest == 0
            || manifest > file_size
            || caps.iter().any(|cap| *cap == 0 || *cap > file_size)
        {
            return Err("artifact budget has an invalid admitted scalar bound".into());
        }
        Ok(Self {
            caps,
            payload_total,
            manifest,
            file_size,
        })
    }
}

/// Fixed-size observed DATA. It is not an authenticated generation descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArtifactFileData {
    name: &'static str,
    bytes: u64,
    sha256: [u8; 32],
    dev: u64,
    ino: u64,
    allocated_bytes: u64,
}

impl ArtifactFileData {
    pub(crate) fn name(&self) -> &'static str {
        self.name
    }
    pub(crate) fn bytes(&self) -> u64 {
        self.bytes
    }
    pub(crate) fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    pub(crate) fn identity(&self) -> (u64, u64) {
        (self.dev, self.ino)
    }
    /// Observed file blocks times 512; excludes inode and directory metadata.
    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ArtifactPayloadData {
    files: [ArtifactFileData; 9],
    payload_bytes: u64,
    // Checked payload bytes plus manifest cap, for parent accounting only.
    reserved_composite_bytes: u64,
}

impl ArtifactPayloadData {
    pub(crate) fn files(&self) -> &[ArtifactFileData; 9] {
        &self.files
    }
    pub(crate) fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }
    pub(crate) fn reserved_composite_bytes(&self) -> u64 {
        self.reserved_composite_bytes
    }
}

pub(crate) struct ArtifactClosureData {
    payloads: ArtifactPayloadData,
    manifest: ArtifactFileData,
    composite_bytes: u64,
    allocated_bytes: u64,
}

impl ArtifactClosureData {
    pub(crate) fn payloads(&self) -> &ArtifactPayloadData {
        &self.payloads
    }
    pub(crate) fn manifest(&self) -> &ArtifactFileData {
        &self.manifest
    }
    pub(crate) fn composite_bytes(&self) -> u64 {
        self.composite_bytes
    }
    /// Checked sum of ten regular-file block observations only. Excludes the
    /// role directory and other stage metadata; no prewrite quota or stage grant.
    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes
    }
}

/// Borrowed descriptor and payloads: no retained whole-file copies. Root owns
/// the role/custody and all caller buffers, and forbids another emitter instance.
/// Constructor failure creates no writer; root also retains that failed attempt.
pub(crate) struct ArtifactDirectory<'d, 'p> {
    directory: &'d RetainedDirectory,
    payloads: [&'p [u8]; 9],
    budget: ArtifactBudget,
    deadline: Instant,
    records: [Option<ArtifactFileData>; SLOTS],
    started: [bool; SLOTS],
    fault: Option<String>,
    payload_bytes: u64,
    reserved_composite_bytes: u64,
}

/// Consuming transition after nine actual byte/identity/closure observations.
/// DATA is exposed for manifest construction; only write_manifest remains.
pub(crate) struct FinishedPayloads<'d, 'p> {
    writer: ArtifactDirectory<'d, 'p>,
    data: ArtifactPayloadData,
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn create_private(path: &Path) -> Result<File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(super::NOFOLLOW | super::NONBLOCK)
        .open(path)
        .map_err(|error| format!("artifact create-new failed: {error}"))
}

#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn create_private(_: &Path) -> Result<File, String> {
    Err("artifact descriptor I/O requires qualified Linux".into())
}

#[cfg(target_os = "linux")]
fn properties(metadata: &Metadata) -> Result<(u64, u64, u64, u32, u64), String> {
    use std::os::unix::fs::MetadataExt;
    let (dev, ino) = object_identity(metadata)?;
    let allocated = metadata
        .blocks()
        .checked_mul(512)
        .ok_or("artifact allocated-byte observation overflow")?;
    Ok((
        dev,
        ino,
        metadata.nlink(),
        metadata.mode() & 0o7777,
        allocated,
    ))
}

#[cfg(not(target_os = "linux"))]
fn properties(_: &Metadata) -> Result<(u64, u64, u64, u32, u64), String> {
    Err("artifact descriptor identity requires qualified Linux".into())
}

fn admit_payloads(payloads: &[&[u8]; 9], budget: &ArtifactBudget) -> Result<(u64, u64), String> {
    let mut total = 0_u64;
    for (bytes, cap) in payloads.iter().zip(budget.caps) {
        let length = u64::try_from(bytes.len())
            .map_err(|error| format!("artifact payload length conversion: {error}"))?;
        if length > cap || length > budget.file_size {
            return Err("artifact payload exceeds its admitted cap before creation".into());
        }
        total = total
            .checked_add(length)
            .ok_or("artifact payload sum overflow")?;
        if total > budget.payload_total {
            return Err(
                "artifact nine-payload aggregate exceeds its admitted cap before creation".into(),
            );
        }
    }
    let composite = total
        .checked_add(budget.manifest)
        .ok_or("artifact composite accounting overflow")?;
    Ok((total, composite))
}

impl<'d, 'p> ArtifactDirectory<'d, 'p> {
    pub(crate) fn new(
        directory: &'d RetainedDirectory,
        payloads: [&'p [u8]; 9],
        budget: ArtifactBudget,
        deadline: Instant,
    ) -> Result<Self, String> {
        check_deadline(deadline)?;
        let (payload_bytes, reserved_composite_bytes) = admit_payloads(&payloads, &budget)?;
        directory.require_empty(deadline)?;
        check_deadline(deadline)?;
        Ok(Self {
            directory,
            payloads,
            budget,
            deadline,
            records: [None; SLOTS],
            started: [false; SLOTS],
            fault: None,
            payload_bytes,
            reserved_composite_bytes,
        })
    }

    fn ready(&self) -> Result<(), String> {
        if let Some(error) = &self.fault {
            return Err(error.clone());
        }
        self.directory.verify_current(self.deadline)?;
        check_deadline(self.deadline)
    }

    fn remember<T>(&mut self, result: Result<T, String>) -> Result<T, String> {
        if let Err(error) = &result
            && self.fault.is_none()
        {
            self.fault = Some(error.clone());
        }
        result
    }

    fn named_file(&self, index: usize) -> Result<File, String> {
        self.ready()?;
        let file = super::open_no_follow(
            &descriptor_path(&self.directory.file)?.join(name(index)),
            false,
            false,
        )?;
        check_deadline(self.deadline)?;
        Ok(file)
    }

    fn validate_file(
        &self,
        file: &File,
        index: usize,
        identity: (u64, u64),
        bytes: u64,
    ) -> Result<Metadata, String> {
        self.ready()?;
        let metadata = file_metadata(file, false)?;
        let (dev, ino, links, mode, _) = properties(&metadata)?;
        if (dev, ino) != identity || links != 1 || mode != 0o600 || metadata.len() != bytes {
            return Err("artifact held file identity/link/mode/length differs".into());
        }
        let current = self.named_file(index)?;
        let current_metadata = file_metadata(&current, false)?;
        let (current_dev, current_ino, links, mode, _) = properties(&current_metadata)?;
        if (current_dev, current_ino) != identity
            || links != 1
            || mode != 0o600
            || current_metadata.len() != bytes
        {
            return Err("artifact name no longer identifies its held file".into());
        }
        self.ready()?;
        Ok(metadata)
    }

    fn read_data(
        &self,
        index: usize,
        identity: (u64, u64),
        bytes: u64,
        expected: [u8; 32],
    ) -> Result<ArtifactFileData, String> {
        let mut file = self.named_file(index)?;
        self.validate_file(&file, index, identity, bytes)?;
        let mut digest = Sha256::new();
        let mut count = 0_u64;
        let mut scratch = [0_u8; CHUNK];
        loop {
            self.ready()?;
            let read = file
                .read(&mut scratch)
                .map_err(|error| format!("artifact full-EOF read failed: {error}"))?;
            self.validate_file(&file, index, identity, bytes)?;
            if read == 0 {
                break;
            }
            count = count
                .checked_add(
                    u64::try_from(read)
                        .map_err(|error| format!("artifact read count conversion: {error}"))?,
                )
                .ok_or("artifact read count overflow")?;
            if count > bytes {
                return Err("artifact full-EOF read exceeded declared bytes".into());
            }
            digest.update(&scratch[..read]);
        }
        let metadata = self.validate_file(&file, index, identity, bytes)?;
        let actual: [u8; 32] = digest.finalize().into();
        if count != bytes || actual != expected {
            return Err("artifact full-EOF bytes/hash differ".into());
        }
        let (dev, ino, _, _, allocated_bytes) = properties(&metadata)?;
        self.ready()?;
        Ok(ArtifactFileData {
            name: name(index),
            bytes,
            sha256: actual,
            dev,
            ino,
            allocated_bytes,
        })
    }

    fn write_bytes(&mut self, index: usize, bytes: &[u8]) -> Result<ArtifactFileData, String> {
        self.ready()?;
        if self.started[index] {
            return Err("artifact slot duplicate/reuse refused".into());
        }
        // Admission preceded every create; keep this once-only reservation on failure.
        self.started[index] = true;
        let mut file = create_private(&descriptor_path(&self.directory.file)?.join(name(index)))?;
        let identity = object_identity(&file_metadata(&file, false)?)?;
        self.validate_file(&file, index, identity, 0)?;
        let mut digest = Sha256::new();
        let mut written = 0_u64;
        for chunk in bytes.chunks(CHUNK) {
            let mut offset = 0_usize;
            while offset < chunk.len() {
                self.validate_file(&file, index, identity, written)?;
                let attempt = file.write(&chunk[offset..]);
                let observed = self.ready();
                let count = match (attempt, observed) {
                    (Err(error), Err(observed)) => {
                        return Err(format!(
                            "artifact chunk write failed: {error}; post-write observation: {observed}"
                        ));
                    }
                    (Err(error), Ok(())) if error.kind() == std::io::ErrorKind::Interrupted => {
                        continue;
                    }
                    (Err(error), Ok(())) => {
                        return Err(format!("artifact chunk write failed: {error}"));
                    }
                    (Ok(_), Err(observed)) => return Err(observed),
                    (Ok(0), Ok(())) => return Err("artifact chunk write made zero progress".into()),
                    (Ok(count), Ok(())) => count,
                };
                if count > chunk.len() - offset {
                    return Err("artifact chunk write returned an invalid count".into());
                }
                let end = offset
                    .checked_add(count)
                    .ok_or("artifact chunk offset overflow")?;
                written = written
                    .checked_add(
                        u64::try_from(count)
                            .map_err(|error| format!("artifact write count conversion: {error}"))?,
                    )
                    .ok_or("artifact written count overflow")?;
                self.validate_file(&file, index, identity, written)?;
                digest.update(&chunk[offset..end]);
                offset = end;
            }
        }
        file.sync_all()
            .map_err(|error| format!("artifact file sync failed: {error}"))?;
        self.validate_file(&file, index, identity, written)?;
        let expected: [u8; 32] = digest.finalize().into();
        drop(file);
        self.read_data(index, identity, written, expected)
    }

    pub(crate) fn write_slot(&mut self, slot: ArtifactSlot) -> Result<(), String> {
        let index = slot.ordinal();
        let bytes = self.payloads[index];
        let result = self
            .write_bytes(index, bytes)
            .map(|data| self.records[index] = Some(data));
        self.remember(result)
    }

    fn membership(&self, expected_count: usize) -> Result<(), String> {
        self.ready()?;
        let mut seen = [false; SLOTS];
        let mut count = 0_usize;
        let mut entries = fs::read_dir(descriptor_path(&self.directory.file)?)
            .map_err(|error| format!("artifact directory listing failed: {error}"))?;
        for entry in entries.by_ref() {
            self.ready()?;
            count = count.checked_add(1).ok_or("artifact name count overflow")?;
            if count > expected_count {
                return Err("artifact closure has an extra entry".into());
            }
            let entry =
                entry.map_err(|error| format!("artifact directory entry failed: {error}"))?;
            let native = entry.file_name();
            let index = (0..expected_count)
                .find(|index| native == std::ffi::OsStr::new(name(*index)))
                .ok_or("artifact closure has an unknown native name")?;
            if seen[index] {
                return Err("artifact closure has a duplicate name".into());
            }
            seen[index] = true;
        }
        drop(entries);
        if count != expected_count || seen[..expected_count].iter().any(|seen| !seen) {
            return Err("artifact closure is missing a required name".into());
        }
        self.ready()
    }

    fn closure(&self, expected_count: usize) -> Result<u64, String> {
        self.membership(expected_count)?;
        let mut allocated = 0_u64;
        for index in 0..expected_count {
            let data = self.records[index].ok_or("artifact closure lacks completed file DATA")?;
            let observed = self.read_data(index, data.identity(), data.bytes, data.sha256)?;
            if observed != data {
                return Err("artifact closure metadata changed".into());
            }
            allocated = allocated
                .checked_add(observed.allocated_bytes)
                .ok_or("artifact allocated accounting overflow")?;
        }
        self.directory
            .file
            .sync_all()
            .map_err(|error| format!("artifact directory sync failed: {error}"))?;
        // Reobserve the same finite namespace after the longer hash/sync pass.
        // This is cooperative currentness, not atomic exclusion of other writers.
        self.membership(expected_count)?;
        self.ready()?;
        Ok(allocated)
    }

    pub(crate) fn finish_payloads(mut self) -> Result<FinishedPayloads<'d, 'p>, String> {
        let result = (|| {
            self.ready()?;
            let [
                Some(a),
                Some(b),
                Some(c),
                Some(d),
                Some(e),
                Some(f),
                Some(g),
                Some(h),
                Some(i),
                None,
            ] = self.records
            else {
                return Err("artifact nine payloads are not all finished".into());
            };
            self.closure(MANIFEST_SLOT)?;
            Ok(ArtifactPayloadData {
                files: [a, b, c, d, e, f, g, h, i],
                payload_bytes: self.payload_bytes,
                reserved_composite_bytes: self.reserved_composite_bytes,
            })
        })();
        let data = self.remember(result)?;
        Ok(FinishedPayloads { writer: self, data })
    }
}

impl FinishedPayloads<'_, '_> {
    pub(crate) fn receipts(&self) -> &ArtifactPayloadData {
        &self.data
    }

    pub(crate) fn write_manifest(mut self, bytes: &[u8]) -> Result<ArtifactClosureData, String> {
        let result = (|| {
            self.writer.ready()?;
            let size = u64::try_from(bytes.len())
                .map_err(|error| format!("artifact manifest length conversion: {error}"))?;
            if size > self.writer.budget.manifest || size > self.writer.budget.file_size {
                return Err(
                    "artifact manifest exceeds its separate admitted cap before creation".into(),
                );
            }
            // Reobserve all nine after root's fallible manifest construction.
            self.writer.closure(MANIFEST_SLOT)?;
            let manifest = self.writer.write_bytes(MANIFEST_SLOT, bytes)?;
            self.writer.records[MANIFEST_SLOT] = Some(manifest);
            let allocated_bytes = self.writer.closure(SLOTS)?;
            let composite_bytes = self
                .data
                .payload_bytes
                .checked_add(size)
                .ok_or("artifact final composite accounting overflow")?;
            self.writer.ready()?;
            Ok(ArtifactClosureData {
                payloads: self.data,
                manifest,
                composite_bytes,
                allocated_bytes,
            })
        })();
        self.writer.remember(result)
    }
}

#[cfg(all(
    test,
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod tests {
    use super::super::DirectoryIdentity;
    use super::*;
    use std::os::unix::fs::{MetadataExt, symlink};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        directory: RetainedDirectory,
        deadline: Instant,
    }
    impl Fixture {
        fn new() -> Result<Self, String> {
            let base = std::env::temp_dir()
                .canonicalize()
                .map_err(|error| format!("artifact fixture root: {error}"))?;
            let root = base.join(format!(
                "ripr-artifact-io-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root)
                .map_err(|error| format!("artifact fixture directory: {error}"))?;
            let path = root.join("artifacts");
            fs::create_dir(&path).map_err(|error| format!("artifact fixture role: {error}"))?;
            let metadata =
                fs::metadata(&path).map_err(|error| format!("artifact fixture stat: {error}"))?;
            let identity = DirectoryIdentity {
                path: path
                    .to_str()
                    .ok_or("artifact fixture path UTF-8")?
                    .to_string(),
                dev: metadata.dev(),
                ino: metadata.ino(),
            };
            let deadline = Instant::now()
                .checked_add(Duration::from_mins(1))
                .ok_or("artifact fixture deadline overflow")?;
            let directory = RetainedDirectory::open_absolute(&identity, 4096, deadline)?;
            Ok(Self {
                root,
                directory,
                deadline,
            })
        }
        fn path(&self, slot: ArtifactSlot) -> PathBuf {
            self.root.join("artifacts").join(slot.path())
        }
        fn writer<'a>(
            &'a self,
            bytes: [&'a [u8]; 9],
        ) -> Result<super::super::ArtifactDirectory<'a, 'a>, String> {
            super::super::ArtifactDirectory::new(
                &self.directory,
                bytes,
                super::super::ArtifactBudget::new([256 * 1024; 9], 9 * 256 * 1024, 64, 256 * 1024)?,
                self.deadline,
            )
        }
        fn cleanup(self) -> Result<(), String> {
            drop(self.directory);
            fs::remove_dir_all(self.root)
                .map_err(|error| format!("artifact fixture cleanup: {error}"))
        }
    }

    fn error<T>(result: Result<T, String>, category: &str) -> Result<String, String> {
        match result {
            Err(error) if error.contains(category) => Ok(error),
            Err(error) => Err(format!(
                "wrong artifact refusal: {error}; expected {category}"
            )),
            Ok(_) => Err(format!("unexpected artifact success; expected {category}")),
        }
    }
    fn write_all(writer: &mut ArtifactDirectory<'_, '_>) -> Result<(), String> {
        for slot in super::super::ArtifactSlot::ALL {
            writer.write_slot(slot)?;
        }
        Ok(())
    }

    #[test]
    fn real_nine_payloads_and_last_manifest_have_full_bytes_identity_and_sync() -> Result<(), String>
    {
        let fixture = Fixture::new()?;
        let large = vec![0xa5_u8; CHUNK + 7];
        let mut payloads = [b"payload".as_slice(); 9];
        payloads[0] = &large;
        payloads[1] = b"";
        let mut writer = fixture.writer(payloads)?;
        write_all(&mut writer)?;
        let finished: super::super::FinishedPayloads<'_, '_> = writer.finish_payloads()?;
        let receipts: &super::super::ArtifactPayloadData = finished.receipts();
        assert_eq!(receipts.files().len(), 9);
        assert_eq!(receipts.payload_bytes(), (large.len() + 7 * 7) as u64);
        assert_eq!(
            receipts.reserved_composite_bytes(),
            receipts.payload_bytes() + 64
        );
        for (slot, data) in ArtifactSlot::ALL.into_iter().zip(receipts.files()) {
            let data: &super::super::ArtifactFileData = data;
            assert_eq!(data.name(), slot.path());
            let content = fs::read(fixture.path(slot))
                .map_err(|error| format!("artifact fixture bytes: {error}"))?;
            assert_eq!(content, payloads[slot.ordinal()]);
            let expected: [u8; 32] = Sha256::digest(&content).into();
            assert_eq!(data.sha256(), &expected);
            let metadata = fs::metadata(fixture.path(slot))
                .map_err(|error| format!("artifact fixture final stat: {error}"))?;
            assert_eq!(metadata.mode() & 0o7777, 0o600);
            assert_eq!(data.identity(), (metadata.dev(), metadata.ino()));
            assert_eq!(data.bytes(), metadata.len());
            assert_eq!(data.allocated_bytes(), metadata.blocks() * 512);
        }
        let data: super::super::ArtifactClosureData = finished.write_manifest(b"manifest")?;
        assert_eq!(data.manifest().name(), MANIFEST);
        assert_eq!(data.manifest().bytes(), 8);
        assert_eq!(data.composite_bytes(), data.payloads().payload_bytes() + 8);
        assert!(data.allocated_bytes() >= data.manifest().allocated_bytes());
        assert_eq!(
            fs::read(fixture.root.join("artifacts").join(MANIFEST))
                .map_err(|error| format!("manifest fixture read: {error}"))?,
            b"manifest"
        );
        fixture.cleanup()
    }

    #[test]
    fn admission_refuses_before_creation_and_manifest_is_outside_nine_aggregate()
    -> Result<(), String> {
        let fixture = Fixture::new()?;
        error(
            ArtifactDirectory::new(
                &fixture.directory,
                [b"xx"; 9],
                ArtifactBudget::new([1; 9], 9, 2, 2)?,
                fixture.deadline,
            ),
            "payload exceeds",
        )?;
        fixture.directory.require_empty(fixture.deadline)?;
        error(
            ArtifactDirectory::new(
                &fixture.directory,
                [b"x"; 9],
                ArtifactBudget::new([1; 9], 8, 2, 2)?,
                fixture.deadline,
            ),
            "nine-payload aggregate",
        )?;
        fixture.directory.require_empty(fixture.deadline)?;
        let mut writer = ArtifactDirectory::new(
            &fixture.directory,
            [b"x"; 9],
            ArtifactBudget::new([1; 9], 9, 2, 2)?,
            fixture.deadline,
        )?;
        write_all(&mut writer)?;
        let finished = writer.finish_payloads()?;
        assert_eq!(finished.receipts().reserved_composite_bytes(), 11);
        let data = finished.write_manifest(b"ok")?;
        assert_eq!(data.composite_bytes(), 11);
        fixture.cleanup()
    }

    #[test]
    fn duplicate_fault_stays_sticky_and_keeps_original_reservation() -> Result<(), String> {
        let fixture = Fixture::new()?;
        let mut writer = fixture.writer([b"payload"; 9])?;
        writer.write_slot(ArtifactSlot::OriginalRaw)?;
        let first = error(
            writer.write_slot(ArtifactSlot::OriginalRaw),
            "duplicate/reuse",
        )?;
        assert_eq!(
            error(
                writer.write_slot(ArtifactSlot::CheckDiff),
                "duplicate/reuse"
            )?,
            first
        );
        assert!(!fixture.path(ArtifactSlot::CheckDiff).exists());
        assert_eq!(error(writer.finish_payloads(), "duplicate/reuse")?, first);
        assert!(fixture.path(ArtifactSlot::OriginalRaw).is_file());
        fixture.cleanup()
    }

    #[test]
    fn missing_and_extra_names_cannot_close_payloads_and_new_stage_recovers() -> Result<(), String>
    {
        for missing in [true, false] {
            let fixture = Fixture::new()?;
            let mut writer = fixture.writer([b"payload"; 9])?;
            write_all(&mut writer)?;
            if missing {
                fs::remove_file(fixture.path(ArtifactSlot::FullCheck))
                    .map_err(|error| format!("missing artifact fixture: {error}"))?;
                error(writer.finish_payloads(), "missing")?;
            } else {
                fs::write(fixture.root.join("artifacts").join("unexpected"), b"x")
                    .map_err(|error| format!("extra artifact fixture: {error}"))?;
                let failure = error(writer.finish_payloads(), "artifact closure")?;
                if !failure.contains("extra") && !failure.contains("unknown") {
                    return Err(format!("extra fixture reached wrong refusal: {failure}"));
                }
            }
            fixture.cleanup()?;
        }
        let fixture = Fixture::new()?;
        let mut writer = fixture.writer([b"payload"; 9])?;
        write_all(&mut writer)?;
        writer.finish_payloads()?.write_manifest(b"recovery")?;
        fixture.cleanup()
    }

    #[test]
    fn symlink_hardlink_and_same_size_replacement_refuse_actual_custody() -> Result<(), String> {
        for kind in 0..3 {
            let fixture = Fixture::new()?;
            let mut writer = fixture.writer([b"payload"; 9])?;
            write_all(&mut writer)?;
            let path = fixture.path(ArtifactSlot::OriginalRaw);
            if kind == 0 {
                fs::remove_file(&path)
                    .map_err(|error| format!("symlink fixture removal: {error}"))?;
                symlink(fixture.path(ArtifactSlot::FullCheck), &path)
                    .map_err(|error| format!("artifact symlink fixture: {error}"))?;
                error(writer.finish_payloads(), "no-follow open")?;
            } else if kind == 1 {
                fs::hard_link(&path, fixture.root.join("outside-link"))
                    .map_err(|error| format!("artifact hardlink fixture: {error}"))?;
                error(writer.finish_payloads(), "identity/link/mode/length")?;
            } else {
                fs::rename(&path, fixture.root.join("old-inode"))
                    .map_err(|error| format!("artifact replacement fixture: {error}"))?;
                let mut replacement = create_private(&path)?;
                replacement
                    .write_all(b"payload")
                    .map_err(|error| format!("artifact replacement bytes: {error}"))?;
                drop(replacement);
                error(writer.finish_payloads(), "identity/link/mode/length")?;
            }
            fixture.cleanup()?;
        }
        Ok(())
    }

    #[test]
    fn same_inode_changed_bytes_and_truncation_refuse_full_eof_receipts() -> Result<(), String> {
        for truncate in [true, false] {
            let fixture = Fixture::new()?;
            let mut writer = fixture.writer([b"payload"; 9])?;
            write_all(&mut writer)?;
            fs::write(
                fixture.path(ArtifactSlot::OriginalRaw),
                if truncate {
                    b"".as_slice()
                } else {
                    b"changed".as_slice()
                },
            )
            .map_err(|error| format!("artifact changed-body fixture: {error}"))?;
            error(
                writer.finish_payloads(),
                if truncate {
                    "identity/link/mode/length"
                } else {
                    "bytes/hash"
                },
            )?;
            fixture.cleanup()?;
        }
        Ok(())
    }

    #[test]
    fn manifest_oversize_and_post_receipt_drift_refuse_before_final_authority() -> Result<(), String>
    {
        for oversize in [true, false] {
            let fixture = Fixture::new()?;
            let mut writer = fixture.writer([b"payload"; 9])?;
            write_all(&mut writer)?;
            let finished = writer.finish_payloads()?;
            if oversize {
                error(
                    finished.write_manifest(&[0_u8; 65]),
                    "separate admitted cap",
                )?;
            } else {
                fs::write(fixture.path(ArtifactSlot::ReviewInput), b"changed")
                    .map_err(|error| format!("post-receipt artifact fixture: {error}"))?;
                error(finished.write_manifest(b"manifest"), "bytes/hash")?;
            }
            assert!(!fixture.root.join("artifacts").join(MANIFEST).exists());
            fixture.cleanup()?;
        }
        Ok(())
    }

    #[test]
    fn original_deadline_failure_is_sticky_and_recovery_uses_new_owned_directory()
    -> Result<(), String> {
        let fixture = Fixture::new()?;
        let held = Instant::now()
            .checked_add(Duration::from_millis(200))
            .ok_or("artifact short clock overflow")?;
        let mut writer = ArtifactDirectory::new(
            &fixture.directory,
            [b"payload"; 9],
            ArtifactBudget::new([64; 9], 576, 64, 64)?,
            held,
        )?;
        std::thread::sleep(Duration::from_millis(250));
        let first = error(
            writer.write_slot(ArtifactSlot::OriginalRaw),
            "deadline expired",
        )?;
        assert_eq!(
            error(
                writer.write_slot(ArtifactSlot::CheckDiff),
                "deadline expired"
            )?,
            first
        );
        assert!(!fixture.path(ArtifactSlot::OriginalRaw).exists());
        error(writer.finish_payloads(), "deadline expired")?;
        fixture.directory.require_empty(fixture.deadline)?;
        fixture.cleanup()?;
        let fixture = Fixture::new()?;
        let mut writer = fixture.writer([b"payload"; 9])?;
        write_all(&mut writer)?;
        writer.finish_payloads()?.write_manifest(b"fresh")?;
        fixture.cleanup()
    }

    #[test]
    fn incomplete_payloads_and_existing_directory_cannot_start_final_manifest() -> Result<(), String>
    {
        let fixture = Fixture::new()?;
        let mut writer = fixture.writer([b"payload"; 9])?;
        writer.write_slot(ArtifactSlot::OriginalRaw)?;
        error(
            writer.finish_payloads(),
            "nine payloads are not all finished",
        )?;
        assert!(!fixture.root.join("artifacts").join(MANIFEST).exists());
        error(fixture.writer([b"payload"; 9]), "not empty")?;
        fixture.cleanup()
    }
}
