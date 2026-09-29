# Exit Codes

ripr uses a three-value exit code contract:

| Code | Meaning |
|------|---------|
| `0`  | The command completed. |
| `2`  | The command could not complete: usage, parse, operational, or internal error. |
| `3`  | The command ran successfully and reached a blocking decision or a typed refusal. |

`ripr check` is advisory: it exits `0` whether or not it found gaps, including
`exposed` and `weakly_exposed` findings. A non-zero exit from `check` means the
analysis did not complete, not that it disapproved of your diff. The blocking
decision belongs to `ripr gate evaluate`, which exits `3` when the gate blocks
after writing its report. Do not build a CI gate on `check`'s exit code; read
its output, or run the gate.

## Why codes 2 and 3?

ripr uses exit code `2` for all could-not-complete conditions (not `1`) to
distinguish it from shell-level errors (which typically use `1`). Code `3` is
reserved for outcomes where the command itself ran to a deliberate, typed
answer — a blocking gate decision, or a typed refusal such as `ripr agent
verify-execute` declining a packet (the refusal JSON document is on stdout)
— so an orchestrator can branch on the exit status alone:

- `0`: the command completed (for `verify-execute`, the verification ran;
  read the disposition in the stdout JSON).
- `3`: the command completed by reaching a blocking decision or typed
  refusal; read the report or the stdout JSON document for the answer
  (standalone `ripr agent verify` is the exception: its refusal is named on
  stderr and stdout stays empty; see below).
- `2`: the invocation or operation failed; retrying differently is
  appropriate.

## When you see exit code 2

- **Analysis error**: the diff could not be read (a missing or unreadable
  `--diff` path, or a directory), the diff exceeded the hard
  `RIPR_MAX_DIFF_*` guards (`diff_scope_oversized` with no analysis run), the
  base ref could not be resolved, or the workspace root could not be
  determined. A diff that is read but does not parse (no file headers or
  hunks) is not an exit-2 error: `ripr check` exits `0` with the typed
  outcome `unsupported_input (analysis incomplete)`. A run that analyzed only
  part of its scope also exits `0`, with `partial_with_limitations`: a diff
  over the smaller partial budget (its limitation is also named
  `diff_scope_oversized`), a changed Rust file the parser refused and read
  lexically, or a changed file whose language adapter is unavailable. The
  parser follows stable Rust, so a changed file using nightly-only syntax it
  cannot parse (guard patterns, never patterns) also makes the run partial.
- **User error**: unknown command, missing required argument, or invalid
  config.
- **Internal error**: a panic occurred (with a `ripr: internal error` message).
- **Closed output pipe**: the reader of stdout went away early (for example
  `ripr doctor | head`). ripr stops quietly with `2`, not `0`, because its
  output was cut short; it never turns a would-be `3` into a pass. Output
  small enough to be written in full before the reader closes never meets
  the closed pipe, so that run keeps its own exit code.

## When you see exit code 3

- **Gate failure**: `ripr gate evaluate` evaluated successfully and blocked
  the PR; the full report was still written to `--out`.
- **Typed refusal**: `ripr agent verify-execute` refused the packet (for
  example `verification_rejected_policy` or `verification_wrong_root`) and
  rendered the typed refusal JSON on stdout; `ripr agent repair --phase
  after` refused with a named cause after selecting its attempt — a diverged
  HEAD, drifted analysis inputs, a no-movement verify refusal, or a replaced
  trust-binding manifest — with the cause named on stderr, the refusal
  recorded on the attempt, and one JSON document on stdout (the
  `repair_after_refusal` document naming the cause and recovery when the
  refusal came before the verify render, otherwise the bare agent verify
  document). Operational errors after attempt selection (an unreadable
  retained packet or manifest, a failed artifact write) still exit `2`.
- **Typed verify refusal**: standalone `ripr agent verify` refused the pair
  for drifted analysis inputs (`analysis input identities differ`) or no
  repository movement between the artifacts — the same named refusals the
  repair after phase maps to `3`. This is the one exit-`3` path whose stdout
  stays empty: `agent verify`'s stdout is the verify artifact (the packet's
  `next` loop redirects it into `agent-verify.json`), and a rejected verify
  renders nothing to it (RIPR-SPEC-0134). The named cause is on stderr.
  Other verify rejections (unreadable or invalid artifacts, lineage or
  metadata mismatches) exit `2`.

These are findings- and policy-driven exits, not operational failures; a
monitoring system should page on `2`, not on `3`.

Some refusals still exit `2` because the command could not do what was asked:
`ripr receipt check` when the receipt is orphaned or its gap does not match
the ledger (the verdict is in the `--json` document), when a named `--ledger`
cannot be read, or when `--gap` names a different gap than the receipt;
`ripr agent receipt` when the attempt is not receipt-ready; `ripr agent
repair --phase verify` without its explicit authorization or on a moved
tree; and a repair `--phase after` whose edit cage recorded a violation.

## `ripr doctor` exit codes

`ripr doctor` uses the same contract: `0` when all checks pass, `2` when
any check fails (including missing language runtimes that are enabled in
the effective configuration).

The `Cargo.toml`, `cargo`, and `rustc` checks apply only when Rust is in
scope for the root: Rust is enabled and either Rust markers (`Cargo.toml` or
`.rs` files) are detected or no other language is detected or enabled. A
Python-only or TypeScript-only root reports those checks as `skipped` with
the reason and does not fail on them. A Rust root, a root with Rust sources but no
`Cargo.toml`, and an empty root under the Rust-only default still fail on a
missing manifest.

The toolchain checks depend on the profile. Under the default `analysis`
profile, a missing `cargo` or `rustc`, or a workspace `rustc` older than
RIPR's build MSRV, is reported as `advisory` and does not change the exit
code: the installed binary can still analyze the workspace. A missing `cargo`
still withholds evidence that reads `cargo metadata`. Under
`--profile source-build`, the same conditions are failures and exit `2`, while
enabled language runtimes stay visible but do not decide that profile's exit.

## CI integration

The GitHub Actions workflow that `ripr init --ci github` generates preserves
the exit code:

```bash
check_status=0
ripr check \
  --root . \
  --base "origin/${{ github.base_ref }}" \
  --format json > target/ripr/pr/check.json || check_status=$?
```

The `|| check_status=$?` pattern captures the exit code without failing the
step, so downstream review-comments and gate steps can consume the
output even when the analysis failed. Because `check` exits `0` on findings,
a non-zero `check_status` here means the analysis itself did not complete
(code `2`; a gate step consumes code `3` as its blocking signal).

A `check` that exits `0` can still be incomplete. `review-comments` carries
the check's analysis outcome, and `ripr gate evaluate` treats an
incomplete, partial, or unsupported outcome as a `config_error` in every
mode and exits `2`, so a gate never passes on a partial denominator.
