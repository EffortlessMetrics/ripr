//! Proof for the #3841 scratch ownership contract.
//!
//! Two layers:
//!
//! - wiring: the checked-in workflows and action satisfy the contract, and
//!   each targeted regression (age selector restored, one cleaner bypassing
//!   the authority, lease taken after trees exist) is reported;
//! - behavior (Linux): the action's own script, extracted from
//!   `action.yml`, runs against temporary roots only. It keeps a live leased
//!   tree whose top-level directory is 40 minutes old while nested writes are
//!   fresh, reclaims owned terminal orphans, and skips foreign, unleased,
//!   malformed, and symlinked entries. Two negative controls mutate the
//!   script (no lock check; lock check replaced by an mtime test) and require
//!   the same harness to fail, so the harness cannot pass vacuously.

use std::path::Path;

use super::{
    LOCK_PROBE, RUST_GATES_PATH, SCRATCH_GC_PATH, SCRATCH_LEASE_ACTION_PATH,
    scratch_lease_contract_violations,
};

fn repo_text(path: &str) -> Result<String, String> {
    crate::read_text_lossy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path))
}

fn repo_contract_inputs() -> Result<(Vec<(String, String)>, String), String> {
    let workflows = vec![
        (RUST_GATES_PATH.to_string(), repo_text(RUST_GATES_PATH)?),
        (SCRATCH_GC_PATH.to_string(), repo_text(SCRATCH_GC_PATH)?),
    ];
    Ok((workflows, repo_text(SCRATCH_LEASE_ACTION_PATH)?))
}

fn violations_after_edit(path: &str, from: &str, to: &str) -> Result<Vec<String>, String> {
    let (mut workflows, action) = repo_contract_inputs()?;
    let mut edited = false;
    for (workflow_path, text) in &mut workflows {
        if workflow_path == path {
            if !text.contains(from) {
                return Err(format!("fixture edit anchor missing from {path}: {from}"));
            }
            *text = text.replacen(from, to, 1);
            edited = true;
        }
    }
    if !edited {
        return Err(format!("no fixture workflow named {path}"));
    }
    Ok(scratch_lease_contract_violations(&workflows, Some(&action)))
}

#[test]
fn checked_in_workflows_consume_one_lease_authority() -> Result<(), String> {
    let (workflows, action) = repo_contract_inputs()?;
    for (path, text) in &workflows {
        assert!(
            text.contains("/.github/actions/ci-scratch-lease\n"),
            "{path} must call the shared lease authority"
        );
        assert!(
            !text.contains("-mmin +30 -exec rm -rf"),
            "{path} still carries the age-only selector"
        );
    }
    let violations = scratch_lease_contract_violations(&workflows, Some(&action));
    assert!(violations.is_empty(), "{violations:#?}");
    Ok(())
}

#[test]
fn restoring_the_age_only_selector_is_rejected_in_either_cleaner() -> Result<(), String> {
    let selector = "          find /mnt/ci-scratch/cargo-home /mnt/ci-scratch/target /mnt/ci-scratch/tmp -mindepth 1 -maxdepth 1 -mmin +30 -exec rm -rf {} + 2>/dev/null || true\n";
    for (path, anchor) in [
        (RUST_GATES_PATH, "          find /mnt/ci-cache/sccache"),
        (SCRATCH_GC_PATH, "          find /mnt/ci-cache/sccache"),
    ] {
        let violations = violations_after_edit(path, anchor, &format!("{selector}{anchor}"))?;
        assert!(
            violations
                .iter()
                .any(|violation| violation.starts_with(path) && violation.contains("`-mmin`")),
            "{path}: {violations:#?}"
        );
    }
    let larger_threshold =
        "          find /mnt/ci-scratch/target -mindepth 1 -maxdepth 1 -mtime +2 -delete\n";
    let violations = violations_after_edit(
        SCRATCH_GC_PATH,
        "          find /mnt/ci-cache/sccache",
        &format!("{larger_threshold}          find /mnt/ci-cache/sccache"),
    )?;
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("`-mtime`")),
        "a larger age threshold is still an age selector: {violations:#?}"
    );
    Ok(())
}

#[test]
fn a_cleaner_that_bypasses_the_authority_is_rejected() -> Result<(), String> {
    let violations = violations_after_edit(
        SCRATCH_GC_PATH,
        "          mode: reclaim\n",
        "          mode: audit\n",
    )?;
    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with(SCRATCH_GC_PATH)
                && violation.contains("same authority as rust-gates.yml")),
        "{violations:#?}"
    );

    let violations = violations_after_edit(
        RUST_GATES_PATH,
        "          mode: reclaim\n",
        "          mode: audit\n",
    )?;
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("`mode: reclaim` lease step must run before")),
        "{violations:#?}"
    );

    let violations = violations_after_edit(
        RUST_GATES_PATH,
        "          mode: release\n",
        "          mode: audit\n",
    )?;
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("`mode: release` lease step must follow")),
        "{violations:#?}"
    );
    Ok(())
}

#[test]
fn a_gc_checkout_in_the_build_workspace_is_rejected() -> Result<(), String> {
    for (from, to) in [
        ("          path: .ci-scratch-gc\n", ""),
        (
            "uses: ./.ci-scratch-gc/.github/actions/ci-scratch-lease",
            "uses: ./.github/actions/ci-scratch-lease",
        ),
    ] {
        let violations = violations_after_edit(SCRATCH_GC_PATH, from, to)?;
        assert!(
            violations
                .iter()
                .any(|violation| violation.starts_with(SCRATCH_GC_PATH)
                    && violation.contains("path: .ci-scratch-gc")),
            "{from}: {violations:#?}"
        );
    }
    Ok(())
}

#[test]
fn a_lease_taken_after_trees_exist_is_rejected() -> Result<(), String> {
    let (mut workflows, action) = repo_contract_inputs()?;
    let Some((_, text)) = workflows
        .iter_mut()
        .find(|(path, _)| path == RUST_GATES_PATH)
    else {
        return Err("rust-gates fixture missing".to_string());
    };
    let toolchain_temp = "      - name: Prepare toolchain temp\n        if: inputs.use-scratch\n        env:\n          TMPDIR: /mnt/ci-scratch/tmp/${{ github.run_id }}-${{ github.run_attempt }}\n        run: mkdir -p \"$TMPDIR\"\n\n";
    if !text.contains(toolchain_temp) {
        return Err("Prepare toolchain temp anchor missing from rust-gates.yml".to_string());
    }
    *text = text.replacen(toolchain_temp, "", 1);
    let anchor = "      - name: Configure scratch environment\n";
    *text = text.replacen(anchor, &format!("{toolchain_temp}{anchor}"), 1);
    let acquire = "      - name: Acquire scratch lease\n";
    let Some(acquire_at) = text.find(acquire) else {
        return Err("acquire step anchor missing".to_string());
    };
    let Some(toolchain_at) = text.find("      - name: Prepare toolchain temp\n") else {
        return Err("moved toolchain step missing".to_string());
    };
    assert!(
        acquire_at < toolchain_at,
        "fixture must still acquire first"
    );

    // Now move the tree creation ahead of the lease instead.
    let (mut workflows_late, _) = repo_contract_inputs()?;
    let Some((_, late)) = workflows_late
        .iter_mut()
        .find(|(path, _)| path == RUST_GATES_PATH)
    else {
        return Err("rust-gates fixture missing".to_string());
    };
    *late = late.replacen(toolchain_temp, "", 1);
    *late = late.replacen(
        "      - name: Acquire scratch lease\n",
        &format!("{toolchain_temp}      - name: Acquire scratch lease\n"),
        1,
    );
    let violations = scratch_lease_contract_violations(&workflows_late, Some(&action));
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("`mode: acquire` lease step must run before")),
        "{violations:#?}"
    );
    assert!(
        scratch_lease_contract_violations(&workflows, Some(&action)).is_empty(),
        "control: moving toolchain temp later keeps the contract"
    );
    Ok(())
}

#[test]
fn removing_the_lock_from_the_action_is_rejected_structurally() -> Result<(), String> {
    let (workflows, action) = repo_contract_inputs()?;
    if !action.contains(LOCK_PROBE) {
        return Err("lock probe anchor missing from the action".to_string());
    }
    let unlocked = action.replacen(LOCK_PROBE, "if false; then", 1);
    let violations = scratch_lease_contract_violations(&workflows, Some(&unlocked));
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("reclaim must take `flock -n`")),
        "{violations:#?}"
    );
    let aged = action.replacen(
        LOCK_PROBE,
        "if [[ -n \"$(find \"$entry\" -maxdepth 0 -mmin -30)\" ]]; then",
        1,
    );
    let violations = scratch_lease_contract_violations(&workflows, Some(&aged));
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("`-mmin`")),
        "{violations:#?}"
    );
    Ok(())
}

#[test]
fn a_new_scratch_workflow_without_a_lease_is_rejected() {
    let workflow = "jobs:\n  build:\n    steps:\n      - name: Configure\n        env:\n          CARGO_TARGET_DIR: /mnt/ci-scratch/target/${{ github.run_id }}-${{ github.run_attempt }}\n        run: echo ok\n".to_string();
    let violations = scratch_lease_contract_violations(
        &[(".github/workflows/new.yml".to_string(), workflow)],
        None,
    );
    assert!(
        violations.iter().any(|violation| violation
            .contains("without `uses: ./.github/actions/ci-scratch-lease` `mode: acquire`")),
        "{violations:#?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("is missing; workflows use /mnt/ci-scratch")),
        "{violations:#?}"
    );
}

#[cfg(target_os = "linux")]
mod behavior {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{LOCK_PROBE, SCRATCH_LEASE_ACTION_PATH, repo_text};

    /// Scenario harness. `$1` is the extracted action script. Every scenario
    /// prints `ok   <label>` or `FAIL <label>`; the exit status is non-zero
    /// when any scenario fails. Jobs are simulated by running `acquire` in a
    /// fresh session and later killing that session's process group, which is
    /// what the Actions runner does to a job's orphan processes at job end.
    const HARNESS: &str = r#"
set -euo pipefail
script="$1"
work="$(mktemp -d)"
cleanup() {
  local holder
  for holder in $(pgrep -f -- "$work/scratch/leases/" || true); do
    kill -TERM -- "-$(ps -o pgid= -p "$holder" | tr -d ' ')" 2>/dev/null || true
  done
  rm -rf -- "$work"
}
trap cleanup EXIT
root="$work/scratch"; mkdir -p "$root/cargo-home" "$root/target" "$root/tmp"
REPO=EffortlessMetrics/ripr-swarm
fail=0
check() { if eval "$2"; then echo "ok   $1"; else echo "FAIL $1"; fail=1; fi; }
run_mode() {
  GITHUB_REPOSITORY="${4:-$REPO}" GITHUB_RUN_ID="$2" GITHUB_RUN_ATTEMPT="$3" GITHUB_JOB=rust-gates RUNNER_NAME=test-runner \
  GITHUB_OUTPUT="$work/out.$2.$1" GITHUB_STEP_SUMMARY="$work/summary" CI_SCRATCH_ROOT="$root" CI_SCRATCH_MODE="$1" \
  CI_SCRATCH_JOB_TIMEOUT_MINUTES=5 setsid -w bash --noprofile --norc -eo pipefail "$script"
}
acquire() {
  run_mode acquire "$1" 1 "${2:-$REPO}" > "$work/acq.$1" 2>&1
  local pid; pid="$(sed -n 's/^holder_pid=//p' "$work/out.$1.acquire")"
  test -n "$pid"
  echo "$pid"
}
kill_job() {
  local g; g="$(ps -o pgid= -p "$1" | tr -d ' ')"; kill -TERM -- "-$g"
  flock -x -w 10 "$root/leases/$2-1.lock" true
}
mktree() {
  for c in cargo-home target tmp; do
    mkdir -p "$root/$c/$1/nested"; printf 'work\n' > "$root/$c/$1/nested/output"
    touch -d '40 minutes ago' "$root/$c/$1"; printf 'fresh\n' >> "$root/$c/$1/nested/output"
  done
}
alltree() { for c in cargo-home target tmp; do test -d "$root/$c/$1" || return 1; done; }
notree() { for c in cargo-home target tmp; do test ! -e "$root/$c/$1" || return 1; done; }
notrees() { local i; for i in $(seq "$1" "$2"); do notree "$i-1" || return 1; done; }
reclaim() { run_mode reclaim "${1:-999}" 1 > "$work/reclaim.log" 2>&1; cat "$work/reclaim.log"; }

echo "== issue reproduction: retired age-only selector"
mktree 101-1
check "fixture: top-level dir is older than 30 minutes" '[ -n "$(find "$root/target/101-1" -maxdepth 0 -mmin +30)" ]'
check "fixture: nested output was written just now" '[ -n "$(find "$root/target/101-1/nested/output" -mmin -1)" ]'
find "$root/cargo-home" "$root/target" "$root/tmp" -mindepth 1 -maxdepth 1 -mmin +30 -exec rm -rf {} + 2>/dev/null || true
check "retired selector deletes the active tree" 'notree 101-1'

echo "== lease authority: live kept, terminal orphan reclaimed"
h1="$(acquire 201)"; mktree 201-1
h2="$(acquire 202)"; mktree 202-1; kill_job "$h2" 202
reclaim
check "live leased tree aged 40 minutes with fresh writes survives" 'alltree 201-1 && grep -q fresh "$root/target/201-1/nested/output"'
check "owned terminal orphan is reclaimed" 'notree 202-1'
check "terminal lease is removed with its trees" '[ ! -e "$root/leases/202-1.lock" ]'
check "live lease is retained" '[ -f "$root/leases/201-1.lock" ]'
check "reclaim reports ids, entries, bytes, and live skips" 'grep -q "reclaimed_ids=1 reclaimed_entries=3 reclaimed_kib=[1-9]" "$work/reclaim.log" && grep -q "skipped_live=3" "$work/reclaim.log"'
check "lease records owner identity" 'grep -qx "repository=$REPO" "$root/leases/201-1.lock" && grep -qx "run_id=201" "$root/leases/201-1.lock" && grep -qx "run_attempt=1" "$root/leases/201-1.lock" && grep -qx "job=rust-gates" "$root/leases/201-1.lock" && grep -qx "runner=test-runner" "$root/leases/201-1.lock" && grep -q "^created=" "$root/leases/201-1.lock"'
check "second acquire of the same id is refused" '! run_mode acquire 201 1 > /dev/null 2>&1 || grep -q "status=unleased" "$work/out.201.acquire"'

echo "== same-host overlap"
h3="$(acquire 301)"; mktree 301-1
h4="$(acquire 302)"; mktree 302-1
h5="$(acquire 303)"
run_mode reclaim 303 1 > "$work/inline.log" 2>&1
check "inline reclaim by a third job keeps both overlapping live trees" 'alltree 301-1 && alltree 302-1 && alltree 201-1'
reclaim
check "scheduled reclaim keeps both overlapping live trees" 'alltree 301-1 && alltree 302-1'

echo "== cancellation, foreign, missing, malformed, symlink, unavailable liveness"
kill_job "$h3" 301; reclaim
check "cancelled job becomes reclaimable once its holder dies" 'notree 301-1'
check "the overlapping live job is still intact" 'alltree 302-1'
foreign="$(acquire 401 EffortlessMetrics/ripr)"; mktree 401-1; kill_job "$foreign" 401
mktree 402-1
mktree 403-1; printf 'garbage\n' > "$root/leases/403-1.lock"
mktree 404-1; printf 'schema=ci-scratch-lease/v1\n' > "$root/leases/404-1.lock"
mkdir -p "$root/target/ripr-77-1" "$root/tmp/not-an-id"; touch -d '2 hours ago' "$root/target/ripr-77-1"
outside="$work/outside"; mkdir -p "$outside/keep"; printf 'precious\n' > "$outside/keep/file"
ln -s "$outside" "$root/target/405-1"; h6="$(acquire 405)"; kill_job "$h6" 405
reclaim
check "terminal lease owned by another repository is skipped" 'alltree 401-1'
check "tree without a lease is skipped" 'alltree 402-1'
check "trees with malformed or ownerless leases are skipped" 'alltree 403-1 && alltree 404-1'
check "entries with malformed names are skipped" '[ -d "$root/target/ripr-77-1" ] && [ -d "$root/tmp/not-an-id" ]'
check "symlink entry is left and its target untouched" '[ -L "$root/target/405-1" ] && grep -q precious "$outside/keep/file"'
check "foreign and unknown skips are counted" 'grep -q "skipped_foreign=3" "$work/reclaim.log" && grep -q "skipped_unknown=1[0-9]" "$work/reclaim.log"'
mv "$root/tmp" "$work/real-tmp"; ln -s "$outside" "$root/tmp"; mkdir -p "$outside/501-1"; h7="$(acquire 501)"; kill_job "$h7" 501
reclaim
check "symlinked category root is never traversed" '[ -d "$outside/501-1" ] && grep -q precious "$outside/keep/file"'
rm "$root/tmp"; mv "$work/real-tmp" "$root/tmp"
h8="$(acquire 502)"; mktree 502-1; kill_job "$h8" 502
mkdir -p "$work/nobin"
PATH="$work/nobin" GITHUB_REPOSITORY="$REPO" GITHUB_OUTPUT="$work/out.noflock" CI_SCRATCH_ROOT="$root" CI_SCRATCH_MODE=reclaim "$BASH" --noprofile --norc -eo pipefail "$script" > /dev/null 2>&1
check "unavailable liveness tool reclaims nothing" 'alltree 502-1 && grep -q "status=flock_unavailable" "$work/out.noflock"'
GITHUB_REPOSITORY= GITHUB_OUTPUT="$work/out.noowner" CI_SCRATCH_ROOT="$root" CI_SCRATCH_MODE=reclaim bash --noprofile --norc -eo pipefail "$script" > /dev/null 2>&1
check "unknown reclaiming repository reclaims nothing" 'alltree 502-1 && grep -q "status=owner_unknown" "$work/out.noowner"'

echo "== concurrent cleaners"
live="$(acquire 600)"; mktree 600-1
for i in $(seq 601 640); do h="$(acquire "$i")"; mktree "$i-1"; kill_job "$h" "$i"; done
( set +e; run_mode reclaim 900 1 > "$work/gcA.log" 2>&1; echo $? > "$work/gcA.rc" ) &
( set +e; run_mode reclaim 901 1 > "$work/gcB.log" 2>&1; echo $? > "$work/gcB.rc" ) &
wait
check "both concurrent cleaners exit 0" '[ "$(cat "$work/gcA.rc")" = 0 ] && [ "$(cat "$work/gcB.rc")" = 0 ]'
check "concurrent cleaners never delete a live tree" 'alltree 600-1 && alltree 302-1 && alltree 201-1'
check "concurrent cleaners reclaim every terminal tree" 'notrees 601 640 && notree 502-1'
a="$(sed -n 's/.*reclaimed_ids=\([0-9]*\).*/\1/p' "$work/gcA.log")"; b="$(sed -n 's/.*reclaimed_ids=\([0-9]*\).*/\1/p' "$work/gcB.log")"
check "each terminal id is reclaimed exactly once across cleaners" '[ $((a + b)) -eq 41 ]'
cat "$work/gcA.log" "$work/gcB.log"

echo "== own-run release"
run_mode release 201 1 > /dev/null 2>&1
check "release keeps the lease while own trees remain" '[ -f "$root/leases/201-1.lock" ]'
for c in cargo-home target tmp; do rm -rf "$root/$c/201-1"; done
run_mode release 201 1 > /dev/null 2>&1
check "release drops the lease after own cleanup" '[ ! -e "$root/leases/201-1.lock" ]'
check "release reports its lease as held while its holder lives" 'grep -qx "own_lease=held" "$work/out.201.release"'

echo "== own lease self-check"
run_mode reclaim 302 1 > /dev/null 2>&1
check "reclaim reports its lease as held while its holder lives" 'grep -qx "own_lease=held" "$work/out.302.reclaim" && alltree 302-1'
h9="$(acquire 700)"; mktree 700-1; kill_job "$h9" 700
run_mode reclaim 700 1 > "$work/lost.log" 2>&1
check "reclaim flags a lost own lease" 'grep -qx "own_lease=lost" "$work/out.700.reclaim" && grep -q "lease_lost" "$work/lost.log"'
check "reclaim never deletes its own run's trees" 'alltree 700-1'
for c in cargo-home target tmp; do rm -rf "$root/$c/700-1"; done
run_mode release 700 1 > "$work/lost-release.log" 2>&1
check "release flags a lease lost before job end" 'grep -qx "own_lease=lost" "$work/out.700.release" && grep -q "lease_lost" "$work/lost-release.log"'
check "release still drops a lost lease" '[ ! -e "$root/leases/700-1.lock" ]'
exit "$fail"
"#;

    /// Every scenario label the harness must report as `ok`, so a skipped or
    /// renamed scenario cannot pass by absence.
    const SCENARIOS: &[&str] = &[
        "fixture: top-level dir is older than 30 minutes",
        "fixture: nested output was written just now",
        "retired selector deletes the active tree",
        "live leased tree aged 40 minutes with fresh writes survives",
        "owned terminal orphan is reclaimed",
        "terminal lease is removed with its trees",
        "live lease is retained",
        "reclaim reports ids, entries, bytes, and live skips",
        "lease records owner identity",
        "second acquire of the same id is refused",
        "inline reclaim by a third job keeps both overlapping live trees",
        "scheduled reclaim keeps both overlapping live trees",
        "cancelled job becomes reclaimable once its holder dies",
        "the overlapping live job is still intact",
        "terminal lease owned by another repository is skipped",
        "tree without a lease is skipped",
        "trees with malformed or ownerless leases are skipped",
        "entries with malformed names are skipped",
        "symlink entry is left and its target untouched",
        "foreign and unknown skips are counted",
        "symlinked category root is never traversed",
        "unavailable liveness tool reclaims nothing",
        "unknown reclaiming repository reclaims nothing",
        "both concurrent cleaners exit 0",
        "concurrent cleaners never delete a live tree",
        "concurrent cleaners reclaim every terminal tree",
        "each terminal id is reclaimed exactly once across cleaners",
        "release keeps the lease while own trees remain",
        "release drops the lease after own cleanup",
        "release reports its lease as held while its holder lives",
        "reclaim reports its lease as held while its holder lives",
        "reclaim flags a lost own lease",
        "reclaim never deletes its own run's trees",
        "release flags a lease lost before job end",
        "release still drops a lost lease",
    ];

    struct HarnessRun {
        success: bool,
        transcript: String,
    }

    fn action_script() -> Result<String, String> {
        let action = repo_text(SCRATCH_LEASE_ACTION_PATH)?;
        let blocks = crate::extract_workflow_run_blocks(&action);
        let [block] = blocks.as_slice() else {
            return Err(format!(
                "{SCRATCH_LEASE_ACTION_PATH} must have exactly one run block, found {}",
                blocks.len()
            ));
        };
        if !block.text.contains("ci-scratch-lease reclaim:") {
            return Err("extracted run block is not the lease authority".to_string());
        }
        Ok(block.text.clone())
    }

    fn run_harness(script: &str, label: &str) -> Result<HarnessRun, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("clock before epoch: {err}"))?
            .as_nanos();
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "ripr-xtask-ci-scratch-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
        let script_path = dir.join("lease.sh");
        fs::write(&script_path, script)
            .map_err(|err| format!("write {}: {err}", script_path.display()))?;
        let output = Command::new("bash")
            .arg("--noprofile")
            .arg("--norc")
            .arg("-c")
            .arg(HARNESS)
            .arg("ci-scratch-harness")
            .arg(&script_path)
            .env("TMPDIR", &dir)
            .output()
            .map_err(|err| format!("spawn bash: {err}"));
        let _ = fs::remove_dir_all(&dir);
        let output = output?;
        let mut transcript = String::from_utf8_lossy(&output.stdout).into_owned();
        transcript.push_str(&String::from_utf8_lossy(&output.stderr));
        Ok(HarnessRun {
            success: output.status.success(),
            transcript,
        })
    }

    #[test]
    fn lease_authority_keeps_live_trees_and_reclaims_owned_terminal_orphans() -> Result<(), String>
    {
        let run = run_harness(&action_script()?, "authority")?;
        for scenario in SCENARIOS {
            assert!(
                run.transcript.contains(&format!("ok   {scenario}\n")),
                "scenario `{scenario}` did not pass:\n{}",
                run.transcript
            );
        }
        assert!(!run.transcript.contains("FAIL "), "{}", run.transcript);
        assert!(run.success, "{}", run.transcript);
        Ok(())
    }

    fn assert_mutation_is_caught(replacement: &str, label: &str) -> Result<(), String> {
        let script = action_script()?;
        if !script.contains(LOCK_PROBE) {
            return Err(format!("mutation anchor `{LOCK_PROBE}` missing"));
        }
        let run = run_harness(&script.replacen(LOCK_PROBE, replacement, 1), label)?;
        assert!(
            !run.success,
            "mutation `{label}` was not caught by the harness:\n{}",
            run.transcript
        );
        for scenario in [
            "live leased tree aged 40 minutes with fresh writes survives",
            "concurrent cleaners never delete a live tree",
        ] {
            assert!(
                run.transcript.contains(&format!("FAIL {scenario}\n")),
                "mutation `{label}` did not trip `{scenario}`:\n{}",
                run.transcript
            );
        }
        Ok(())
    }

    #[test]
    fn negative_control_without_the_lock_check_deletes_live_trees() -> Result<(), String> {
        assert_mutation_is_caught("if false; then", "no-lock")
    }

    #[test]
    fn negative_control_with_an_mtime_liveness_test_deletes_live_trees() -> Result<(), String> {
        assert_mutation_is_caught(
            "if [[ -n \"$(find \"$entry\" -maxdepth 0 -mmin -30)\" ]]; then",
            "mtime",
        )
    }
}
