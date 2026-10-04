#![forbid(unsafe_code)]

mod startup;

use ripr::cli::CommandError;

fn main() {
    run_startup(startup::run);
}

fn run_startup(startup_run: impl FnOnce() -> Result<(), CommandError>) {
    install_panic_hook();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(startup_run)) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            report_failure(&err);
            std::process::exit(err.exit_code());
        }
        Err(_) => std::process::exit(exit_code()),
    }
}

/// Install a panic hook so an unexpected panic produces a recognizable
/// `ripr:` error message. The top-level startup boundary maps a main-thread
/// panic to code 2; worker panics remain available to their joiners (#2660).
/// A typed command error keeps its own contract instead: failures exit 2 and
/// blocking decisions / typed refusals exit 3 (docs/EXIT_CODES.md).
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("(no panic message)");
        // `ripr doctor | head` closes stdout early. That is the reader being
        // done, not a ripr bug, so skip the internal-error report. The exit
        // stays 2: the output was cut short, and a command that would have
        // exited 3 (a gate block, a typed refusal) must never read as a
        // pass under `pipefail`.
        if is_closed_stdout_panic(message) {
            std::process::exit(2);
        }
        eprintln!(
            "{}",
            format_panic_report(message, info.location().map(|loc| (loc.file(), loc.line())),)
        );
        let backtrace = std::backtrace::Backtrace::capture();
        if matches!(
            backtrace.status(),
            std::backtrace::BacktraceStatus::Captured
        ) {
            eprintln!("stack backtrace:\n{backtrace}");
        } else {
            eprintln!(
                "note: set RUST_BACKTRACE=1 for a backtrace; report at https://github.com/EffortlessMetrics/ripr-swarm/issues"
            );
        }
    }));
}

/// `println!` panics with this message when the reading end of a pipe has
/// closed: `EPIPE` on Unix, `ERROR_NO_DATA` (232) or `ERROR_BROKEN_PIPE`
/// (109) on Windows.
fn is_closed_stdout_panic(message: &str) -> bool {
    message.starts_with("failed printing to stdout")
        && CLOSED_PIPE_MARKERS
            .iter()
            .any(|marker| message.contains(marker))
}

fn format_panic_report(message: &str, location: Option<(&str, u32)>) -> String {
    let location = location
        .map(|(file, line)| format!(" at {file}:{line}"))
        .unwrap_or_default();
    format!("ripr: internal error (this is a bug): {message}{location}")
}

fn report_failure(err: &CommandError) {
    // A reader that closed stdout early (`ripr check --json | head`) is not
    // a failure worth a message; the exit stays 2 (docs/EXIT_CODES.md).
    if is_closed_stdout_error(err.message()) {
        return;
    }
    eprintln!("ripr: {err}");
}

/// A typed stdout write error whose cause is a closed pipe, as produced by
/// the chunked stdout writers and the streamed repo-exposure JSON writer.
fn is_closed_stdout_error(message: &str) -> bool {
    CLOSED_STDOUT_WRITE_PREFIXES
        .iter()
        .any(|prefix| message.starts_with(prefix))
        && CLOSED_PIPE_MARKERS
            .iter()
            .any(|marker| message.contains(marker))
}

const CLOSED_STDOUT_WRITE_PREFIXES: [&str; 3] = [
    "write to stdout failed",
    "flush stdout failed",
    "write repo exposure JSON failed",
];

const CLOSED_PIPE_MARKERS: [&str; 3] = ["Broken pipe", "(os error 232)", "(os error 109)"];

/// The panic-boundary exit code. A main-thread panic is an internal error,
/// indistinguishable operationally from any other "could not complete"
/// condition, so it stays on code 2. Blocking decisions and typed refusals
/// take the dedicated code 3 through `CommandError::exit_code` instead.
const fn exit_code() -> i32 {
    2
}

#[cfg(test)]
mod tests {
    #[test]
    fn panic_boundary_reports_and_exits_with_code_two() -> Result<(), String> {
        if std::env::var_os("RIPR_PANIC_HOOK_CHILD").is_some() {
            super::run_startup(|| {
                let trigger = std::env::var("RIPR_PANIC_HOOK_CHILD").unwrap_or_default();
                assert_eq!(trigger, "trigger", "panic hook regression");
                Ok(())
            });
            return Err("panic boundary returned instead of exiting".to_owned());
        }

        let executable = std::env::current_exe().map_err(|err| err.to_string())?;
        for backtrace in ["0", "1"] {
            let output = std::process::Command::new(&executable)
                .args([
                    "--exact",
                    "tests::panic_boundary_reports_and_exits_with_code_two",
                    "--nocapture",
                ])
                .env("RIPR_PANIC_HOOK_CHILD", "1")
                .env("RUST_BACKTRACE", backtrace)
                .output()
                .map_err(|err| format!("failed to run panic-hook child: {err}"))?;
            if output.status.code() != Some(2) {
                return Err(format!(
                    "panic-hook child exited with {:?}; stderr: {}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("ripr: internal error (this is a bug):")
                || !stderr.contains("panic hook regression")
            {
                return Err(format!(
                    "panic-hook child omitted the formatted report; stderr: {stderr}"
                ));
            }
        }

        let report = super::format_panic_report("panic hook regression", Some(("src/main.rs", 42)));
        if report != "ripr: internal error (this is a bug): panic hook regression at src/main.rs:42"
        {
            return Err(format!("unexpected formatted report: {report}"));
        }
        if !super::is_closed_stdout_panic("failed printing to stdout: Broken pipe (os error 32)")
            || !super::is_closed_stdout_panic(
                "failed printing to stdout: The pipe is being closed. (os error 232)",
            )
            || !super::is_closed_stdout_panic(
                "failed printing to stdout: The pipe has been ended. (os error 109)",
            )
            || super::is_closed_stdout_panic("failed printing to stdout: Permission denied")
            || super::is_closed_stdout_panic("Broken pipe")
        {
            return Err("closed-stdout detection must match only EPIPE on stdout".to_owned());
        }
        if !super::is_closed_stdout_error("write to stdout failed: Broken pipe (os error 32)")
            || !super::is_closed_stdout_error(
                "write repo exposure JSON failed: Broken pipe (os error 32)",
            )
            || !super::is_closed_stdout_error(
                "flush stdout failed: The pipe is being closed. (os error 232)",
            )
            || super::is_closed_stdout_error("write to stdout failed: Permission denied")
            || super::is_closed_stdout_error("git stdin write failed: Broken pipe (os error 32)")
        {
            return Err("closed-stdout error detection must match only stdout EPIPE".to_owned());
        }
        if super::exit_code() != 2 {
            return Err(format!("unexpected exit code: {}", super::exit_code()));
        }
        Ok(())
    }
}
