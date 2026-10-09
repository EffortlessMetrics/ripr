# Agent-as-user runs

A coding agent is given an installed `ripr`, a small real repository whose
latest commit adds a feature with a weak test, and nothing else: no ripr
docs, no ripr source, no hint about the gap. It is told to fix what ripr
flags. An evaluator then checks, with real mutants, whether the tests the
agent wrote actually catch wrong versions of the feature.

This is an informal, repeatable usability probe. It is **not** the blind
installed-agent acceptance owned by #4600: it runs a workspace build rather
than a frozen successor candidate, keeps no `BlindJourneyReceiptV1`, and its
targets are seeded by the evaluator. Use it to find product friction early;
route each finding to its owning issue.

## Setup

### Targets

Each target is an upstream crate at a pinned commit plus one seeded "gap"
commit that adds a feature and a smoke-only test. The commit message of the
gap commit is the only thing the agent is told about the change.

| Target | Upstream commit | Gap commit message |
|---|---|---|
| `hyunsik/bytesize` | `66a3715e33369e99cb428bcd5aee32291f8e0a94` | `feat: add ByteSize::as_whole_units` |
| `chronotope/humantime` | `76c8929b4cc286f675322475a8e1841f35bafc57` | `feat: accept fortnight as a duration unit` |
| `dtolnay/semver` | `280ebcb6edac3aa4cdc545dbff8a26c5ac4861fe` | `feat: add Version::next_minor` |

Gap patches (apply with `git apply`, then commit on a branch named `gap`):

<details><summary>bytesize</summary>

```diff
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -252,6 +252,15 @@ impl ByteSize {
         self.0
     }

+    /// Returns how many whole `unit`s this size holds, or `None` when `unit`
+    /// is zero or the size is not an exact multiple of `unit`.
+    pub const fn as_whole_units(&self, unit: u64) -> Option<u64> {
+        if unit == 0 || self.0 % unit != 0 {
+            return None;
+        }
+        Some(self.0 / unit)
+    }
+
     /// Returns byte count as kilobytes.
     #[inline(always)]
     pub fn as_kb(&self) -> f64 {
@@ -527,6 +536,11 @@ where
 mod core_tests {
     use super::*;

+    #[test]
+    fn test_as_whole_units() {
+        assert!(ByteSize::kib(2).as_whole_units(KIB).is_some());
+    }
+
     #[test]
     fn test_arithmetic_op() {
         let mut x = ByteSize::mb(1);
```

</details>

<details><summary>humantime</summary>

```diff
--- a/src/duration.rs
+++ b/src/duration.rs
@@ -261,6 +261,7 @@ impl Parser<'_> {
             Unit::Hour => (n.mul(3600)?, 0),
             Unit::Day => (n.mul(86400)?, 0),
             Unit::Week => (n.mul(86400 * 7)?, 0),
+            Unit::Fortnight => (n.mul(86400 * 14)?, 0),
             Unit::Month => (n.mul(2_630_016)?, 0), // 30.44d
             Unit::Year => (n.mul(31_557_600)?, 0), // 365.25d
         };
@@ -281,6 +282,7 @@ impl Parser<'_> {
                 Unit::Hour => (n.mul(3600)?.div(d)?, 0),
                 Unit::Day => (n.mul(86400)?.div(d)?, 0),
                 Unit::Week => (n.mul(86400 * 7)?.div(d)?, 0),
+                Unit::Fortnight => (n.mul(86400 * 14)?.div(d)?, 0),
                 Unit::Month => (n.mul(2_630_016)?.div(d)?, 0), // 30.44d
                 Unit::Year => (n.mul(31_557_600)?.div(d)?, 0), // 365.25d
             };
@@ -311,6 +313,7 @@ enum Unit {
     Hour,
     Day,
     Week,
+    Fortnight,
     Month,
     Year,
 }
@@ -328,6 +331,7 @@ impl FromStr for Unit {
             "hours" | "hour" | "hr" | "hrs" | "h" => Ok(Self::Hour),
             "days" | "day" | "d" => Ok(Self::Day),
             "weeks" | "week" | "wk" | "wks" | "w" => Ok(Self::Week),
+            "fortnights" | "fortnight" => Ok(Self::Fortnight),
             "months" | "month" | "M" => Ok(Self::Month),
             "years" | "year" | "yr" | "yrs" | "y" => Ok(Self::Year),
             _ => Err(()),
@@ -474,6 +478,11 @@ mod test {
     use super::Error;
     use super::{format_duration, parse_duration};

+    #[test]
+    fn test_fortnight() {
+        assert!(parse_duration("2fortnights").is_ok());
+    }
+
     #[test]
     #[allow(clippy::cognitive_complexity)]
     fn test_units() {
```

</details>

<details><summary>semver</summary>

```diff
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -396,6 +396,19 @@ impl Version {
         }
     }

+    /// Returns the next minor release: increments `minor`, resets `patch` to
+    /// zero and clears pre-release and build metadata.
+    #[must_use]
+    pub fn next_minor(&self) -> Self {
+        Version {
+            major: self.major,
+            minor: self.minor + 1,
+            patch: 0,
+            pre: Prerelease::EMPTY,
+            build: BuildMetadata::EMPTY,
+        }
+    }
+
     /// Create `Version` by parsing from string representation.
     ///
     /// # Errors
--- a/tests/test_version.rs
+++ b/tests/test_version.rs
@@ -249,3 +249,9 @@ fn test_align() {
     assert_eq!("*****1.2.3-rc1******", format!("{:*^20}", version));
     assert_eq!("           1.2.3-rc1", format!("{:>20}", version));
 }
+
+#[test]
+fn test_next_minor() {
+    let next = version("1.2.3").next_minor();
+    assert_eq!(next.minor, 3);
+}
```

</details>

### Answer key

Three hand-written mutants per target. Each is one textual replacement in the
feature; a tree "catches" a mutant when `cargo test` fails with it applied.
Before the run all nine are missed by the gap commit's own test. Score with
a positive control first: a tree carrying one known-good test (for semver,
`assert_eq!(version("1.2.3-a+b").next_minor(), version("1.3.0"))`) must catch
its target's mutants, or the scorer is broken.

| Target | Mutant | Replacement |
|---|---|---|
| bytesize | zero-guard | `unit == 0 \|\| self.0 % unit != 0` → `self.0 % unit != 0` |
| bytesize | exact-guard | `unit == 0 \|\| self.0 % unit != 0` → `unit == 0` |
| bytesize | quotient | `Some(self.0 / unit)` → `Some(self.0)` |
| humantime | int-factor | whole-number arm `86400 * 14` → `86400 * 7` |
| humantime | frac-factor | fractional arm `86400 * 14` → `86400 * 7` |
| humantime | singular | `"fortnights" \| "fortnight"` → `"fortnights"` |
| semver | patch-reset | `patch: 0` → `patch: self.patch` |
| semver | pre-clear | `pre: Prerelease::EMPTY` → `pre: self.pre.clone()` |
| semver | build-clear | `build: BuildMetadata::EMPTY` → `build: self.build.clone()` |

`cargo mutants --in-diff` over the feature hunks is a second, generated
oracle. It produces few mutants for struct literals (three for semver, two of
them caught by the gap test), so the hand-written key carries the semver
result.

### Instrument

Put a logging shim first on the agent's `PATH` so the evaluator keeps every
invocation independently of the agent's self-report. The shim must keep
stdout and stderr on separate streams: an earlier version merged them, which
put a stderr notice into a redirected `--format repo-exposure-json` file and
made `ripr outcome` fail to parse it. That failure was the shim's, not ripr's.

```bash
#!/usr/bin/env bash
log="${AAU_LOG:?}"
o=$(mktemp); e=$(mktemp)
/path/to/real/ripr "$@" >"$o" 2>"$e"; rc=$?
{ printf '\n### %s cwd=%s rc=%s\n$ ripr' "$(date -u +%H:%M:%S)" "$PWD" "$rc"
  printf ' %q' "$@"
  printf '\n--- stdout\n'; cat "$o"; printf '\n--- stderr\n'; cat "$e"; } >>"$log"
cat "$o"; cat "$e" >&2; rm -f "$o" "$e"; exit $rc
```

Keep the answer key, mutant output and logs outside any directory the agent
may read. Give each agent its own `AAU_LOG` path: the shim appends a header and
the output in separate writes, so agents sharing a log would interleave.

### Agent brief

One fresh agent per target, same text, only the paths substituted:

```text
You are a Rust developer using a tool called `ripr` for the first time. You
have never seen its documentation or source.

Repository: {REPO_DIR}
The most recent commit on the current branch added a small feature. Your job:
use `ripr` to find places where the existing tests may not catch the changed
behavior, and fix what ripr flags by adding or strengthening tests. Use your
own judgment about when you are done.

Rules:
- Start every shell command with `source {ENV_FILE} && ` (it puts ripr on PATH
  and cd's into the repo).
- Learn ripr only from `ripr --help`, `ripr help <command>`, and what ripr
  itself prints. Do not look for ripr's source code, docs, or anything on the
  internet about ripr.
- Do not read or write anything outside {REPO_DIR}.
- Only change test code. Do not change the feature's production code, even if
  you think it has a bug (report it instead).
- You may run `cargo test` and other ordinary cargo commands. Do not install
  tools.

When finished, hand back: every ripr command you ran and whether it answered
what you wanted; the gaps ripr named; what you changed and which finding each
change addresses; every point of friction, quoting the exact output; the
final ripr state and `cargo test` result; anything that looked like a bug in
the feature.
```

## Run 2026-10-04

ripr `0.11.0 (a7a089e1c51dfa51a1727eeefe7e561aaecd7abc)`, release build,
Linux. Three agents, one per target, run in parallel. No agent was given any
help beyond the brief.

### Outcome

| Target | ripr commands | Answer key before → after | `cargo mutants --in-diff` after | ripr after (`--worktree`) |
|---|---|---|---|---|
| bytesize | 17 | 0/3 → 3/3 | 10/10 caught | 1 exposed, 2 weakly_exposed |
| humantime | 21 | 0/3 → 3/3 | 6/6 caught (1 unviable) | 2 exposed, 2 weakly_exposed, 1 static_unknown |
| semver | 20 | 0/3 → 3/3 | 2/2 caught (1 unviable) | 5 exposed, 1 reachable_unrevealed |

ripr steered every agent to the real gap: each finished with tests that catch
every answer-key mutant, and no agent edited production code. On the
evaluator side (`cargo mutants` before and after each run, kept with the run
logs outside the repo), ripr never reported `exposed` for a probe whose
mutants survived. All three agents ended with findings they judged,
correctly, to be ripr limitations rather than missing tests.

This run's logs were written by an earlier shim that merged stdout and
stderr; the shim above is the corrected one (see Instrument).

### Where the commands went

Counted from the shim logs (one `### HH:MM:SS` header per invocation), by
what each invocation was for:

| Purpose | bytesize | humantime | semver | Avoidable by |
|---|---|---|---|---|
| Orientation (`--help`, `doctor`, `help check`) | 3 | 3 | 3 | none; expected |
| First `check` and drill-in (`human-full`, `explain`, `context`) | 4 | 4 | 4 | the `explain` call, which added nothing to human-full (#5356) |
| Looking for a repair route (`help agent`, `agent repair`/`status`, `help pilot`, `pilot`) | 3 | 4 | 3 | a route from a `check` finding into repair (#5355) |
| Re-check that read HEAD and showed no change | 1 | 1 | 1 | reading the worktree when dirty (#5358) |
| After adding tests: re-checks, explains, `help`, and bytesize's pilot `outcome` pair | 6 | 9 | 9 | most followed verdicts that stayed weak under tests that catch every mutant (#5352, which also covers bytesize's `observation_unverified` case; #5353) |
| **Total** | **17** | **21** | **20** | |

Without that friction a run needs about seven or eight commands: the three
orientation calls, one `check`, two drill-ins (`human-full` and `context`),
and one or two re-checks after the test is written. Counting the post-test
loop beyond two commands as avoidable (a read of the logs, not a
measurement), the issues above account for 9 (bytesize), 13 (humantime) and
12 (semver) of the commands, leaving 8 in each run. Bytesize's post-test
loop includes an `outcome` call that failed because of the shim, not ripr.

### Findings

1. **The re-check after adding a test reads HEAD.** All three agents re-ran
   `ripr check` after editing tests, saw identical output, and lost a cycle
   before finding `--worktree`. The note at the end of the output
   (`uncommitted source and test changes were not analyzed`) was read by one
   agent and missed by two. The header names `mode` and `root` but not the
   base or the head it analyzed. `ripr --help` describes the loop as "you add
   one focused test -> ripr records whether the gap closed", which the
   default does not do for an uncommitted test.
   Tracked in #5358; the decision comment there records reading the
   worktree by default when the tree is dirty and naming base and head in
   the header. Not implemented when this was written.
2. **Tests that discriminate stay `weakly_exposed` when they don't quote the
   changed text.** Measured:
   - humantime: exact-value tests through `parse_duration` catch all answer-key
     mutants and all `cargo mutants` mutants, yet both `parse_unit` arms stay
     `weakly_exposed` (`reach weak: No test is seen calling parse_unit`).
     The agent flipped them to `exposed` only by writing a white-box test that
     constructs the private `Parser` and calls `parse_unit`. In an evaluator
     re-run with that one test removed, both arms return to `weakly_exposed`
     while every mutant is still caught. ripr rewarded an
     implementation-coupled test over the end-to-end tests that already did
     the job.
   - bytesize: `return None` and `Some(self.0 / unit)` stay `weakly_exposed`
     (`observation_unverified: no assertion text references this probe's
     changed expression`) under exact `assert_eq!(…, None)` and
     `assert_eq!(…, Some(bytes / unit))` checks that kill every mutant.
   - semver: the `return_value` probe stays `reachable_unrevealed` with
     `rust_assertion_context_unestablished` even for a new test file holding
     only `assert_eq!(Version::new(1, 2, 3).next_minor(), Version::new(1, 3, 0));`.
     In the same run, the field probes rate the same test file's
     `assert_eq!(next.minor, 3)` as a strong exact-value oracle.
3. **Fix sites and related tests come from name or file proximity.**
   `ripr context` proposed `test_format_micros` (humantime), `test_default`
   (bytesize) and `tests/test_identifier.rs` `test_prerelease`
   (`weak_token_substring`, semver) as the place to fix. The human top gap for
   humantime listed `all_86400_seconds` (`same_test_file`) as "Related test
   (1 of 129)" ahead of the test that calls the feature.
4. **A `ripr check` finding cannot start the advertised repair loop.**
   `ripr --help` lists `ripr agent repair --seam-id ID` under "Repair one named
   gap", but `check` prints only `probe:` IDs, and `ripr pilot` ranks seams
   repo-wide: in two of three runs it recommended seams unrelated to the diff
   and said none could start a repair. One agent used pilot's "after" command
   and `ripr outcome` successfully; pilot's named missing value (`0`, equality
   boundary) was the most actionable advice any agent received.
5. **`ripr explain <id>` repeats the `--format human-full` block** for that
   finding and adds only a `Next:` line. Two agents ran it expecting more.
6. **Smaller output defects.**
   - `--format human-full` cut a short source line mid-token
     (`BuildMetadata::EM` / `PTY,`) when a multi-line changed fragment
     passed the 180-column budget in total. Fixed in #5293.
   - For the bytesize predicate, `context` reports the stage
     `"discriminate": "yes"` (infection is the weak stage) while `explain`
     prints the verdict line `discriminator not established`. Both may be
     right, but the agent read them as a contradiction.
   - Constants are labelled `source enum variant value crate::KIB`.
   - Pilot's suggested assertion calls a method as a free function:
     `assert_eq!(as_whole_units(/* boundary input where unit == 0 */), …)`.

## Run 2026-10-04, second pass

ripr `0.11.0 (d19bc3caa6ffe1d4bb9cd6bc64f013f77069f31d)`, the merge of #5423,
which adds `ripr agent stub` and a `Write a test for it:` line under the top
`check` finding. Same targets, brief and shim as the first pass; fresh agents
and fresh copies of the gap commits. #5352, #5353, #5356 and #5358 were still
open.

| Target | ripr commands (pass 1 → 2) | Answer key after | `cargo mutants --in-diff` after | ripr after (`--worktree`) |
|---|---|---|---|---|
| bytesize | 17 → 22 | 3/3 | 10/10 caught | 1 exposed, 2 weakly_exposed |
| humantime | 21 → 29 | 3/3 | 6/6 caught (1 unviable) | 2 exposed, 2 weakly_exposed, 1 static_unknown |
| semver | 20 → 24 | 3/3 | 2/2 caught (1 unviable) | 5 exposed, 1 reachable_unrevealed |

Outcomes did not change: every agent closed the gap, and ripr again reported
no `exposed` probe whose mutants survived. Commands per fix went up. The new
stub route accounts for part of it: agents ran `ripr agent stub` five times,
four of them as suggested by `check`, and got no stub (#5471). The refusals
were `ambiguous_test_module`, `owner_unsupported`, `observer_required`,
`field_type_unresolved`, and, after `check --worktree`, `no reported gap is in
the function`. Humantime's count also includes eight
scripted `--json` re-checks while the agent bisected which test shape moved a
verdict.

New in this pass: replacing a smoke `assert!(parse_duration("2fortnights").is_ok())`
with an exact `assert_eq!` moves humantime's `FromStr` return_value probe from
`weakly_exposed` to `reachable_unrevealed` (reproduced by the evaluator;
recorded on #5353). The humantime agent again wrote a white-box `parse_unit`
test to move the private arms to `exposed` (#5352).

## Run 2026-10-04, third pass

ripr `0.11.0 (f147c8ee5db98d4f038b0e16d157a7fc5c5ad9b8)`, current main with
#5424 (named tests and miss reasons in `explain`) and #5569 (opaque instead
of false gaps). Same targets, brief and shim; fresh agents and fresh copies of
the gap commits. #5352, #5353 and #5358 were still open.

| Target | ripr commands (pass 1 → 2 → 3) | Answer key after | `cargo mutants --in-diff` after | ripr after (`--worktree`) |
|---|---|---|---|---|
| bytesize | 17 → 22 → 20 | 3/3 | 10/10 caught | 1 exposed, 2 weakly_exposed |
| humantime | 21 → 29 → 35 | 3/3 | 6/6 caught (1 unviable) | 2 exposed, 2 weakly_exposed, 1 static_unknown |
| semver | 20 → 24 → 20 | 3/3 | 2/2 caught (1 unviable) | 5 exposed, 1 reachable_unrevealed |

Outcomes again did not change, and the final verdict counts on all three
targets match passes 1 and 2, so #5569 changed nothing measurable
here. #5424 did: in its final report the humantime agent called the "Why this verdict"
block the most useful output ripr gave.

The stub route now produces stubs: four of five calls returned one, against
none in pass 2. Three of those four do not help, and one call still fails:

- bytesize (three calls): `--at src/lib.rs:258` stubbed the predicate on that
  line. After the agent's first test, `check` suggested
  `agent stub --at src/lib.rs:259` for the `return None` probe. That stub
  also targets the line-258 predicate, and `--at 261` printed it again
  (#6298).
- humantime: the `parse_unit` arm's value leaves through `out: &mut Duration`,
  but the stub asserts only the `Result`, which is `Ok(())` either way. Filled
  in honestly, it passes on the arm mutant (#6300).
- semver: `--at src/lib.rs:403`, as suggested by `check --worktree`, was
  still refused with `no reported gap is in the function` (#5458).

Humantime's 35 commands include 15 `explain` calls: one first drill-in,
eleven while the agent worked out which test shape moved a verdict, and
three right after one surprise.
Rewriting the `matches!` assertions in `test_fortnight_unit_from_str`, which
does not call `parse_unit`, moved both `parse_unit` arms from `exposed` to
`weakly_exposed`. The evaluator reproduced this in a minimal crate. The
token that confirms observation can come from any related test while the
oracle's strength comes from another, so the `exposed` before the edit was
credited by a test that cannot observe the arm (#6297). Each agent again lost one
re-check to reading HEAD before switching to `--worktree` (#5358).
