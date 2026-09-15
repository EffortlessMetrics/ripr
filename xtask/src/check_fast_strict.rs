use std::fs;

const BASE_REF: &str = "origin/main";
const FAST_REPORT_PATH: &str = "target/ripr/reports/check-fast.md";
const SELECTOR_REPORT: &str = "check-fast-selector.md";

pub(super) fn run() -> Result<(), String> {
    super::super::ensure_reports_dir()?;
    let mut discover = super::super::changed_files_vs_origin_main;
    let mut write_receipt = |status: &str, reason: &str, count: usize, detail: &str| {
        super::super::write_report(
            SELECTOR_REPORT,
            &selector_report(status, reason, count, detail),
        )
    };
    run_transaction(
        &mut discover,
        super::super::check_fast,
        || {
            fs::read_to_string(FAST_REPORT_PATH)
                .map_err(|error| format!("read {FAST_REPORT_PATH}: {error}"))
        },
        &mut write_receipt,
    )
}

fn run_transaction<D, F, R, W>(
    discover: &mut D,
    run_fast: F,
    read_fast_report: R,
    write_receipt: &mut W,
) -> Result<(), String>
where
    D: FnMut() -> Result<Vec<String>, String>,
    F: FnOnce() -> Result<(), String>,
    R: FnOnce() -> Result<String, String>,
    W: FnMut(&str, &str, usize, &str) -> Result<(), String>,
{
    let before = match discover() {
        Ok(files) => files,
        Err(error) => {
            let primary = format!("check-fast selector unavailable for {BASE_REF}: {error}");
            return fail(write_receipt, "selector_unavailable", 0, &primary);
        }
    };

    if let Err(error) = run_fast() {
        let primary = format!("check-fast gate failed after selector establishment: {error}");
        return fail(write_receipt, "fast_gate_failed", before.len(), &primary);
    }

    let after = match discover() {
        Ok(files) => files,
        Err(error) => {
            let primary = format!(
                "check-fast selector became unavailable after the gate run for {BASE_REF}: {error}"
            );
            return fail(
                write_receipt,
                "selector_unavailable_after_run",
                before.len(),
                &primary,
            );
        }
    };
    if before != after {
        let primary = format!(
            "check-fast selector changed during the gate run: before={} file(s), after={} file(s)",
            before.len(),
            after.len()
        );
        return fail(
            write_receipt,
            "selector_changed_during_run",
            before.len(),
            &primary,
        );
    }

    let report = match read_fast_report() {
        Ok(report) => report,
        Err(error) => {
            let primary = format!("check-fast success report unavailable: {error}");
            return fail(
                write_receipt,
                "fast_report_unavailable",
                before.len(),
                &primary,
            );
        }
    };
    if let Err(error) = validate_fast_report(&before, &report) {
        let primary = format!("check-fast report does not match its selector: {error}");
        return fail(
            write_receipt,
            "fast_report_mismatch",
            before.len(),
            &primary,
        );
    }

    write_receipt(
        "pass",
        "stable",
        before.len(),
        "selector and conditional gate receipt agree",
    )
    .map_err(|error| format!("write {SELECTOR_REPORT}: {error}"))
}

fn validate_fast_report(files: &[String], report: &str) -> Result<(), String> {
    if !report.contains("Status: pass") {
        return Err("success report does not declare pass".to_string());
    }
    let ran = report
        .split_once("Ran:\n")
        .and_then(|(_, remainder)| remainder.split_once("\n\nSkipped:\n"))
        .map(|(ran, _)| ran)
        .ok_or_else(|| "success report has no bounded Ran/Skipped sections".to_string())?;

    let categories = super::super::categorize_changed_files(files);
    let mut required = Vec::new();
    if categories.rust_src {
        required.extend([
            "check-no-panic-family",
            "check-allow-attributes",
            "check-file-policy",
            "clippy",
        ]);
    }
    if categories.workflow {
        required.push("check-workflows");
    }
    if categories.policy {
        required.extend(["check-process-policy", "check-network-policy"]);
    }
    if categories.fixture {
        required.push("check-fixture-contracts");
    }

    let missing = required
        .into_iter()
        .filter(|gate| {
            let expected = format!("- {gate}");
            !ran.lines().any(|line| line.trim() == expected)
        })
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "required ran gate(s) absent: {}",
            missing.join(", ")
        ))
    }
}

fn fail<W>(write_receipt: &mut W, reason: &str, count: usize, primary: &str) -> Result<(), String>
where
    W: FnMut(&str, &str, usize, &str) -> Result<(), String>,
{
    match write_receipt("failed", reason, count, primary) {
        Ok(()) => Err(primary.to_string()),
        Err(report_error) => Err(format!(
            "{primary}; additionally failed to write {SELECTOR_REPORT}: {report_error}"
        )),
    }
}

fn selector_report(status: &str, reason: &str, count: usize, detail: &str) -> String {
    let detail = detail.lines().next().unwrap_or("none");
    format!(
        "# check-fast selector\n\nStatus: {status}\nBase ref: {BASE_REF}\nSelector: {reason}\nChanged files: {count}\nDetail: {detail}\n"
    )
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::{run_transaction, selector_report};

    const EMPTY_FAST_REPORT: &str = concat!(
        "# check-fast report\n\n",
        "Status: pass\n\n",
        "Ran:\n",
        "- fmt --check\n",
        "- check-static-language\n",
        "- check-command-catalog\n",
        "- check-generated\n",
        "- check-generated-clean\n",
        "- check-lint-policy\n\n",
        "Skipped:\n",
        "- check-no-panic-family\n",
        "- check-allow-attributes\n",
        "- check-file-policy\n",
        "- clippy\n",
        "- check-fixture-contracts\n",
    );

    #[test]
    fn selector_failure_stops_before_fast_gate() -> Result<(), String> {
        let fast_ran = Cell::new(false);
        let receipt = RefCell::new(String::new());
        let mut discover = || Err("origin/main unavailable".to_string());
        let mut write = |status: &str, reason: &str, count: usize, detail: &str| {
            *receipt.borrow_mut() = selector_report(status, reason, count, detail);
            Ok(())
        };

        let result = run_transaction(
            &mut discover,
            || {
                fast_ran.set(true);
                Ok(())
            },
            || Ok(EMPTY_FAST_REPORT.to_string()),
            &mut write,
        );
        if result.is_ok() {
            return Err("selector failure unexpectedly passed".to_string());
        }
        if fast_ran.get() {
            return Err("fast gate ran without selector authority".to_string());
        }
        if !receipt.borrow().contains("Selector: selector_unavailable") {
            return Err(format!("unexpected selector receipt: {}", receipt.borrow()));
        }
        Ok(())
    }

    #[test]
    fn legitimate_empty_selection_is_distinct_from_failure() -> Result<(), String> {
        let receipt = RefCell::new(String::new());
        let mut discover = || Ok(Vec::new());
        let mut write = |status: &str, reason: &str, count: usize, detail: &str| {
            *receipt.borrow_mut() = selector_report(status, reason, count, detail);
            Ok(())
        };

        run_transaction(
            &mut discover,
            || Ok(()),
            || Ok(EMPTY_FAST_REPORT.to_string()),
            &mut write,
        )?;
        let receipt = receipt.borrow();
        if !receipt.contains("Status: pass") || !receipt.contains("Changed files: 0") {
            return Err(format!("empty selector receipt is ambiguous: {receipt}"));
        }
        Ok(())
    }

    #[test]
    fn selected_rust_path_requires_rust_gate_receipt() -> Result<(), String> {
        let receipt = RefCell::new(String::new());
        let selected = vec!["xtask/src/check_fast_strict.rs".to_string()];
        let mut discover = || Ok(selected.clone());
        let mut write = |status: &str, reason: &str, count: usize, detail: &str| {
            *receipt.borrow_mut() = selector_report(status, reason, count, detail);
            Ok(())
        };

        let result = run_transaction(
            &mut discover,
            || Ok(()),
            || Ok(EMPTY_FAST_REPORT.to_string()),
            &mut write,
        );
        let error = match result {
            Ok(()) => return Err("missing Rust gate receipt unexpectedly passed".to_string()),
            Err(error) => error,
        };
        if !error.contains("check-no-panic-family") || !error.contains("clippy") {
            return Err(format!("missing Rust gates were not identified: {error}"));
        }
        if !receipt.borrow().contains("Selector: fast_report_mismatch") {
            return Err(format!("unexpected mismatch receipt: {}", receipt.borrow()));
        }
        Ok(())
    }

    #[test]
    fn selector_change_during_gate_run_fails_closed() -> Result<(), String> {
        let receipt = RefCell::new(String::new());
        let mut calls = 0usize;
        let mut discover = || {
            calls += 1;
            if calls == 1 {
                Ok(vec!["crates/ripr/src/lib.rs".to_string()])
            } else {
                Ok(vec![
                    "crates/ripr/src/lib.rs".to_string(),
                    "xtask/src/main.rs".to_string(),
                ])
            }
        };
        let mut write = |status: &str, reason: &str, count: usize, detail: &str| {
            *receipt.borrow_mut() = selector_report(status, reason, count, detail);
            Ok(())
        };

        let result = run_transaction(
            &mut discover,
            || Ok(()),
            || Ok(EMPTY_FAST_REPORT.to_string()),
            &mut write,
        );
        if result.is_ok() {
            return Err("selector drift during the gate run unexpectedly passed".to_string());
        }
        if !receipt
            .borrow()
            .contains("Selector: selector_changed_during_run")
        {
            return Err(format!("unexpected drift receipt: {}", receipt.borrow()));
        }
        if !receipt.borrow().contains("Changed files: 1") {
            return Err(format!(
                "drift receipt must keep the before-run denominator: {}",
                receipt.borrow()
            ));
        }
        Ok(())
    }
}
