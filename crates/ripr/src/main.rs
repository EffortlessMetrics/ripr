#![forbid(unsafe_code)]

mod startup;

fn main() {
    run_startup(startup::run);
}

fn run_startup(startup_run: impl FnOnce() -> Result<(), String>) {
    install_panic_hook();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(startup_run)) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            report_failure(&err);
            std::process::exit(exit_code());
        }
        Err(_) => std::process::exit(exit_code()),
    }
}

/// Install a panic hook so an unexpected panic produces a recognizable
/// `ripr:` error message. The top-level startup boundary maps a main-thread
/// panic to code 2; worker panics remain available to their joiners (#2660).
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("(no panic message)");
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

fn format_panic_report(message: &str, location: Option<(&str, u32)>) -> String {
    let location = location
        .map(|(file, line)| format!(" at {file}:{line}"))
        .unwrap_or_default();
    format!("ripr: internal error (this is a bug): {message}{location}")
}

fn report_failure(err: &str) {
    eprintln!("ripr: {err}");
}

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
        if super::exit_code() != 2 {
            return Err(format!("unexpected exit code: {}", super::exit_code()));
        }
        Ok(())
    }
}
