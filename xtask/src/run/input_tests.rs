//! Composition controls for the byte-input owner used by candidate Git custody.
//! Unix execution controls; native Windows Job Object proof remains separate.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Root(std::path::PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn root() -> Result<Root, String> {
    let path = std::env::temp_dir().join(format!(
        "ripr-byte-input-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(Root(path))
}
fn require_reaped(pid: &str) -> Result<(), String> {
    let output = capture_bytes_in_dir_with_timeout(
        Path::new("kill"),
        &["-0".to_string(), pid.trim().to_string()],
        Path::new("."),
        &[],
        &[],
        Duration::from_secs(5),
        "observe owned child cleanup",
    )?;
    if output.timed_out || output.status.is_none_or(|status| status.success()) {
        return Err(format!(
            "owned child {pid:?} is still live or cleanup observation failed"
        ));
    }
    Ok(())
}

#[test]
fn byte_input_round_trips_more_than_pipe_capacity() -> Result<(), String> {
    let root = root()?;
    let input = vec![0x80; 1024 * 1024];
    let output = capture_bytes_in_dir_with_input_timeout(
        Path::new("cat"),
        &[],
        &root.0,
        &input,
        &[],
        Duration::from_secs(5),
        "binary round trip",
    )?;
    if output.timed_out
        || !output.status.is_some_and(|status| status.success())
        || output.stdout != input
    {
        return Err("binary stdin/output did not round-trip completely".to_string());
    }
    Ok(())
}

#[test]
fn byte_input_nonreader_timeout_and_early_exit_are_bounded_and_reaped() -> Result<(), String> {
    for (label, script, timeout_expected) in [
        (
            "nonreader timeout",
            "printf '%s\\n' \"$$\" > child.pid; exec sleep 30",
            true,
        ),
        (
            "early exit",
            "printf '%s\\n' \"$$\" > child.pid; exit 0",
            false,
        ),
    ] {
        let root = root()?;
        let start = Instant::now();
        let result = capture_bytes_in_dir_with_input_timeout(
            Path::new("sh"),
            &["-c".to_string(), script.to_string()],
            &root.0,
            &vec![b'x'; 1024 * 1024],
            &[],
            Duration::from_millis(500),
            label,
        );
        let elapsed = start.elapsed();
        let pid = fs::read_to_string(root.0.join("child.pid"))
            .map_err(|e| format!("fixture did not start: {e}"))?;
        require_reaped(&pid)?;
        if elapsed > Duration::from_secs(8) {
            return Err(format!(
                "{label} exceeded its timeout/drain bound: {elapsed:?}"
            ));
        }
        match result {
            Ok(output)
                if timeout_expected
                    && output.timed_out
                    && output.status.is_some_and(|status| !status.success()) => {}
            Err(error) if !timeout_expected && error.contains("write stdin") => (),
            Err(error) => {
                return Err(format!(
                    "{label} lost its actual timeout/early-exit classification: {error}"
                ));
            }
            Ok(_) => {
                return Err(format!(
                    "{label} silently accepted incomplete stdin or lost timeout status"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn dropping_owned_byte_command_reaps_the_cancelled_primary() -> Result<(), String> {
    let mut command = Command::new("sleep");
    command
        .arg("30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    configure_timed_child_command(&mut command);
    let owned = OwnedProcess::spawn(command).map_err(|e| e.to_string())?;
    let pid = owned.id();
    let start = Instant::now();
    drop(owned);
    require_reaped(&pid.to_string())?;
    if start.elapsed() > Duration::from_secs(5) {
        return Err("owner cancellation did not return promptly".to_string());
    }
    Ok(())
}

#[test]
fn bounded_byte_capture_rejects_stdout_and_stderr_overflow_and_reaps_child() -> Result<(), String> {
    let exact = capture_bytes_in_dir_with_budget(
        Path::new("sh"),
        &["-c".to_string(), "printf 1234; printf 5678 >&2".to_string()],
        (Path::new("."), None),
        &[],
        ByteCaptureBudget {
            timeout: Duration::from_secs(5),
            stdout_bytes: 4,
            stderr_bytes: 4,
        },
        "exact output budget",
    )?;
    if exact.stdout != b"1234"
        || exact.stderr != b"5678"
        || exact.timed_out
        || !exact.status.is_some_and(|s| s.success())
    {
        return Err("exact output budget lost bytes/status".to_string());
    }
    for (script, stream) in [
        (
            "printf '%s\\n' \"$$\" > child.pid; printf 12345; exec sleep 30",
            "stdout",
        ),
        (
            "printf '%s\\n' \"$$\" > child.pid; printf 12345 >&2; exec sleep 30",
            "stderr",
        ),
    ] {
        let root = root()?;
        let start = Instant::now();
        let result = capture_bytes_in_dir_with_budget(
            Path::new("sh"),
            &["-c".to_string(), script.to_string()],
            (&root.0, None),
            &[],
            ByteCaptureBudget {
                timeout: Duration::from_millis(500),
                stdout_bytes: 4,
                stderr_bytes: 4,
            },
            "overflow cleanup",
        );
        let pid = fs::read_to_string(root.0.join("child.pid")).map_err(|e| e.to_string())?;
        require_reaped(&pid)?;
        if start.elapsed() > Duration::from_secs(8) {
            return Err("overflow escaped timeout/cleanup bound".to_string());
        }
        match result {
            Err(error) if error.contains(&format!("{stream} exceeds its 4-byte output budget")) => {
            }
            Err(error) => return Err(format!("wrong overflow refusal: {error}")),
            Ok(_) => return Err(format!("{stream} overflow was accepted")),
        }
    }
    let mut bytes = std::io::Cursor::new(b"123456789");
    if read_stream_bytes_limited(&mut bytes, "stdout", 4).is_ok() || bytes.position() != 5 {
        return Err("bounded process reader consumed beyond limit+1".to_string());
    }
    Ok(())
}

#[test]
fn bounded_byte_drain_requires_terminal_output() -> Result<(), String> {
    for require_complete in [true, false] {
        let (_sender, receiver) = mpsc::channel();
        let handle = thread::spawn(|| {});
        match drain_byte_reader_bounded(
            receiver,
            handle,
            Duration::from_millis(10),
            "stdout",
            "missing terminal output",
            require_complete,
        ) {
            Err(error) if require_complete && error.contains("byte output is not established") => {}
            Ok(bytes)
                if !require_complete
                    && String::from_utf8_lossy(&bytes).contains("output truncated") => {}
            _ => {
                return Err(
                    "byte drain changed the selected strict/legacy reporting contract".to_string(),
                );
            }
        }
    }
    Ok(())
}
