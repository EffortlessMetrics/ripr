//! Inactive observed-self-image preparation for a future complete parent.
//!
//! This retains actual executable bytes and native/process observations. It
//! grants no analyzer, source-build, cohort, cleanup or publication authority.
//! The module remains test-only until a genuine consuming caller is assembled.

#![cfg(target_os = "linux")]

use crate::process_owner::{NativeLimitTuple, ObservedProcessIdentity};
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::time::Instant;

const IMAGE_BYTES_MAX: u64 = 256 * 1024 * 1024;
const HASH_CHUNK_BYTES: usize = 64 * 1024;
const KERNEL_SELF_EXE: &str = "/proc/self/exe";

#[derive(Debug, PartialEq, Eq)]
struct ExecutableMetadata {
    device: u64,
    inode: u64,
    links: u64,
    bytes: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl ExecutableMetadata {
    fn observe(metadata: Metadata, maximum: u64) -> Result<Self, String> {
        if !metadata.is_file() {
            return Err("complete self image is not a regular file".into());
        }
        if metadata.nlink() == 0 {
            return Err("complete self image is deleted or has no retained link".into());
        }
        if metadata.len() == 0 || metadata.len() > maximum {
            return Err("complete self image exceeds its admitted nonempty byte bound".into());
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            links: metadata.nlink(),
            bytes: metadata.len(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
}

/// A retained actual self executable, not a complete-execution capability.
///
/// The File and its descriptor never leave this owner. A derived execution
/// identifier is observation DATA; its caller must retain this owner through
/// actual launch/settlement and independently qualify source/build provenance.
pub(super) struct ObservedSelfImage {
    file: File,
    native: NativeLimitTuple,
    owner: ObservedProcessIdentity,
    metadata: ExecutableMetadata,
    digest: String,
    held_deadline: Instant,
}

fn checkpoint(held_deadline: Instant) -> Result<(), String> {
    if Instant::now() >= held_deadline {
        Err("complete self image original held deadline expired".into())
    } else {
        Ok(())
    }
}

fn observe_native(
    expected_address_space: u64,
    expected_file_size: u64,
    held_deadline: Instant,
) -> Result<NativeLimitTuple, String> {
    checkpoint(held_deadline)?;
    let actual = NativeLimitTuple::observe(held_deadline)?;
    checkpoint(held_deadline)?;
    if actual.address_space_bytes() != expected_address_space
        || actual.file_size_bytes() != expected_file_size
    {
        return Err(
            "complete self image actual native limits differ from the expected tuple".into(),
        );
    }
    Ok(actual)
}

fn observe_owner(held_deadline: Instant) -> Result<ObservedProcessIdentity, String> {
    checkpoint(held_deadline)?;
    let actual = ObservedProcessIdentity::read(std::process::id())?;
    checkpoint(held_deadline)?;
    if actual.pid() != std::process::id()
        || actual.start() == 0
        || matches!(actual.state(), 'Z' | 'X' | 'x')
    {
        return Err("complete self image actual owner is not the live current process".into());
    }
    Ok(actual)
}

fn verify_owner(expected: &ObservedProcessIdentity, held_deadline: Instant) -> Result<(), String> {
    let actual = observe_owner(held_deadline)?;
    if actual.pid() != expected.pid()
        || actual.start() != expected.start()
        || actual.group() != expected.group()
        || actual.parent() != expected.parent()
    {
        return Err("complete self image actual process identity changed".into());
    }
    Ok(())
}

fn held_metadata(
    file: &File,
    maximum: u64,
    held_deadline: Instant,
) -> Result<ExecutableMetadata, String> {
    checkpoint(held_deadline)?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("complete self image held metadata: {error}"))?;
    checkpoint(held_deadline)?;
    ExecutableMetadata::observe(metadata, maximum)
}

fn verify_held_metadata(
    file: &File,
    expected: &ExecutableMetadata,
    held_deadline: Instant,
) -> Result<(), String> {
    if held_metadata(file, IMAGE_BYTES_MAX, held_deadline)? != *expected {
        return Err("complete self image held file identity or immutable metadata changed".into());
    }
    Ok(())
}

fn verify_kernel_image(
    expected: &ExecutableMetadata,
    held_deadline: Instant,
) -> Result<(), String> {
    checkpoint(held_deadline)?;
    // This fixed kernel magic link is intentional. No caller-selected path or
    // filesystem fallback is accepted, and the executable File is not reopened.
    let metadata = fs::metadata(KERNEL_SELF_EXE)
        .map_err(|error| format!("complete self image current kernel link: {error}"))?;
    checkpoint(held_deadline)?;
    verify_kernel_metadata(metadata, expected)
}

fn verify_kernel_metadata(metadata: Metadata, expected: &ExecutableMetadata) -> Result<(), String> {
    if ExecutableMetadata::observe(metadata, IMAGE_BYTES_MAX)? != *expected {
        return Err("complete self image differs from the current kernel executable".into());
    }
    Ok(())
}

fn hash_exact(
    reader: &mut impl Read,
    expected_bytes: u64,
    held_deadline: Instant,
) -> Result<String, String> {
    checkpoint(held_deadline)?;
    if expected_bytes > IMAGE_BYTES_MAX {
        return Err("complete self image hash exceeds its unchanged byte bound".into());
    }
    #[cfg(test)]
    HASH_STARTS.with(|count| count.set(count.get() + 1));
    let mut scratch = [0_u8; HASH_CHUNK_BYTES];
    let mut consumed = 0_u64;
    let mut hash = Sha256::new();
    loop {
        checkpoint(held_deadline)?;
        let read = reader.read(&mut scratch);
        checkpoint(held_deadline)?;
        let read = match read {
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("complete self image bounded read: {error}")),
        };
        if read == 0 {
            if consumed != expected_bytes {
                return Err("complete self image reached EOF before its admitted size".into());
            }
            checkpoint(held_deadline)?;
            return Ok(format!("sha256:{:x}", hash.finalize()));
        }
        consumed = consumed
            .checked_add(
                u64::try_from(read)
                    .map_err(|error| format!("complete self image native read length: {error}"))?,
            )
            .filter(|count| *count <= expected_bytes && *count <= IMAGE_BYTES_MAX)
            .ok_or("complete self image bytes exceed its admitted size")?;
        hash.update(&scratch[..read]);
        checkpoint(held_deadline)?;
    }
}

impl ObservedSelfImage {
    pub(super) fn observe(
        expected_address_space: u64,
        expected_file_size: u64,
        held_deadline: Instant,
    ) -> Result<Self, String> {
        // Actual native admission precedes executable open/hash. This does not
        // impose limits or replace an unlimited parent with a selected profile.
        let native = observe_native(expected_address_space, expected_file_size, held_deadline)?;
        let owner = observe_owner(held_deadline)?;
        checkpoint(held_deadline)?;
        #[cfg(test)]
        IMAGE_OPENS.with(|count| count.set(count.get() + 1));
        let mut file = File::open(KERNEL_SELF_EXE)
            .map_err(|error| format!("complete self image kernel executable open: {error}"))?;
        checkpoint(held_deadline)?;
        let metadata = held_metadata(&file, IMAGE_BYTES_MAX, held_deadline)?;
        verify_kernel_image(&metadata, held_deadline)?;
        verify_owner(&owner, held_deadline)?;
        let observed_native = observe_native(
            native.address_space_bytes(),
            native.file_size_bytes(),
            held_deadline,
        )?;
        if observed_native != native {
            return Err("complete self image native observation changed before hash".into());
        }
        let digest = hash_exact(&mut file, metadata.bytes, held_deadline)?;
        verify_held_metadata(&file, &metadata, held_deadline)?;
        verify_kernel_image(&metadata, held_deadline)?;
        verify_owner(&owner, held_deadline)?;
        let observed_native = observe_native(
            native.address_space_bytes(),
            native.file_size_bytes(),
            held_deadline,
        )?;
        if observed_native != native {
            return Err("complete self image native observation changed after hash".into());
        }
        checkpoint(held_deadline)?;
        Ok(Self {
            file,
            native,
            owner,
            metadata,
            digest,
            held_deadline,
        })
    }

    pub(super) fn recheck(&mut self) -> Result<(), String> {
        checkpoint(self.held_deadline)?;
        let actual = observe_native(
            self.native.address_space_bytes(),
            self.native.file_size_bytes(),
            self.held_deadline,
        )?;
        if actual != self.native {
            return Err("complete self image actual native profile changed".into());
        }
        verify_owner(&self.owner, self.held_deadline)?;
        verify_held_metadata(&self.file, &self.metadata, self.held_deadline)?;
        verify_kernel_image(&self.metadata, self.held_deadline)?;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("complete self image held rewind: {error}"))?;
        checkpoint(self.held_deadline)?;
        let digest = hash_exact(&mut self.file, self.metadata.bytes, self.held_deadline)?;
        if digest != self.digest {
            return Err("complete self image actual retained bytes changed".into());
        }
        verify_held_metadata(&self.file, &self.metadata, self.held_deadline)?;
        verify_kernel_image(&self.metadata, self.held_deadline)?;
        verify_owner(&self.owner, self.held_deadline)?;
        let actual = observe_native(
            self.native.address_space_bytes(),
            self.native.file_size_bytes(),
            self.held_deadline,
        )?;
        if actual != self.native {
            return Err("complete self image actual native profile changed after hash".into());
        }
        checkpoint(self.held_deadline)
    }

    pub(super) fn execution_identifier(&mut self) -> Result<PathBuf, String> {
        self.recheck()?;
        checkpoint(self.held_deadline)?;
        // The descriptor belongs to the observed SELF (future launch parent),
        // never to owner.parent(). Retaining this owner is a caller obligation.
        let identifier = PathBuf::from(format!(
            "/proc/{}/fd/{}",
            self.owner.pid(),
            self.file.as_raw_fd(),
        ));
        checkpoint(self.held_deadline)?;
        Ok(identifier)
    }

    pub(super) fn sha256(&self) -> &str {
        &self.digest
    }

    pub(super) fn byte_len(&self) -> u64 {
        self.metadata.bytes
    }

    pub(super) fn owner_identity(&self) -> &ObservedProcessIdentity {
        &self.owner
    }

    pub(super) fn held_deadline(&self) -> Instant {
        self.held_deadline
    }
}

#[cfg(test)]
thread_local! {
    static IMAGE_OPENS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static HASH_STARTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        file: PathBuf,
    }

    impl Fixture {
        fn new(bytes: &[u8]) -> Result<Self, String> {
            let root = std::env::temp_dir().join(format!(
                "ripr-observed-image-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root).map_err(|error| format!("image fixture directory: {error}"))?;
            let fixture = Self {
                file: root.join("image"),
                root,
            };
            fs::write(&fixture.file, bytes)
                .map_err(|error| format!("image fixture bytes: {error}"))?;
            Ok(fixture)
        }

        fn open(&self) -> Result<File, String> {
            File::open(&self.file).map_err(|error| format!("image fixture open: {error}"))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn held() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }

    fn refusal<T>(result: Result<T, String>, expected: &str) -> Result<(), String> {
        match result {
            Ok(_) => Err(format!("image control unexpectedly accepted {expected}")),
            Err(error) if error.contains(expected) => Ok(()),
            Err(error) => Err(format!("wrong image refusal for {expected}: {error}")),
        }
    }

    #[test]
    fn actual_native_refusal_precedes_self_image_open_and_hash() -> Result<(), String> {
        let deadline = held();
        let opens = IMAGE_OPENS.with(std::cell::Cell::get);
        let hashes = HASH_STARTS.with(std::cell::Cell::get);
        let actual = NativeLimitTuple::observe(deadline);
        match actual {
            Err(native_error) => {
                let error = match ObservedSelfImage::observe(
                    2 * 1024 * 1024 * 1024,
                    IMAGE_BYTES_MAX,
                    deadline,
                ) {
                    Ok(_) => return Err("unqualified actual parent opened its self image".into()),
                    Err(error) => error,
                };
                assert_eq!(error, native_error);
            }
            Ok(actual) => {
                // A genuinely finite harness may run this suite. Mismatched
                // expected DATA still cannot enter executable open or hash.
                refusal(
                    ObservedSelfImage::observe(0, actual.file_size_bytes(), deadline),
                    "actual native limits differ",
                )?;
            }
        }
        assert_eq!(IMAGE_OPENS.with(std::cell::Cell::get), opens);
        assert_eq!(HASH_STARTS.with(std::cell::Cell::get), hashes);
        Ok(())
    }

    #[test]
    fn actual_self_observation_preserves_native_refusal_or_real_identity() -> Result<(), String> {
        let deadline = held();
        let actual = match NativeLimitTuple::observe(deadline) {
            Ok(actual) => actual,
            Err(error) => {
                // This is an actual negative observation, not finite-positive
                // evidence. A bounded native harness owns the positive proof.
                return refusal(
                    ObservedSelfImage::observe(2 * 1024 * 1024 * 1024, IMAGE_BYTES_MAX, deadline),
                    &error,
                );
            }
        };
        let opens = IMAGE_OPENS.with(std::cell::Cell::get);
        let mut image = ObservedSelfImage::observe(
            actual.address_space_bytes(),
            actual.file_size_bytes(),
            deadline,
        )?;
        assert_eq!(IMAGE_OPENS.with(std::cell::Cell::get), opens + 1);
        assert_eq!(image.owner_identity().pid(), std::process::id());
        assert_eq!(image.held_deadline(), deadline);
        if image.byte_len() == 0 || !image.sha256().starts_with("sha256:") {
            return Err("actual self image lacks nonempty retained bytes/digest".into());
        }
        let digest = image.sha256().to_string();
        let identifier = image.execution_identifier()?;
        assert_eq!(
            identifier,
            PathBuf::from(format!(
                "/proc/{}/fd/{}",
                image.owner_identity().pid(),
                image.file.as_raw_fd(),
            )),
        );
        image.recheck()?;
        assert_eq!(image.sha256(), digest);
        assert_eq!(IMAGE_OPENS.with(std::cell::Cell::get), opens + 1);
        Ok(())
    }

    #[test]
    fn expired_original_clock_refuses_before_native_image_admission() -> Result<(), String> {
        let opens = IMAGE_OPENS.with(std::cell::Cell::get);
        let hashes = HASH_STARTS.with(std::cell::Cell::get);
        refusal(
            ObservedSelfImage::observe(0, 0, Instant::now()),
            "original held deadline",
        )?;
        assert_eq!(IMAGE_OPENS.with(std::cell::Cell::get), opens);
        assert_eq!(HASH_STARTS.with(std::cell::Cell::get), hashes);
        Ok(())
    }

    #[test]
    fn held_descriptor_hash_has_actual_eof_and_known_sha256() -> Result<(), String> {
        let fixture = Fixture::new(b"abc")?;
        let mut file = fixture.open()?;
        let metadata = held_metadata(&file, IMAGE_BYTES_MAX, held())?;
        assert_eq!(
            hash_exact(&mut file, metadata.bytes, held())?,
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
        verify_held_metadata(&file, &metadata, held())
    }

    struct ShortReads<'a> {
        file: &'a mut File,
    }

    impl Read for ShortReads<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let length = buffer.len().min(3);
            self.file.read(&mut buffer[..length])
        }
    }

    #[test]
    fn short_reads_preserve_binary_digest_without_treating_them_as_eof() -> Result<(), String> {
        let bytes = b"binary\0\xffshort-reads";
        let fixture = Fixture::new(bytes)?;
        let mut file = fixture.open()?;
        let actual = hash_exact(
            &mut ShortReads { file: &mut file },
            u64::try_from(bytes.len()).map_err(|error| error.to_string())?,
            held(),
        )?;
        assert_eq!(actual, format!("sha256:{:x}", Sha256::digest(bytes)));
        Ok(())
    }

    #[test]
    fn actual_eof_shortfall_and_extra_bytes_refuse_without_truncation() -> Result<(), String> {
        let fixture = Fixture::new(b"abc")?;
        let mut file = fixture.open()?;
        refusal(
            hash_exact(&mut file, 4, held()),
            "EOF before its admitted size",
        )?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| error.to_string())?;
        refusal(
            hash_exact(&mut file, 2, held()),
            "bytes exceed its admitted size",
        )
    }

    #[test]
    fn executable_path_swap_keeps_held_bytes_and_rejects_replacement_identity() -> Result<(), String>
    {
        let fixture = Fixture::new(b"held-original")?;
        let mut file = fixture.open()?;
        let original = held_metadata(&file, IMAGE_BYTES_MAX, held())?;
        verify_kernel_metadata(
            file.metadata().map_err(|error| error.to_string())?,
            &original,
        )?;
        fs::rename(&fixture.file, fixture.root.join("retained-original"))
            .map_err(|error| error.to_string())?;
        fs::write(&fixture.file, b"replaced-path").map_err(|error| error.to_string())?;
        let retained = held_metadata(&file, IMAGE_BYTES_MAX, held())?;
        assert_eq!(
            (retained.device, retained.inode),
            (original.device, original.inode)
        );
        // Rename may update ctime. The held descriptor still selects the old
        // inode/bytes; a real owner's full metadata recheck would refuse drift.
        assert_eq!(
            hash_exact(&mut file, original.bytes, held())?,
            format!("sha256:{:x}", Sha256::digest(b"held-original")),
        );
        let replacement = fixture.open()?;
        refusal(
            verify_kernel_metadata(
                replacement.metadata().map_err(|error| error.to_string())?,
                &original,
            ),
            "differs from the current kernel executable",
        )
    }

    #[test]
    fn actual_write_only_file_read_error_never_returns_a_digest() -> Result<(), String> {
        let fixture = Fixture::new(b"read-error")?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&fixture.file)
            .map_err(|error| error.to_string())?;
        refusal(hash_exact(&mut file, 10, held()), "bounded read")
    }

    #[test]
    fn mutated_deleted_and_oversized_descriptors_refuse_before_hash() -> Result<(), String> {
        let fixture = Fixture::new(b"abc")?;
        let file = fixture.open()?;
        let original = held_metadata(&file, IMAGE_BYTES_MAX, held())?;
        let mut writer = fs::OpenOptions::new()
            .write(true)
            .open(&fixture.file)
            .map_err(|error| error.to_string())?;
        writer
            .write_all(b"defg")
            .map_err(|error| error.to_string())?;
        refusal(
            verify_held_metadata(&file, &original, held()),
            "identity or immutable metadata changed",
        )?;
        fs::remove_file(&fixture.file).map_err(|error| error.to_string())?;
        refusal(
            held_metadata(&file, IMAGE_BYTES_MAX, held()),
            "deleted or has no retained link",
        )?;
        let bounded = Fixture::new(b"12345")?;
        let hashes = HASH_STARTS.with(std::cell::Cell::get);
        refusal(held_metadata(&bounded.open()?, 4, held()), "byte bound")?;
        assert_eq!(HASH_STARTS.with(std::cell::Cell::get), hashes);
        Ok(())
    }

    struct DelayedRead<'a> {
        file: &'a mut File,
        delayed: bool,
    }

    impl Read for DelayedRead<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let read = self.file.read(buffer)?;
            if !self.delayed {
                self.delayed = true;
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(read)
        }
    }

    #[test]
    fn real_read_crossing_original_clock_never_returns_a_digest() -> Result<(), String> {
        let fixture = Fixture::new(b"deadline")?;
        let mut file = fixture.open()?;
        let deadline = Instant::now() + Duration::from_millis(10);
        refusal(
            hash_exact(
                &mut DelayedRead {
                    file: &mut file,
                    delayed: false,
                },
                8,
                deadline,
            ),
            "original held deadline",
        )
    }

    struct InterruptedReads {
        attempts: usize,
    }

    impl Read for InterruptedReads {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            self.attempts += 1;
            std::thread::sleep(Duration::from_millis(5));
            Err(io::Error::from(io::ErrorKind::Interrupted))
        }
    }

    #[test]
    fn interrupted_reads_cannot_restart_the_original_clock() -> Result<(), String> {
        let mut reader = InterruptedReads { attempts: 0 };
        refusal(
            hash_exact(&mut reader, 1, Instant::now() + Duration::from_millis(100)),
            "original held deadline",
        )?;
        if reader.attempts == 0 {
            return Err("interrupted read control never exercised actual reads".into());
        }
        Ok(())
    }
}
