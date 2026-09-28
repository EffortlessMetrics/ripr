//! Self-hosted CI scratch ownership contract (#3841).
//!
//! Per-run trees under `/mnt/ci-scratch/{cargo-home,target,tmp}` are owned by
//! a kernel `flock` lease that the job holds for its whole lifetime. The only
//! reclamation authority is `.github/actions/ci-scratch-lease`, and both the
//! inline `rust-gates.yml` reclaim and the scheduled `scratch-gc.yml` sweep
//! must consume it. A top-level directory age is never evidence that a job is
//! inactive: nested writes do not refresh it, so the retired
//! `find ... -mmin +30 -exec rm -rf` selector deleted live job trees.
//!
//! This module pins the wiring. The executable proof that the authority keeps
//! live trees and reclaims only owned terminal orphans lives in `tests.rs`,
//! which runs the action's own script against temporary roots.

use std::path::Path;

pub(crate) const SCRATCH_LEASE_ACTION_PATH: &str = ".github/actions/ci-scratch-lease/action.yml";
const SCRATCH_LEASE_USES: &str = "uses: ./.github/actions/ci-scratch-lease";
const RUST_GATES_PATH: &str = ".github/workflows/rust-gates.yml";
const SCRATCH_GC_PATH: &str = ".github/workflows/scratch-gc.yml";
/// scratch-gc.yml checks the action out here, never at the workspace root: a
/// sparse checkout is blob-filtered and its promisor config would persist
/// into the `fetch-depth: 0` rust-gates checkout on the same runner.
const SCRATCH_GC_CHECKOUT: &str = ".ci-scratch-gc";
const SCRATCH_ROOT: &str = "/mnt/ci-scratch";
const SCRATCH_TREE_MARKER: &str = "CARGO_TARGET_DIR: /mnt/ci-scratch/";

/// `find` predicates and shell tests that select by timestamp. None of them
/// may choose what to delete under the shared scratch root.
const AGE_SELECTORS: &[&str] = &[
    "-mmin", "-mtime", "-cmin", "-ctime", "-amin", "-atime", "-newer", " -nt ", " -ot ",
];
const DELETE_ACTIONS: &[&str] = &["-delete", "rm -", "rm \"", "rm /", "-exec rm"];

/// Anchors of the lock-held deletion inside the action script. The execution
/// test proves the behavior; these keep a structural edit from silently
/// dropping the lock or moving deletion ahead of it.
const LOCK_PROBE: &str = "if ! flock -n -x \"$fd\"; then";
const INODE_CHECK: &str = "stat -L -c %d:%i \"/dev/fd/$fd\"";
const LOCKED_DELETE: &str = "rm -rf -- \"$tree\"";

pub(crate) fn scratch_lease_contract_violations_for_repo() -> Result<Vec<String>, String> {
    let mut workflows = Vec::new();
    let workflow_root = Path::new(".github/workflows");
    if workflow_root.exists() {
        for path in crate::collect_files(workflow_root)? {
            let normalized = crate::normalize_path(&path);
            if normalized.ends_with(".yml") || normalized.ends_with(".yaml") {
                workflows.push((normalized, crate::read_text_lossy(&path)?));
            }
        }
    }
    let action_path = Path::new(SCRATCH_LEASE_ACTION_PATH);
    let action = if action_path.exists() {
        Some(crate::read_text_lossy(action_path)?)
    } else {
        None
    };
    Ok(scratch_lease_contract_violations(
        &workflows,
        action.as_deref(),
    ))
}

pub(crate) fn scratch_lease_contract_violations(
    workflows: &[(String, String)],
    action: Option<&str>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let uses_scratch = workflows
        .iter()
        .any(|(_, text)| text.contains(SCRATCH_ROOT));

    for (path, text) in workflows {
        violations.extend(age_or_raw_delete_violations(path, text));
        if text.contains(SCRATCH_TREE_MARKER) && lease_step_line(text, "acquire").is_none() {
            violations.push(format!(
                "{path}: creates per-run /mnt/ci-scratch trees without `{SCRATCH_LEASE_USES}` `mode: acquire`; unleased trees are never reclaimable (#3841)"
            ));
        }
    }

    if let Some((_, text)) = workflows.iter().find(|(path, _)| path == RUST_GATES_PATH) {
        violations.extend(rust_gates_violations(text));
    }
    if let Some((_, text)) = workflows.iter().find(|(path, _)| path == SCRATCH_GC_PATH) {
        violations.extend(scratch_gc_violations(text));
    }

    match action {
        Some(text) => violations.extend(action_violations(text)),
        None if uses_scratch => violations.push(format!(
            "{SCRATCH_LEASE_ACTION_PATH} is missing; workflows use {SCRATCH_ROOT} and need the shared lease authority (#3841)"
        )),
        None => {}
    }
    violations
}

fn age_or_raw_delete_violations(path: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim_start().starts_with('#') || !line.contains(SCRATCH_ROOT) {
            continue;
        }
        if let Some(selector) = AGE_SELECTORS.iter().find(|token| line.contains(**token)) {
            violations.push(format!(
                "{path}:{}: selects {SCRATCH_ROOT} entries by age (`{}`); directory age is not a lease on a live job tree — use {SCRATCH_LEASE_ACTION_PATH} (#3841)",
                index + 1,
                selector.trim()
            ));
        } else if let Some(action) = DELETE_ACTIONS.iter().find(|token| line.contains(**token)) {
            violations.push(format!(
                "{path}:{}: deletes under {SCRATCH_ROOT} outside the lease authority (`{}`); own-run cleanup must use its own $CARGO_HOME/$CARGO_TARGET_DIR/$TMPDIR and cross-run reclamation must use {SCRATCH_LEASE_ACTION_PATH} (#3841)",
                index + 1,
                action.trim()
            ));
        }
    }
    violations
}

fn rust_gates_violations(text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let order = [
        (
            lease_step_line(text, "acquire"),
            step_name_line(text, "Prepare toolchain temp"),
            "the `mode: acquire` lease step must run before `Prepare toolchain temp` creates the first per-run tree",
        ),
        (
            lease_step_line(text, "reclaim"),
            step_name_line(text, "Prepare scratch"),
            "the `mode: reclaim` lease step must run before `Prepare scratch` applies the disk guard",
        ),
        (
            step_name_line(text, "Clean scratch"),
            lease_step_line(text, "release"),
            "the `mode: release` lease step must follow own-run `Clean scratch`",
        ),
    ];
    for (earlier, later, message) in order {
        match (earlier, later) {
            (Some(first), Some(second)) if first < second => {}
            _ => violations.push(format!("{RUST_GATES_PATH}: {message} (#3841)")),
        }
    }
    if lease_step_line(text, "acquire").is_some()
        && !text.contains("job-timeout-minutes: ${{ inputs.job-timeout-minutes }}")
    {
        violations.push(format!(
            "{RUST_GATES_PATH}: the acquire step must pass the job deadline so a missed orphan kill cannot pin a lease past it (#3841)"
        ));
    }
    violations
}

fn scratch_gc_violations(text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let checkout = lines
        .iter()
        .position(|line| line.trim_start().starts_with("- uses: actions/checkout@"));
    let dedicated = checkout.is_some_and(|index| {
        let indent = lines[index].len() - lines[index].trim_start().len();
        lines[index + 1..]
            .iter()
            .take_while(|next| {
                let trimmed = next.trim_start();
                trimmed.is_empty() || next.len() - trimmed.len() > indent
            })
            .any(|next| next.trim() == format!("path: {SCRATCH_GC_CHECKOUT}"))
    });
    let consumed = text.contains(&format!(
        "uses: ./{SCRATCH_GC_CHECKOUT}/.github/actions/ci-scratch-lease"
    ));
    if !(dedicated && consumed) {
        violations.push(format!(
            "{SCRATCH_GC_PATH}: check out the lease action into `path: {SCRATCH_GC_CHECKOUT}` and call it from there; a sparse checkout at the workspace root leaves blob-filter promisor config in the rust-gates checkout (#3841)"
        ));
    }
    match (checkout, lease_step_line(text, "reclaim")) {
        (Some(first), Some(second)) if first < second => {}
        (_, None) => violations.push(format!(
            "{SCRATCH_GC_PATH}: scheduled reclamation must use `{SCRATCH_LEASE_USES}` `mode: reclaim`, the same authority as rust-gates.yml (#3841)"
        )),
        _ => violations.push(format!(
            "{SCRATCH_GC_PATH}: the lease action needs a checkout step before `mode: reclaim` (#3841)"
        )),
    }
    violations
}

fn action_violations(text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(selector) = AGE_SELECTORS.iter().find(|token| line.contains(**token)) {
            violations.push(format!(
                "{SCRATCH_LEASE_ACTION_PATH}:{}: uses an age selector (`{}`); liveness must come from the flock lease only (#3841)",
                index + 1,
                selector.trim()
            ));
        }
    }
    let position = |needle: &str| text.find(needle);
    match (
        position(LOCK_PROBE),
        position(INODE_CHECK),
        position(LOCKED_DELETE),
    ) {
        (Some(lock), Some(inode), Some(delete)) if lock < inode && inode < delete => {}
        _ => violations.push(format!(
            "{SCRATCH_LEASE_ACTION_PATH}: reclaim must take `flock -n` on the lease, verify the locked inode is still the lease path, and only then delete (`{LOCK_PROBE}` < `{INODE_CHECK}` < `{LOCKED_DELETE}`) (#3841)"
        )),
    }
    violations
}

/// Line index of a `uses: ./.github/actions/ci-scratch-lease` step whose
/// `with.mode` is `mode`.
fn lease_step_line(text: &str, mode: &str) -> Option<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let expected = format!("mode: {mode}");
    lines.iter().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim_start();
        let step = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        let dedicated = format!("uses: ./{SCRATCH_GC_CHECKOUT}/.github/actions/ci-scratch-lease");
        if step != SCRATCH_LEASE_USES && step != dedicated {
            return None;
        }
        let indent = line.len() - trimmed.len();
        lines[index + 1..]
            .iter()
            .take_while(|next| {
                let next_trimmed = next.trim_start();
                next_trimmed.is_empty()
                    || (next.len() - next_trimmed.len() >= indent
                        && !next_trimmed.starts_with("- "))
            })
            .any(|next| next.trim() == expected)
            .then_some(index)
    })
}

fn step_name_line(text: &str, name: &str) -> Option<usize> {
    let expected = format!("- name: {name}");
    text.lines().position(|line| line.trim() == expected)
}

#[cfg(test)]
mod tests;
