# RIPR-SPEC-0086: Python Tier A External-Repo Eval Sweep

Status: accepted

Acceptance note (2026-08-14): #1161 landed the Tier A `cargo xtask
eval-sweep` command (`xtask/src/command.rs` dispatch plus
`xtask/src/reports/eval_sweep.rs`). The run algorithm's classification
contract was completed by #3259, which threads the captured exit status
into `classify` — a nonzero exit after parseable JSON reads `crash`, as
step 3 requires, pinned by classifier and `run_check` boundary tests on
both hosts. The Required Evidence golden exists as
`rendered_report_matches_golden_from_fixed_run_vector`: byte-exact JSON
and Markdown from a fixed two-run vector (stable `ok`, unstable
`parse_failure`), so every rendering change must re-bless it
deliberately. Tier B judgment semantics live in RIPR-SPEC-0092, which
remains proposed.

Acceptance note (2026-09): #3565 landed the accepted-artifact validator
(`cargo xtask eval-sweep check`, `xtask/src/reports/eval_sweep_check.rs`)
and #3566 landed the managed candidate refresh (`cargo xtask eval-sweep
refresh`, `xtask/src/reports/eval_sweep_refresh.rs`) — the first
schema-0.3 producer, self-validated through the #3565 loader before any
candidate is written, with the refresh-produces-what-check-validates
symmetry proved offline by the `python_eval_sweep_refresh` tests over
synthetic local subjects. The live eight-repository network refresh is a
manual, authorized operation. #3567 landed the promotion step
(`cargo xtask eval-sweep report`,
`xtask/src/reports/eval_sweep_report.rs`): deterministic candidate
acceptance into immutable content-addressed receipts plus a mechanical
currentness gate.

Owner: language-adapter / swarm

Linked proposal:

- None. This is a standalone evidence-tooling contract; it adds no product
  library behavior and no public API. It anchors the eval-sweep-driven Python
  reliability campaign tracked by
  [#1160](https://github.com/EffortlessMetrics/ripr-swarm/issues/1160) and
  `plans/python-repair-routing/` (the former `.ripr/goals/` tracker was
  deleted with the goals scheduler, #2056).

Linked ADRs:

- [ADR 0009](../adr/0009-python-parser-substrate.md) (Python parser substrate;
  the sweep measures the current `rustpython-parser`-backed lane).

Linked plan:

- [RIPR-PLAN-0017: Python Repair Routing Implementation Plan](../../plans/python-repair-routing/implementation-plan.md)

Linked issues:

- [release(py): Python usable-tier readiness checklist](https://github.com/EffortlessMetrics/ripr-swarm/issues/1160)
- [eval(py): validate the retained eight-repository sweep manifest and run schema](https://github.com/EffortlessMetrics/ripr-swarm/issues/3565)
- [eval(py): add a managed currentness-bound external sweep refresh](https://github.com/EffortlessMetrics/ripr-swarm/issues/3566)

Linked PRs:

- [#1161](https://github.com/EffortlessMetrics/ripr-swarm/pull/1161) — Tier A
  command and report implementation.
- [#3259](https://github.com/EffortlessMetrics/ripr-swarm/pull/3259) — captured
  exit-status classification and boundary regressions.
- [#3261](https://github.com/EffortlessMetrics/ripr-swarm/pull/3261) — fixed-run
  JSON/Markdown golden test and accepted lifecycle reconciliation.

## Problem

The in-repo Python dogfood corpus is saturated: every metric passes on a curated
set authored by the same people who wrote the analyzer. That confirms the repair
loop is internally consistent, but it says nothing about how `ripr check` behaves
on Python repositories we did not write. The release-readiness question —
**does the analyzer stay crash-free, parse-robust, and gap-ID-stable on external
code?** — has no measured answer.

This spec defines a **Tier A external-repo eval sweep**: a report-only `xtask`
command that runs `ripr check` over a pinned manifest of real external Python
repositories and records only machine-checkable robustness facts. Tier A is a
stability floor; it deliberately does **not** judge actionability or usefulness
(that is Tier B, a later spec).

## Behavior

### One production delta

Add `cargo xtask eval-sweep`: a report-only command. It introduces no change to
`crates/ripr` (the analyzer library) — it is automation that exercises the
existing `ripr check` surface and aggregates results.

### Inputs

- A pinned manifest (`fixtures/python-eval-sweep/manifest.json`): a versioned
  envelope listing external repos as `{ id, url (https), sha, license, shape,
  synthetic_diff?, why }`.
- A synthetic Python diff per repo (or a shared fallback diff). The diff is read
  from a file; the external repo working tree is never mutated.

### Run algorithm (per manifest entry)

1. Resolve the repo checkout. When `--clone` is passed, clone at the pinned
   `sha` into a `target/`-local checkout dir via the existing
   `run::run_with_envs` helper; otherwise expect a pre-placed checkout and record
   `skipped_missing_checkout` if absent (never fails on absence).
2. Run `ripr check --root <checkout> --diff <synthetic-diff> --mode fast --json`
   under a wall-clock timeout via `run::capture_output_with_timeout`. The harness
   builds `ripr` once and invokes the built binary directly (not `cargo run`), so
   `runtime_ms` measures analysis time rather than cargo's per-invocation overhead.
3. Classify the outcome from exit code and JSON:
   - `crash` — process abort or `ripr: <err>` failure exit;
   - `timed_out` — exceeded the timeout;
   - `parse_failure` — JSON parsed but the file degraded to a named static-unknown
     limitation (graceful, **not** a crash);
   - `ok` — exit 0 with well-formed JSON.
4. Collect the set of `canonical_gap_id` values and the run-1 runtime. From the
   same run-1 JSON, also tally — per repo and aggregate — the `classification`
   distribution (the 7 exposure classes), and, where the Python sink-alignment
   fields are emitted (RIPR-SPEC-0028), the `oracle_alignment` distribution and
   repair-packet presence counts. No extra invocation: these are read from the
   already-captured JSON.
5. Re-run steps 2–4 once; gap-ID stability is `set(run1) == set(run2)`. The
   distributions are taken from run-1 only; the re-run is for gap-ID stability.

### Metrics

Across non-skipped entries: `crash_rate`, `parse_failure_rate`, `timed_out_count`,
runtime min/median/max/total, and `gap_id_stability_rate`. The run also records,
across the `counts_as_run` set, the `classification_counts` and `alignment_counts`
distributions. **Distributions are descriptive only: they are informational, make
no usefulness/actionability judgement, and never affect `gate_status`.** The
`gate_status` is:

- `not_run` when `repos_run == 0` (zero repos analyzed — e.g. a default no-clone
  run with no pre-placed checkouts). A pass/review verdict is only meaningful once
  at least one repo was analyzed, so an empty run is **never** a vacuous `pass`.
- `pass` when `repos_run >= 1`, `crash_rate == 0`, and `gap_id_stability_rate == 1.0`.
- `review` when `repos_run >= 1` and the crash/stability gate failed.

Each carries a recorded reason. Empty `repos_run` guards division (rates default
to `0.0` crash / `1.0` stability).

### Accepted-manifest and retained-receipt validation (`eval-sweep check`)

`cargo xtask eval-sweep check [--manifest <path>] [--runs <receipt>]` is the
typed semantic validator for the accepted artifacts (offline: no repository
materialization, no RIPR execution, no lookups beyond the two artifact files):

- One loader owns manifest and run-row semantics. The strict
  duplicate-key-rejecting parse is the check-path contract; the lenient
  in-memory parse in the sweep-report path stays historical-tolerant on
  purpose (check validates retained accepted artifacts, the report path
  renders in-flight sweeps), and the two converge when #3566/#3567 own the
  refresh/report commands. The accepted manifest
  (`python_eval_sweep_manifest`) must declare schema/kind/spec/tier and
  **exactly eight** uniquely identified subjects — the canonical denominator.
  Subject content is data-driven: any eight well-formed subjects validate, not
  only the retained fixture bytes. The manifest schema is closed (deny-unknown):
  the owned top-level keys are `schema_version`/`kind`/`spec`/`tier`/
  `description`/`limits`/`synthetic_diff`/`repos` and the owned per-subject
  keys are `id`/`url`/`sha`/`license`/`shape`/`synthetic_diff`/`why` plus the
  optional `tree_digest`/`snapshot`/`provenance`/`retention_class` identities —
  exactly the keys the canonical fixture carries, so unknown keys are schema
  rot, not forward compatibility. License is required (null/empty fails); an
  optional identity is either absent (typed `incomplete`) or well-formed — an
  explicit null, an empty string, or a malformed value fails, because a
  present-but-garbage identity is not an absent one.
- Retained run receipts (`python_eval_sweep_report`) validate in two owned
  shapes. Schema `0.2` is the historical shape the sweep command writes.
  Schema `0.3` adds the currentness identities: binary/features/config/profile/
  input identity, materialization/detection/corpus-selection/execution states,
  the `complete`/`partial`/`parse-failed`/`timed-out`/`crashed`/`unsupported`/
  `tempfail`/`stale` status vocabulary, raw/output/evidence digests,
  repeat-run comparison identity, and a sha256 manifest-digest binding.
  Schema `0.3` is deliberately ahead of the live `0.2` producer — no sweep
  command writes it yet — and producer parity is expected to land with
  #3566/#3567, so a 0.3 receipt validates here as the accepted target shape
  before any producer emits it.
- Fail closed (nonzero exit, diagnostic names subject/field/reason plus the
  deterministic rerun command) on: duplicate or missing subjects, a changed
  denominator, receipt rows contradicting the manifest pins, a receipt
  identity that contradicts the manifest binding (when the manifest and the
  receipt both record a comparable identity — `license`, `tree_digest`,
  `snapshot`, `provenance`, `retention_class` — they must match; a receipt
  value with no manifest side to bind discloses `incomplete` on the manifest
  side instead of fabricating a binding), unsafe
  (non-portable, absolute, or secret-bearing) paths and URLs, unknown state
  vocabulary, contradictory status (e.g. `complete`/`partial` with an
  execution state that did not run — `partial` with `not-executed` included —
  or stable gap IDs listed as unstable in either direction, or a false
  stability claim whose `unstable_gap_ids` list is omitted; each at the 0.2
  row level or inside the 0.3 `repeat` block), duplicate identity copies
  inside one receipt that disagree (the row-level `tree_digest`/`snapshot` vs
  the `repository` block, and a row's `binary` identity vs the receipt-level
  `ripr` block — a mismatch fails naming both locations), a known field whose
  recorded value has the wrong type or shape for its emitted shape
  (`description`/`gate_reason`/`why` are strings, `stderr_excerpt` is a
  possibly-empty string, `gap_ids`/`limits` are arrays of strings), malformed digests,
  a stale manifest digest (receipt bound to different manifest bytes), a
  required summary aggregate missing from an analyzed receipt, a nonzero
  analysis-bearing aggregate on a zero-run receipt, and
  hand-edited aggregates that disagree with the derived rows (denominator,
  outcome counts, stability counts, runtime aggregates, rates, distributions,
  gate status). Aggregate agreement is checked in both directions: an analyzed
  (run-status) row must carry the aggregate source evidence the sweep records
  on every row (`runtime_ms`, both distributions, and the 0.2 `gap_ids_stable`;
  0.3 stability lives in the optional `repeat` block, so a recorded summary
  stability value over rows lacking it fails while an unrecorded one is
  disclosed `incomplete`), an analyzed receipt must carry the full emitted
  summary (a deleted aggregate would silently disable its row-agreement check;
  the stability aggregates are required exactly when the rows fully evidence
  stability), summary distributions must equal the row-derived
  key set exactly (zero-valued buckets included; with zero run rows the
  zero-run summary law bounds every recorded bucket to zero and the recorded
  keys are still vocabulary-checked), the supplied `gate_status` must
  equal the gate derived from the rows (`not_run` at zero runs, `pass` only
  with zero crashes and full per-row stability evidence, `review` otherwise),
  and runtime totals and distribution merges use checked arithmetic (overflow
  is a structured failure naming the aggregate field, never a panic).
- Failed, unavailable, timeout, parse-failed, unsupported, partial, and stale
  rows remain selected: they are valid rows and stay in the denominator. Of
  the eight 0.3 statuses, exactly `complete`/`partial`/`parse-failed`/
  `timed-out`/`crashed` evidence an analysis attempt and count toward
  `repos_run`; `unsupported`/`tempfail`/`stale` do not. A row
  that did not run must carry no analysis counts, and `repos_run == 0` gates to
  `not_run` — never a vacuous `pass`, and a `pass` claim requires per-row
  stability evidence and zero crashes. The not-a-vacuous-pass law extends to
  the summary: with zero run rows every analysis-bearing aggregate
  (classification/alignment counts, runtime min/median/max/total, stability
  counts) must be zero or absent, and the stability rate must be zero, absent,
  or exactly the live emitter's zero-run default `1.0` — that one value is
  what the emitter itself records when nothing ran (its empty-set guard), so
  it is accepted as a named `incomplete` disclosure (`vacuous zero-run
  stability rate`) instead of a failure, and the disclosed receipt verdict
  stays `incomplete`/`not_run`, never a pass; any other nonzero rate is a
  fabricated claim about rows that never ran.
- Missing identities are typed `incomplete` with a per-field disclosure; they
  are not invented and not errors (the retained manifest carries no
  provenance/retention/snapshot identities by design, and historical 0.2 rows
  carry no currentness fields). Missing owned identity fields — `ripr.*` at
  receipt level, and every owned field of a present row-level `binary` block
  (`digest`/`version`/`features`/`build_profile`) — disclose `incomplete` the
  same way; present-but-malformed values fail. An explicit null is present,
  not missing: the receipt-side identity fields the binding and copy checks
  cover (the row-level binding identities
  `tree_digest`/`snapshot`/`license`/`retention_class`/`provenance`, the
  receipt-level `ripr.*` fields, and the fields of a present row
  `repository`/`binary` block) fail naming the field when null — the same
  present-but-garbage rule the manifest-side optional identities follow —
  while a key left out discloses `incomplete`. Validation never upgrades or
  rewrites a historical receipt. `alignment_counts` is required on every
  analyzed (run-status) row, and a recorded `alignment_counts` object must
  always carry both keys — `absent` (field not emitted) and `unknown`
  (emitted value), zero-filled when empty — so the two never merge.
- Exit contract: exit 0 when every present artifact is structurally valid —
  including when identities are disclosed `incomplete` or no receipt is
  supplied (`not_run`); nonzero on any fail-closed violation. Top-level
  verdict precedence spans both artifacts: `not_run` only when no receipt is
  supplied; with a receipt, `incomplete` whenever the manifest or the receipt
  discloses incomplete identities (a complete receipt never hides manifest
  gaps), and `valid` only when both artifacts are structurally valid and carry
  zero incompletes. The verdict
  vocabulary is `valid` / `incomplete` / `not_run`: a structural
  currentness-readiness verdict, never a robustness or adequacy claim. The
  check writes `eval-sweep-check.{json,md}` only.

### Managed currentness refresh (`eval-sweep refresh`, #3566)

`cargo xtask eval-sweep refresh --manifest <path> --ripr-bin <path> --out <dir>
--allow-network` is the managed route that reruns the accepted denominator
against one explicit RIPR binary and writes CANDIDATE artifacts outside
accepted/current state. The historical 0.2 sweep receipt is not current for
promotion; this route produces the currentness-bound candidate that #3567
validates and publishes.

- **Authorization gate (load-bearing).** The route refuses closed without BOTH
  the managed env signal `RIPR_EVAL_SWEEP_NETWORK=1` and the explicit
  `--allow-network` flag, before any filesystem work; the typed refusal names
  the missing signals. Ordinary CI never refreshes live repositories, and the
  offline refusal is itself testable.
- **One loader, one validator.** Refresh consumes #3565's validated subjects
  (`eval_sweep_check::validate_accepted_manifest` — the resolved
  `synthetic_diff` identity is retained on each subject) and never re-parses
  the manifest. Before writing any candidate, the route self-validates its own
  schema-0.3 receipt through `eval_sweep_check::validate_run_receipt`, so a
  produced candidate and `eval-sweep check --runs <candidate>` agree by
  construction. That refresh-produces-what-check-validates symmetry is the
  route's core proof, exercised offline with synthetic local subjects.
- **Candidate separation.** `--out` is mandatory and rejected when it equals
  or overlaps accepted state (the `fixtures/` tree, or the repository root).
  The comparison runs on canonicalized paths on BOTH sides — `--out` and the
  accepted-state roots (canonicalized at the deepest existing ancestor when
  the candidate leaf does not exist yet) — so a symlinked `--out` resolving
  into accepted state is refused, and a canonicalization failure is a typed
  refusal, never a skip. A candidate refresh cannot rewrite expected status,
  subject selection, the historical receipt, or the current pointer; only
  #3567 can promote a candidate into accepted state.
- **Explicit binary.** `--ripr-bin` is mandatory and must name an existing
  file; the route resolves it to an absolute path before any invocation, so
  PATH can never select an installed binary. The route records the binary's
  sha256 digest, its reported version, and (when the parent directory names a
  cargo build profile) the build profile, and re-verifies the digest after
  EACH subject's run: a content drift (concurrent rebuild) or an unreadable
  binary is a typed `tempfail` row disposition named in the managed execution
  receipt — a run is never attributed to a stale binary identity. The
  analyzer source SHA and feature set of an arbitrary supplied binary have no
  producer here and stay typed incomplete.
- **Materialization.** Per subject, the pinned tree is materialized into
  `<out>/subjects/<id>`: a prior candidate directory is reused only when
  `git rev-parse HEAD` verifies the exact pin (after a bounded detached
  re-checkout when HEAD drifted) AND `git status --porcelain` is empty — a
  reused checkout carrying local modifications or untracked files cannot have
  its content attributed to the accepted sha and is `stale`, never analyzed
  as the pinned tree; a status command that fails is fail-closed `stale` for
  the same reason. Otherwise a local seed checkout under
  `--checkout-root` is cloned through git's local transport (no network), and
  only then is the manifest URL cloned over the network. Every failure is a
  typed disposition that keeps the subject selected: an unverifiable or
  unmaterializable pin is `stale` (materialization state `failed`), a
  clone/checkout infrastructure failure is `tempfail` (state `failed`), and
  an unusable synthetic diff is `tempfail` with materialization state
  `skipped` — no clone is spent on an input that cannot be analyzed.
- **Terminal states stay distinct.** The eight 0.3 statuses are derived from
  producer facts with no two merged: `timed-out` (rail-enforced deadline),
  `crashed` (failure exit or non-JSON stdout), `unsupported`
  (`unsupported_input` analysis kind), `parse-failed` (`analysis_failed` or
  degradation to a named static-unknown limitation, the same producer facts
  the 0.2 sweep reads), `complete` (producer-reported completeness),
  `partial` (an attempt without reported completeness), `tempfail`, `stale`.
  Exactly the five run statuses count toward `repos_run`; a failed or partial
  row retains its available evidence and can never count as complete.
  Post-materialization infrastructure failures — cache-dir creation, analysis
  spawn, raw-evidence retention — are contained the same way: each becomes a
  typed `tempfail` row over the verified facts with its cause named in the
  managed execution receipt, so the route ALWAYS produces the full eight-row
  candidate denominator instead of aborting.
- **Per-subject retention.** Each candidate row carries: the repository
  identity restating the accepted pin (`url`/`sha`; manifest-carried
  `tree_digest`/`snapshot`/`provenance`/`retention_class` are copied through,
  never invented); the selected root (recorded out-relative so every receipt
  path stays portable) and layout tag; the binary identity block; the config
  identity, recorded honestly (`--mode fast`; `subject-ripr-toml` with the
  relative path when the materialized subject root carries its own
  `ripr.toml` — the configuration `ripr check --root` actually loads —
  `default` only when the file is genuinely absent, `unobserved` when no tree
  was materialized to inspect) and the synthetic-diff
  input path with its sha256 `input_digest`; the materialization/detection/
  execution/corpus-selection states with source/test/generated/vendor counts
  from a bounded working-set walk (`partial` at the cap — never a silently
  truncated count); phase status, timeout, exit, completeness, and
  limitations in the managed execution receipt; each pass's raw stdout/stderr
  retained under `<out>/raw/` with real raw/output/evidence digests; and the
  classification/alignment distributions (descriptive, never gating).
- **Stability law.** The route runs a second pass ONLY where the first result
  is `complete` — the one state where a comparison is meaningful; the gate
  lives inside the stability pass itself, so a non-complete first result
  never enters the repeat phase and never claims stability (equal failure
  gap sets of two crashed passes are not `stable`). It records the gap-ID
  comparison in the row's `repeat` block (stable, or a typed
  `unstable_gap_ids` mismatch list). The repeat pass's raw stdout AND stderr
  are retained under `<out>/raw/`, and the retained second-pass stderr is
  digested in the row's `repeat.repeat_stderr`. Raw-output identity across
  passes is compared too; drift with stable gap identity is a typed
  execution-receipt note, never folded into the gap verdict.
- **Determinism.** Equivalent managed reruns over the same inputs produce
  identical identities, digests, and rows; wall-clock telemetry is the only
  run-varying field and is declared as such in the execution receipt, which
  names the binary, manifest (path + sha256 + exact subject ids), host class,
  and network authorization.
- **Process hygiene.** Every spawn — `ripr check`, `git clone`/`checkout`/
  `rev-parse`, the version probe — routes through the allowlisted
  `crate::run` bounded-capture helpers (wall-clock timeout, captured
  stdout/stderr, process-tree termination, cwd anchored inside the candidate
  tree, isolated per-subject `RIPR_CACHE_DIR`, terminal prompts disabled for
  git). No new process-spawn surface is introduced.
- **Live path.** The real eight-repository network refresh is operated
  MANUALLY with both authorization signals and an explicit built binary; no
  automated test performs a live network run. The offline tests prove the
  route end to end over synthetic local subjects whose real git HEADs are the
  manifest pins.
- **Claim boundary.** A candidate receipt is structural currentness evidence
  over the retained denominator. It is not accepted promotion evidence; no
  structural-accuracy, repair-correctness, gate, badge, or support claim is
  inferred from it.

### Accepted-receipt publication and the currentness gate (`eval-sweep report`, #3567)

`cargo xtask eval-sweep report` is the deterministic promotion step that turns
one #3566 candidate into accepted evidence. Three surfaces, all offline:

- **Dry run.** `report --candidate <receipt.json> [--dispositions <path>]`
  validates the candidate through the exact `eval-sweep check` semantics
  (`validate_run_receipt` — one validator owns receipt semantics), then
  renders the accepted receipt JSON and a bounded Markdown report derived
  from the SAME validated rows to `target/ripr/reports/`. No accepted state
  is written.
- **Acceptance.** `... --accept` appends the immutable accepted receipt
  `<state-dir>/receipts/<receipt-sha256>.json` (content-addressed over the
  exact written bytes; an existing file is never overwritten — identical
  bytes are an idempotent no-op, different bytes are a typed refusal), the
  paired Markdown, and the retained candidate
  (`receipts/<candidate-sha256>.candidate.json`, addressed by the
  candidate's own digest), then atomically updates the current pointer
  `<state-dir>/current.json`. Previously accepted artifacts are never
  rewritten, so historical receipts remain immutable and separately
  addressable by digest. Acceptance refuses non-0.3 candidates with a typed
  refusal (a 0.2 historical receipt carries no currentness identities to
  bind).
- **Currentness gate.** `report --check-currentness
  [--ripr-bin <path>] [--ripr-source-sha <sha>]` recomputes the identities
  the pointer binds against current state and reports `current`, `stale`,
  `unverifiable`, or `not_run` (no pointer; never a pass).
  `current`/`unverifiable`/`not_run` exit 0 with the non-current verdicts
  disclosed in full; `stale` exits nonzero — the gate signal that the
  accepted receipt must be re-accepted before promotion consumption.

The current pointer contains NO independently editable totals — only
identity: the accepted receipt's digest and portable filename, an optional
as-of disclosure string, the manifest digest, the toolchain identity block
(source sha, binary digest, features, build profile), the command-contract
version, and per-subject bound identities (tree digest, accepted-row digest,
input digest, config identity).

- **The currentness law is mechanical.** The check recomputes: the accepted
  receipt's file digest, every retained-candidate row digest, the manifest
  file digest (compared over the raw file bytes BEFORE the manifest is
  parsed or validated, so a moved manifest whose new bytes also fail
  accepted-state validation reaches the promised `stale` verdict with both
  reasons named — the digest movement and the validation failure — instead
  of aborting the verdict path with a schema error), the
  retained-candidate re-validation against the CURRENT
  accepted manifest (which fails on any tree-pin/license/manifest movement),
  the pointer's bound toolchain and per-subject identity copies, the
  analyzer source sha (`--ripr-source-sha`, else bounded
  `git rev-parse HEAD`), the binary bytes (`--ripr-bin`), the per-subject
  input bytes (the manifest-declared synthetic diff, resolved
  manifest-relative then repo-root-relative), and the command-contract
  version. Any movement flips the verdict to `stale` with the exact reason
  named. A pointer that binds no toolchain identity field (`features`,
  `build_profile`) discloses that identity `unverifiable` with the field
  named: a missing identity never leaves the verdict `current`. Editing the
  pointer's as-of string can never repair staleness:
  as-of is never an input to the comparison. Identities the loader cannot
  recompute offline are disclosed `unverifiable` — never assumed current —
  and the check-path policy is fail-closed: the pointer's schema is
  deny-unknown, its digests are format-checked, its portable paths are
  component-checked (no empty components from consecutive separators, no
  `.`/`..` components, never a separator-only path, so a crafted pointer
  cannot walk outside the accepted state directory), and a pointer naming a
  missing artifact is a typed integrity failure.
- **Accepted receipt content** (all derived from the validated rows — the
  receipt never carries a hand-entered total, and every count is emitted as
  `{numerator, denominator}` with the denominator each count's contract
  defines and no denominator-free number: the top-level counts over the
  eight-subject denominator, each outcome and each distribution bucket over
  the selected denominator, each health tally over the selected rows it
  tallies, and the runtime-envelope count over the analyzed (run) rows —
  the `min_ms`/`median_ms`/`max_ms`/`total_ms` fields are duration
  measurements, not counts): the selected/materialized/available/stale/
  license-blocked/tempfail counts over the eight-subject denominator; the
  complete/partial/parse-failed/timed-out/crashed/unsupported/tempfail/stale
  outcome split; project-detection and corpus-selection health tallies with
  the unrecorded share disclosed; the runtime envelope where reliable (every
  run row carries a runtime) and an explicit `unavailable` disclosure
  otherwise; repeat-run identity stability with the per-subject mismatch
  list (unstable gap IDs); the classification and oracle-alignment
  distributions (each bucket a numerator/denominator count; informational,
  never gating; a bucket sum below its selected denominator is the honest
  shape, since non-run rows contribute no buckets) plus a named disclosure
  that no limitation distribution exists (the 0.3 row schema records none);
  per-subject identity, evidence digests, and dispositions; source/binary/
  feature/config/input/manifest/candidate/accepted-receipt identities; the
  candidate's own typed incomplete disclosures (copied verbatim, with the
  candidate's host display path replaced by a portable subject label); and
  the promotion-relevant non-claims embedded in the artifact itself.
- **Dispositions.** Non-complete subjects carry one typed terminal
  disposition from the owned vocabulary (`reproduced-current`,
  `dispositioned-current`, `infrastructure-tempfail`,
  `upstream-pin-unavailable`, `unsupported-input`,
  `historical-not-reproduced`), supplied through an optional strict-JSON
  sidecar (`--dispositions`) because the 0.3 row schema is deny-unknown and
  acceptance-time judgment has no place in it. Every owned disposition type
  is terminal and actionable by definition, so a non-empty evidence
  reference, owner, and recovery route are REQUIRED on every disposition;
  dispositions for complete rows, unknown ids, duplicates, unknown
  vocabulary, hygiene violations, and notes over a 512-character bound all
  fail closed. The bounded Markdown escapes pipe characters and flattens
  newlines in rendered disposition rows, so a free-text value cannot split a
  table cell (the receipt JSON keeps the raw value). The retained historical
  two parse failures and one timeout
  are dispositioned through this machinery when a current candidate lands:
  a fresh candidate row with the same failure is `reproduced-current`;
  otherwise the failure is explicitly dispositioned against current source
  via `dispositioned-current`/`historical-not-reproduced`. The live
  reproduction run remains the manual, authorized #3566 refresh.
- **Accepted-artifact hygiene.** The rendered receipt, pointer, and Markdown
  are scanned before any byte is written: secret-shaped tokens (the shared
  validator tripwire list, reused — never forked), absolute host paths
  (a Windows drive-letter prefix whose letter is not preceded by another
  letter, so URL schemes never trip while a Windows path does; POSIX-absolute
  JSON string values; `file://`), and
  oversized free text all fail closed.
- **Claim boundary.** Acceptance establishes a current, reproducible
  operational-robustness denominator over the retained eight external
  Python subjects. It does not establish repair correctness and does not
  authorize any support-tier change; robustness and distribution metrics
  stay informational and never become judged accuracy.

### Policy boundary (load-bearing)

- `--clone` is **opt-in and off the default CI path.** No `.github/workflows`
  step clones or fetches. The default command runs against pre-placed checkouts
  only, so the gated `check-pr` path never touches the network.
- All subprocess work routes through the already-allowlisted `xtask/src/run.rs`
  helpers, so no `process_allowlist.txt` or `network_allowlist.txt` change is
  required.

## Required Evidence

- This spec, registered in `policy/doc-artifacts.toml` and `docs/specs/README.md`.
- A `[[behavior]]` entry in `.ripr/traceability.toml` mapping this spec to the
  unit tests and the manifest fixture.
- `fixtures/python-eval-sweep/{SPEC.md, manifest.json, synthetic-diff.diff}`.
- Unit tests in `eval_sweep.rs`: manifest load/validate (rejects duplicate ids,
  non-https url, empty repos); outcome classifier over sample JSON; gap-ID set
  comparison flags an injected instability; metrics arithmetic with empty-set
  guards; deterministic JSON/markdown report rendering.
- A golden of the rendered report from a fixed in-memory run vector.
- The `eval-sweep check` validator tests (`python_eval_sweep` module): an
  alternate valid eight-subject manifest in a temp directory proves the
  validator is data-driven; each fail-closed shape (duplicate/missing/unknown
  subjects, changed denominator, non-https or credential-bearing URLs,
  absolute and secret-bearing paths, unknown shape tags, malformed SHAs,
  null/empty/malformed optional manifest identities, present-null
  receipt-side identity fields (row-level binding identities, `ripr.*`, and
  the fields of a present `repository`/`binary` block), unknown
  outcome/status/state values, contradictory status (including `partial` with
  `not-executed`, and a false stability claim with its `unstable_gap_ids`
  list omitted), disagreeing duplicate identity copies (row-level
  tree/snapshot vs the `repository` block, and a row `binary` field vs its
  receipt-level `ripr` copy), wrong-typed owned fields
  (`gate_reason`/`gap_ids`/`stderr_excerpt`/`description`/`limits`/`why`),
  stale and
  malformed digests, a receipt identity contradicting the manifest binding,
  a required summary aggregate missing from an analyzed receipt, a nonzero
  analysis-bearing aggregate on a zero-run receipt (the live emitter's vacuous
  zero-run stability rate `1.0` excepted: exactly that value discloses
  `incomplete` as a vacuous zero-run rate while any other nonzero rate fails),
  hand-edited aggregates,
  vacuous pass, pass without
  stability evidence, merged absent/unknown distributions) fails with a
  subject/field/reason diagnostic and the rerun command; missing identities
  (including `ripr.features`/`binary.features`, and a present `binary` block
  without `build_profile`) are typed incomplete, and a
  receipt value with no manifest side to bind discloses the manifest gap
  instead of fabricating a binding; a zero-run receipt gates `not_run` (the
  emitter-shaped zero-run receipt with the vacuous `1.0` rate validates as
  `incomplete`, never a pass); the run/non-run split over the eight 0.3
  statuses is pinned per status; the
  historical 0.2 receipt validates with disclosed incompletes and is never
  rewritten.
- The managed refresh route tests (`python_eval_sweep_refresh` module in
  `eval_sweep_refresh.rs`): the typed authorization refusal (env signal and
  flag, each missing alone, and a wrong env value) fires before any
  filesystem work; the authorized route requires `--ripr-bin` naming an
  existing file and a `--out` candidate directory; `--out` overlapping
  accepted state (`fixtures/`, the repo root, or an ancestor) is rejected
  while a dedicated directory under `target/` is accepted; rows assembled by
  the route's own `assemble_row` over the full eight-status vocabulary
  validate through `validate_run_receipt` (denominator retained, run/non-run
  split exact, crashed rows keep the derived gate at review, unstable repeat
  claims carry their `unstable_gap_ids` list); row and summary assembly are
  deterministic over identical inputs; corpus counting uses real path-shaped
  producers with a bounded walk; stale and clone-failed materializations keep
  subjects selected without analysis counts; and the end-to-end symmetry
  proof — an authorized offline refresh over eight synthetic local seed
  clones with the real built binary produces a candidate receipt plus a
  managed execution receipt (binary digest, manifest sha256 + subject ids,
  host class, network authorization) that passes the full
  `eval-sweep check` artifact path, including a stale-subject run that keeps
  all eight subjects selected.
- The accepted-receipt publication tests (`python_eval_sweep_report` module
  in `eval_sweep_report.rs`): a valid candidate's dry run renders JSON and
  bounded Markdown that agree (same rows, same counts, numerator +
  denominator on every count, including every outcome, health tally,
  runtime-envelope count, and distribution bucket — no bare denominator-free
  number); acceptance appends immutably (a second accept
  adds a content-addressed receipt without mutating the first, the pointer
  moves to the newest, and re-accepting the same candidate is an idempotent
  no-op); the pointer's field set is exactly identity fields with no
  total/rate-shaped field anywhere in the tree; hand-edited candidate totals,
  missing/duplicate subjects, non-complete rows without dispositions,
  dispositions without owner/recovery route, dispositions for complete rows,
  duplicate/unknown disposition ids, unknown disposition vocabulary,
  absolute-path and secret-bearing notes, and oversized notes all fail
  closed; a schema-0.2 candidate is refused with a typed historical note and
  nothing is accepted; and the currentness law is mechanical end to end —
  binary-digest, manifest-digest, retained-candidate-row, input-bytes,
  pointer-config-copy, pointer-features-copy, and source-sha movements each
  flip the verdict to stale with the exact reason, a moved manifest whose
  new bytes also fail accepted-state validation flips stale with BOTH the
  digest-movement and validation-failure reasons (never a schema abort),
  editing the as-of string
  never repairs staleness (and never breaks a current pointer), a missing
  binary recompute input and a pointer binding no toolchain identity field
  (`features`/`build_profile`) each leave the verdict `unverifiable` (never
  `current`) with the gap named, pointer portable paths with empty/`.`/`..`
  components are refused naming the field while the normal receipt shape
  still reads, disposition values render table-safe (escaped pipes, flattened
  newlines) in the bounded Markdown while the JSON keeps the raw value, no
  pointer is `not_run`, and stability mismatch reasons plus
  the derived counts/health/runtime envelope are recorded in the accepted
  receipt.

## Non-Goals

- No actionability, usefulness, false-actionable, or false-`exposed`
  (over-credit) judgement (that is Tier B). A robustness sweep counts emitted
  findings, so it is structurally blind to false-`exposed`: a silent over-credit
  emits nothing. Measuring it needs ground-truthed should-stay-quiet cases, not
  only boundary-flip diffs that exercise the should-gap direction.
- The `classification_counts` and `alignment_counts` distributions count emitted
  facts only. They make no usefulness, actionability, false-actionable, or
  false-`exposed` judgement, and never change `gate_status` — they add
  visibility, not a verdict. `absent` (the alignment field was not emitted) is
  kept distinct from the `unknown` enum value so the distribution is not
  silently overcounted.
- No mutation execution, provider calls, generated tests, or production-code edits.
- No network access on the default CI path; external clone is opt-in only.
- No change to `crates/ripr` analyzer behavior or public API.
- No support-tier claim; this produces evidence, not a promotion.

## Acceptance Examples

### A passing sweep

```text
repos_run = 12, crash_rate = 0.0, parse_failure_rate = 0.08,
gap_id_stability_rate = 1.0  ->  gate_status = "pass"
```

### A sweep that fails closed to review

```text
repos_run = 12, crash_rate = 0.08 (1 repo aborted)  ->  gate_status = "review",
reason = "1/12 repos crashed; investigate before promotion"
```

### Missing checkout without --clone

```text
repo skipped, outcome = "skipped_missing_checkout"  ->  excluded from rates,
never fails the command
```

### A default no-clone run analyzes nothing

```text
repos_total = 3, repos_run = 0 (all skipped_missing_checkout)
  ->  gate_status = "not_run"  (never a vacuous "pass")
```

### Accepted validation without a retained receipt

```text
cargo xtask eval-sweep check
  ->  manifest valid (8 subjects), receipt dimension "not_run",
      missing identities disclosed as incomplete  ->  exit 0 (not a pass)
```

### A hand-edited aggregate fails closed

```text
receipt rows derive repos_run = 8 but the summary claims 7
  ->  exit nonzero, diagnostic: subject=<receipt> field=`summary.repos_run`:
      hand-edited aggregate ... (rerun: cargo xtask eval-sweep check)
```

## Test Mapping

- `eval_sweep::manifest_load_rejects_invalid` -> manifest validation contract.
- `eval_sweep::classifier_maps_outcomes` -> outcome classification contract.
- `eval_sweep::gap_id_instability_detected` -> gap-ID stability contract.
- `eval_sweep::metrics_guard_empty_run_set` -> metrics arithmetic contract.
- `eval_sweep::report_render_is_deterministic` -> deterministic report rendering.
- `eval_sweep::count_distributions_tallies_classification` -> classification distribution.
- `eval_sweep::count_distributions_uses_classification_key_not_class` -> reads the real `classification` key.
- `eval_sweep::count_distributions_oracle_alignment_buckets` -> alignment distribution (`absent` ≠ `unknown`).
- `eval_sweep::count_distributions_counts_packet_completeness_presence` -> packet-presence counts.
- `eval_sweep::report_includes_distribution_and_gate_is_unaffected` -> distributions render and never change the gate.
- `eval_sweep::distribution_does_not_rescue_not_run_gate` -> `not_run` preserved.
- `eval_sweep::run_check_classifies_failure_exit_with_valid_json_as_crash` ->
  captured failure-exit boundary through the real run path.
- `eval_sweep::rendered_report_matches_golden_from_fixed_run_vector` ->
  byte-exact JSON/Markdown rendering from the fixed two-run vector.
- `eval_sweep_check::python_eval_sweep::accepts_alternate_valid_manifest_not_fixture_bytes`
  -> data-driven eight-subject acceptance (#3565).
- `eval_sweep_check::python_eval_sweep::check_artifacts_passes_on_alternate_manifest_in_temp_dir`
  -> end-to-end offline check on an alternate temp-dir manifest.
- `eval_sweep_check::python_eval_sweep::rejects_manifest_with_seven_subjects_changed_denominator`
  -> exactly-eight canonical denominator.
- `eval_sweep_check::python_eval_sweep::receipt_rejects_unknown_subject_and_missing_subject`
  -> subject-coverage fail-closed family (with the duplicate-row sibling).
- `eval_sweep_check::python_eval_sweep::receipt_rejects_hand_edited_aggregates`
  -> row/aggregate denominator agreement.
- `eval_sweep_check::python_eval_sweep::emitter_shaped_zero_run_stability_rate_discloses_instead_of_failing`
  -> the live emitter's zero-run `1.0` stability rate validates as a vacuous
  zero-run disclosure (verdict stays `incomplete`), any other nonzero rate fails.
- `eval_sweep_check::python_eval_sweep::present_null_receipt_identities_fail_while_absent_discloses_incomplete`
  -> present-null receipt-side identity fields fail naming the field; the
  absent key discloses `incomplete`.
- `eval_sweep_check::python_eval_sweep::run_status_split_pins_which_statuses_count_as_run`
  -> the exact run/non-run denominator split over the eight 0.3 statuses
  (`complete`/`partial`/`parse-failed`/`timed-out`/`crashed` count toward
  `repos_run`; `unsupported`/`tempfail`/`stale` do not).
- `eval_sweep_check::python_eval_sweep::receipt_rejects_vacuous_pass_and_accepts_not_run`
  -> `repos_run == 0` is `not_run`, never a vacuous pass.
- `eval_sweep_check::python_eval_sweep::current_receipt_all_eight_statuses_validate_and_stay_selected`
  -> the full status vocabulary remains selected (denominator-preserving).
- `eval_sweep_check::python_eval_sweep::current_receipt_rejects_contradictory_status_pairs`
  -> contradictory status fail-closed family.
- `eval_sweep_check::python_eval_sweep::receipt_rejects_stale_manifest_digest`
  -> stale digest binding fail-closed.
- `eval_sweep_check::python_eval_sweep::receipt_keeps_absent_distinct_from_unknown_distributions`
  -> `absent` remains distinct from emitted `unknown`.
- `eval_sweep_check::python_eval_sweep::historical_receipt_validates_incomplete_without_rewrite`
  -> historical receipts stay historical; missing identities type incomplete.
- `eval_sweep_check::python_eval_sweep::top_level_verdict_is_incomplete_when_manifest_gaps_survive_a_complete_receipt`
  -> top-level verdict precedence across both artifacts (`valid` only when both
  carry zero incompletes; `not_run` only without a receipt).
- `eval_sweep_check::python_eval_sweep::gate_status_must_equal_the_derived_gate`
  -> supplied `gate_status` must equal the derived gate (`not_run`/`pass`/`review`).
- `eval_sweep_check::python_eval_sweep::summary_distribution_extra_bucket_fails`
  -> exact summary-distribution key-set equality (zero-valued buckets included).
- `eval_sweep_check::python_eval_sweep::analyzed_row_missing_runtime_fails`
  (with `analyzed_row_missing_distribution_fails_and_rejects_summary_totals` and
  `under_evidenced_stability_rejects_recorded_summary_and_discloses_absent`) ->
  missing aggregate source evidence never silently disables a summary comparison.
- `eval_sweep_check::python_eval_sweep::runtime_total_overflow_fails_structurally_without_panic`
  (with `distribution_merge_overflow_fails_structurally_without_panic`) ->
  checked aggregate arithmetic.
- `eval_sweep_check::python_eval_sweep::manifest_rejects_unknown_top_level_and_repo_keys`
  (with `canonical_fixture_manifest_passes_with_owned_keys_only`) -> closed
  accepted-manifest schema.
- `eval_sweep_check::python_eval_sweep::receipt_rejects_absolute_and_secret_bearing_paths`
  (with `rejects_non_https_and_credential_urls` and
  `current_receipt_rejects_malformed_digests_and_paths`) -> portable-path,
  no-secret, and digest-format fail-closed family.
- `eval_sweep_refresh::python_eval_sweep_refresh::refuses_without_managed_authorization`
  (with `full_route_refuses_before_any_filesystem_work`) -> the typed
  managed-authorization refusal names both signals and fires before any
  filesystem work (#3566).
- `eval_sweep_refresh::python_eval_sweep_refresh::authorized_route_requires_explicit_binary_and_out`
  -> `--ripr-bin` (existing file; PATH cannot select) and `--out` are
  mandatory.
- `eval_sweep_refresh::python_eval_sweep_refresh::rejects_out_overlapping_accepted_state`
  -> candidate separation: accepted state (`fixtures/`, the repo root, an
  ancestor) is rejected; a dedicated directory under `target/` is accepted.
- `eval_sweep_refresh::python_eval_sweep_refresh::symlinked_out_into_accepted_state_is_refused`
  -> an existing `--out` symlink resolving into accepted state is refused:
  the overlap comparison runs on canonicalized paths on both sides.
- `eval_sweep_refresh::python_eval_sweep_refresh::dirty_reused_checkout_is_stale_not_the_pinned_tree`
  -> a reused checkout at the pinned HEAD with a modified file is
  dispositioned `stale`, never analyzed as the pinned tree; the full
  denominator stays validatable.
- `eval_sweep_refresh::python_eval_sweep_refresh::all_eight_statuses_produce_a_validatable_receipt`
  -> producer/validator symmetry over the full 0.3 status vocabulary; every
  terminal state stays selected with the exact run/non-run split.
- `eval_sweep_refresh::python_eval_sweep_refresh::unstable_repeat_comparison_records_the_mismatch_list`
  -> a false stability claim carries its typed `unstable_gap_ids` list and
  digests the retained second-pass stderr in `repeat.repeat_stderr`.
- `eval_sweep_refresh::python_eval_sweep_refresh::non_complete_first_results_never_enter_the_repeat_phase`
  -> the complete-first gate lives inside the stability pass: a non-complete
  first result never spawns a repeat and never claims stability.
- `eval_sweep_refresh::python_eval_sweep_refresh::binary_swap_between_subjects_is_detected`
  -> the per-subject post-run identity re-verification names a swapped
  (rebuilt) binary's drift and fails closed on an unreadable binary.
- `eval_sweep_refresh::python_eval_sweep_refresh::infrastructure_failure_row_keeps_the_denominator_shape`
  -> a contained infrastructure failure is a typed `tempfail` row over the
  verified facts with its cause named — no analysis counts, no invented
  digests.
- `eval_sweep_refresh::python_eval_sweep_refresh::cache_creation_failure_becomes_tempfail_and_route_completes`
  -> a subject whose cache dir cannot be created becomes a `tempfail` row and
  the route still produces the full validatable eight-row denominator with
  the cause in the execution receipt.
- `eval_sweep_refresh::python_eval_sweep_refresh::subject_ripr_toml_is_recorded_not_default`
  -> a subject root carrying `ripr.toml` records the configured identity with
  its relative path; `default` only when genuinely absent.
- `eval_sweep_refresh::python_eval_sweep_refresh::deterministic_row_and_summary_assembly`
  -> identical inputs assemble identical rows and summary (declared
  telemetry apart).
- `eval_sweep_refresh::python_eval_sweep_refresh::stale_materialization_keeps_subject_selected_without_counts`
  (with `clone_failure_row_shape_is_tempfail`) -> stale/tempfail
  materializations keep the subject selected and carry no analysis counts.
- `eval_sweep_refresh::python_eval_sweep_refresh::corpus_classification_counts_by_real_path_shapes`
  -> bounded working-set counting over real path-shaped producers.
- `eval_sweep_refresh::python_eval_sweep_refresh::evidence_digest_binds_raw_stderr_and_repeat`
  -> the evidence digest preimage binds raw stdout, stderr, and the repeat
  pass.
- `eval_sweep_refresh::python_eval_sweep_refresh::refresh_candidate_validates_through_eval_sweep_check`
  -> the end-to-end refresh-produces-what-check-validates symmetry over
  synthetic local subjects with the real built binary (offline; no network).
- `eval_sweep_refresh::python_eval_sweep_refresh::stale_subject_path_keeps_the_denominator_without_loss`
  -> a stale subject is dispositioned `stale` and all eight rows remain in
  the validatable denominator.
- `eval_sweep_report::python_eval_sweep_report::valid_candidate_dry_run_renders_agreeing_json_and_markdown`
  -> JSON and bounded Markdown derive from the same validated rows and agree.
- `eval_sweep_report::python_eval_sweep_report::accept_appends_immutably_and_moves_pointer`
  -> acceptance appends content-addressed receipts without mutating prior
  ones; the pointer identifies exactly the newest; re-accept is idempotent.
- `eval_sweep_report::python_eval_sweep_report::pointer_contains_no_totals_only_identity`
  -> the pointer's field set is exactly identity fields.
- `eval_sweep_report::python_eval_sweep_report::currentness_is_current_with_matching_identities`
  and `..::editing_as_of_never_repairs_staleness` -> the currentness law is
  mechanical; the as-of disclosure is never load-bearing.
- `eval_sweep_report::python_eval_sweep_report::binary_digest_change_flips_stale`,
  `..::manifest_digest_change_flips_stale`,
  `..::moved_malformed_manifest_still_reaches_stale_with_both_reasons`,
  `..::accepted_row_or_tree_change_flips_stale`, `..::input_change_flips_stale`,
  `..::pointer_config_edit_flips_stale`, `..::pointer_features_edit_flips_stale`,
  and `..::source_sha_change_flips_stale` -> each bound identity movement
  flips the pointer stale with the exact reason named; the malformed-manifest
  movement reaches that stale verdict with BOTH the digest-movement and
  validation-failure reasons (the digest comparison runs on the raw bytes
  first, so a schema-invalid changed manifest never aborts into a schema
  error).
- `eval_sweep_report::python_eval_sweep_report::split_portable_rejects_traversal_component_shapes`
  and `..::pointer_receipt_file_with_empty_or_dot_components_is_refused` ->
  portable-path containment: empty components (consecutive separators), `.`,
  `..`, and separator-only shapes are refused naming the field while the
  normal `receipts/<sha>.json` shape still reads.
- `eval_sweep_report::python_eval_sweep_report::missing_toolchain_identity_is_never_current`
  -> a pointer binding no `features`/`build_profile` copy discloses that
  identity `unverifiable` with the field named — never `current`.
- `eval_sweep_report::python_eval_sweep_report::disposition_owner_pipes_and_newlines_cannot_split_the_markdown_table`
  -> free-text disposition values render escaped and flattened in the bounded
  Markdown table while the receipt JSON keeps the raw value.

## Implementation Mapping

| Concern | Code |
| --- | --- |
| Command logic (arg parse, manifest load, run orchestration, classify, metrics, render) | `xtask/src/reports/eval_sweep.rs` |
| Accepted-manifest and retained-receipt validator (`eval-sweep check`) | `xtask/src/reports/eval_sweep_check.rs` |
| Managed candidate refresh (`eval-sweep refresh`) | `xtask/src/reports/eval_sweep_refresh.rs` |
| Accepted-receipt publication + currentness gate (`eval-sweep report`) | `xtask/src/reports/eval_sweep_report.rs` |
| Subcommand registration | `xtask/src/command.rs`, `xtask/src/dispatch.rs`, `xtask/src/reports/mod.rs` |
| Subprocess helpers (build, clone, `ripr check`) | `xtask/src/run.rs` (`run`, `run_with_envs`, `capture_output_with_timeout`) |
| Pinned manifest + synthetic diff | `fixtures/python-eval-sweep/manifest.json`, `fixtures/python-eval-sweep/synthetic-diff.diff` |
| Rendered report | `target/ripr/reports/eval-sweep.{json,md}` |
| Check verdict report | `target/ripr/reports/eval-sweep-check.{json,md}` |
| Refresh candidate artifacts (candidate receipt, execution receipt, raw outputs) | `<out>/eval-sweep-refresh-receipt.json`, `<out>/execution-receipt.json`, `<out>/raw/` |
| Refresh run report | `target/ripr/reports/eval-sweep-refresh.{json,md}` |
| Accepted state (accepted receipt + markdown + retained candidate, current pointer) | `<state-dir>/receipts/<sha256>.{json,md}`, `<state-dir>/receipts/<sha256>.candidate.json`, `<state-dir>/current.json` (default state dir `fixtures/python-eval-sweep/accepted`) |
| Accepted-receipt dry-run report | `target/ripr/reports/eval-sweep-report.{json,md}` |

## Metrics

| Metric | Meaning |
| --- | --- |
| `repos_run` | external repos analyzed (excludes skipped/clone-failed) |
| `crash_rate` | fraction of `repos_run` that aborted or failed-exit |
| `parse_failure_rate` | fraction degrading to a named static-unknown limitation |
| `timed_out_count` | repos exceeding the wall-clock timeout |
| `runtime_ms_median` | median run-1 `ripr check` wall-clock per repo (built binary, excludes cargo overhead) |
| `gap_id_stability_rate` | fraction with identical canonical gap-ID sets across a re-run |
| `classification_counts` | per-repo + aggregate 7-way exposure-class distribution (descriptive; never gates) |
| `alignment_counts` | per-repo + aggregate `oracle_alignment` distribution (`direct`/`alias`/`changed_sink_token`/`orthogonal`/`unknown`/`absent`, Python-only) plus repair-packet presence counts (`repair_placement`/`verify_command`/`python_repair_card`) |
| `gate_status` | `not_run` if `repos_run == 0`; else `pass` iff `crash_rate == 0` and `gap_id_stability_rate == 1.0`; else `review` (distributions never affect this) |
