# Operator Signal Integrity Closeout

Date: 2026-07-29

Campaign: `operator-signal-integrity` (Campaign 35)

## Objective

Ensure operator-facing disclosures, gate failures, and GitHub annotations
preserve the actual limitation or severity instead of silently disappearing,
being downgraded, or hiding the first actionable reason.

## Landed work

- #2720 detects confined gitlink additions, deletions, and changes, forces
  internally generated diffs to use short submodule output, and discloses that
  submodule contents are not analyzed.
- #2721 surfaces the first config, blocking-gap, or exception-policy reason in
  the failing gate error while retaining the full JSON/Markdown report.
- #2722 maps annotation severity to GitHub `warning`/`notice`, but its merged
  implementation initially missed literal `warning` values. #2726 adds the
  literal-warning regression and preserves the warning level.

## Proof receipt

Final repair merge SHA: `d5f15338` from PR #2726, final head `03891bf5`.

Required proof for #2726 passed on the exact final head: `Ripr Rust Small
Result`, CX53 hosted Rust gates, source-of-truth, tests, coverage, dependency,
security, and `ub-review`. The first coverage attempt failed in an unchanged
doctor test; current-main coverage passed and the same-head coverage rerun
passed, so the first red is recorded as a flaky/baseline signal rather than a
repair failure. The original implementation PRs also passed their required
hosted proof before merge. Local formatting passed; the focused xtask test
command did not complete in the Windows checkout after the Cargo contention
timeout and is not claimed as local proof.

## Claim boundary

Users may claim that these operator surfaces disclose the selected limitation,
first gate reason, and annotation severity. They may not infer runtime
mutation, test or coverage adequacy, release readiness, or default blocking.

PR #2718's broad LSP severity inversion merged independently before this
closeout and is not part of Campaign 35's acceptance or proof claim.

Issue #2675 is closed with this receipt after #2720 and #2726 merged; #2599 and
#2632 were already closed by their merged implementation PRs.
