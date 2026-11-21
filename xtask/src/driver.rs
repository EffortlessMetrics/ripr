//! Keeps the running xtask driver image outside Cargo's replaceable build
//! output on Windows (issue #3699).
//!
//! `.cargo/config.toml` aliases `xtask = "run -p xtask --"`, so `cargo xtask
//! ...` launches `<target-root>/<profile>/xtask.exe` as the driver process.
//! The gates that driver runs nest Cargo commands (`cargo test --workspace`
//! inside `ci-fast`), and Cargo relinks that same bin path during a workspace
//! build even when `cargo run -p xtask` had just produced it (the `-p xtask`
//! and `--workspace` target universes fingerprint differently). On Windows a
//! file that backs a running process image cannot be removed or replaced, so
//! the nested relink dies with `Access is denied. (os error 5)` before the
//! workspace-test verdict exists.
//!
//! [`bootstrap`] is the first statement of `main`. On Windows, when the
//! running image is the workspace's replaceable `xtask` bin, it:
//!
//! 1. sweeps leftover driver artifacts from earlier launches out of
//!    `<target-root>/ripr/drivers`,
//! 2. copies the current image byte-for-byte into that directory as
//!    `xtask-driver-<pid>-<unix-millis>-<seq>.exe` — freshness by
//!    construction: no cached copy is ever reused,
//! 3. renames its own running image into the same directory as
//!    `xtask-stale-<pid>-<unix-millis>-<seq>.exe`. Windows forbids deleting
//!    or replacing a running image but allows renaming it, so this releases
//!    the replaceable path (letting a nested Cargo relink succeed) without
//!    stopping the process. Without this step a waiting parent would keep the
//!    replaceable path mapped and the child's nested relink would hit the
//!    same `os error 5` the bootstrap exists to avoid,
//! 4. immediately restores the image bytes at the replaceable path via a
//!    second staged copy, so the path is only absent between two atomic
//!    renames. Direct spawns of the canonical path — the workspace's own
//!    `CARGO_BIN_EXE_xtask` integration tests, scripts, editor tooling — must
//!    keep finding the file. The restored file is not a running image, so a
//!    later nested Cargo relink can remove and replace it freely,
//! 5. spawns the copy with identical argv, an inherited environment plus
//!    `RIPR_DRIVER_REEXEC=1`, and inherited stdio, waits for it, and exits
//!    the process with the child's exact exit code (falling back to 1 only
//!    when the child was terminated without a code),
//! 6. best-effort deletes the driver copy — deletable now that the child has
//!    exited — and the stale image. Renaming releases the image's deletion
//!    lock on current Windows builds, so both deletes normally succeed and a
//!    clean launch leaves the drivers directory empty; if a delete is denied
//!    anyway, the failure is ignored and the next launch's sweep collects the
//!    file.
//!
//! Two independent guards stop the bootstrap from re-running inside an
//! already-relocated image: the `xtask-driver-` / `xtask-stale-` /
//! `xtask-restore-` file-name prefix and the `RIPR_DRIVER_REEXEC=1`
//! environment variable. Setting `RIPR_DRIVER_REEXEC=1` in the environment is
//! also the documented escape hatch for running the replaceable image in
//! place.
//!
//! Every relocation failure falls back to plain in-place execution (the
//! pre-#3699 arrangement) with a short stderr note; the bootstrap never turns
//! a working arrangement into an error. On non-Windows targets it compiles
//! to a no-op.
//!
//! The artifact sweep is deliberately age-less. Every owned artifact is
//! disposable by construction: a driver copy runs only while its child lives
//! (removing a running image at its launch path is denied by the OS), and a
//! stale image is rename-released — deletable even while its parent still
//! maps it, with the mapping keeping the bytes alive until the process exits.
//! A denied removal is ignored, so safety never depends on a delete
//! succeeding; pid or mtime bookkeeping would only duplicate what the OS
//! already decides. The sweep never touches paths outside the drivers
//! directory and never recurses.

#[cfg(windows)]
mod windows_driver {
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Prefix of the staged driver copy that the child process runs from.
    const DRIVER_COPY_PREFIX: &str = "xtask-driver-";
    /// Prefix of this process' own image after it has been renamed out of the
    /// replaceable build-output path.
    const DRIVER_STALE_PREFIX: &str = "xtask-stale-";
    /// Prefix of the staged copy that briefly holds the image bytes while the
    /// replaceable path is released and restored.
    const DRIVER_RESTORE_PREFIX: &str = "xtask-restore-";
    /// Environment marker set only on the driver copy, guarding against
    /// re-running the bootstrap inside a relocated image.
    const DRIVER_REEXEC_ENV: &str = "RIPR_DRIVER_REEXEC";
    const DRIVER_REEXEC_VALUE: &str = "1";
    /// File stem of the replaceable xtask bin target. Guards unrelated
    /// executables that happen to sit under a target-style directory (hash-
    /// named test binaries, manually renamed copies, installed `ripr`).
    const DRIVER_BIN_STEM: &str = "xtask";
    /// Directory name Cargo uses for build output in the canonical layout.
    const TARGET_DIR_NAME: &str = "target";
    /// Exit code used when the driver child terminates without a code.
    const FALLBACK_EXIT_CODE: i32 = 1;

    /// Suffix shared by both artifact names.
    const ARTIFACT_SUFFIX: &str = ".exe";

    pub(crate) fn bootstrap() {
        if env_guard_blocks(std::env::var_os(DRIVER_REEXEC_ENV).as_deref()) {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        if is_driver_artifact_image(&exe) {
            return;
        }
        let cwd = std::env::current_dir().ok();
        let Some(plan) = relocation_plan_with(
            &exe,
            std::env::var_os("CARGO_TARGET_DIR").as_deref(),
            cwd.as_deref(),
        ) else {
            return;
        };
        sweep_driver_artifacts(&plan.drivers_dir);
        if let Err(err) = stage_relocation(&exe, &plan) {
            let _ = fs::remove_file(&plan.copy_path);
            note_fallback(&format!(
                "cannot stage the relocated driver: {err}; running in place"
            ));
            return;
        }
        match delegate_to_copy(&plan, std::env::args_os().skip(1)) {
            Ok(code) => std::process::exit(code),
            Err(err) => note_fallback(&format!(
                "cannot launch the driver copy: {err}; running in place from the relocated image"
            )),
        }
    }

    /// Copy the running image into the drivers directory and move the original
    /// out of the replaceable path, then restore the path at once. The path
    /// must be released before any child exists to race a nested relink, and
    /// it must be back before any concurrent direct spawn can miss it, so the
    /// gap is kept between two atomic renames. If the restore rename fails,
    /// the mapped image is renamed back: the arrangement must never end up
    /// worse than running in place.
    fn stage_relocation(exe: &Path, plan: &RelocationPlan) -> io::Result<()> {
        fs::create_dir_all(&plan.drivers_dir)?;
        fs::copy(exe, &plan.restore_path)?;
        let relocated = fs::rename(exe, &plan.stale_path);
        if let Err(err) = relocated {
            let _ = fs::remove_file(&plan.restore_path);
            return Err(err);
        }
        if let Err(err) = fs::rename(&plan.restore_path, exe) {
            let _ = fs::rename(&plan.stale_path, exe);
            let _ = fs::remove_file(&plan.restore_path);
            return Err(err);
        }
        fs::copy(exe, &plan.copy_path).map(|_| ())
    }

    /// Spawn the staged copy with identical argv, an inherited environment
    /// plus the re-exec guard, and inherited stdio; wait; report the child's
    /// exit code; then clean up both artifacts. The rename in
    /// [`stage_relocation`] releases the image's deletion lock, so the stale
    /// delete succeeds on current Windows builds even though this process
    /// still runs from that file; any denial is ignored and left to the sweep.
    fn delegate_to_copy(
        plan: &RelocationPlan,
        args: impl IntoIterator<Item = OsString>,
    ) -> io::Result<i32> {
        let mut child = Command::new(&plan.copy_path);
        child.args(args);
        child.env(DRIVER_REEXEC_ENV, DRIVER_REEXEC_VALUE);
        let status = child.status();
        let _ = fs::remove_file(&plan.copy_path);
        let _ = fs::remove_file(&plan.stale_path);
        Ok(exit_code_from(&status?))
    }

    /// Exact exit-code propagation, with one documented fallback: when the
    /// child was terminated without a code (signal-shaped), the parent exits 1
    /// rather than inventing a success.
    fn exit_code_from(status: &std::process::ExitStatus) -> i32 {
        status.code().unwrap_or(FALLBACK_EXIT_CODE)
    }

    struct RelocationPlan {
        drivers_dir: PathBuf,
        copy_path: PathBuf,
        stale_path: PathBuf,
        restore_path: PathBuf,
    }

    /// Plan the relocation for `exe`, or `None` when the image is not this
    /// workspace's replaceable bin (installed copies, renamed copies, hash-
    /// named test binaries) and must keep running exactly where it is.
    fn relocation_plan_with(
        exe: &Path,
        cargo_target_dir: Option<&OsStr>,
        cwd: Option<&Path>,
    ) -> Option<RelocationPlan> {
        if exe.file_stem()? != OsStr::new(DRIVER_BIN_STEM) {
            return None;
        }
        let exe_dir = exe.parent()?;
        let target_root = replaceable_target_root(exe_dir, cargo_target_dir, cwd)?;
        let drivers_dir = target_root.join("ripr").join("drivers");
        let tag = launch_tag();
        Some(RelocationPlan {
            copy_path: drivers_dir.join(driver_artifact_name(DRIVER_COPY_PREFIX, &tag)),
            stale_path: drivers_dir.join(driver_artifact_name(DRIVER_STALE_PREFIX, &tag)),
            restore_path: drivers_dir.join(driver_artifact_name(DRIVER_RESTORE_PREFIX, &tag)),
            drivers_dir,
        })
    }

    /// Resolve the Cargo target directory whose build output may replace the
    /// running image, from `CARGO_TARGET_DIR` or the canonical
    /// `<workspace>/target` layout, or `None` when `exe_dir` does not sit
    /// directly inside one. A false positive only relocates the image (safe);
    /// a false negative degrades to the pre-#3699 in-place arrangement.
    fn replaceable_target_root(
        exe_dir: &Path,
        cargo_target_dir: Option<&OsStr>,
        cwd: Option<&Path>,
    ) -> Option<PathBuf> {
        let root = exe_dir.parent()?;
        if root.file_name() == Some(OsStr::new(TARGET_DIR_NAME)) {
            return Some(root.to_path_buf());
        }
        let ctd = cargo_target_dir?;
        let ctd_path = PathBuf::from(ctd);
        let mut candidates: Vec<PathBuf> = Vec::new();
        if ctd_path.is_absolute() {
            candidates.push(ctd_path);
        } else {
            if let Some(workspace) = root.parent() {
                candidates.push(workspace.join(&ctd_path));
            }
            if let Some(cwd) = cwd {
                candidates.push(cwd.join(&ctd_path));
            }
        }
        candidates
            .into_iter()
            .find(|candidate| candidate.as_path() == root)
    }

    /// Remove leftover driver artifacts from earlier launches. Best effort by
    /// design: see the module docs for why no age or pid bookkeeping is used,
    /// and why a denied removal is safe to ignore.
    fn sweep_driver_artifacts(drivers_dir: &Path) {
        let Ok(entries) = fs::read_dir(drivers_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let owned = entry
                .file_name()
                .to_str()
                .is_some_and(is_driver_artifact_name);
            if !owned {
                continue;
            }
            let _ = fs::remove_file(entry.path());
        }
    }

    fn is_driver_artifact_image(exe: &Path) -> bool {
        exe.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_driver_artifact_name)
    }

    fn is_driver_artifact_name(name: &str) -> bool {
        name.starts_with(DRIVER_COPY_PREFIX)
            || name.starts_with(DRIVER_STALE_PREFIX)
            || name.starts_with(DRIVER_RESTORE_PREFIX)
    }

    /// Belt-and-suspenders guard: the marker is set only for the driver copy,
    /// never in a normal launch environment.
    fn env_guard_blocks(value: Option<&OsStr>) -> bool {
        value == Some(OsStr::new(DRIVER_REEXEC_VALUE))
    }

    fn launch_tag() -> String {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        format_launch_tag(std::process::id(), current_unix_millis(), seq)
    }

    fn format_launch_tag(pid: u32, unix_millis: u128, seq: u64) -> String {
        format!("{pid}-{unix_millis}-{seq}")
    }

    fn current_unix_millis() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    }

    fn driver_artifact_name(prefix: &str, tag: &str) -> String {
        format!("{prefix}{tag}{ARTIFACT_SUFFIX}")
    }

    fn note_fallback(reason: &str) {
        eprintln!("xtask: driver relocation unavailable ({reason})");
    }

    #[cfg(all(test, windows))]
    mod tests {
        use super::*;

        fn unique_test_dir(tag: &str) -> PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "ripr-driver-test-{}-{}-{}",
                tag,
                std::process::id(),
                current_unix_millis()
            ));
            let _ = fs::remove_dir_all(&dir);
            dir
        }

        fn write_file(path: &Path) -> Result<(), String> {
            fs::write(path, b"payload")
                .map_err(|err| format!("failed to write {}: {err}", path.display()))
        }

        fn require(condition: bool, message: &str) -> Result<(), String> {
            if condition {
                Ok(())
            } else {
                Err(message.to_string())
            }
        }

        #[test]
        fn artifact_names_carry_prefix_tag_and_exe_suffix() -> Result<(), String> {
            let tag = format_launch_tag(4321, 1_700_000_000_123, 7);
            let copy = driver_artifact_name(DRIVER_COPY_PREFIX, &tag);
            let stale = driver_artifact_name(DRIVER_STALE_PREFIX, &tag);
            let restore = driver_artifact_name(DRIVER_RESTORE_PREFIX, &tag);
            require(
                copy == "xtask-driver-4321-1700000000123-7.exe",
                "driver copy name must embed prefix, pid, millis, and seq",
            )?;
            require(
                stale == "xtask-stale-4321-1700000000123-7.exe",
                "stale image name must embed prefix, pid, millis, and seq",
            )?;
            require(
                restore == "xtask-restore-4321-1700000000123-7.exe",
                "restore staging name must embed prefix, pid, millis, and seq",
            )?;
            require(
                copy != stale && stale != restore && copy != restore,
                "artifact names must stay distinguishable",
            )
        }

        #[test]
        fn artifact_name_guard_accepts_only_owned_prefixes() {
            assert!(is_driver_artifact_name("xtask-driver-1-2-3.exe"));
            assert!(is_driver_artifact_name("xtask-stale-1-2-3.exe"));
            assert!(is_driver_artifact_name("xtask-restore-1-2-3.exe"));
            assert!(!is_driver_artifact_name("xtask.exe"));
            assert!(!is_driver_artifact_name("xtask-driverish.exe"));
            assert!(!is_driver_artifact_name("my-xtask-driver-1-2.exe"));
            assert!(!is_driver_artifact_name("notes.txt"));
        }

        #[test]
        fn image_guard_recognizes_artifact_paths_by_file_name() {
            assert!(is_driver_artifact_image(Path::new(
                "target/ripr/drivers/xtask-driver-1-2-3.exe"
            )));
            assert!(!is_driver_artifact_image(Path::new(
                "target/debug/xtask.exe"
            )));
            assert!(!is_driver_artifact_image(Path::new("xtask.exe")));
        }

        #[test]
        fn env_guard_blocks_only_exact_marker_value() {
            assert!(env_guard_blocks(Some(OsStr::new("1"))));
            assert!(!env_guard_blocks(None));
            assert!(!env_guard_blocks(Some(OsStr::new("0"))));
            assert!(!env_guard_blocks(Some(OsStr::new("1x"))));
            assert!(!env_guard_blocks(Some(OsStr::new("on"))));
        }

        #[test]
        fn replaceable_target_root_resolves_conservatively() -> Result<(), String> {
            let base = unique_test_dir("roots");
            let canonical_profile = base.join("target").join("debug");
            require(
                replaceable_target_root(&canonical_profile, None, None).as_deref()
                    == Some(base.join("target").as_path()),
                "canonical <workspace>/target/debug layout must resolve to the target root",
            )?;
            let release_profile = base.join("target").join("release");
            require(
                replaceable_target_root(&release_profile, None, None).is_some(),
                "non-default profile dirs directly under target must also resolve",
            )?;
            let deps_dir = canonical_profile.join("deps");
            require(
                replaceable_target_root(&deps_dir, None, None).is_none(),
                "hash-named artifacts nested under deps must never be treated as the bin",
            )?;
            let custom_root = base.join("cache");
            let custom_profile = custom_root.join("debug");
            require(
                replaceable_target_root(&custom_profile, None, None).is_none(),
                "without CARGO_TARGET_DIR a non-target-named root must not match",
            )?;
            require(
                replaceable_target_root(&custom_profile, Some(custom_root.as_os_str()), None)
                    .as_deref()
                    == Some(custom_root.as_path()),
                "an absolute CARGO_TARGET_DIR equal to the image root must match",
            )?;
            require(
                replaceable_target_root(&custom_profile, Some(OsStr::new("cache")), None)
                    .as_deref()
                    == Some(custom_root.as_path()),
                "a relative CARGO_TARGET_DIR resolves against the workspace guess",
            )?;
            require(
                replaceable_target_root(&custom_profile, Some(OsStr::new("cache")), Some(&base))
                    .as_deref()
                    == Some(custom_root.as_path()),
                "a relative CARGO_TARGET_DIR also resolves against the cwd",
            )?;
            require(
                replaceable_target_root(
                    &custom_profile,
                    Some(OsStr::new("elsewhere")),
                    Some(&base),
                )
                .is_none(),
                "an unrelated CARGO_TARGET_DIR must not claim the image",
            )?;
            let _ = fs::remove_dir_all(&base);
            Ok(())
        }

        #[test]
        fn relocation_plan_targets_drivers_dir_with_unique_names() -> Result<(), String> {
            let base = unique_test_dir("plan");
            let exe = base.join("target").join("debug").join("xtask.exe");
            let expected_drivers = base.join("target").join("ripr").join("drivers");
            let plan = relocation_plan_with(&exe, None, None).ok_or("canonical image must plan")?;
            require(
                plan.drivers_dir == expected_drivers,
                "driver artifacts belong under <target>/ripr/drivers",
            )?;
            require(
                plan.copy_path.parent() == Some(expected_drivers.as_path()),
                "the driver copy must live inside the drivers directory",
            )?;
            require(
                plan.copy_path != plan.stale_path,
                "copy and stale paths must never collide",
            )?;
            let second = relocation_plan_with(&exe, None, None).ok_or("second plan must exist")?;
            require(
                second.copy_path != plan.copy_path && second.stale_path != plan.stale_path,
                "two launches must never share an artifact name",
            )?;
            let renamed = base.join("target").join("debug").join("xtask-verify.exe");
            require(
                relocation_plan_with(&renamed, None, None).is_none(),
                "a manually renamed copy must keep running where it is",
            )?;
            let _ = fs::remove_dir_all(&base);
            Ok(())
        }

        #[test]
        fn sweep_removes_only_owned_artifacts_inside_drivers_dir() -> Result<(), String> {
            let base = unique_test_dir("sweep");
            let drivers = base.join("drivers");
            let sibling = base.join("sibling");
            fs::create_dir_all(&drivers).map_err(|err| err.to_string())?;
            fs::create_dir_all(&sibling).map_err(|err| err.to_string())?;
            write_file(&drivers.join("xtask-driver-1-2-3.exe"))?;
            write_file(&drivers.join("xtask-stale-1-2-3.exe"))?;
            write_file(&drivers.join("xtask-restore-1-2-3.exe"))?;
            write_file(&drivers.join("xtask.exe"))?;
            write_file(&drivers.join("notes.txt"))?;
            write_file(&sibling.join("xtask-driver-1-2-3.exe"))?;
            fs::create_dir_all(drivers.join("xtask-driver-9-9-9.exe"))
                .map_err(|err| err.to_string())?;
            sweep_driver_artifacts(&drivers);
            require(
                !drivers.join("xtask-driver-1-2-3.exe").exists(),
                "sweep must remove leftover driver copies",
            )?;
            require(
                !drivers.join("xtask-stale-1-2-3.exe").exists(),
                "sweep must remove leftover stale images",
            )?;
            require(
                !drivers.join("xtask-restore-1-2-3.exe").exists(),
                "sweep must remove leftover restore staging files",
            )?;
            require(
                drivers.join("xtask.exe").exists() && drivers.join("notes.txt").exists(),
                "sweep must keep files it does not own",
            )?;
            require(
                drivers.join("xtask-driver-9-9-9.exe").is_dir(),
                "sweep must leave directories alone even when their name matches",
            )?;
            require(
                sibling.join("xtask-driver-1-2-3.exe").exists(),
                "sweep must never reach outside the drivers directory",
            )?;
            let _ = fs::remove_dir_all(&base);
            Ok(())
        }

        #[test]
        fn child_exit_codes_propagate_through_the_driver_helper() -> Result<(), String> {
            let mut failing = Command::new("cmd");
            failing.args(["/C", "exit 42"]);
            let failing_status = failing.status().map_err(|err| err.to_string())?;
            require(
                exit_code_from(&failing_status) == 42,
                "a failing child's code must reach the parent unchanged",
            )?;
            let mut clean = Command::new("cmd");
            clean.args(["/C", "exit 0"]);
            let clean_status = clean.status().map_err(|err| err.to_string())?;
            require(
                exit_code_from(&clean_status) == 0,
                "a clean child's code must reach the parent unchanged",
            )
        }

        /// The full launch lifecycle against a staged fake workspace: the
        /// replaceable path is released and immediately restored, the child
        /// runs from the fresh copy, the copy is deleted after exit, and the
        /// now-unmapped stale image is deletable too. The child here is a copy
        /// of the current test binary invoked with a filter that matches no
        /// test, so libtest exits 0 quietly. The true running-image behavior
        /// (rename of a mapped image, deletion denied until exit, relink under
        /// a live driver) is validated manually against real `cargo xtask`
        /// launches; see the PR evidence.
        #[test]
        fn relocation_releases_path_and_delegates_to_fresh_copy() -> Result<(), String> {
            let base = unique_test_dir("lifecycle");
            let profile = base.join("target").join("debug");
            fs::create_dir_all(&profile).map_err(|err| err.to_string())?;
            let exe = profile.join("xtask.exe");
            let test_image =
                std::env::current_exe().map_err(|err| format!("locate test image: {err}"))?;
            fs::copy(&test_image, &exe).map_err(|err| format!("stage fake image: {err}"))?;
            let plan = relocation_plan_with(&exe, None, None)
                .ok_or("lifecycle fake workspace must plan a relocation")?;
            sweep_driver_artifacts(&plan.drivers_dir);
            stage_relocation(&exe, &plan).map_err(|err| format!("stage: {err}"))?;
            require(
                exe.exists(),
                "the replaceable path must be restored for concurrent direct spawns",
            )?;
            require(
                !plan.restore_path.exists(),
                "the restore staging file must be consumed by the restore rename",
            )?;
            require(
                plan.copy_path.exists() && plan.stale_path.exists(),
                "both driver artifacts must exist while the child runs",
            )?;
            let args = [
                OsString::from("zzz_no_such_test_3699"),
                OsString::from("--exact"),
            ];
            let code = delegate_to_copy(&plan, args).map_err(|err| format!("delegate: {err}"))?;
            require(code == 0, "the filter-matched libtest child must exit 0")?;
            require(
                !plan.copy_path.exists(),
                "the child copy must be deleted after the child exits",
            )?;
            require(
                !plan.stale_path.exists(),
                "the stale image must be deletable once nothing maps it",
            )?;
            require(
                exe.exists(),
                "the restored replaceable path must survive the delegation",
            )?;
            let _ = fs::remove_dir_all(&base);
            Ok(())
        }
    }
}

#[cfg(windows)]
pub(crate) use windows_driver::bootstrap;

#[cfg(not(windows))]
pub(crate) fn bootstrap() {}
