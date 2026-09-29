# Changelog

All notable repository-level changes are tracked here.

This project uses a human-readable changelog. Versioned release notes summarize
user-visible behavior, compatibility notes, and migration guidance. Internal
planning, ADR, and spec changes are called out when they affect how future PRs
are scoped or reviewed.

## Unreleased

### Fixed

- Security: Rust source discovery skips symlinked `.rs` entries, as the
  Python and TypeScript readers already did. A cloned repository or pull
  request that committed `src/zero.rs -> /dev/zero` made `ripr check` read
  until it ran out of memory (#4751).
- Security: ripr's git calls pass `-c core.fsmonitor=false`, so a
  repository's own `core.fsmonitor` program (reachable from an extracted
  archive or a planted nested repository) does not run on `git status`
  (#4744).
- Security: `[perl].cache_dir` must be a repository-relative path without
  `..`. An absolute or escaping value is now a config error instead of a
  directory ripr creates and writes outside the checkout. The cache directory
  now resolves under the analyzed root rather than the working directory, so
  `--root <checkout>` cannot place it elsewhere either (#4745).
- Security: `ripr doctor` probes every language runtime (`node`, `bun`,
  `pnpm`, `python3`, `pytest`) outside the checkout, as it already did for
  `yarn`. Run inside it, pnpm fetched and ran the release a project's
  `packageManager` named, and version managers read project files (#4742).
- Security: ripr no longer runs `cargo` or `rustc` in a repository whose
  nearest `rust-toolchain.toml` selects a toolchain by `path`. rustup would
  execute that path, and `/proc/self/cwd/...` points it into the checkout, so
  `ripr doctor` in a cloned repository ran the repository's own program.
  Doctor now reports the check as not run and names the file, and the
  test-harness `cargo metadata` probe fails closed. Setting
  `RUSTUP_TOOLCHAIN` restores the probes (#4740).
- Security: the workflow `ripr init --ci github` writes no longer consumes
  gate inputs a pull request can commit under `target/ripr` or `target/ci`
  (#4731), only treats ripr comments posted by `github-actions[bot]` as its
  own (#4732), and no longer leaves the job token in `.git/config`, prints
  unfolded repository paths to the log, or interpolates composite-action
  inputs into shell (#4733). Regenerate the workflow with
  `ripr init --ci github --force` to pick this up.
- Security: `ripr lsp` no longer reads a whole client-named file to digest
  an opened document. It digests only a regular file no larger than one LSP
  message, so `didOpen` for `/dev/zero`, a FIFO or a multi-GB file can no
  longer exhaust memory or hang the server (#4729).
- Security: a base ref starting with `-` is refused before `git diff` runs,
  including from LSP `baseRef` settings, so it can never be parsed as a diff
  option such as `--output` (#4730).
- Security: `ripr pilot` and other commands that write to default paths
  inside the analyzed repository no longer write through a symlink committed
  there. A cloned repository could commit
  `target/ripr/pilot/pilot-summary.md` as a link to any file the user can
  write, and `ripr pilot` replaced that file. Those writes, and ripr's
  temporary files, now refuse a symlink, FIFO or directory at the output
  path (#4719).
- Nested `rerun --json` cache-identity versions in `docs/OUTPUT_SCHEMA.md`
  now track live `FILE_FACT_CACHE_SCHEMA_VERSION` (`1.10`) and
  `CACHE_SCHEMA_VERSION` (`1.16`). Producer-backed docs tests fail when those
  nested values, the command-to-version table, swarm-queue envelope, or
  cache-status field contract drift from producers (#4618).
- Default human `Hidden:` output names the language and preview status of
  omitted findings (`Python preview: 1`) so a mixed-repo remainder is not a
  bare count. Rust-only remainder stays the count line. (#4395)
- `docs/CONFIGURATION.md` no longer groups Python with opt-in TypeScript and
  JavaScript; Python preview is marker-auto when no `ripr.toml` exists. (#4395)
- TypeScript: a change inside a module-private helper now relates to tests
  that call an exported function reaching it in the same module, including a
  value a same-module factory built. unjs/defu tests call `defu(...)`, built
  by `export const defu = createDefu()`, whose returned closure calls the
  changed `_defu`; ripr reported `no_static_path` for the tested change. The
  relation follows at most three same-module calls, respects parameter and
  local shadowing, and reports such reach as `weakly_exposed`, naming the
  exported callers, never `exposed` on its own.
- Commands ripr prints now run. For a missing agent receipt, `ripr reports
  index` suggests `ripr agent status`, which names the repair attempt's
  next step, instead of an `agent receipt` call missing its required
  flags. It no longer suggests the repository-internal `cargo xtask
  check-pr` and `cargo xtask pr-summary`. Invalid-receipt
  guidance names `--seam-id`, and Perl receipt commands use the canonical
  `ripr receipt write` form instead of a `--verify-cmd` flag `outcome` never
  had. Help screens and guides that contradicted the CLI were corrected,
  including the `first-pr` cost disclosure, which described an analysis the
  command never runs, and `docs/CONFIGURATION.md`'s claim that `context`
  accepts `--format`. A test now fails when a public guide passes a flag
  that its command's help does not list (#4573).
- LSP: a request whose method starts with `$/` and that ripr does not handle
  now gets a `-32601` method-not-found error, as the LSP spec requires. It got
  no response at all, so a client that sent one waited on it forever.
  Unhandled `$/` notifications are still ignored.
- MCP: a client that opens with `server/discover` (protocol `2026-07-28`)
  now receives the same instructions as an `initialize` client, including the
  CLI route that analyzes the diff. Before, only `initialize` carried them.
  Workspace status no longer says a `ripr.toml` is detected when the root has
  none; that limitation now appears only when one was found.
- TypeScript: when a test imports the changed function through an alias
  ripr could not resolve, such as `@/lib/math` with
  `[typescript] resolve_tsconfig_paths` unset, `ripr check` no longer says
  no test references the function and asks for a new test. The missing
  discriminator and next step now name the test, the import path and why it
  was not resolved, with the same fix the limitation evidence gives. The
  finding stays `no_static_path`. (#4550)
- TypeScript: `[typescript] resolve_tsconfig_paths` now reads
  `tsconfig.json` and `jsconfig.json` the way `tsc` does, with `//` and
  `/* */` comments, trailing commas and a leading byte-order mark. Before, any comment (and
  `tsc --init` output is mostly comments) made alias resolution give up, and
  the finding told users to rewrite the file as strict JSON. Malformed
  files, including an unclosed block comment, still resolve no aliases and
  say the file could not be parsed. (#4549)
- TypeScript/JavaScript preview: a test that loads its subject by directory,
  such as `var mimeTypes = require('..')`, now relates to the owner. `.` and
  `..` were not treated as relative specifiers, and a directory specifier did
  not resolve to the module it loads, so ripr reported `no_static_path` for
  code the test calls. A directory now resolves through its `package.json`
  `main`, else its `index` file; a sibling file module still wins, and a
  root-escaping or unresolvable `main` keeps the specifier unresolved.
  (#4546)
- TypeScript/JavaScript preview: a change inside a CommonJS export such as
  `exports.thrice = function thrice(x) { ... }` now maps to an owner. These
  assignments produced no owner, so the changed line yielded zero candidates
  and `no_behavioral_candidates`. `exports.NAME` / `module.exports.NAME`
  functions and arrows, `module.exports = function ...`, and function
  properties of `module.exports = { ... }` are now owners that `require()`
  tests relate to; non-function values and computed keys still produce none,
  and an export name assigned twice in one file produces no owner. (#4545)
- TypeScript/JavaScript preview: mocha, `node:test` and Vitest suites written
  with `context`, `suite` or `specify`, with an options object before the
  callback (`it(name, { timeout }, fn)`), or with a `describe` title that is
  not a string literal (`describe(Div.name, fn)`) were skipped, so their tests
  were never related to the code they cover. These forms are now walked like
  `describe` / `it`; `.skip`, `xit` and `xcontext` stay uncredited, as does
  a registration whose options object skips it (`{ skip: true }`,
  `{ todo: true }`, Vitest `{ fails: true }`), and `test(name, fn, timeout)`
  is unchanged (#4548).
- TypeScript/JavaScript preview: tests that assert with `node:assert` or
  chai now count as oracles. `assert.strictEqual(charset('text/html'),
  'UTF-8')` in a mocha suite was read as an `unknown` oracle, and ripr
  suggested adding `toBe`. Assertions made through an imported `assert`,
  `node:assert`, `assert/strict` or chai binding now map to exact-value,
  relational, smoke or broad-error evidence, including bare named imports
  (`strictEqual(a, b)`) and chai `expect(x).to.equal(y)` chains. Loose
  `==` equality (legacy `node:assert` `equal` / `deepEqual`, chai
  `assert.equal`) counts as relational, not exact-value. A local helper
  named `assert`, or an imported binding re-declared in the test or its
  suite, is still not credited, and Jest/Vitest `expect` is unchanged
  (#4547).
- `ripr rerun --changed-test` with an unknown test node, an unparsed test
  file, or an ambiguous owner now returns the documented `limited` report
  (`changed_test_unresolved`, `changed_test_owner_unresolved`,
  `changed_test_owner_ambiguous`) with exit 0. It used to exit 2 with empty
  stdout, so a `--json` caller got nothing to parse (#4571).
- `ripr check` spends less time rescanning test files. The same-name-import
  gate re-masked every related test file's source for every probe; one scan
  per file now serves the whole run. On a ripr commit, a warm check went from
  8.6 s to 6.6 s with byte-identical JSON.
- `ripr review-comments --gap-ledger` now renders repair cards for gaps from
  `ripr reports gap-ledger --check-output`, such as Python repair gaps. Those
  ledger rows carry no seam ID, and every one was suppressed as
  `missing_seam_identity` even though the ledger marked it eligible for a PR
  comment, so the documented route produced no cards. A gap-ledger card is
  keyed by its gap record and now omits `seam_id` when the row has none; the
  schema requires `seam_id` only on diff-scoped cards (#4524).
- LSP: the server now asks clients for `textDocument/didSave`. It advertised
  only the numeric full-sync kind, which under the LSP spec does not request
  save notifications, so a strictly conforming editor could save without ripr
  re-analyzing. The capability is now the options form with `save: true`,
  which the VS Code extension's compatibility check already accepts.
- LSP: opening a second repository in the same Helix session no longer stops
  ripr for the first. Helix adds the new repository as a workspace folder to
  the running server, which made the folder set ambiguous and stopped
  analysis for both. Editors without the VS Code integration now keep the
  root they started with, are told which folder is not analyzed, and hover
  on a file from that folder says it is outside the analyzed root. The VS
  Code extension keeps its folder-picker behavior.
- `ripr agent brief --json`: `before_snapshot_command` now creates
  `target/ripr/workflow` before redirecting into it, so the first loop command
  works in a fresh checkout. When the requested scope matches nothing and no
  other agent-actionable seam is visible, the warning says so instead of
  claiming it is showing all repo-actionable seams (#4592).
- LSP: an editor that opens two workspace folders, or none, now hears why
  ripr is silent. Before, the server stopped analysis and sent nothing: the
  startup `ripr/analysisStatus` was dropped because the transport discards
  custom notifications during `initialize`, and hover showed the generic
  `ripr check` pointer. The server now publishes the startup status from
  `initialized`, logs a warning naming the root state and folders, and shows
  it with `window/showMessage` to clients without the VS Code integration,
  at startup and when a later folder change stops analysis. Hover names the
  blocked root, a file outside the analyzed root, an edited buffer whose
  evidence is paused until the file is saved, or a file no refresh has
  analyzed yet.
- CI: the `ripr init --ci github` workflow pins `shell: bash` for every job,
  so its bash-only steps still parse on a Windows runner, and the README
  names `ripr init --ci github` as the CI entry point (#4391).
- `ripr review-comments` no longer times out on a large diff. It evaluates
  seams on changed lines and in changed owner functions first, and skips the
  rest of the scope when those already fill the ten review slots; a warning
  gives the skipped count. On one 11-file ripr change it went from 398 s, past
  the 120 s default bound, to 18.5 s with the same comments. Agent brief and
  review warnings now name the first ten hidden matching seams and count the
  rest, so that report shrank from 1.7 MB to 34 KB.
- Generated GitHub workflow (`ripr init --ci github`): it now checks out the
  PR head instead of GitHub's `refs/pull/N/merge` commit, so review comments
  and annotations land on the PR diff's lines after the base branch moves.
  Before, they carried merge-commit line numbers. The install now pins the
  generating ripr version; an unpinned `cargo install ripr` took the newest
  crates.io release, whose CLI need not match the workflow's steps.
- A repository's `ripr.toml` can no longer choose a program for ripr to run.
  `[perl].executable` was spawned by `ripr check`, probed by `ripr doctor`, and
  spawned by `ripr lsp` on file open or save, so a cloned repository could run
  its own code (for example `executable = "sh"` plus a committed `ripr-facts`
  script). ripr now ignores that key, says so on stderr or in the doctor
  report, and uses the exporter on PATH unless the user sets
  `RIPR_ALLOW_REPO_PERL_EXECUTABLE=1`. The VS Code extension already required
  a trusted workspace to start the server.
- `docs/OUTPUT_SCHEMA.md`, `docs/LLM_OPERATOR_GUIDE.md`, and
  `docs/interop/mcp.md` now match the JSON agents receive: five missing
  version-table rows, the `swarm queue` `0.2` example and its `python`
  language default, the receipt's omitted `safe_to_merge` and `--test` flag,
  the seventh `agent status` artifact, the `rerun` cache versions, the receipt
  movement vocabulary, and a `cache status --json` field contract (#4608).
- Rust related tests are the ones that name or reach the changed code, not
  every test that shares a word with it. A test name now relates only when it
  contains a probe token as a whole word (`new` no longer matches `renews_`), a
  test file only when its own stem spells the source stem as a word or it is a
  test file inside a directory named for that stem, and neither a test-name
  word nor an assertion-observed token counts when more than 16 tests and more
  than 1% of the tests in the changed code's crate use it. Tests that match
  only the old substring rules still relate when nothing else does, so the
  finding keeps weak reach instead of reading `no_static_path`. On a 14-file
  diff of ripr itself (246 findings), the longest related-test list on one
  finding fell from 13,007 rows to 1,750, and the rows across all findings
  from 246,958 to 50,751. No finding on that diff gained exposure. Six lost
  `exposed` (three to `weakly_exposed`, three to `infection_unknown`) and
  eight others moved to `infection_unknown`. For example, a predicate in `reach.rs` read
  `exposed` from 1,608 related tests; it now relates 129 and reads
  `infection_unknown`, because none of those supplies an input at its
  boundary. The seam evidence behind `review-comments`, `agent` and repo
  exposure uses a limit of 1% of the suite (at least 64 tests): past it, a
  parent module relates only itself, the owner's own module and test-named
  siblings; an asserted token that common within the seam's crate relates
  nothing; and when several target tokens together pass the limit, the tests
  asserting the most of them are kept. One `init.rs` seam had related 3,990
  tests. On `review-comments` for one ripr commit, the run fell from 50 s to
  17 s with identical comments.
- Rust: a changed function that no test calls now reads `no_static_path`.
  Before, a same-file test of a sibling function made it `weakly_exposed`
  with "strong oracle found", and its unknown-shape lines said "escalate to
  real mutation testing". This applies only when nothing in the workspace
  names the function outside its own `fn` line: a caller, function pointer,
  `use` alias, doctest or macro block that names it, a crate that includes a
  README as docs, a trait-impl method, or a nearby test that invokes a
  non-assertion macro keeps reach undecided.
- Rust: a struct-field initializer is no longer `exposed` when no assertion
  reads that field. On anyhow, a `Box` token in an unrelated downcast
  assertion credited `ptr: NonNull::from(Box::leak(ptr))` with confidence 1.00
  while its own evidence said nothing observes the field. An assertion that
  reads the field on the function's result (`cfg.retries` after
  `let cfg = default_config()`) still counts; the same field name on another
  value does not.
- Rust: the one-line signature of a new function whose body is added too is no
  longer probed; it only repeated the body's findings.
- `ripr check` run from a crate subdirectory such as `src/` now analyzes the
  crate. In a repository whose `Cargo.toml` has no `[workspace]` table, the
  root stayed the subdirectory, the diff fell outside it, and the JSON reported
  `analysis_complete: true` with no findings. The implicit root now walks up to
  the nearest `Cargo.toml`, or to the git top level when there is none, and
  says so on stderr. The walk stops at the git top level, so a `[workspace]` in
  an enclosing repository no longer claims a nested one. `ripr cache` resolves
  the same root (#4610).
- `review-comments` observes its cooperative analysis budget during canonical
  inventory and rejects cancelled evidence before classification. Git diff
  discovery consumes the remaining budget; deadline cancellation records a
  typed timeout while ordinary source failures retain their failure status.
  Individual operations can still overrun a checkpoint interval (#1778).
- Cold LLM-agent walks of 0.11 no longer dead-end on four routes. Passing a
  `ripr check` finding ID (`probe:...`) to `ripr agent repair --seam-id` now
  says it is not a seam ID and names `ripr pilot --root .`. After a repair,
  the after phase and `ripr agent status` say the repair receipt records no
  test run (`test_run.status: "not_recorded"`), because a failing test can
  still show movement `improved`. The MCP
  server's instructions and tool description say it does not analyze the diff
  and name the CLI route that does; an unusable root and unknown tool or
  resource names now carry a recovery.
- `ripr check` analyzes Rust crate roots declared outside `src/`
  (`[lib] path = "lib/foo.rs"`, `[[bin]] path = ...`). A change there used
  to report zero candidate lines as a complete analysis, and Draft mode
  dropped the package's tests, so a tested change read as
  `no_static_path`.
- Base-resolution failures name their cause and the next step. A shallow
  CI checkout no longer stops at raw `fatal: ... no merge base`; it names
  `git fetch --unshallow` and `fetch-depth: 0`. A repository with no
  commits, or whose default branch is not `main`/`master`, is told which
  `--base` would work. A branch that shares no history with HEAD is never
  offered as that base.
- A diff that touches conflict markers in a file no enabled adapter reads
  (for example resolving markers committed to a workflow `.yml`) no longer
  turns the whole run into `unsupported_input`.
- `--root` at a workspace member scopes the diff to that member and reads
  its paths relative to it. Repository-relative paths used to miss the
  member's files, so a tested change read as `no_static_path`.
- In diff analysis, the generated-code skip limitation names up to three
  skipped files and lists the generated-code conventions and the
  `[languages.rust] generated_file_patterns` setting.
- `ripr check` on committed history (the default, or `--base <rev>`) now reads
  the committed version of every tracked file you have edited but not
  committed, tests included, and leaves out new files that are not committed,
  git-ignored ones included.
  Before, an uncommitted edit that shifted lines in a changed file could
  attach findings to the wrong function or expression, and an uncommitted test
  edit already moved the counts while the note said uncommitted changes were
  not analyzed. The note naming `--worktree` now appears only when a source or
  test file has uncommitted changes, not for an edited README or workflow.
  Tracked files deleted from the working tree are named on stderr.
- An empty `ripr check --diff` result now leads with its true cause. A config
  whose `[languages].enabled` leaves out `rust` records a typed
  `language_adapter_unavailable` limitation for the Rust files it skipped
  (`partial_with_limitations`, naming the effective set) instead of claiming a
  complete analysis, and the zero-findings stderr line names a disabled or
  unavailable adapter instead of suggesting the diff may be invalid; that
  diff-validity hint now appears only when nothing parsed. The non-source
  disclosure names extensionless and `.`-ending paths and no longer calls the
  empty result correct for a truncated `+++` header, and a directory passed as
  `--diff` is reported as a directory rather than as the OS read error
  ([#4376](https://github.com/EffortlessMetrics/ripr-swarm/issues/4376),
  [#4395](https://github.com/EffortlessMetrics/ripr-swarm/issues/4395)).

- First-hour output no longer strands the reader. `--format human-full` carries
  each finding's `ripr explain` / `ripr context` commands, which the digest
  sends readers there for
  ([#4379](https://github.com/EffortlessMetrics/ripr-swarm/issues/4379)).
  `--format github` prints a denominator notice when findings are suppressed by
  policy or are base-side, so an all-suppressed run is no longer silent
  ([#4393](https://github.com/EffortlessMetrics/ripr-swarm/issues/4393)).
  `ripr cache status` points at `ripr cache clear` instead of repository-only
  xtask, and corrupt cache warnings name the entry file
  ([#4383](https://github.com/EffortlessMetrics/ripr-swarm/issues/4383)). The
  digest's "Why weakly_exposed" line names the incomplete stage; unclassifiable
  lines in a function no test reaches ask for a test first instead of real
  mutation testing; zero-count languages and the empty-result caveat on
  non-empty preview runs are dropped; digest lines no longer end mid-word or
  inside an open code span; and a closed stdout pipe (`ripr doctor | head`)
  ends quietly with exit `2` instead of an internal-error report.

- `cargo xtask ripr-pr` timeout packets now give one host-shell-labeled retry
  command that keeps base, head, and root arguments literal when copied, including
  refs with shell syntax and roots with spaces (#4367).

- `ripr receipt check --ledger` explains each cross-reference state after its
  token. `receipt_ok` now says it only means the ledger still lists the
  receipt's gap, not that the gap is closed, so it no longer reads as a fix
  confirmation. The Python context witness's `fix_site` names the same
  suggested test as `check`, `explain` and the repair card.

- TypeScript preview boundary findings get a delegatable repair packet when
  the boundary input is statically derivable: a changed `amount >= 5000` or
  `amount >= DISCOUNT_THRESHOLD` (single immutable integer module `const`)
  with tests only off the boundary now targets `expect(shipping(5000))` /
  `expect(discountedTotal(10000))` instead of failing closed, `ripr check`'s
  Start-here line names the packet's action, test file, and verify command,
  and an exposed TypeScript finding no longer leaves the gap ledger empty
  (first-pr no longer loops on "blocked" after the boundary test lands).
  Rebindable (`let`/`var`), computed, imported, or shadowed constants,
  written parameters, and comparisons with arithmetic, a sign, or a member
  read on either side (`OFFSET + amount >= LIMIT`, `amount >= LIMIT + 1`)
  still fail closed. So does a comparison that some calls may skip: the
  changed line must be a top-level statement of the owner's own body that
  opens with the comparison (`if (`, `return`, or `const|let|var NAME =`),
  with no earlier `return`/`throw`/`break`/`continue`/`yield`, loop or
  `await`, and the owner may not be a generator or a curried or returned
  function.

- `ripr pilot` on a Python-only change with a repair card now ends with the
  card's route (`ripr first-pr` before the edit to name the receipt command,
  the test edit, its verify command, then that receipt command) instead of
  `ripr check --root .`, which only led back to pilot.

- `ripr check`'s Start here prefers a Python preview finding that has a
  repair card over one that has none, so it no longer reports "no repair card"
  while `ripr pilot` and `ripr first-pr` route a card for the same diff.

- `ripr check` now names the repair loop for a Rust top gap that has a repair
  route: one `Repair loop:` line under the drill-in commands points to
  `ripr pilot --root <root>`, which prints the `ripr agent repair` start.
  Preview-language, static-limited, and route-less findings do not get it.

- Actionable working-set review cards write the verify and analysis-outcome
  artifacts consumed by their receipt command. Gate and onboarding projections
  carry the complete optional command chain, preserving the selected base in
  the analysis-outcome command even without a conventional default branch;
  older cards and deferred
  GapRecord routes remain compatible (#4307).

- LSP `ripr.collectContext`, `ripr.collectEvidenceContext` and
  `ripr.collectRepairPacket` no longer answer `null`. An unreadable argument
  or an id missing from the current snapshot is a `-32602` InvalidParams
  error naming the accepted shapes, and a repair packet with no source says
  which artifacts are missing and names the CLI route. `ripr help lsp` lists
  every server-executed command's arguments. `ripr/listActionableItems` adds
  `selected` and `omitted` item lists, so its self-named continuation route
  returns items rather than only counts, and a `hidden_gaps` list naming the
  gaps the actionable profile never publishes because they have no repair
  route, such as a new function no test calls.
- LSP fallback diagnostics explain their static classification and point to
  hover evidence without promising an unavailable clipboard action or repair
  route. Missing-path guidance remains explicitly static (#4328).
- TypeScript/JavaScript and Python preview adapters no longer probe the
  declaration line of a new function whose body adds its own lines. The line
  had no behavior of its own, so it either stayed `weakly_exposed` after a
  correct test was added or, in Python, claimed unearned `exposed` credit
  beside a weakly exposed body predicate. Changed signatures, default values,
  and one-line bodies keep their probe.
- Static discrimination keeps oracle strength and confirmation on the same
  assertion. An unrelated exact assertion can no longer borrow a weaker
  assertion's token match to promote a finding to `exposed`; equally strong
  confirmed assertions retain their classification regardless of order
  ([#4404](https://github.com/EffortlessMetrics/ripr-swarm/issues/4404)).
- Diff-scoped SARIF (`ripr check --format sarif`) now renders the same
  `artifactLocation.uri`, `fingerprints` and `partialFingerprints` whether
  `--root` is `.`, `./` or the checkout's absolute path. An absolute root used
  to leak the checkout path into the uri and change every fingerprint between
  a local and a CI run. SARIF shares the path owner GitHub annotations already
  used.
- Server qualification builds the Linux server archives, which the editor
  extension downloads, on Ubuntu 22.04 with `--locked`, and fails a Linux
  binary that needs a glibc newer than 2.34 (RHEL 9), the floor the 0.11.0
  release sync adds to `cargo xtask release-server-archive`. The 0.10.0 Linux archives were built on Ubuntu 24.04 and
  failed on Ubuntu 22.04 and Debian 12 with `GLIBC_2.39 not found`.
- `RIPR_GIT_TIMEOUT` with a non-numeric or out-of-range value now fails
  closed (exit 2) naming the variable and the value, like `--git-timeout` and
  the `RIPR_PARTIAL_*_BUDGET` overrides. It used to keep the default deadline
  silently (#4374).
- Python: a function a package re-exports from its `__init__.py` is now
  related to tests that call it through the package. On humanize
  (`import humanize`, `humanize.naturaldelta(...)`) and more-itertools
  (`import more_itertools as mi`, `mi.one(...)` via `from .more import *`)
  every changed line read `no_static_path` although mutating those lines
  fails the projects' own tests. Renamed re-exports, `_private` names under a
  star import, and names a declared `__all__` omits are not followed.
- TypeScript preview: a test that imports a changed function through a barrel
  now reaches it. On unjs/ufo, `import { withoutBase } from "../src"` names
  the directory whose `src/index.ts` does `export * from "./utils"`; ripr
  resolved `../src` to a module that matched no barrel, so every changed line
  of `withBase`/`withoutBase` read `no_static_path` with 0 related tests. A
  directory specifier now resolves to its `index` module when no file module
  of that name exists, and `export *` / `export { N } from` chains are
  followed for up to 4 hops inside the repository. A star hop forwards a name
  only when the target module exports it; a name two star sources export, a
  cycle, a longer chain, a test-local or `describe`-scoped redeclaration of
  the imported name, or a mock of any module on the chain gives no credit,
  and a test that imports only another name from the same barrel stays
  unrelated. (ufo's own `withBase` tests are still missed: they
  register from a `for` loop with computed titles, which test extraction does
  not index and discloses as partial.)
- TypeScript: tests declared inside a `for`, `for...of` or `for...in` loop
  or a `.forEach` callback are now extracted, including tests with a
  template-literal or other computed title. Before, a suite written like
  unjs/ufo (``for (const t of tests) { test(`${t.input}`, ...) }``) left the
  owners it calls reading `no_static_path`. A computed title is named
  `<computed title, line N>` under its `describe`; the test relates to an
  owner only when its own body calls it. A loop is walked only when it is
  known to run at least once (a non-empty literal, or a `const` bound to
  one), and a loop variable or `describe` parameter that reuses an imported
  owner's name shadows it.

- Python pytest verify commands now run as `python -m pytest path::node`
  instead of bare `pytest path::node`. `-m` puts the repository root on
  `sys.path`, so a flat-layout package such as `pricing/__init__.py` imports
  during collection; bare `pytest` stopped there with `ModuleNotFoundError`
  (exit 4). The interpreter is spelled `python`, like the unittest route's
  `python -m unittest`, because it names the virtual environment's interpreter
  on every platform. Repair-card, LSP skeleton, and dogfood consumers accept
  both the new form and the bare form earlier artifacts carry.

- `ripr first-pr` now prints the receipt path its receipt command writes. For a
  Python or TypeScript preview gap, `Receipt path:` named a
  `gap-pr-...targeted-test-outcome.json` file while the printed
  `ripr receipt write` command wrote `--out gap-python-....json`; the path now
  is the file the printed command writes (its `--out`, or the receipt
  writer's default for its `--gap`), else the ledger's recorded path, and the
  first-pr default only when first-pr builds the command itself.

- `ripr first-pr` no longer leaves `--status not_run` unexplained in the
  receipt it presents as the step after verify. A `Receipt status` line now
  follows a `ripr receipt write ... --status not_run` command in the CLI
  summary and `start-here.md`, telling the reader to pass `--status passed`
  when the verify command exited 0 and `--status failed` when it did not. The
  command itself is unchanged, still runs as printed, and records `not_run`
  when left as is.

- The generated CI job summary's `PR review summary` and `Recommended next
  test` blocks, their collapsed full reports included, now print copyable
  commands at the repository root (`ripr agent verify --root . ...`,
  `> ./target/...`) instead of the runner's absolute checkout path that
  `ripr agent start` binds into `workflow.json` and `agent-brief.json`, like
  the `Agent review packet` block already did. The stored artifacts keep
  their bound root; only the summary rendering rewrites the checkout path, and
  only where it is a whole path token.
- `ripr review-comments` now reads its diff the way `ripr check` does. It ran
  its own `git diff`, so `color.diff=always` in the repository's git config
  produced zero guidance with exit 0, and `diff.submodule=diff` put guidance
  on files inside a submodule. A base or head that does not resolve, and a
  shallow clone with no merge base, now get the same named cause and repair
  as `check` instead of git's `ambiguous argument` advice. `ripr first-pr`
  names the shallow clone behind a missing merge base and offers
  `git fetch --unshallow` as its next command (#4538).
- A symlink in the diff no longer counts as changed source. Git shows a
  symlink as a one-line file holding its target path, so `ripr check` built a
  probe from that path and `ripr review-comments` annotated unchanged lines of
  the file the link points at, under the link's name (#4577).

- LSP: `ripr.collectRepairPacket` and `ripr.collectContext` now reject a
  `gap_id` that is present but not a string (such as `42` or `true`) with an
  error naming `gap_id`. The repair command used to return the top gap's
  packet instead of the one asked for, and the context command blamed another
  field. An absent, `null`, empty or blank `gap_id` still means "not given"
  (the top packet), as RIPR-SPEC-0077 specifies. A `gap_id` that
  `actionable-gaps.json` does not hold no longer gets that report's first
  packet: the gap ledger is tried, then a status packet naming the gap.

- `ripr check` human output for a budget-stopped (`limited_partial_scope`)
  run now names the budget that stopped it and its size (for example
  `the file budget of 200 changed file(s) (RIPR_PARTIAL_DIFF_FILE_BUDGET=200)`),
  says how many findings were produced before the stop and that more may
  exist beyond the budget, and tells you to raise that variable, noting the
  other budget the next file may also need. When every changed file ripr's language adapters read was
  selected (a single oversized first file), it no longer prints "at least 0
  changed file(s) ... may contain additional findings"; it says the result
  stays partial instead.
- Rust cache entries now reject same-key semantic payload edits before serving
  facts or classified evidence. File-fact, full/compact classified, shard and
  corpus-fingerprint generations cold-recompute once; checksums do not
  authenticate writers able to recompute them (#4382).
- `docs/OUTPUT_SCHEMA.md` now lists every finding enum value `ripr check
  --format json` can emit: `static_limit_kind` gains
  `wrapper_error_binding_unresolved` and
  `rust_subprocess_binary_reach_unresolved`, and `stop_reason` gains
  `transitive_reach_unresolved`. `cargo xtask check-output-contracts` now
  derives each governed enum from its declaration and fails when the doc list
  or `policy/output_contracts.txt` misses or invents a value, instead of
  accepting any substring match (#4539).

- LSP code lenses now offer the registered saved-workspace refresh command with an
  explicit action label, avoiding unsupported empty-command clicks in standard clients.
  Cached related-test advisories remain static; clicking does not run tests or repair code
  (#4357).

- Advisory report outputs refuse planted destination links and nonregular files
  before truncation while preserving fresh writes and regular-file updates.
  The shared index, outcome, calibration, and agent-receipt write path uses
  no-follow acquisition; supported Unix FIFO outputs do not wait for a reader.
  Review-comments receipts exclusively create staging files, refusing planted
  temp paths without clobbering their targets (#4360). This is leaf acquisition
  hardening, not ancestor-directory or hard-link confinement.

- `ripr explain` and `ripr context` now reject an explicit `--base` combined
  with `--diff` at parse time, in either flag order and before any pipeline
  run, instead of silently analyzing the `--diff` input while appearing to
  assert the base (the loader gave `--diff` precedence and never validated
  `--base` beside it). Beside `--from`, both flags remain scope assertions
  verified against the recording and are unaffected. Running a `--diff -`
  command directly at a prompt (instead of piped) now prints a one-line
  stderr disclosure before ripr blocks reading the diff from the attached
  terminal, so the documented `git diff origin/main | ripr check --diff -`
  right half no longer looks like a silent hang; the disclosure lives in the
  CLI adapter and piped, redirected, or captured stdin — including library
  calls into the analysis API — stay silent and byte-identical
  ([#4319](https://github.com/EffortlessMetrics/ripr-swarm/issues/4319)).

- Every flag `ripr` parses is now documented on a surface a reader scans,
  and mistyped flags can be suggested from anywhere the command's help
  documents them. An audit against the parsers found parsed-but-undocumented
  flags (check's `--perl-facts`, context's `--finding`, agent status's
  `--out`) that were invisible to `ripr <command> --help` and could never be
  proposed by a typo suggestion; those help entries now exist. Unknown-flag
  suggestions mine the same surfaces the flag/help parity gate checks — the
  Options list plus the command's own `Usage:` line — so a flag documented
  only in usage syntax (explain's `--base`, context's `--at`) is suggestible
  too: `ripr explain --bas x` now suggests `--base`. The #2342 parity gate is
  revived as a two-directional test: for every command that ships a help
  body, each parsed flag must be documented and each documented flag must be
  parsed, with `--help` and named hidden aliases as the only exceptions
  ([#4317](https://github.com/EffortlessMetrics/ripr-swarm/issues/4317)).

- The VS Code download test no longer commits a localhost TLS private key.
  The suite generates a one-day `127.0.0.1` certificate when it starts.
  The removed pair was self-signed for that name only
  ([#4143](https://github.com/EffortlessMetrics/ripr-swarm/issues/4143)).

- Python pytest and unittest verify commands single-quote a test path that
  is not a plain relative path, matching the TypeScript command quoter.
  A name containing a shell metacharacter stays inside quotes in the command
  text. The stored test path and node id stay unquoted, and a path made only
  of letters, digits, `.`, `_`, `/`, and `-` is unchanged
  ([#4211](https://github.com/EffortlessMetrics/ripr-swarm/issues/4211)).

- `ripr outcome` no longer reports zero movement for check-output snapshots
  whose findings carry no canonical gap id, such as Rust `ripr check --json`.
  It refuses the pair, points Rust users to `ripr check --format
  repo-exposure-json`, and says that preview-language findings without an id
  have no comparable receipt
  ([#3797](https://github.com/EffortlessMetrics/ripr-swarm/issues/3797)).
- `cargo install ripr` without `--locked` compiles. It had resolved
  `unicode-ident` 1.0.26, which fails a compile-time Unicode-version assert in
  `ra-ap-rustc_lexer`; the 1.0.24 pin moved from `Cargo.lock` into the
  `lang-rust` feature's manifest, and release CI now builds the packaged crate
  from a fresh resolution
  ([#3787](https://github.com/EffortlessMetrics/ripr-swarm/issues/3787)).
- `cargo xtask actionable-gap-outcomes` no longer reports an agent receipt's
  `verification.status: "verification_not_run"` as the attempt's verify
  result. It counts as a missing verify result
  ([#4234](https://github.com/EffortlessMetrics/ripr-swarm/issues/4234)).

- `ripr gate evaluate --gap-ledger` no longer reports an already-observed
  (closed) gap under "Suppressed" as configured-hidden. The ledger's
  `not_policy_targeted` state also covers no-action records, so the gate now
  reads suppression only from an explicit suppressed state or predicate. Such a
  record is `not_applicable` with the reason "already observed; no action
  required", and `evidence.configured_off` is `false`. The gate stays
  non-blocking and the status is unchanged
  ([#3903](https://github.com/EffortlessMetrics/ripr-swarm/issues/3903)).

- A TypeScript change that only edits type syntax on a signature or
  declaration line (a return type, a parameter or variable annotation, an
  optional marker, a generic parameter list) no longer produces a `predicate`
  probe. TypeScript erases those types, so there is no behavior for a test to
  notice. A default-value, parameter, body, or export change on the same line
  keeps its probe
  ([#4282](https://github.com/EffortlessMetrics/ripr-swarm/issues/4282)).

- The CLI smoke test that copies `ripr` and runs `doctor` retries only
  `ETXTBSY` (`ExecutableFileBusy`), up to three times. A parallel test can
  `fork` while that copy is still open for writing, and the copy cannot be
  executed until the child reaches `exec`. Any other error, and any process
  that actually started, is still returned unchanged. The same bound covers
  the other smoke test that executes a copied binary
  ([#4296](https://github.com/EffortlessMetrics/ripr-swarm/issues/4296)).

- TypeScript/JavaScript preview: a class method tested through an instance
  built outside the test body is no longer reported `no_static_path`. The
  receiver may now come from the enclosing `describe` scope, a
  `beforeEach`/`beforeAll` hook, a default import of the owner's
  default-exported class, or a namespace import (`new shop.Cart()`). Scope
  bindings are read from the syntax tree and resolved to the innermost scope,
  where the last hook write wins over the declaration's initializer. The
  receiver is withheld when anything in the file could rebind it outside a
  recognized declaration or hook write: a write anywhere else (including
  destructuring, casts and closures), a parameter or redeclaration of the
  same name, a second hook of the same kind writing it, a hook the file
  defines or imports under another name, or a declaration or write of the
  class name itself (`class Cart` or `function Cart` in a hook), a method
  assignment such as `cart.total = ...` or any use of `Cart.prototype` (a
  spy or replaced method), a hook write that follows a possible early
  `return`, a generator hook, or any `eval` or escaped identifier in the
  file. Member reads, `expect(cart)`, `typeof cart`, comments, import paths
  and describe/test/mock name strings do not count; any other string or
  template that mentions the name does.

- A generated command prints no PowerShell form only when PowerShell reads it
  the same way. Commands with a quoted program path, `$` expansion, globs,
  braces, `~`, `@`, comments, `--%`, non-spaced `>` forms, a second redirect or
  an unbalanced quote used to be labelled as running unchanged in PowerShell.
  They now get a translation (a quoted program path gains the `&` call
  operator) or a "PowerShell form unavailable" line
  ([#4244](https://github.com/EffortlessMetrics/ripr-swarm/issues/4244)).

- With TypeScript enabled, a diff that classifies no TypeScript owner (for
  example a Rust-only change) is no longer reported
  `partial_with_limitations` because some unchanged TypeScript test file uses
  a test shape the adapter cannot extract. The
  `typescript_test_extraction_partial` limitation is still reported when a
  changed TypeScript or JavaScript owner, or a Bun cross-language finding,
  reads the test index
  ([#4261](https://github.com/EffortlessMetrics/ripr-swarm/issues/4261)).

- `ripr check --candidate-tree` no longer reads the worktree `ripr.toml`. A
  subject run configures itself from its candidate tree, but the CLI still
  loaded the worktree file first, so an unparseable worktree file, or a
  `languages.enabled` entry the binary lacks (for example `python` in a
  Rust-only build), made the subject run exit 2, and a worktree
  `[analysis] mode` reached a subject whose tree sets none. The Rust-only
  feature set (`--no-default-features --features lang-rust`) now passes its
  test suite, and CI runs it on Linux for pull requests that change the Rust
  crate, fixtures or Cargo manifests, and on every push to `main`
  ([#4252](https://github.com/EffortlessMetrics/ripr-swarm/issues/4252)).

- An improved `agent receipt` (including the one `ripr agent repair --phase
  after` writes) no longer says "Keep the focused test": ripr never runs the
  project's tests, and a test that fails `cargo test` can still move static
  grip. The guidance now says to run the focused test and keep it only if it
  passes, and `verification` carries `status: "verification_not_run"` and the
  non-claim `static_only_assurance`
  ([#4234](https://github.com/EffortlessMetrics/ripr-swarm/issues/4234)).

- TypeScript/JavaScript preview: a changed ambient declaration
  (`declare function`, `export declare const`, `declare module`) or any
  change in a `.d.ts`/`.d.mts`/`.d.cts` declaration file no longer yields a
  `predicate` probe reading `no_static_path`. These are type-only and erased
  at compile time; declaration files still count as changed files. Ambient
  statements are found from the syntax tree, so `declare` used as a
  JavaScript identifier, or at the start of a template-literal line, is still
  probed.

- Perl preview: with Perl enabled but no fact packet, the reason now says
  to pass `--perl-facts <packet.json>` or configure `[perl].producer` instead
  of citing an internal campaign issue, and the note reads "1 Perl file was
  not analyzed".

- `ripr doctor` now lists Perl under "Detected languages" whenever its Perl
  section appears: any `.pm`, `.pl` or `.t` file at any depth, such as a
  CPAN module under `lib/Name/`, or a `Makefile.PL`, `Build.PL` or `cpanfile`.
  It no longer prints "none detected" beside a Perl section that counts the
  same files. Files under `target/`, `node_modules/`, `blib/` or hidden
  directories detect nothing, as they already counted nothing.

- `cargo xtask vscode-package` now reads the built VSIX and fails if it
  carries workspace build output (anything under `extension/target/`, Cargo
  `.fingerprint` or `incremental` state, `.rlib` or `.rmeta`) or exceeds 1,500
  entries or 64 MiB unpacked. On the 0.11.0 trial join, Cargo output left under
  `editors/vscode/target/` was packed into a 725 MB VSIX; this check fails
  packaging if an ignore rule ever misses that output again.

- The LSP local file-URI decoder refuses a parent-directory segment (`..`),
  including one written with percent-encoding or backslashes, instead of
  admitting it as an absolute path. Saved-content digest reads use only an
  admitted path, so a refused URI's display fallback is not opened even when
  the working directory contains a `file:` directory that would let that
  relative string follow `..`. A path this process builds may still contain
  `..` from a relative join, and that spelling is collapsed before a `file:`
  URI is emitted. Filenames that only contain two dots (`foo..bar`,
  `..hidden`) stay ordinary local paths. This refuses the client-supplied
  read; it is not a claim that a client can disclose the bytes
  ([#4145](https://github.com/EffortlessMetrics/ripr-swarm/issues/4145)).

- Local LSP file URIs now treat an empty authority and `localhost` as the
  same local path. Workspace paths spelled as UNC shares or Windows
  extended-length/device paths are refused when producing a local file URI,
  instead of emitting a URI the server cannot read back. Workspaces on those
  paths remain unsupported by this local-only URI path
  ([#4060](https://github.com/EffortlessMetrics/ripr-swarm/issues/4060)).
- TypeScript predicate-boundary evidence no longer credits assertions whose
  boundary value is in an unread argument or nested expression, whose owner
  name is shadowed, or whose expected value cannot discriminate the change.
  These cases may move from `exposed` to a weaker class and regain guidance
  to add a discriminating test; live boundary assertions retain their credit
  ([#4102](https://github.com/EffortlessMetrics/ripr-swarm/issues/4102)).
- TypeScript related-test discovery now recognizes supported optional-chain
  calls, awaited dynamic imports, named default exports, and re-exports
  through star or default-as barrels. Findings backed by those relations can
  gain related-test evidence; a relation alone does not establish a
  discriminating oracle
  ([#4103](https://github.com/EffortlessMetrics/ripr-swarm/issues/4103)).
- TypeScript owner-call relations no longer credit unanchored bare calls,
  unrelated destructures, calls into supported mocked modules, or spies that
  fabricate the owner's value. These cases retain weaker advisory evidence
  where available instead of crediting observation of the changed behavior;
  genuine anchored calls and call-through spies remain eligible
  ([#4125](https://github.com/EffortlessMetrics/ripr-swarm/pull/4125)).
- TypeScript and JavaScript preview repair, targeted rerun, and output paths
  now recognize `.mts`, `.cts`, `.mjs`, and `.cjs` sources and tests through
  the same extension authority used by analysis. These module forms no longer
  disappear solely because a later consumer used the narrower extension list
  ([#4116](https://github.com/EffortlessMetrics/ripr-swarm/issues/4116)).
- Oversized diffs stop parsing when the distinct accepted file count exceeds
  the configured limit, before reading the remaining file bodies. The error
  reports an observed lower bound and no partial analysis result; this does
  not cap the bytes of one large file or the already acquired diff text
  ([#3858](https://github.com/EffortlessMetrics/ripr-swarm/issues/3858)).
- Perl preview evidence preserves static observations when the producer
  reports a missing test runner. Runner absence still blocks repair authority;
  it does not by itself cap the static exposure class. This bounded consistency
  repair does not complete the broader Perl fact-packet integration
  ([#4059](https://github.com/EffortlessMetrics/ripr-swarm/pull/4059)).
- `check-file-policy` builds test binaries before it lists `covered_by`
  subjects. A cold compile is no longer charged against the five-minute
  list cap, and a timeout is reported as an instrument failure rather than
  an unresolved pointer. A `--doc` pointer cannot use `--no-run`, and Cargo
  recompiles doctests on every listing, so that pointer is enumerated once
  under the compile budget instead of again under the five-minute cap
  ([#4141](https://github.com/EffortlessMetrics/ripr-swarm/issues/4141)).
- `ripr init --ci github` encodes PR guidance annotations inside jq. The
  previous TSV round-trip rewrote backslash, tab, CR, and LF before GitHub
  workflow-command escaping, so a path or message could display transport
  text instead of the comment bytes
  ([#4089](https://github.com/EffortlessMetrics/ripr-swarm/issues/4089)).
- TypeScript and JavaScript diff analysis no longer counts vendored, built,
  or generated files it does not inspect. `node_modules`, `dist`, `build`,
  `out`, `coverage`, `.next`, `.cache`, `vendor`, `__generated__`, and
  `*.generated.*` are refused before the changed-file tally, and the
  workspace walk prunes the same directories so they cannot back findings.
  Near-misses such as `src/build.ts` and `generated.ts` stay ordinary
  source. A repair packet may name a Jest/Vitest, Node, Cypress, Jasmine,
  or `__tests__` test path as its edit target when the TypeScript adapter
  is compiled; a production file still cannot
  ([#3743](https://github.com/EffortlessMetrics/ripr-swarm/issues/3743)).

- Gate baselines now treat canonical gap identity as the normal authority and
  disclose every legacy fallback match. `ripr baseline create` refuses
  `path:line:static_class` fallback identity as primary authority for new
  entries (refusals count under `summary.skipped.fallback_only`) and preserves
  the source-report repository root on the report and each entry.
  `ripr baseline diff` marks fallback-only joins with
  `baseline_match_kind: legacy_path_line_class`, `stale_baseline_warning`,
  the retained legacy identity, and the retained canonical replacement
  candidate; a diverged canonical gap or a cross-root join goes stale instead
  of looking historical, and all such joins count in
  `delta.legacy_fallback_match`, which generated CI summarizes.
  `ripr baseline update --migrate-legacy-identities` deterministically
  replaces reviewed legacy identities with the joined canonical gap id
  (recorded for review; conflicts and cross-root joins refused), while
  removal of resolved debt now requires `--remove-resolved` explicitly, so a
  migration-only run never shrinks the reviewed baseline. Schema stays `0.1`;
  older ledgers remain parseable and comparable
  ([#1964](https://github.com/EffortlessMetrics/ripr-swarm/issues/1964)).

- The stat-only aggregate corpus shortcut no longer grants authoritative
  cache reuse on platforms without a content-change witness. Its signature
  is `(path, mtime, size)` plus the unix inode change time; on unix ctime
  moves on every content write, but on Windows and other non-unix targets
  a same-length edit that restores the modification time reproduced the
  whole signature and could serve stale seam evidence and classification
  after a real source or test edit. `corpus_fingerprint` now produces no
  signature at all on those platforms, so the cache-key fast path, the
  compact projection, and the targeted-rerun identity check all degrade to
  the read-everything path rather than reusing a stale aggregate hash. No
  mapping can be looked up or stored there, which leaves mappings written
  by earlier builds unreachable by construction; the content-keyed
  per-file fact cache and all unix behavior are unchanged. Non-unix runs
  trade the shortcut's speed for correctness until a native change-identity
  design is proven
  ([#3848](https://github.com/EffortlessMetrics/ripr-swarm/issues/3848)).

- `ripr first-pr`, `pr-summary`, `annotations`, `pr-evidence`,
  `impacted-evidence`, and `plus` unknown-flag errors now go through the
  shared help/suggestion authority. A near-miss typo suggests only a flag
  that command's `--help` documents; a flag owned by a sibling command still
  fails closed with no suggestion
  ([#3812](https://github.com/EffortlessMetrics/ripr-swarm/issues/3812)).

- `ripr cache status` and `ripr cache clear` unknown-flag errors now suggest
  the accepted flags from the same help bodies those commands print
  (`--json`, `--dry-run`, `--force`). Typos no longer fall through to the
  bare no-suggestion branch
  ([#3786](https://github.com/EffortlessMetrics/ripr-swarm/issues/3786)).

- Live `parse_old_path_for_confinement` and `git::run_git` no longer carry
  leftover `#[allow(dead_code)]` attributes whose reasons cited closed
  follow-ups. Matching `.ripr/allow-attributes.txt` rows were dropped
  ([#3801](https://github.com/EffortlessMetrics/ripr-swarm/issues/3801)).

- PR review guidance retains unresolved headline-eligible recommendations
  when the nearby recommended test file changes. Test-file proximity no
  longer erases these cards; evidence limitations and output caps remain
  unchanged ([#3771](https://github.com/EffortlessMetrics/ripr-swarm/issues/3771)).

- Rust diff analysis now treats ownerless field declarations in resolved
  test-required module children as evidence-only, including literal-path and
  transitive module children. Production, mixed, missing, and unresolved
  contexts and lexical-fallback children retain production eligibility
  ([#3695](https://github.com/EffortlessMetrics/ripr-swarm/issues/3695)).

- Diff-path textual identities now escape literal percent signs consistently on
  every platform. Unix invalid bytes retain their native `%XX` encoding, while
  a valid filename that literally contains `%FF` remains distinct
  ([#3609](https://github.com/EffortlessMetrics/ripr-swarm/issues/3609),
  [#3611](https://github.com/EffortlessMetrics/ripr-swarm/pull/3611)).

- Rust source-role analysis now classifies source-visible `cfg` and
  `cfg_attr` predicates through one closed authority shared by the parser
  producer and the facts normalizer. Nested `test` conjunctions in
  `#[cfg(all(...))]` earn the evidence role at nesting depths within the supported bound (deeper predicates fail closed) and any
  conjunct order, whitespace and multi-line attribute spellings no longer
  lose a producer-granted role in the normalizer, and `cfg_attr`
  introductions never promote production code to test-only. Alternatives,
  negation, comments, literals, lookalike identifiers, and malformed input
  stay fail-closed. Cache generations advance so warm analysis cannot reuse
  the previous role classification
  ([#3530](https://github.com/EffortlessMetrics/ripr-swarm/issues/3530)).

- Rust source-role analysis now recognizes a direct top-level `test` conjunct
  in `#[cfg(all(...))]` regardless of conjunct order. Helpers under
  `#[cfg(all(feature = "slow", test))]` are evidence role and no longer seed
  production findings. `any(test, ...)`, `not(test)`, nested alternatives, and
  `test` text inside literals stay production/fail-closed. Cache generations
  advance so warm analysis cannot reuse the previous role classification
  ([#3213](https://github.com/EffortlessMetrics/ripr-swarm/issues/3213)).

- Rust repository analysis now recognizes parser-backed, file-level,
  repository-local literal `include!` fragments as part of their parent
  compilation unit.
  Included functions keep their real fragment path and line in findings while
  owner identity and related-test evidence use the parent file. Unsafe,
  ambiguous, dynamic, missing, cyclic, oversized, or out-of-scope include
  boundaries stay fail-closed and emit stable limitation reason codes
  ([#3211](https://github.com/EffortlessMetrics/ripr-swarm/issues/3211)).

- The README's example output was missing two lines the renderer has been
  emitting: the `Why <class>:` classification hint, and the whole
  `Next: drill into the top finding:` block naming the `explain` and `context`
  commands. The second is the most actionable thing on screen — the product's
  own advertisement of its output omitted the step that tells a reader what to
  run next. Both now match `fixtures/boundary_gap/expected/human.txt`, which is
  the same example.

- `ripr explain` and `ripr context` now reject a mistyped flag with a
  suggestion. Neither parser routed through the shared unknown-argument
  helper: `ripr context --fromm` failed with a bare
  `unexpected context argument "--fromm"` — no suggestion and no help
  pointer — and `ripr explain --fromm` did not report an argument error at
  all, because the positional-selector arm accepted any token, so `--fromm`
  was taken as the finding selector and analysis ran against it. Both parsers
  now use the shared helper, and `explain`'s positional arm rejects a
  `-`-prefixed token unless it is selector-shaped, so a `file:line` selector
  whose path begins with `-` still resolves. Output becomes
  `unknown context argument "--fromm". Did you mean \`--from\`? Run \`ripr context --help\`.`

  Both commands are also registered in the help lookup and the command-path
  list the flag-parity guard iterates, which previously omitted them from both
  sides and so could not see the gap.

- The release external-cwd journey test no longer flakes with `Text file busy`
  under the full `reports::release` filter. It published its spawnable stub with
  `fs::copy` from the running test binary, which holds a writable descriptor open
  across a multi-megabyte transfer. `ETXTBSY` is raised while any process holds
  the file open for writing, and `FD_CLOEXEC` closes an inherited descriptor at
  `exec`, not during the fork/exec window — so a peer test forking inside that
  transfer left the stub transiently un-executable. This is the same mechanism
  recorded for the doctor atomic-publication test (#2441), but it is removed
  rather than retried here: the stub is now published by hard link, so no
  writable descriptor ever exists on the executed inode and the precondition for
  `ETXTBSY` cannot arise. A staged copy-then-rename fallback covers hosts without
  hard links. Test-only change; no `ripr` behavior is affected
  ([#3051](https://github.com/EffortlessMetrics/ripr-swarm/issues/3051)).

- `cargo xtask check-public-api` now observes the transitive public surface.
  Its collector matched two line prefixes in `crates/ripr/src/lib.rs`, so every
  `pub` item reachable through an allowlisted `pub mod` was invisible: a new
  `pub const` in `domain/mod.rs` was reported as `pass`. The gate now parses the
  crate's module tree and records module-level items whose visibility is a bare
  `pub`, following `pub mod` into other files. `policy/public_api.txt` is
  rewritten as a `<kind> <path>` recording of the surface that already existed —
  214 entries where 18 lines were recorded before. No item's visibility changed;
  the previously unrecorded items were already public. The gate does not cover
  public struct fields, enum variants, trait items, or associated functions in
  `impl` blocks, and it records a glob re-export as a glob because a syntax walk
  cannot expand one. Both limits are stated in the gate's own report.

  Three blind spots in that first walk are closed. `cfg` predicates were
  decided by looking for the identifiers `test` and `not` anywhere in the
  predicate, which was wrong in both directions: `#[cfg(any(test, feature =
  "x"))]` was dropped although a feature-enabled build exports it, so an
  accidental public addition passed the gate, and `#[cfg(all(test, not(feature
  = "x")))]` was recorded although nothing but a test build compiles it. Each
  predicate is now evaluated with `test = false` and every other option left
  unknown, and an item is dropped only when no non-test build can compile it.
  Non-`pub` modules were skipped entirely, so a `#[macro_export] macro_rules!`
  declared in one was missed even though it binds at the crate root whatever
  the declaring module's visibility; such modules are now walked for their
  exported macros and nothing else. Completed work was keyed by file path
  alone, so with two `#[path]` modules sharing one file only the first path was
  recorded; it is now keyed by file and module path, with a separate
  in-progress set bounding a module tree that names itself. A `#[path]` on a
  module declared at the top level of a non-`mod.rs` file also resolved against
  the wrong base directory — it is relative to the directory holding that file,
  not to the file's child-module directory, which is how
  `crates/ripr/src/cli/commands.rs` declares its subcommands.

  `policy/public_api.txt` is unchanged: none of these corrections moves the
  `ripr` crate's own recorded surface.

- The `fabricated_result` case in `fixtures/assurance_vocabulary/assurance/corpus.json`
  omitted `runtime_mutation`, so it failed the assurance schema for a missing
  required field rather than for the producer-bound digest mismatch it exists to
  demonstrate. The field is now present and the case discriminates on its
  intended axis
  ([#2923](https://github.com/EffortlessMetrics/ripr-swarm/issues/2923)).

- Human output no longer restates the `Missing discriminator` label inside its
  own value. The classifier builds these entries as
  `Missing discriminator value: <value>`, so the digest rendered
  `Missing discriminator: Missing discriminator value: AuthError::RevokedToken`.
  It now renders `Missing discriminator: AuthError::RevokedToken`. Entries that
  are not value-shaped ("No strong discriminator was detected") are unchanged,
  as is every non-human format. 18 `human.txt` goldens re-blessed as
  `formatting_only`.

- `Cargo.lock` now resolves the current workspace manifest. The `serial_test`
  dev-dependency was added without refreshing the lock, so `cargo` commands
  that pass `--locked` — including release qualification and lock-resolving
  package commands — failed to resolve. No declared manifest dependency
  requirement changed; the refresh only records the missing resolution.

- An unexpected panic now produces a recognizable `ripr: internal error`
  message and exits with code 2, not the default Rust panic output with
  exit code 101. The message includes the panic location when available and
  a link to report the bug
  ([#2660](https://github.com/EffortlessMetrics/ripr-swarm/issues/2660)).

- `ripr check --diff -` now reads the diff from stdin, enabling pipe
  workflows like `git diff origin/main | ripr check --diff -`
  ([#2655](https://github.com/EffortlessMetrics/ripr-swarm/issues/2655)).

- CLI git operations now have a bounded default timeout (5 minutes) instead
  of running unbounded. A stuck git invocation surfaces as a named
  `git_invocation_timeout` error instead of blocking indefinitely. Override
  with `--git-timeout <seconds>` or `RIPR_GIT_TIMEOUT` env var; set to 0 to
  disable the deadline ([#2613](https://github.com/EffortlessMetrics/ripr-swarm/issues/2613)).

- AGENTS.md example commands used a stale probe ID (`error_path:8ee9f771`)
  that no longer matches the sample fixture. Updated to the current
  `error_path:c1a03250` so the examples work when copy-pasted.
- `docs/CONFIGURATION.md` said the LSP reads "six keys" but the table below
  and the code confirm seven governed keys. Corrected to "seven."
- `crates/ripr/README.md` distribution row called GitHub Releases
  "unpublished working drafts" but the crate is published on crates.io.
  Reworded to accurately reflect the crates.io distribution channel.
- VS Code settings `ripr.check.mode` and `ripr.trace.server` now carry
  `enumDescriptions` so the Settings UI dropdown explains each option.
  The `ripr.trace.server` description now clarifies it traces LSP transport,
  not analysis reasoning.
- VS Code settings `ripr.gitTimeoutMs` and `ripr.refreshDeadlineMs` now
  declare `minimum: 1000` to prevent silent acceptance of 0 or negative
  values.

- Three test modules used fixed (non-unique) temporary directory names that
  could collide under parallel test execution, causing intermittent flakes.
  The paths now include the process ID so parallel test threads never share
  a directory ([#2685](https://github.com/EffortlessMetrics/ripr-swarm/issues/2685)).

- The Python LSP server now reloads its configuration when project
  markers change, and auto-detection invalidates when Python source
  presence changes
  ([#3503](https://github.com/EffortlessMetrics/ripr-swarm/pull/3503),
  [#3576](https://github.com/EffortlessMetrics/ripr-swarm/pull/3576)).

- Policy covered_by enumeration is now reliable and diagnosable
  ([#3577](https://github.com/EffortlessMetrics/ripr-swarm/pull/3577)).

- The PR review front panel now preserves movement vocabulary
  ([#3491](https://github.com/EffortlessMetrics/ripr-swarm/pull/3491)).

- Unrecognized CLI flags now suggest the nearest documented flag and point at
  the command's own help, matching what unknown *commands* already did. A
  mistyped flag is the more common slip, but it produced a bare
  `unknown check argument "--forma"` with no suggestion and nowhere to go.
  It now reads
  ``unknown check argument "--forma". Did you mean `--format`? Run `ripr check --help`.``
  Candidate flags are read out of each command's help text, so a suggestion can
  never name a flag that `--help` does not document, and adding a flag to help
  makes it suggestible with no second edit. Applied across 48 argument-parsing
  sites covering 46 command paths (#2583).

- `ripr init --dry-run` now previews the run it is actually a preview of.
  It previously returned before every precondition check and printed file
  bodies unconditionally, so it reported success for two runs that fail: an
  existing `ripr.toml` without `--force`, and a `--root` that is not a
  directory. `--dry-run` and the real run now resolve the same plan, so the
  dry run fails with the same message and exit status when the real run would
  fail. On a run that can proceed, `--dry-run` prints a plan naming each
  target path and its action (`create`, `overwrite`, `leave existing`) before
  the file bodies, and closes with `Rerun without --dry-run to apply.`.
  Body headers now carry the full target path for both files; the config
  header previously showed only the bare file name (#2576).

- Default `ripr check --format human` output no longer prints a `Hidden:`
  heading over a literal `0 lower-priority finding(s) omitted from default
  human output.` line when nothing was actually omitted. That block claimed a
  suppressed remainder that did not exist, and it was the dominant case: 136 of
  175 human goldens rendered it. When the omitted count is zero the heading is
  now `More:` and the count line is dropped; when findings really were omitted
  the `Hidden:` heading and the non-zero count line are unchanged. The two
  `Full evidence:` / `Machine data:` pointer lines render identically in both
  states, so consumers scraping them are unaffected (#2571).

- Default `ripr check` human output now suggests `ripr explain <finding-id>`
  and `ripr context --at <finding-id>` for the selected finding. `ripr explain`
  now points to the matching context packet, while empty and fully suppressed
  results remain free of misleading follow-up commands (#2598).

- Finding follow-up commands now preserve the analyzed `--root`, `--diff` or
  `--base`, analysis mode, and artifact identity. Unreplayable worktree runs no
  longer emit commands that silently analyze a different scope, and dynamic
  arguments are shell-safe (#2659, follow-up to #2598).

- Windows LSP refreshes now isolate shared Git subprocesses from the JSON-RPC
  server stdin and terminate timed-out process trees with bounded pipe draining.
  Explicit refreshes therefore return trustworthy results within the ordinary
  compatibility budget instead of hanging on inherited descendant handles
  (#2430).

- LSP ordinary findings that survive the configured diagnostic profile now
  carry an explicit producer-owned delivery-eligibility signal, so the finite
  diagnostic budget publishes them without weakening gap-ledger, seam, or
  preview-family precedence (#2527).

- `ripr check --diff <path>` now discloses when the diff input contains no
  parseable file changes (0 hunks, 0 files). Previously a non-diff file (a
  log, a source file, random text) silently produced "0 probe(s)" with exit
  0, which could be mistaken for a clean bill of health. The stderr
  disclosure now explains the empty result may reflect an empty analysis
  scope, not sufficient tests (#2425).

- The doctor atomic-publication test no longer fails intermittently with
  `ExecutableFileBusy`. Publishing a tool atomically removes this process's
  writer, but it cannot remove host-level exec contention: `ETXTBSY` is raised
  while any process holds the file open for writing, and under full-suite
  parallelism another thread can `fork` while such a descriptor is open.
  `FD_CLOEXEC` closes the descriptor at `exec`, not during the fork/exec
  window. The bounded launch retry that already guarded the sibling
  hanging-tool test now lives in one shared helper used by both, so every test
  that publishes and immediately executes a tool agrees on what a retryable
  launch failure is. Only a retryable launch failure is retried; a real timeout
  or non-executing tool still fails (#2441).

- Advisory command strings emitted by `ripr` (first-PR packets, agent loop
  commands, receipt commands, pilot commands) now encode every argument as a
  single-quoted bash token instead of a double-quoted one. Double quotes do not
  stop bash from expanding `$VAR`, `$(cmd)`, or `` `cmd` ``, and the previous
  encoder additionally left a bare `\` unquoted and emitted the empty string as
  nothing at all. A gap id shaped like `gap:pr:1 > file` could therefore open a
  second redirect when the command was copied into a shell, truncating a file
  the operator never named and changing the id the command received. Values made
  only of unambiguous characters are still emitted unquoted, so ordinary
  commands are unchanged; the published shape changes only where a value
  actually needs quoting (#2347).

- Rust equality-boundary analysis now credits exact, source-ordered direct
  field assignments from same-file literal constants and bounded `+/-` integer
  offsets. Other owners, fields, similarly named constants, helper-only writes,
  control-flow-nested writes, values invalidated by an intervening mutable
  borrow, and opaque expressions remain fail-closed; unsupported direct writes
  report `field_assignment_value_unresolved` instead of an ineffective repair
  route.

- Rust literal match-arm observation is now bound to the input that
  selects the changed arm. A test asserting a sibling arm's result, a
  diagnostic-only call, an ambiguous owner, a conditional input, or an
  unsatisfied guard no longer credits the changed arm with observation.
  Arm separators, guard keywords, and literal comparisons are scanned
  with cooked, byte, and raw strings, character literals, and nested
  comments treated as opaque, so `=>` inside a literal no longer cuts a
  pattern short and quoted text inside a comment no longer supplies an
  arm value. Malformed input fails closed. The
  `match_arm_shared_result_no_promotion` and
  `match_arm_comment_literal_no_promotion` fixtures pin non-promotion in
  the evidence-promotion honesty corpus, and classified-seam cache
  generations advance so warm caches cannot serve the previous credit
  ([#3759](https://github.com/EffortlessMetrics/ripr-swarm/pull/3759),
  [#3766](https://github.com/EffortlessMetrics/ripr-swarm/pull/3766),
  [ripr#1714](https://github.com/EffortlessMetrics/ripr/issues/1714)).

- A changed Rust function-parameter declaration or named struct-field
  declaration (for example `value: Marker`) is now reported as an
  explicit `static_unknown` probe instead of a `field_construction`
  probe, which described it as executable field initialization. The
  parser confirms the declaration against the exact current line;
  receivers, trivia, visibility, and enclosing `unsafe` identity are
  preserved. Real record initializers keep their field-construction
  probe, and unparseable source keeps the previous lexical fallback. No
  exposure credit, schema, or threshold changes
  ([#3755](https://github.com/EffortlessMetrics/ripr-swarm/pull/3755),
  [#3758](https://github.com/EffortlessMetrics/ripr-swarm/pull/3758)).

- `ripr doctor` no longer lists the start-here packet as if it already
  exists. The line now reads
  `target/ripr/reports/start-here.md (present; open it first)` when the
  file exists and `(not yet generated; run the safe next action below)`
  otherwise. A directory at that path does not count as present. JSON
  output is unchanged
  ([#3866](https://github.com/EffortlessMetrics/ripr-swarm/pull/3866)).

- `ripr first-pr --check` on a workspace without a start-here packet
  now suggests a recovery command whose `--out-dir` is the resolved
  directory `--check` validated, not the raw relative option, so the
  command writes the packet where `--check` looks even when pasted from
  another working directory. The `Missing:` path, the `Start here:` and
  `Artifacts:` summary lines, and the `Wrote` lines now render with
  forward slashes on every platform instead of mixing separators on
  Windows. Packet contents, JSON, and path resolution are unchanged
  ([#3869](https://github.com/EffortlessMetrics/ripr-swarm/pull/3869),
  [#3873](https://github.com/EffortlessMetrics/ripr-swarm/pull/3873)).

- `ripr first-pr` recovery guidance now pairs a bash regeneration or
  next command that writes through `>` with its PowerShell form, in
  both the CLI summary and the start-here Markdown
  (`Regeneration command (PowerShell): ...`). The PowerShell form writes
  BOM-free UTF-8, so stock Windows PowerShell no longer produces UTF-16
  artifacts that readers reject. The two-step `check && gap-ledger`
  bridge renders as numbered `PowerShell 1/2` and `2/2` steps rather
  than a `;`-joined line that would run the second step after a failed
  first. Commands the translator cannot translate keep only the bash
  line, which stays byte-identical
  ([#3870](https://github.com/EffortlessMetrics/ripr-swarm/issues/3870)).

- `ripr first-pr --check` without a start-here packet in a checkout where
  no default base resolves (a detached HEAD with no branches, as in some CI
  checkouts) now prints a recovery command that requires `--base <ref>` and
  names the resolution error, instead of a write command that fails on the
  same missing base. When a default base resolves, the recovery still omits
  `--base`
  ([#4285](https://github.com/EffortlessMetrics/ripr-swarm/issues/4285),
  [#4290](https://github.com/EffortlessMetrics/ripr-swarm/pull/4290)).
- Generated `first-pr`, first-useful-action, PR-review front-panel and
  agent workflow commands now carry the absolute selected root in `--root`
  (and anchor their `--repo-exposure` and redirect paths to it), so a
  copied command analyzes the same repository from any working directory
  instead of re-resolving a relative root such as `.` against wherever it
  is pasted. A user-authored `--root .` keeps its ordinary meaning, and
  typed `command_specs` keep the portable `--root .` with `cwd` at the
  repository root
  ([#3999](https://github.com/EffortlessMetrics/ripr-swarm/issues/3999),
  [#4000](https://github.com/EffortlessMetrics/ripr-swarm/issues/4000),
  [#4287](https://github.com/EffortlessMetrics/ripr-swarm/pull/4287)).
- `ripr check`, `ripr pilot` and `ripr agent repair` now name one gap with the
  same word. The changed line `check` reports as `weakly_exposed` and the
  seam `pilot` reports as `weakly_gripped` both read `weak` first, for
  example `Static exposure: weak (weakly_exposed, warning, ...)`,
  `(weak, weakly_gripped)` and `weak -> exposed (weakly_gripped ->
  strongly_gripped, improved)`. The words are the ones the `check` summary
  line already uses (weak, unrevealed, no path, unknown). The schema values
  are unchanged. The repair packet's actionability reason now says "add a
  focused test with the missing discriminator next to the nearest related
  test", matching the new test `pilot` names, where it used to say "extend
  the nearest related test".
- A deeply nested Rust file anywhere in the workspace no longer aborts
  `ripr check`, `ripr pilot`, or the LSP with a stack overflow. A lexical scan
  now refuses a source file before parsing when its estimated nesting depth
  passes 256, an `else if` chain passes 2,048 links, or an operator chain
  passes 4,096. That file gets lexical-fallback facts, and the
  fallback disclosure names the `rust_nesting_budget` reason on cold and warm
  runs. Cache generations bumped, so a warm cache cannot serve facts from
  before the budget (#4475).
- `ripr check --format repo-exposure-md` puts the file path in each Top gaps
  heading in a code span, as the owner line already did. A file name holding
  Markdown link brackets or `*` rendered as a link or emphasis in that heading
  (#4605).
- Each repair attempt keeps its own result. The after phase copies the
  receipt and the verify document into the attempt's artifacts directory and
  records them in `attempt.json` as `terminal_artifacts`, bound by path,
  size, and SHA-256. Finishing a second attempt used to overwrite
  `target/ripr/reports/agent-receipt.json`, the only copy of the first
  attempt's result; that file is now a compatibility copy of the latest
  finish, and `ripr agent status` reads the attempt's own receipt first
  (#4636).

### Added

- Zed: a Zed extension in `editors/zed` starts `ripr lsp --stdio` from your
  `PATH` for Rust, Python, TypeScript, TSX, and JavaScript files. Zed runs
  only language servers an extension registers, so ripr could not run in Zed
  before. Install it with `zed: install dev extension`; it is not in the Zed
  extension registry. Settings under `lsp.ripr.settings` answer ripr's
  `ripr` configuration section (#4460).
- `ripr --version` now names the commit the binary was built from, as
  `ripr <version> (<commit>)`, with `-dirty` when the sources that build it differed
  from that commit. Packaged crates (crates.io, `cargo install ripr`) read the
  commit that `cargo package` recorded, so an installed candidate can be bound
  to source without hashing it. `ripr doctor` reports the same identity with
  the running executable and the first `ripr` on PATH, and warns, without
  failing, when that PATH entry is a Cargo workspace build or a different
  binary ([#4256](https://github.com/EffortlessMetrics/ripr-swarm/issues/4256)).

- New repository-governed Rust test-harness registry
  (`[analysis.test_harnesses]` in `ripr.toml`): repositories can teach
  ripr, through exact registrations only, about bounded custom test
  harnesses and test-producing source forms. A registered
  `harness = false` custom target (libtest-mimic adapter) is evidence
  role whose exact source-visible `Trial::test("name", ...)` trials with
  stable names become the executable subjects, its inert `#[test]`
  attributes never enter the test denominator, and one exact registered
  test-producing attribute is classified through the shared source-role
  authority. Subject facts carry harness kind, adapter generation,
  provenance, subject identity, and a named-unexecuted selector
  capability; dynamic trial names, loop-driven registration, ambiguous
  imports, lookalike markers, stale or conflicting registrations, and
  unknown adapter versions are named fail-closed limitations. Check JSON
  gains a `test_harnesses` projection only when registrations exist
  ([#3532](https://github.com/EffortlessMetrics/ripr-swarm/issues/3532)).

- Registered `custom_harness` targets now validate against the parsed
  Cargo target metadata of the declaring manifest: only a declared
  `[[test]]` target with `harness = false` keeps file-wide evidence role,
  helper demotion, and adapter subjects. Explicit declarations resolve
  lexically (`..` segments collapsed) and claim their target across
  nested and sibling manifest directories, including shared targets
  declared via a parent-relative path; nearest-manifest resolution still
  governs package autodiscovery, and workspace-inherited
  (`edition.workspace = true`) editions are honored. A registration
  whose target is missing from Cargo metadata, whose Cargo target still
  has `harness = true`, or whose workspace manifests cannot all be read
  and parsed records the typed limitations `target_not_declared`,
  `harness_flag_conflict`, or `manifest_unavailable` — naming the target
  — and degrades to per-function behavior
  ([#3608](https://github.com/EffortlessMetrics/ripr-swarm/issues/3608)).

- Registered `custom_harness` target validation now sources workspace
  membership and the test-target inventory from `cargo metadata`
  itself instead of a bounded manifest TOML emulation: a member's
  workspace-inherited (`[workspace.dependencies]`) path dependency
  validates its declarations, character-class member globs expand as
  cargo expands them, and `[workspace.exclude]` patterns match as
  cargo's literal path prefixes (a wildcard exclude component
  excludes nothing; a bare directory prefix excludes its subtree). The
  `harness` flag still comes from the owning manifest because metadata
  output omits it, every registration batch runs one bounded offline
  probe, and every unresolvable state - no cargo binary, a workspace
  cargo rejects, an unreachable probe deadline - fails closed to
  `manifest_unavailable`; the classified-seam caches bump their schema
  generations so pre-change entries cannot serve the flipped verdicts
  ([#3634](https://github.com/EffortlessMetrics/ripr-swarm/issues/3634)).

- Registered libtest-mimic trial subjects now carry evidence parity
  with equivalent ordinary `#[test]` functions: a bare-identifier
  callback contributes its resolved helper's parsed body evidence one
  level deep (calls, oracles, literals with real line attribution)
  only when binding identity is provable — local, import, const/static,
  and nested-module shadows fail closed; method-position `.unwrap()` /
  `.expect()` calls register smoke oracles with receiver-ful text
  (keyword, indexed, cast, operator, and negation receiver forms);
  assertion macros keep their complete invocation text in every
  delimiter and full qualified path; and dormant `macro_rules!`
  templates in any delimiter — and commented-out code — contribute no
  evidence while live surrounding evidence still admits. Warm caches
  are invalidated by the changed extraction generations
  ([#3603](https://github.com/EffortlessMetrics/ripr-swarm/issues/3603)).

- The named-invocation capability claim carried by libtest-mimic trial
  subjects is now documented as syntactic-only: a subject whose
  `Trial::test` registration is statically reached from the registered
  entry point is claimed as a named invocation of that trial even when
  the surrounding construction is dead at runtime (unreachable
  registration path) or the adapter's `run` call is absent. The claim
  docs, adapter documentation, check-JSON schema, and spec
  (RIPR-SPEC-0173) state this boundary explicitly, and a fixture pins
  that dead construction does not suppress the named-invocation claim
  ([#3604](https://github.com/EffortlessMetrics/ripr-swarm/issues/3604)).

- Registered libtest-mimic trial subjects gain a bounded, fail-closed
  reachability authority: the adapter anchors the registered run entry
  point (`<marker>::run` or a marker-anchored `run` import) and
  resolves its trial argument through supported forms — direct
  `vec![]`/array literals (including trials inside macro token trees),
  `&`/`&mut`/`local[..]` container peeling, immutable let-bound chains
  in the same function body, and one level of builder-function
  resolution. A trial construction provably excluded from every
  resolved run argument — or a target with no run entry call at all —
  keeps its subject fact and syntactic claim but no longer enters the
  executable-test denominator, and a per-trial
  `registration_unreachable` limitation names it. Reachability the
  bounded resolver cannot establish keeps today's denominator behavior
  and is disclosed by one aggregate `registration_reachability_unknown`
  limitation naming the trials — never a fabricated per-subject field,
  and never a silent exclusion: unknown is the bias. Classified-seam
  cache generations bump so pre-change caches cannot serve the old
  denominator
  ([#3636](https://github.com/EffortlessMetrics/ripr-swarm/issues/3636)).

- New `ripr mcp --stdio [--root PATH]` command: a bounded, read-only
  Model Context Protocol server that exposes exact workspace status
  (`ripr_workspace_status` tool and `ripr://workspace/status` resource)
  over newline-delimited JSON-RPC. Protocol errors always carry a
  response `id` (`null` when the request id is unreadable), discovery
  requires current-protocol `_meta`, and invalid roots fail closed
  without leaking paths. Startup routes `ripr mcp` (and `ripr help mcp`)
  ahead of general CLI initialization, and the global `--verbose` flag
  works in any position without contaminating the protocol stdout stream
  ([#3088](https://github.com/EffortlessMetrics/ripr-swarm/issues/3088),
  [#3525](https://github.com/EffortlessMetrics/ripr-swarm/pull/3525),
  [#3587](https://github.com/EffortlessMetrics/ripr-swarm/pull/3587)).

- New `ripr rerun` command: changed-test targeted re-analysis. Selects
  ledger gaps whose guarding tests changed, invalidates stale analysis
  by input and content fingerprints, and runs the bounded check pipeline
  only for the impacted scope
  ([#1520](https://github.com/EffortlessMetrics/ripr-swarm/pull/1520)).

- Rust test discovery now recognizes test-case parameterized tests
  ([#3522](https://github.com/EffortlessMetrics/ripr-swarm/pull/3522)).

- Rust test discovery now recognizes explicit nonstandard test
  attributes
  ([#3513](https://github.com/EffortlessMetrics/ripr-swarm/pull/3513)).

- TypeScript test discovery now recognizes active Jest/Vitest test
  modifiers
  ([#3506](https://github.com/EffortlessMetrics/ripr-swarm/pull/3506)).

- The Perl preview lane no longer lets operational producer limitations
  mask an earned exposure class: the class cap and the actionability
  gate are now separate in the static-limit projection
  ([#3583](https://github.com/EffortlessMetrics/ripr-swarm/pull/3583)).

- New binary-first evidence and gate surface: `ripr plus` (repo-level
  quality receipt), `ripr pr-summary` (PR readiness summary),
  `ripr pr-evidence` (PR evidence packet), `ripr annotations` (GitHub
  Actions annotations), and `ripr impacted-evidence` (mutation routing
  evidence). Each is advisory output, not a merge gate
  ([#1476](https://github.com/EffortlessMetrics/ripr-swarm/pull/1476),
  [#1460](https://github.com/EffortlessMetrics/ripr-swarm/pull/1460),
  [#1468](https://github.com/EffortlessMetrics/ripr-swarm/pull/1468),
  [#1467](https://github.com/EffortlessMetrics/ripr-swarm/pull/1467),
  [#1474](https://github.com/EffortlessMetrics/ripr-swarm/pull/1474)).

- `ripr check --suppression-policy <toml>` applies path-glob finding
  suppression from a committed policy file, and `ripr gate evaluate
  --exception-policy <toml>` applies dated burndown exceptions from a
  ledger. Suppressed findings stay visible as suppressed, never deleted
  ([#1475](https://github.com/EffortlessMetrics/ripr-swarm/pull/1475),
  [#1477](https://github.com/EffortlessMetrics/ripr-swarm/pull/1477)).

- Cache management surface: `ripr cache status` reports the analysis
  cache state per workspace, `ripr cache clear` removes it (with
  `--dry-run` and `--force` gates), and cache and receipt writes are
  atomic, so a crashed run can no longer leave a half-written cache
  entry behind
  ([#1822](https://github.com/EffortlessMetrics/ripr-swarm/pull/1822),
  [#2865](https://github.com/EffortlessMetrics/ripr-swarm/pull/2865),
  [#2738](https://github.com/EffortlessMetrics/ripr-swarm/pull/2738)).

- LSP capability wave: pull diagnostics with stable result IDs, the
  `ripr/listActionableItems` handler, transport framing/payload/
  concurrency bounds, work-done progress for long refreshes, UTF-16
  position encoding, refresh-status disclosure, and a versioned
  diagnostic-code catalog. The server remains an experimental sidecar
  over saved workspaces
  ([#1669](https://github.com/EffortlessMetrics/ripr-swarm/pull/1669),
  [#3012](https://github.com/EffortlessMetrics/ripr-swarm/pull/3012),
  [#2185](https://github.com/EffortlessMetrics/ripr-swarm/pull/2185)).

- Analysis performance: a per-file fact cache on the diff path (the
  largest single win — unchanged files reuse their previous facts
  instead of re-parsing), parallel index-build parsing via rayon, and
  artifact reuse across `check`/`explain`/`context` so follow-up
  commands do not re-run the analysis
  ([#2039](https://github.com/EffortlessMetrics/ripr-swarm/pull/2039),
  [#2322](https://github.com/EffortlessMetrics/ripr-swarm/pull/2322),
  [#2250](https://github.com/EffortlessMetrics/ripr-swarm/pull/2250)).

- The `ripr agent start` workflow packet now states that its generated commands
  assume bash. `commands.md` carries a prose note above the first command block,
  and `workflow.json` gains an additive `command_shell: "bash"` field. The
  command strings have always used POSIX single-quote quoting and `>`
  redirection, so copying one into cmd.exe (which treats `'` as a literal
  character) or PowerShell (which rejects the `'\''` escape) mis-passes or
  rejects quoted arguments. The note names Git Bash specifically rather than
  "bash on Windows": generated paths keep their Windows drive-letter prefix,
  which WSL resolves as a relative path, so WSL needs each path translated to
  `/mnt/c/...` and `ripr` installed inside it. This is disclosure only: no
  command string, schema version, or existing field changed. The PowerShell
  variants (#2964) and typed argv command specs (#1617) that this entry left
  as separate work landed later in this release; see the entries below
  ([#2963](https://github.com/EffortlessMetrics/ripr-swarm/issues/2963)).

- Published schemas that had only a reverse-direction `schema_version` check are
  now bound to real producer bytes by the verification-contract registry. The
  Rust repair trust corpus of record (`metrics/rust-repair-trust/corpus.json`)
  validates as itself rather than through a copy; the `command_spec` and
  `verification_command_spec` shapes in `schemas/ripr/repair-assurance.schema.json`
  validate against the `command_specs` a generated agent packet actually emits;
  and the design-only `RepairAssuranceV1` envelope validates against the
  assurance vocabulary corpus records that carry a `record` and are not marked
  `invalid`, making a claim that `fixtures/assurance_vocabulary/SPEC.md`
  previously stated but nothing enforced. Patch-shaped cases and advertised
  negatives stay outside that walk and are covered by their own tests, so the
  subject count does not imply coverage it lacks. Each pair carries a negative mutation that must fail.
  `docs/verification/schema-producer-audit.md` records the producer, canonical
  subject, negative mutation, and explicit exemption for every published schema
  — including the `riprAgent` protocol schemas, which remain reserved and
  routed to [#3009](https://github.com/EffortlessMetrics/ripr-swarm/issues/3009)
  ([#2923](https://github.com/EffortlessMetrics/ripr-swarm/issues/2923)).

- The contract validator now evaluates `oneOf`, `if`/`then`/`else`, `not`,
  `minItems`, `maxItems`, `uniqueItems`, `maximum`, and `pattern`. Conditional
  requirements and identity commitments — 40-character head SHAs, `sha256:`
  digests, and the relative `working_directory` constraint — were declared by
  the published schemas and enforced by nothing. `pattern` support is
  fail-closed: an uninterpretable expression is reported as a violation instead
  of assumed to match
  ([#2923](https://github.com/EffortlessMetrics/ripr-swarm/issues/2923)).

- `policy/release-targets.toml` records the committed release-candidate
  membership graph, and `cargo xtask check-release-targets` validates it
  offline. The manifest distinguishes the release goal, claim blockers,
  qualification/proof blockers, release companions, conditional candidates, and
  release-referenced rolling work, and records umbrella parents with an explicit
  `counted_in` and justification so parent/leaf double counting cannot be
  silent. Eight rules are enforced — schema, release identity, role uniqueness,
  committed disjointness, conditional/rolling exclusion, prerequisite ordering,
  parent accounting, and referential closure — each with a fixture that violates
  exactly that rule. Reports land at
  `target/ripr/reports/release-targets.{json,md}`, and the check runs inside
  `cargo xtask precommit` and the CI policy-check pass.

  The checker deliberately does not parse release-goal issue prose. Those bodies
  write some membership as en-dash ranges (`#2665 / #2968-#2970`), so a prose
  parser would silently miss the members inside a range and then report a clean
  graph over issues it never saw. The manifest is the parsed authority; the goal
  bodies remain human-validated documentation. This check is network-free and
  does not compare against live GitHub milestones, does not qualify a candidate,
  and does not represent publication
  ([#3013](https://github.com/EffortlessMetrics/ripr-swarm/issues/3013)).

- `[profile.dev]` now uses `debug = "line-tables-only"` instead of the cargo
  default (`debug = "full"`). Line tables give backtraces with file:line
  resolution without the full variable-debuginfo cost, cutting link time and
  binary size (~9% smaller debug binaries). Full debuginfo is still available
  via `CARGO_PROFILE_DEV_DEBUG=true cargo test` when a developer needs
  step-debugging with variable inspection (#2420).

- `cargo xtask module-health` now reports a **responsibility signal** alongside
  its line count: a heuristic count of distinct top-level concerns (distinct
  `impl` blocks plus distinct public-API identifier prefixes) per file, flagged
  when it exceeds a fixed threshold. This surfaces the "structurally entangled
  even if not huge" case that a pure line count misses (e.g. a small file
  exposing many distinct concern families). Both signals appear in the JSON and
  Markdown reports (`module-health.json` schema bumped to `0.2`, additive). The
  responsibility signal is documented as a smell, not a measurement; the
  advisory still always exits 0 and is never wired into CI gates.

- Property-based tests (`proptest`) added for the diff parser, covering parser
  totality (never panics on arbitrary input), structural invariants (no empty
  paths, no newlines in line text), and line-number validity (`new_side_line >= 1`).
  This is the first property-based testing infrastructure in the repo (#2751).

- New `cargo xtask eval-sweep check` command: the typed offline validator for
  the accepted Python eval-sweep artifacts (RIPR-SPEC-0086). It validates the
  accepted eight-subject manifest under a deny-unknown schema (exactly eight
  uniquely identified subjects, immutable https/sha pins, portable secret-free
  diff paths) and retained run receipts in both owned shapes — the historical
  0.2 report the sweep command writes and the 0.3 currentness shape (binary/
  features/config/profile/input identity, materialization/detection/
  corpus-selection/execution states, evidence digests, repeat-run comparison
  identity, manifest-digest binding) — failing closed on changed denominators,
  unknown state vocabulary, contradictory status, malformed or stale digest
  bindings, and hand-edited aggregates that disagree with the derived rows
  (including a supplied `gate_status` that differs from the gate the rows
  derive and summary distributions that do not match the row-derived key set
  exactly); missing identities are typed `incomplete`, never invented. It
  writes the versioned `eval-sweep-check.{json,md}` reports (kind
  `python_eval_sweep_check_report`, schema `0.1`) whose verdict vocabulary is
  `valid` / `incomplete` / `not_run` — a structural currentness-readiness
  verdict, never a robustness or adequacy claim
  ([#3565](https://github.com/EffortlessMetrics/ripr-swarm/issues/3565)).
- `ripr feedback record` writes a local usefulness receipt bound to one
  analysis snapshot (`--snapshot`, required) with a reason from a closed list,
  such as `useful_actionable` or `false_actionable`, under
  `target/ripr/feedback/`. `ripr feedback export` joins those receipts onto an
  existing `route-quality.json`, listing receipts that match no row instead of
  inventing movement. Recording changes no diagnostic, classification,
  baseline, suppression, gate, or gap closure (#4684).

### Changed

- The Python preview adapter now resolves a module-level named constant used
  as a comparison threshold (`if amount >= DISCOUNT_THRESHOLD:` with
  `DISCOUNT_THRESHOLD = 10_000`), matching the Rust and TypeScript adapters.
  The boundary gets a repair card for `amount == DISCOUNT_THRESHOLD`, the
  missing-discriminator reason names the constant's value, and a test that calls the owner with `10_000` or with
  the imported constant now counts as observing the boundary. A name that can
  be rebound (a second binding, `global`, walrus, star import, `exec`/`globals`/
  `sys.modules`, a nested scope in the owner, a test-file attribute
  assignment) or a non-literal value stays unresolved and gets no repair card
  ([#4227](https://github.com/EffortlessMetrics/ripr-swarm/issues/4227)).
  Two guided-route loops on that card are closed: `ripr agent status` after a
  pilot run that produced only a Python repair card now names the
  `ripr first-pr` route instead of sending the user back to `ripr pilot`, and
  `ripr first-pr` on a Python or TypeScript root reports `stale_artifact` with
  the refresh command when the named test or changed source was edited after
  the gap ledger was written, instead of repeating the finished repair.

- `ripr doctor` now separates installed-binary analysis readiness from the
  prerequisites for building RIPR from source. The default `analysis` profile
  reports a missing `cargo` or `rustc`, or a workspace `rustc` older than
  RIPR's build MSRV (1.95), as `advisory` and exits `0`, so a workspace pinned
  to an older toolchain is no longer told it cannot be analyzed; a missing
  `cargo` still discloses that evidence read from `cargo metadata` is
  withheld. `--profile source-build` fails on those conditions and exits `2`.
  Both toolchain probes run in the selected root. `ripr doctor --json` moves
  to schema `0.3`, with top-level `profile`, `ripr_version`, and `ripr_build_msrv`
  fields and an `advisory` check status
  ([#3907](https://github.com/EffortlessMetrics/ripr-swarm/issues/3907)).

- Test, eval-sweep, diff-load, gate, `cli_smoke` and `seam_cache` cleanup no
  longer discards `remove_dir_all`, `remove_file`, permission-restore or
  `set_current_dir` results with `let _ =`; a cleanup failure is still
  ignored. `clippy-debt-0001` stays deferred, and its `blocked_by` text counts
  the remaining `let _ =` sites
  ([#4013](https://github.com/EffortlessMetrics/ripr-swarm/issues/4013),
  [#4027](https://github.com/EffortlessMetrics/ripr-swarm/issues/4027),
  [#4029](https://github.com/EffortlessMetrics/ripr-swarm/issues/4029),
  [#4038](https://github.com/EffortlessMetrics/ripr-swarm/issues/4038),
  [#4040](https://github.com/EffortlessMetrics/ripr-swarm/issues/4040),
  [#4046](https://github.com/EffortlessMetrics/ripr-swarm/issues/4046)).






- `path_dependencies.rs` no longer carries `allow(dead_code)`.
  `cycle_manifests`, `contains_node`, `forward_walk`, and the scope
  expansion `status` accessor are `#[cfg(test)]`. Cycle-set recording
  moves with the getter; reverse diff-scope reachability is unchanged.
  The `.ripr/allow-attributes.txt` row for that file is removed
  ([#3997](https://github.com/EffortlessMetrics/ripr-swarm/issues/3997)).

- `PanicAllowEntryVersioned::V2` is now `Box<PanicAllowEntryV2>`, so
  `clippy::large_enum_variant` no longer needs an allow on that enum.
  `policy/clippy-exceptions.toml` has no live rows;
  `clippy-exception-0001` is retired
  ([#3995](https://github.com/EffortlessMetrics/ripr-swarm/issues/3995)).

- `cargo xtask check-lint-policy` now requires a non-MSRV `blocked_by` on
  every `[[planned]]` row whose `activate_when_msrv` is already met by
  workspace `rust-version`. Empty or MSRV-only `blocked_by` fails. A
  narrative `reason` does not satisfy the gate. The four current planned
  lints copy their existing `reason` into `blocked_by` and are not promoted
  ([#3990](https://github.com/EffortlessMetrics/ripr-swarm/issues/3990)).

- `cargo xtask check-allow-attributes` now fails a
  `.ripr/allow-attributes.txt` row whose `max_count` is higher than the
  current source count, including a row whose suppression is gone. The
  live `path_dependencies.rs` `allow(dead_code)` budget is tightened from
  7 to 4, matching the four remaining suppressions. Over-budget
  suppressions still fail
  ([#3923](https://github.com/EffortlessMetrics/ripr-swarm/issues/3923)).

- `cargo xtask check-covered-by` now parses `policy/clippy-exceptions.toml`
  as TOML: unique ids, required nonblank fields, optional ISO `expires`
  dates that are not in the past, unknown fields, duplicate keys, and
  trailing garbage. Test-valued `covered_by` resolution uses the
  TOML-decoded command (including single-quoted strings)
  ([#3867](https://github.com/EffortlessMetrics/ripr-swarm/issues/3867)).

- `cargo xtask check-lint-policy` now compares `[[planned]]`
  `activate_when_msrv` to `[workspace.package] rust-version`. The four current
  planned lints are not promoted; the remaining-blocker field is `blocked_by`
  ([#3990](https://github.com/EffortlessMetrics/ripr-swarm/issues/3990))
  ([#3809](https://github.com/EffortlessMetrics/ripr-swarm/issues/3809)).

- `cargo xtask check-lint-policy` now parses `policy/clippy-debt.toml`
  as TOML: unique ids, required nonblank fields, ISO `target` dates that
  are not in the past, unknown fields, duplicate keys, and debt lints
  that are not already active, planned, or present in `Cargo.toml`.
  `clippy-debt-0001` stays deferred
  ([#3833](https://github.com/EffortlessMetrics/ripr-swarm/issues/3833)).

- `crates/ripr/README.md` Development now leads with `cargo xtask precommit`
  and names `ci-full` as the complete local pass. The sequential cargo
  block is labeled targeted-rerun inventory, matching `AGENTS.md` (#3775)
  and `docs/IMPLEMENTATION_PLAN.md` (#3817)
  ([#3830](https://github.com/EffortlessMetrics/ripr-swarm/issues/3830)).

- `cargo xtask cache report` / `gc` honor `RIPR_CACHE_DIR` instead of always
  scanning `target/ripr/cache`. Relocated roots must be absolute, must not
  traverse `..`, and must look like a ripr cache before any walk or delete.
  `ripr cache status` prints a cleanup hint that exports the same
  `RIPR_CACHE_DIR` the status process used
  ([#3808](https://github.com/EffortlessMetrics/ripr-swarm/issues/3808)).

- `policy/clippy-exceptions.toml` and `docs/CLIPPY_POLICY.md` no longer say
  the exceptions ledger is empty by default. The commented example no
  longer reuses a real id or a past `expires` date
  ([#3820](https://github.com/EffortlessMetrics/ripr-swarm/issues/3820)).

- `policy/clippy-lints.toml` no longer says planned lints wait for MSRV
  and a matching xtask gate. The values are not verified available-since
  ([#3809](https://github.com/EffortlessMetrics/ripr-swarm/issues/3809)).

- Live entry docs no longer name the deleted `.ripr/goals/active.toml` file as
  current selection authority. `docs/IMPLEMENTATION_PLAN.md`,
  `docs/agent-context/CONTEXT_SYSTEM.md`, and
  `plans/rust-one-shot-evidence-to-repair.md` now point at GitHub issues/PRs
  for live selection. `.allow/spec-system/slices/` remains PR-local scope
  (`ImplementationSliceV1`), not a second live selector, matching the repo
  tracking model
  ([#3780](https://github.com/EffortlessMetrics/ripr-swarm/issues/3780)).

- `docs/POLICY_ALLOWLISTS.md` no longer claims `cargo xtask
  check-allow-attributes` matches `policy/clippy-exceptions.toml`. That
  gate still counts source suppressions against `.ripr/allow-attributes.txt`
  only; the TOML receipts remain advisory until a follow-up wires them
  ([#3800](https://github.com/EffortlessMetrics/ripr-swarm/issues/3800)).

- `docs/POLICY_ALLOWLISTS.md` no longer claims `cargo xtask check-lint-policy`
  consumes `policy/clippy-debt.toml`; the gate reads it since
  [#3833](https://github.com/EffortlessMetrics/ripr-swarm/issues/3833).
  `clippy::let_underscore_must_use`
  is now a real `[[debt]]` row (`clippy-debt-0001`, target 2027-03-31)
  instead of a commented example whose target had already passed
  ([#3782](https://github.com/EffortlessMetrics/ripr-swarm/issues/3782)).

- Reviewer-facing architecture copies in `docs/ENGINEERING.md` and
  `.factory/skills/review-guidelines/SKILL.md` now name `agent`, `config`,
  `mcp`, and `provider_contract`, matching the product map. The
  `check-agent-skills` pin remains on `AGENTS.md`, `CLAUDE.md`, and
  `docs/ARCHITECTURE.md`
  ([#3779](https://github.com/EffortlessMetrics/ripr-swarm/issues/3779)).
- Agent and human architecture maps now name the `mcp` protocol adapter and
  the `provider_contract` DTO surface. `cargo xtask check-agent-skills` pins
  the required module tokens in `AGENTS.md`, `CLAUDE.md`, and
  `docs/ARCHITECTURE.md` so the `#1943` drift class cannot recur silently
  ([#3774](https://github.com/EffortlessMetrics/ripr-swarm/issues/3774)).
- `AGENTS.md` local validation now leads with `cargo xtask precommit` and
  names `ci-full` as the complete pass. The 40-command block is labeled
  targeted-rerun inventory, not sequential required work
  ([#3775](https://github.com/EffortlessMetrics/ripr-swarm/issues/3775)).
- `docs/IMPLEMENTATION_PLAN.md` Required Gates now leads with
  `cargo xtask precommit` and names `ci-full` as the complete local pass.
  The cargo command block is labeled targeted-rerun inventory, matching
  `AGENTS.md` (#3775). Extension compile/package remains required for
  `editors/vscode` changes; neither `precommit` nor `ci-full` covers it
  ([#3817](https://github.com/EffortlessMetrics/ripr-swarm/issues/3817)).
- `docs/handoffs/README.md` labels retained campaign closeouts as historical.
  `docs/agent-context/repo-map.md` no longer names the retired active-goal
  manifest as a live selector
  ([#3777](https://github.com/EffortlessMetrics/ripr-swarm/issues/3777)).
- Schema 0.3 no-panic allowlist `id` values are now unique and gated.
  `cargo xtask check-no-panic-family` rejects a colliding `id` even when
  the selectors differ. Four reused `panic-0051`..`panic-0054` rows for
  the RIPR-SPEC-0112 `cli_smoke` sites were renumbered to `panic-0071`..
  `panic-0074`
  ([#3799](https://github.com/EffortlessMetrics/ripr-swarm/issues/3799)).

- The 0.11.0 support claim now describes the Rust gap-repair loop as `usable
  alpha`, not unqualified `usable`. Fixture, package, editor, bounded test-only
  packet, and before/after receipt proof remains intact, but the governed
  real-repository corpus currently contains zero eligible attempts, so route
  yield and ordinary-user success are not established. `cargo xtask
  check-support-tiers` now hard-caps the uniquely named canonical row at
  `usable alpha` until one promotion decision covers both the full governed
  corpus and the installed CLI/packaged VS Code pilot. A complete trust report
  with real movement is necessary evidence, but cannot promote the claim by
  itself (#3077).

- `exposed` findings stay at `info` severity, as in 0.10.0: `--format github`
  emits `::notice` and the LSP reports INFORMATION, below `weakly_exposed` and
  `reachable_unrevealed` at `warning`. A development-only change that raised
  the `[severity.findings] exposed` default to `warning`
  ([#2592](https://github.com/EffortlessMetrics/ripr-swarm/issues/2592)) was
  reverted before release
  ([#4429](https://github.com/EffortlessMetrics/ripr-swarm/pull/4429)). Set
  `exposed = "warning"` under `[severity.findings]` in `ripr.toml` to raise it.

- Generated commands now come with a PowerShell form next to the bash one.
  The agent workflow packet, pilot summaries, agent status and review
  summaries, PR evidence reproduction commands, and the first-pr start-here
  packet pair each bash fence with a `powershell` fence produced by one shared
  translator. The bash bytes are unchanged and cmd.exe stays explicitly
  unsupported. Quoted redirect tokens keep their quoting, and multiline
  command lists that cannot be translated safely show only the bash form
  ([#2964](https://github.com/EffortlessMetrics/ripr-swarm/issues/2964),
  [#3438](https://github.com/EffortlessMetrics/ripr-swarm/pull/3438),
  [#3617](https://github.com/EffortlessMetrics/ripr-swarm/pull/3617),
  [#3625](https://github.com/EffortlessMetrics/ripr-swarm/pull/3625),
  [#3661](https://github.com/EffortlessMetrics/ripr-swarm/pull/3661),
  [#3662](https://github.com/EffortlessMetrics/ripr-swarm/pull/3662)).

- Rust match-arm analysis gains one bounded source of discrimination
  evidence: when a changed arm belongs to a `match` over two direct,
  immutable `bool` parameters (four distinct unguarded tuple arms with
  simple string results), and a test that calls the owner directly
  asserts equality between that exact input tuple and the arm's
  current result, the arm can now reach `exposed` instead of staying
  `weakly_exposed` with `observation_unverified`. The assertion must sit
  at a conventional Cargo compilation-unit root (`src/lib.rs`,
  `src/main.rs`, `src/bin/<target>.rs`) or the owner package's direct
  `tests/<target>.rs`. Sibling, reordered, transformed, guarded,
  aliased, lexical-fallback, nested or inherited-macro, custom-root, and
  cross-package shapes stay unverified. The producer refines only the
  discrimination stage; reach, propagation, and the final class combiner
  are unchanged
  ([#3767](https://github.com/EffortlessMetrics/ripr-swarm/pull/3767),
  [ripr#1714](https://github.com/EffortlessMetrics/ripr/issues/1714)).

- The published `ripr` crate now depends on `toml` 1 (was 0.9),
  `ra_ap_syntax` 0.0.349 (was 0.0.330), and, on Windows, `winsafe`
  0.0.29 (was 0.0.28), alongside 85 compatible lockfile updates.
  `unicode-ident` stays held at 1.0.24, and `oxc` and `sha2` stay on
  their current majors. MSRV is unchanged
  ([#3827](https://github.com/EffortlessMetrics/ripr-swarm/pull/3827)).

### Docs

- Documented the proposed `ripr-rs` PyPI distribution and
  `@effortlessmetrics/ripr` npm launcher/native package family, including
  the development-in-swarm/source-owned-publication boundary, maintainer
  registry setup, package bootstrap ordering, and explicit non-claims. This is
  planning and review guidance; it does not claim that either package family is
  built, published, reserved, or installable
  ([#4487](https://github.com/EffortlessMetrics/ripr-swarm/issues/4487),
  [#4496](https://github.com/EffortlessMetrics/ripr-swarm/pull/4496)).

- The README and quickstart first run now define "discriminator" where it
  first appears and state `ripr check`'s exit codes. They add a one-line
  `cargo install --locked --git` development install and a short section on
  running ripr from a coding agent, including keeping `target/` gitignored
  between repair phases. A stale `ripr doctor` troubleshooting claim was removed
  ([#4413](https://github.com/EffortlessMetrics/ripr-swarm/pull/4413)).

- `docs/REPAIR_ATTEMPT.md` and `docs/COMMAND_HIERARCHY.md` now document
  the three-phase governed Python repair sequence: trust-selection flags
  on `before` only, matching `--edit-authorized` / `--edit-authority`
  on `before` and `after`, and a separately authorized
  `--phase verify` that accepts only `--attempt`. They cover the
  optional `--verify-rollback` request and its `proved` / `blocked` /
  `not_run` disposition, the retained receipt and execution-record
  paths, and the rule that command success and static movement are
  separate observations
  ([#3747](https://github.com/EffortlessMetrics/ripr-swarm/issues/3747)).

- `AGENTS.md` and `CLAUDE.md` gate inventories now list
  `cargo xtask check-agent-skills`, which routed Rust CI already
  requires, and name the formatter check as `cargo fmt --check`
  ([#3826](https://github.com/EffortlessMetrics/ripr-swarm/pull/3826)).

## 0.10.0 - Honest-by-construction evidence and downstream gate adoption

Release date: 2026-06-15 (crates.io publication; the GitHub release draft for this version remains unfinalized).

RIPR 0.10.0 hardens the central promise that evidence is only credited to a seam
it actually observes. The headline is honesty-by-construction: across Rust,
TypeScript, and Python, over-claims now fail closed into named limitations
instead of becoming a confident `exposed` / strong-oracle finding, and that
property is enforced by a standing meta-gate rather than re-checked per surface.
The release also makes RIPR adoptable as a downstream CI gate — a single,
documented receipt number a generic thresholding gate can consume — and finishes
the content-addressed finding-id migration so suppressions track code, not lines.

### Breaking changes

- **Content-addressed finding/probe IDs** (#1053): The `id` field format changed from `probe:<path>:<line>:<family>` to `probe:<path>:<family>:<fp8>[.<n>]` where `<fp8>` is the first 8 hex chars of SHA-256 over `path\0family\0owner\0expression\0`. The `line` segment is removed from the id. The `line` field in the `probe` JSON object is unchanged. Existing `.ripr/suppressions.toml` entries keyed by `finding_id` must be updated to the new id format; stale ids will fail closed (no match = not suppressed). The new ids track the code (expression + owner + family), so a suppression survives line movement and invalidates when the expression changes.

### Added

- **Canonical `new_unsuppressed` gate receipt** (RIPR-SPEC-0111, #1038): `gate-decision.json` now carries `new_unsuppressed { basis, count, reason }` — a documented, stable count a generic thresholding gate (e.g. `max_new_unsuppressed = 0`) can consume without modelling RIPR's full decision logic. It is a filter over the gate's own `decisions[]` (consumer-verifiable), includes policy-eligible advisory candidates so an external policy can be applied independently, and fails closed (`basis: null`) when analysis did not run. Additive field; `schema_version` unchanged.
- **Receipt ledger cross-reference** (RIPR-SPEC-0110, #1261): `ripr receipt check --ledger <path>` cross-references a receipt's `canonical_gap_id` against the gap ledger; absence of `--ledger` is not interpreted as "receipt ok", and orphan / gap-mismatch receipts exit non-zero.
- **`cargo xtask module-health`** (#1147): advisory report flagging oversized Rust source files as a refactor-before-extend signal. Advisory only — never fails CI.
- **VS Code cockpit inspection commands** (#1119, #1123, #1134, #1138): receipt-status and route-quality inspection wired into the command palette (inspect/copy only).
- **Confidence ceiling** (RIPR-SPEC-0109, #1258): Low/Unknown evidence confidence caps the displayed confidence score so it can only lower, never inflate, a finding's apparent strength.

### Fixed (honesty hardening)

- **Evidence-promotion honesty meta-gate** (RIPR-SPEC-0108): a standing gate asserts each charter fixture's exposure class independently of its pinned golden, catching a dishonest re-bless that golden-equality alone would accept. Closes the cross-language "fake-clean" class (evidence credited to a seam it does not observe) across Rust, TypeScript, and Python.
- **Error-path seams require a variant-observing oracle** (RIPR-SPEC-0106/0107, #1252/#1254/#1255): an error-return seam reaches `exposed` only when a test pins the exact error variant; `unwrap_err()` + `assert_eq!(err, Variant)` is recognized as an exact-error-variant oracle, and that recognition is robust to single-line / un-`rustfmt`'d test formatting (no more discriminated seam contradicting its own `missing_discriminators`).
- **Python owner identity** (#1260, #1264, #1271): method-owner and free-function exposed credit now require receiver / import-source-module identity, so a same-named symbol in another module no longer borrows a test's evidence.
- **TypeScript assertion-level filtering** (#1236, #1248): oracle-kind matching and the SideEffect observation guard operate at the assertion level, so a broad `.toThrow()` or a mock-call assertion no longer over-credits a value seam.

### Internal / CI

- Advisory proof-routing with the first docs-only lane skip, a `ci-budget` hygiene report, and a scheduled scratch-GC lane across all three self-hosted runner pools (#1028).

## 0.9.0 - Multi-language evidence-to-repair preview

Release date: 2026-06-10.

RIPR 0.9.0 extends the Lane 1 evidence-to-repair foundation beyond Rust-only
assumptions. The headline is not that RIPR understands TypeScript; it is that
TypeScript, JavaScript, and Bun evidence is now visible, bounded, and honest.
Cross-language uncertainty fails closed into named limitations instead of
becoming fake repair guidance.

This release syncs release-intended `ripr-swarm` work back into source `ripr`
with a history-preserving merge commit (swarm sync candidate `ff902885`,
delta `74b1fab0..ff902885`, 145 commits). The post-freeze delta over the
original `1cce26df` candidate is behavior-preserving repository infrastructure
(advisory proof-routing CI, the first docs-only lane-skip, contract/xtask proof
wiring, golden re-bless, and `ripr-plus` timeout hardening); `crates/ripr`
runtime behavior is unchanged and the release Claim and Non-Claims are
unchanged. Source `ripr` remains the release, publishing, signing, marketplace,
badge, and distribution authority.

### Release themes

- TypeScript/Bun bounded preview adapter (opt-in, advisory).
- Perl strict actionability **model** (fixture-only / test-scoped — the adapter is `#[cfg(test)] mod perl;` and not production-routable yet; see Campaign 31, #1379).
- Preview cards across every output surface (renderers exist; Perl cards project only from synthetic test findings until the production exporter/consumer bridge lands).
- Diff-first changed-surface review.
- Cache sharding and explicit large-repo cache limits.
- No new authority: preview evidence does not emit public repair packets.

### Added

#### TypeScript/Bun preview adapter (opt-in, advisory)

- Added Bun `ArrayBuffer` discriminator facts, stable-byte oracle
  classification, bridge hint evidence, and bounded verdict wording.
- Added cross-language oracle gap routing as named limitations: TS
  discriminator witness routes, unknown-bridge witnesses, and missing
  oracle-edge routing.
- Added `copy_to_unshared` bridge evidence with a configured route and a Bun
  Markdown cross-language profile.
- Added the configured bridge inventory report with boundary hardening.
- Added live Bun stable-byte dogfood receipts and first-run copy-pasteable
  Bun UB preview docs.
- Calibrated Bun stable-byte evidence distinguishes
  `rust_ungripped_ts_discriminated`, `rust_ungripped_ts_missing_discriminator`,
  `ts_mention_not_observer`, `bridge_unknown`, and named static limitation
  states. This is a Bun UB review signal, not a support-tier promotion.

#### Perl strict actionability and preview projection

- Hardened strict actionability packet bounds with focused helper tests.
- Added preview cards to check JSON, human output, SARIF, GitHub annotations,
  and gap ledger Markdown, so preview evidence renders consistently on every
  surface without becoming a public repair packet.

**Scope caveat (recorded retroactively for honesty, Campaign 31 #1379):** the
Perl work in 0.9.0 is fixture-only and test-scoped. The adapter module is
`#[cfg(test)] mod perl;` (`crates/ripr/src/analysis/language/mod.rs:25-26`),
`lang-perl` is an empty Cargo feature not in `default`, the production path
router recognizes no `.pm`/`.pl`/`.t`/`.psgi` extension, and the pipeline
returns a fail-closed stub even with the feature on. The preview-card
renderers are real production code, but they project only from synthetic test
findings — no production Perl source can feed them until the
`perl-lsp ripr-facts` exporter and the production `PerlAdapter` bridge land.
Perl's support tier is `scaffold`, not `preview`; see
[Support Tiers](docs/status/SUPPORT_TIERS.md).

#### Diff-first review

- Added `ripr diff` changed-surface mode v1 and a diff-scoped review fast
  path for draft-time iteration, especially where full-repo analysis is
  limited or expensive.

#### Cache sharding and large-repo limits

- Added seam-cache sharding for large repositories, sharded cache set
  reporting, and surfaced large seam-cache skips as explicit states.

#### Python projection

- Added no-action preview-state annotations to SARIF output so Python
  findings without a safe action are visibly advisory instead of silent.

### Changed

- Conditional receiver helper owner tracing improves owner-call routing.
- Runtime completeness remains explicit: limited, sampled, and incomplete
  runs carry `repair_route` and `downstream_consumable = false`.

### Known limitations

- TypeScript and JavaScript support is opt-in preview. Unknown frameworks,
  helper-gated assertions, unresolved targets, and unknown bridges are named
  limitations with analyzer routes, not repair packets.
- Cross-language oracle visibility is resolved only for configured bridges;
  there is no full Bun binding graph.
- Bun stable-byte evidence is advisory and calibrated to currently modeled
  routes.
- `ripr check --format json` can produce very large JSON for some diffs and can
  fail when writing large output to stdout on Windows; prefer redirecting JSON
  to a report file rather than piping it through stdout. Output-size bounds and
  a stdout-write fallback are planned for a 0.9.x follow-up.

### Non-claims

This release does not claim:

- TypeScript or JavaScript stable support, or Bun UB proof.
- Runtime Bun, Jest, Vitest, `tsc`, `tsserver`, Miri, or mutation execution.
- Generated tests or source edits.
- Default gates, badges, baselines, or RIPR Zero contribution from
  TypeScript/Bun preview evidence.
- Public repair packets from TypeScript/Bun preview evidence.
- A full Bun binding graph or generic cross-language support for every mixed
  TypeScript/Rust repository.
- Autonomous edits, provider integration, mutation execution, or a default
  blocking CI / badge semantic switch.

### Validation

- `cargo fmt --check`, `cargo test --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo xtask check-pr`, `check-output-contracts`, `check-static-language`,
  `check-traceability`, `check-capabilities`, `check-doc-index`
- Lane proof: `lane1-evidence-audit`, `ripr-swarm plan`, `ripr-swarm
  readiness`, `evidence-quality-scorecard`, `evidence-quality-trend`,
  `receipts check` (limited states explicit and routed)
- `cargo package -p ripr --locked`, `cargo publish -p ripr --dry-run --locked`

## 0.8.0 - Evidence-to-repair foundation

Release date: 2026-06-02.

RIPR 0.8.0 makes the Lane 1 evidence-to-repair foundation release-ready. The
main change is not that RIPR finds more things; it is that RIPR is more careful
about what it calls actionable. Public repair packets now require a bounded
proof, receipt, and edit contract, while unsupported findings remain named
limitations with analyzer routes.

This release syncs release-intended `ripr-swarm` work back into source `ripr`
with a history-preserving merge commit. Source `ripr` remains the release,
publishing, signing, marketplace, badge, and distribution authority.
`ripr-swarm` remains the development trunk after the release branch.

### Release themes

- Evidence-to-repair trust loop.
- Explicit full/limited runtime status.
- Strict public actionability projection.
- Fail-closed repair packet readiness.
- Static limitation routes as analyzer backlog.
- Attempt, readiness, and route-quality foundations.
- Cache and report operability.
- Source/swarm release boundary cleanup.

### Added

#### Runtime completeness

- Added explicit runtime status across Lane 1 JSON and Markdown reports so
  report consumers can distinguish full runs from limited inputs.
- Added named limited states for timeout, runner failure, large-cache skip,
  incomplete input, malformed input, stale input, and warning cases.
- Added `downstream_consumable` status so partial reports cannot masquerade as
  full run output.
- Added runtime-status sections to human-facing Markdown reports, including the
  Lane 1 audit, actionable gaps, swarm plan, readiness, scorecard, and trend
  reports.

#### Cache and report operability

- Added `cargo xtask cache report` for inspecting `target/ripr/cache` growth.
- Added `cargo xtask cache gc --dry-run` with bounded defaults for maximum
  cache size and TTL.
- Added cache-GC safeguards so cleanup only targets RIPR analysis cache and
  does not remove reports, receipts, source files, workflow artifacts, build
  output, or PR/review packets.
- Added release and CI cleanup paths for RIPR analysis cache where reports are
  generated or artifacts are uploaded.

#### Actionability and repair packets

- Added stricter public projection rules for actionable gaps.
- Added stable projection exclusion reasons for packets missing safe handoff
  fields.
- Added `allowed_edit_surface` as part of the delegated repair contract.
- Added fail-closed routing for packets missing `gap_state = actionable`,
  `verify_command`, `receipt_command`, `must_not_change`, `allowed_edit_surface`,
  confidence, target shape, related context, repair route, or `raw_evidence_refs`.
- Added stronger separation between internal/actionable audit packets and
  public or swarm-ready packets.

#### Readiness and blocked-state routing

- Added readiness blocked-state routes for missing context, static limitations,
  public projection exclusions, field-level handoff blockers, and
  operator-judgment blockers.
- Added readiness counts and examples for the dominant blockers that prevent a
  packet from becoming swarm-ready.
- Added `top_next_action` as a stable first-action projection for thin
  consumers.
- Added `top_limitation_routes` so analyzer backlog remains visible even when
  no public repair packet is safe.

#### Static limitation routing

- Added named analyzer routes for previously vague static limitations.
- Added route splits for affinity-only owner-call absence.
- Added route splits for iterator-derived boundary operands versus local or
  computed boundary operands.
- Added limitation-backlog semantics so non-actionable evidence still gives
  maintainers a next analyzer move without becoming user repair work.

#### Attempt and outcome foundations

- Added or hardened `cargo xtask ripr-swarm attempt-ledger`.
- Added attempt-history visibility in readiness.
- Added outcome categories for improved, unchanged, regressed, resolved,
  missing-receipt, attempted-without-receipt, expected-unchanged, and orphan
  receipt states.
- Added repair-route quality surfaces where attempt evidence exists.

#### User-surface alignment

- Added stronger alignment between reports, readiness, LSP actions, badge
  inputs, PR summaries, and CI/advisory surfaces.
- Added LSP behavior that consumes bounded repair-card data rather than
  inventing repair actions from raw findings.
- Added guardrails so public surfaces consume canonical actionability state
  rather than raw findings or sampled report fragments.

### Changed

- Public actionability now requires `gap_state = actionable`.
- Non-actionable but repair-shaped packets now fail closed instead of becoming
  public repair work.
- Static limitations now remain named limitations or analyzer backlog until
  RIPR can provide a safe bounded repair route.
- Readiness now explains why packets are blocked instead of only reporting
  aggregate blocked counts.
- Markdown reports now preserve the same runtime-completeness story as JSON.
- Repair packet projection now prefers existing module or ancestor test files
  when available instead of inventing broad fallback paths.
- Rust call-presence and related-test evidence handles more helper shapes,
  including same-file helper-owner chains, imported production helper wrappers,
  negated condition helpers, eager wrapper calls such as `extend`, and
  `unwrap_or_default` helper paths.
- Rust match-arm and predicate routing is narrower for generic or
  value-insensitive shapes, reducing false-actionable risk without claiming
  runtime mutation outcomes.
- Preview Python, TypeScript, and JavaScript surfaces continue to report
  advisory/static-limit information without promotion to stable gate authority.

### Fixed

- Fixed cases where incomplete or stale packet artifacts could lose field-level
  blocker information.
- Fixed missing target-shape routing for actionable packets.
- Fixed missing allowed-edit-surface routing.
- Fixed missing raw-evidence reference routing.
- Fixed public projection leakage for unresolved or non-actionable gap states.
- Fixed Windows-sensitive xtask timeout validation needed for release gates.
- Fixed cache command catalog and policy metadata for the new cache commands.
- Fixed several documentation/schema mismatches around Lane 1 output contracts.

### Documentation and schema

- Updated `docs/OUTPUT_SCHEMA.md` for runtime status, repair packet projection,
  readiness, blocked-state routes, limitation routes, and attempt/outcome
  surfaces.
- Updated Lane 1 specs for canonical actionability, public projection, external
  agent handoff, repair-loop readiness, and surface translation.
- Updated capability and traceability metadata for new report contracts,
  validation fixtures, and release proof commands.
- Added release-freeze handoff expectations for source `ripr` and development
  `ripr-swarm`.

### Validation

0.8.0 release validation is expected to include:

- `cargo fmt --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --all-targets`
- `cargo xtask check-pr`
- `cargo xtask check-output-contracts`
- `cargo xtask check-static-language`
- `cargo xtask check-traceability`
- `cargo xtask check-capabilities`
- `cargo xtask check-doc-index`
- `cargo xtask markdown-links`
- `cargo xtask cache report`
- `cargo xtask lane1-evidence-audit`
- `cargo xtask actionable-gaps`
- `cargo xtask ripr-swarm plan --top 10`
- `cargo xtask ripr-swarm readiness`
- `cargo xtask ripr-swarm attempt-ledger`
- `cargo xtask evidence-quality-scorecard`
- `cargo xtask evidence-quality-trend`
- `cargo xtask receipts check`
- `cargo package -p ripr --locked`
- `cargo publish -p ripr --dry-run --locked`

The source release candidate was also checked with package and editor proof:
`cargo package -p ripr --list`, `npm --prefix editors/vscode ci`,
`npm --prefix editors/vscode run compile`, and
`npm --prefix editors/vscode run test:e2e`.

### Known limitations

- Some Lane 1 runs may still be limited or sampled; limited runs must say so
  explicitly.
- A high static-limitation count is expected where RIPR refuses to invent
  unsafe repair packets.
- `0 actionable` can be correct when no packet satisfies the full actionability
  contract.
- Limitation backlog routes are analyzer work, not user repair tasks.
- Advisory infrastructure checks may still fail independently of required
  release gates.
- Preview-language evidence remains advisory unless a later release explicitly
  promotes it.

### Non-claims

RIPR 0.8.0 does not claim:

- autonomous code editing;
- provider integration;
- mutation execution;
- generated tests;
- default blocking CI gate semantics;
- default public badge semantic changes;
- complete full-repo analysis in every local environment;
- that limited or sampled reports are full runs;
- that every static signal is actionable;
- that static limitations are repair packets;
- that badge, LSP, PR, or CI surfaces should count raw findings as product
  truth;
- killed/survived mutation status, coverage adequacy, or correctness proof.

### Upgrade notes

- Consumers of Lane 1 JSON should expect new runtime-status, readiness,
  blocked-state, projection-exclusion, limitation-route, and attempt/outcome
  fields.
- Consumers should treat raw findings as diagnostic input, not public
  actionability.
- Public packet consumers should require `gap_state = actionable` plus complete
  repair packet fields, including `verify_command`, `receipt_command`,
  `must_not_change`, `allowed_edit_surface`, and `raw_evidence_refs`.
- Limited reports should not be treated as full runs unless
  `downstream_consumable` explicitly allows downstream use.
- Public badge, LSP, PR, and CI consumers should use canonical actionability
  and runtime-status projections rather than recomputing actionability from raw
  report rows.

## 0.7.0 - 2026-05-20

- Added a 0.7 swarm repair-loop dogfood receipt that records live
  `lane1-evidence-audit` fail-closed timeout behavior, fixture-backed
  `ripr-swarm plan` ranking, ready and static-limit dry-run attempt packets,
  verify/receipt command separation, and actionable-gap outcome joins for
  not-attempted, improved, unchanged, and orphaned receipt states. The receipt
  also records that current live-repo packet repair proof remains a
  release-readiness decision because the full live audit timed out before
  producing actionable packets.
- Added a 0.7 release-readiness closeout that keeps release and publishing
  authority in source `ripr`, accepts the current `ripr-swarm` live audit
  timeout as a bounded 0.7 limitation rather than a source-promotion blocker,
  and records the source promotion, version bump, release proof, and publish
  boundaries for the actionable static repair-loop release.
- Added `cargo xtask ripr-swarm plan --top <n>` to rank existing
  `actionable-gaps.json` packets into swarm-ready, blocked, and
  missing-verify-or-receipt buckets. The report writes
  `target/ripr/reports/swarm-plan.{json,md}`, blocks static limitations and
  missing receipt/verify context, and remains dry-run/report-only with no file
  edits, provider calls, generated tests, mutation execution, receipts, gates,
  badges, PR/CI rendering, or editor/LSP behavior changes.
- `cargo xtask evidence-health` now bounds its preflight `cargo build -p ripr`
  phase with the evidence-health timeout and writes diagnostic warning reports
  with `phase = evidence_health_build` if that phase times out or fails. This
  prevents cold or pathological builds from consuming the outer shell timeout
  without producing `evidence-health.json` / `.md`; the fallback artifact names
  a limitation and does not claim user test debt.
- `cargo xtask evidence-health` now uses a 4-minute default bounded runtime for
  both build and report-generation phases so pathological live-repo evidence
  health runs degrade to `evidence_health_timeout` warning artifacts before
  abnormal termination can silently drop `evidence-health.json` / `.md`.
- Lane 1 now has an actionable-gap outcome report that joins existing
  actionable packets with optional agent receipt and targeted-test outcome
  artifacts. `cargo xtask actionable-gap-outcomes` writes
  `target/ripr/reports/actionable-gap-outcomes.{json,md}` with bounded outcome
  states such as `not_attempted`, `evidence_improved`, `evidence_unchanged`,
  `evidence_regressed`, and `resolved` so repair attempts can be tracked
  without running repairs, generated tests, provider calls, mutation execution,
  public badge changes, or PR/CI rendering.
- Public actionable projection docs now define internal badge-readiness stages:
  packet readiness, scorecard readiness, and badge-basis readiness. The spec
  records that Lane 1 scorecard/trend packet readiness is internal evidence
  only and does not authorize public endpoint refreshes without the generated
  badge workflow and an explicitly scoped badge PR.
- Lane 1 evidence-quality scorecard and trend reports now carry actionable-gap
  packet public-projection readiness from the live audit. The scorecard reports
  eligible and excluded packet counts plus projection-exclusion reasons, and
  the trend tracks eligible packets as higher-is-better and excluded packets as
  lower-is-better. This is internal badge-readiness evidence only; it does not
  change public badges, PR/CI rendering, gate policy, providers, generated
  tests, source edits, or mutation execution.
- Added `RIPR-PROP-0014` to define the `ripr-swarm` campaign rationale:
  consume actionable canonical packets, rank bounded repair attempts, require
  receipts and evidence movement, reject raw-finding queues and arbitrary agent
  repairs, and keep providers, generated tests, mutation execution, public
  badges, PR/CI rendering, and editor/LSP changes out of scope.
- Added `RIPR-SPEC-0057` for the planned `ripr-swarm` repair loop. The spec
  defines swarm work as bounded execution over actionable canonical gap
  packets, not raw findings, and requires verify commands, receipt commands,
  must-not-change boundaries, typed attempt states, and outcome joins before a
  repair attempt can claim movement.
- Lane 1 evidence audit generation now treats a nominally successful
  repo-exposure subprocess with an empty or malformed captured JSON file as a
  bounded `lane1_repo_exposure_incomplete` limitation. The audit removes the
  partial input, preserves phase/input diagnostics and the latency trace tail,
  and writes limited reports instead of failing before downstream scorecards can
  surface the incomplete run. This is repo-local audit reliability only; it does
  not change analyzer behavior, PR/CI rendering, gates, badges, providers,
  generated tests, source edits, or mutation execution.
- Lane 1 evidence audit JSON now emits free-form text counts as complete
  `{label, count}` rows instead of arbitrary object keys for
  missing-discriminator reasons and values, static-limitation reasons, and
  oracle-semantics strings. This preserves case-only variants such as `Path`
  and `path` while keeping the live audit artifact parseable for
  Windows/PowerShell consumers. This is a repo-local Lane 1 reporting contract
  hardening; it does not change analyzer behavior, PR/CI rendering, gates,
  badges, providers, generated tests, source edits, or mutation execution.
- Lane 1 evidence-quality scorecard generation now writes a bounded diagnostic
  scorecard when it cannot regenerate a missing Lane 1 audit artifact. The
  diagnostic names
  `evidence_quality_scorecard_audit_regeneration_failed`, records a repair
  route, and keeps counts diagnostic-only so missing audit regeneration does
  not silently drop scorecard evidence or claim user test debt.
- Lane 1 evidence audit and scorecard now report runtime confidence coverage by
  canonical evidence class. The reports show calibrated-supported,
  fixture-backed, static-only, unknown-confidence, uncalibrated, actionable,
  and limitation item counts by class so badge-readiness and repair-class
  planning can see where static confidence still lacks runtime support. This is
  repo-local Lane 1 reporting only; it does not change public badges, PR/CI
  rendering, gate policy, provider calls, generated tests, source edits, or
  mutation execution.
- Lane 1 evidence audit now emits bounded actionable-gap packet artifacts for
  agent and maintainer triage. `cargo xtask lane1-evidence-audit` writes
  `target/ripr/reports/actionable-gaps.json` and
  `target/ripr/reports/actionable-gaps.md` from
  `evidence_record.canonical_item`, keeping raw findings as supporting evidence
  while carrying repair kind, verification command or explicit unknown, related
  test or observer context, confidence basis, and conservative
  `must_not_change` boundaries. This is repo-local Lane 1 reporting only; it
  does not change public badges, PR/CI rendering, gate policy, provider calls,
  generated tests, source edits, or mutation execution.
- Lane 1 actionable-gap packets now carry audit-only public projection
  readiness fields. Packets distinguish canonical repair/verify guidance from
  badge-readiness prerequisites by reporting `public_projection_eligible`,
  `projection_exclusion_reasons`, repair/verify field sources, and missing
  receipt command or path reasons. This keeps agent-usable packets from being
  mistaken for public badge-ready items and does not change public badges,
  PR/CI rendering, gate policy, provider calls, generated tests, source edits,
  or mutation execution.
- Actionable Lane 1 canonical evidence items now carry a canonical
  `receipt_command` for the existing agent receipt loop. The audit packet layer
  can now reduce `missing_receipt_path` exclusions from producer evidence
  rather than a report-side guess while leaving public badges, PR/CI rendering,
  gate policy, providers, generated tests, source edits, and mutation execution
  unchanged.
- Lane 1 run-reliability reports now emit bounded warning artifacts for the
  expensive live paths instead of leaving stale output or failing without a
  report on timeout. `cargo xtask lane1-evidence-audit` records a named
  `lane1_repo_exposure_timeout` run limitation with repo-exposure latency
  context when input generation exceeds its timeout, `cargo xtask
  evidence-health` records `evidence_health_timeout` when the child report
  times out, and the scorecard surfaces those limited inputs as unknowns rather
  than treating zero or partial counts as complete repo truth.
- Lane 1 evidence audit and scorecard now report bounded actionable canonical
  gap top lists by evidence class, file, repair kind, missing discriminator
  kind, static limitation reason, verify-command unknown class, and
  repair-route unknown class. The lists are derived from
  `evidence_record.canonical_item` so maintainers can choose the next
  fixture-backed repair class from live evidence without changing PR/CI
  rendering, gates, badges, providers, generated tests, or mutation execution.
- VS Code setup/status now treats workspace root selection as an explicit
  adoption state: single-root workspaces are named, multi-root workspaces fail
  closed until an active editor selects one folder, and first-pr/receipt
  wrong-root reports include the expected workspace root. This preserves the
  read-only editor contract: no hidden analysis rerun, source edit, generated
  test, provider call, mutation execution, default gate, or preview-language
  promotion.
- Lane 1 evidence audit generation now streams the live repo-exposure JSON
  subprocess directly into the temporary audit input file instead of buffering
  hundreds of megabytes in memory. Stderr and latency breadcrumbs remain
  captured for bounded phase/input diagnostics, and timeout detection now
  reports a timeout whenever xtask requested subprocess termination.
  `cargo xtask evidence-health` now uses the already-built debug `ripr` binary
  instead of nesting `cargo run`, preserving the same live-repo evidence path
  with fewer Windows process-wrapper failures. This is run-reliability
  hardening for Lane 1 reports, not a new evidence class, score, gate, PR/CI
  rendering, provider, generated-test, or mutation-execution behavior.
- Lane 1 activation evidence now treats direct owner calls as activation for
  value-insensitive seams, including no-argument calls and calls whose argument
  values remain opaque. This burns down measured
  `activation_value_unresolved` sub-shapes for return/call/error/effect style
  seams without inventing observed values or relaxing predicate-boundary value
  checks. The live Lane 1 audit moved from 26,277 to 19,106 total static
  limitations and from 25,908 to 18,859 `activation_value_unresolved`
  limitations, with actionable canonical gaps unchanged at 162. This does not
  change PR/CI rendering, gates, providers, generated tests, or mutation
  execution.

## 0.6.0 - 2026-05-18

0.6.0 makes static test-gap review easier to trust and easier to operate. The
release keeps RIPR static and advisory by default while adding better evidence
alignment, clearer editor and generated-CI guidance, preview
TypeScript/JavaScript/Python
visibility, policy operation packets, and repo-local cockpit tools for safe
agentic PR work. Rust remains the stable evidence path.
TypeScript/JavaScript/Python remain preview evidence paths: visible and useful,
but not default gates, RIPR Zero blocking debt, calibrated confidence, or
runtime proof.

Release themes:

- Evidence trust: raw findings now roll up into canonical evidence items,
  actionability and repair routes are explicit, static limitations are named,
  and raw findings remain supporting evidence instead of hidden truth.
- Operator trust: policy operations, history, promotion packets, preview
  promotion packets, generated-evidence discipline, command mutability,
  PR-ready, repo cockpit, merge-watch policy, PR triage dispositions,
  Rust-conversion candidate inventory, and deterministic suggested fixes make
  high-throughput PR work reviewable.
- Editor and CI trust: first-run status, related-test safety, release-copy
  guardrails, generated-CI policy packets, report indexing, and user-facing
  static-limit docs make advisory evidence easier to act on without source
  edits or generated tests.
- Preview-language honesty: TypeScript/JavaScript and Python preview adapters
  ship in the normal binary build, but their evidence remains opt-in,
  syntax-first, visibly preview/advisory, and non-gating until a later explicit
  policy promotion changes that.

Detailed changes:
- Changelog source range: the 0.6.0 notes were reconciled against the tagged
  `v0.6.0` candidate at `fd4d9cb` / #1218. Internal learning-doc polish
  remains outside the public release story, while #1210 is included because the
  final proof and publish-decision packets accepted it into the 0.6.0
  candidate. Post-tag generated badge refresh and release-note correction PRs
  are release-state housekeeping, not new 0.6.0 product claims.
- Removed-only diff hunks now still seed probes, so deleting or changing a
  behavior-bearing line without an added replacement does not disappear from
  static review. Related diff hardening covers quoted and metadata-bearing diff
  paths, indented lexical probe shapes, and clearer CLI typo recovery.
- Rust numeric literal extraction now handles a broader set of literal forms
  and requires valid exponent digits, improving stable Rust evidence facts
  without adding runtime execution or mutation-test claims.
- Rust decimal exponent literals now canonicalize equivalent `e` and `E`
  spellings before boundary comparison, so equivalent exponent forms line up in
  stable Rust predicate infection evidence without changing output schemas,
  policy, CI, or gate behavior.
- Lane 1 value resolution now treats same-test struct literal field
  projections as fixture-backed activation values when the field value is
  literal. Resolution is now scoped to the owner-call line, so later shadows do
  not erase earlier safe calls while before-call shadows, helper-built structs,
  fixture-parameter collisions, common non-`let` shadowing binders, non-simple
  `let` pattern binders, and non-literal fields remain named
  `activation_value_unresolved` limitations. This burns down one
  fixture-backed sub-shape of the top live static-limitation bucket without
  claiming strong grip, cross-file, helper, semantic, gate, PR/CI rendering,
  source-edit, generated-test, provider, or mutation-execution behavior.
- Canonical evidence items now expose explicit `primary_anchor` and
  `raw_spans[]` fields in supported `finding_alignment.items[]` and
  `evidence_record.canonical_item` records. Downstream PR/CI, editor, and agent
  surfaces get one preferred placement hint plus all contributing raw spans
  without inferring actionability from line-local raw findings.
- Generated CI and report packets now align their first screen on the canonical
  repair unit, keeping the start-here path centered on the actionable evidence
  item rather than on raw supporting signals.
- Report packet index availability now requires the primary Markdown artifact
  instead of treating a JSON sibling as sufficient, keeping the uploaded review
  artifact front door honest.
- TypeScript preview assertion extraction now sees common nested test-body
  statements and returned or awaited async expectation chains. This improves
  preview advisory evidence collection while staying syntax-only, non-gating,
  and separate from Rust stable evidence authority.
- Output-format command names and repo-scope metadata now come from one
  behavior-preserving metadata table, reducing duplicated CLI/report wiring
  without changing output contracts or release behavior.
- Shared Markdown and JSON value-path helpers now back policy promotion report
  rendering, reducing duplicated output code without changing output schemas,
  policy authority, or gate behavior.
- Shared JSON output helpers now back additional agent, review, evidence-health,
  outcome, and mutation-calibration report renderers, reducing duplicated
  serialization code without changing output schemas or release behavior.
- Human output rendering is split into focused evidence-line and section
  modules, with source-of-truth and PR-summary surface detection updated for the
  new module path. This is behavior-preserving output organization, not a
  report-contract or release-behavior change.
- Gate output rendering is split into focused model and presentation modules,
  reducing renderer size while preserving existing gate report behavior,
  policies, schemas, and default advisory boundaries.
- Agent review summary generation is split into focused artifact-loading,
  receipt-parsing, report-assembly, JSON, Markdown, type, and helper modules
  while preserving the public facade and existing agent review packet behavior.
- Mutation calibration import now handles additional nested mutation outcome
  record shapes, including nested mutation identifiers, locations, spans, and
  detail-only runtime records, with parser coverage. This improves advisory
  runtime-calibration ingestion without running mutation tests or changing
  policy, gates, CI, or release behavior.
- Output fixture tests now share common helper setup, reducing duplicated test
  scaffolding without changing report behavior or output contracts.
- `cargo xtask help` now gives clearer command-discovery output and preserves
  command lookup behavior, improving contributor and agent repo-ops flow
  without changing release, policy, or analyzer behavior.
- Classify text helper internals are split into focused modules while keeping
  the existing helper facade and classifier behavior unchanged.
- Output report path rendering now uses one slash-normalized helper across
  report modules, reducing duplicated renderer code while keeping existing
  report surfaces, schemas, analyzer behavior, policy authority, gate behavior,
  and release behavior unchanged.
- Public charter language now matches the 0.6.0 release boundary by describing
  RIPR as static mutation-exposure guidance between coverage signals and
  mutation testing, not as a test-adequacy layer.
- Lane 1 evidence audit generation now streams repo-exposure latency
  breadcrumbs during long live-repo runs and records bounded generation
  diagnostics in `inputs.repo_exposure_generation`, including timeout, status,
  duration, output byte counts, and the latency trace tail. Large best-effort
  classified-seam cache entries are skipped with an explicit `cache_store`
  trace instead of blocking report generation after analysis completes. This is
  operational audit reliability, not a new evidence-accuracy or gate claim.
- The 0.6.x finalization proof was refreshed with install, VSIX, generated-CI,
  public-copy, and external-adopter smoke evidence. It still does not tag,
  publish, create a GitHub Release, or refresh generated badge endpoints.
- Added the Lane 1 Finding Alignment Burn-Down rail and issue-backed
  implementation plan for post-0.6 cleanup. This is planning and documentation
  only; PR/CI rendering, LSP/editor behavior, gates, badges, generated tests,
  source edits, provider calls, and mutation execution remain unchanged.
- Closed the Lane 1 shippable finding-alignment pass for 0.6.0. Current
  repo-local audit evidence rolls 47,181 raw alignment signals into 38,027
  canonical items and 149 actionable canonical gaps, with zero actionable
  canonical items missing repair routes or verify commands and zero
  `static_unknown` items missing named limitations. This is evidence-truth
  closeout only: gates, badges, PR/CI rendering, LSP behavior, source edits,
  generated tests, provider calls, preview-language authority, and mutation
  execution remain unchanged.
- Added public `ripr first-pr` / `ripr start-here` command routing for the
  first successful PR start-here packet. The existing `cargo xtask first-pr`
  path now delegates to the same public implementation, preserving explicit
  artifact composition, advisory defaults, gate-authority separation, and the
  no-source-edit/no-generated-test/no-mutation-execution boundaries.
- Generated GitHub CI now renders the gap decision ledger and first-run
  `start-here` packet, then opens the advisory summary with that front door.
  Release readiness now verifies installed `ripr first-pr --help`, generated-CI
  start-here markers, and the VS Code `ripr: Start Current Repair` command
  contribution.
- `cargo xtask lane1-evidence-audit` now generates its temporary repo-exposure
  input through a bounded direct `ripr` invocation with latency tracing, so a
  cold full-repo evidence pass reports a clear timeout with phase context
  instead of waiting indefinitely or leaving an orphaned analyzer process.
- Focused release hardening added coverage for extraction helpers, oracle
  parsing, LSP URI edges, language routing, domain support primitives, and
  selector location matching, plus behavior-preserving app/report refactors
  that reduce duplicated wiring without changing public output contracts.
- Source-of-truth rails now make plan, goal, claim-boundary, validation, and
  rollback prompts explicit for future agent PRs without adding a competing
  release or policy authority surface.
- Lane 1 evidence-quality scorecards and trends now surface finding-alignment
  coverage gaps for unnamed static unknowns, actionable canonical items missing
  repair routes, and actionable canonical items missing verify commands.
- Actionable predicate-boundary evidence records now carry a structured
  canonical repair route for adding an equality-boundary assertion, so Lane 1
  can count them as concrete repair work instead of prose-only guidance.
- The editor first-pr bridge now projects first-pr packet state in status,
  validates bounded packet artifacts, ships first-pr bridge fixtures and smoke
  tests, records dogfood receipts, documents the workflow, and closes the Lane
  3 bridge with the generated-CI first-run card as the reviewer-facing front
  door. VS Code packet actions can open the packet, copy summary/repair
  guidance, copy verify and receipt commands, and show regeneration guidance
  while suppressing stale, wrong-root, unsafe-path, unsafe-command, or malformed
  packet payloads.
- Added `cargo xtask rust-conversion-candidates`, a report-only Rust-first
  policy aid that writes Markdown and JSON candidate reports for unretained
  non-Rust automation and workflow shell logic while documenting retained
  editor and fixture boundaries.
- Release readiness now records the 0.6.0 dry-run proof, repository metadata
  guidance, installed `ripr first-pr --help` verification, first-run install
  surface checks, refreshed public server-asset proof copy for the verified
  `v0.5.0` line, and the current pre-0.6.0 manual VS Marketplace install count
  without refreshing generated badge endpoints.
- Closed the First-Run UX and Adoption Hardening campaign. The closeout ties
  the first successful PR command path, `start-here` reports, recovery states,
  fixtures, receipts, PR repair cards, editor start-current-repair action,
  pasteable agent packets, advisory generated-CI summary, gate adoption
  checklist, README, and Quickstart into one adopter-facing Rust repair loop
  while preserving advisory defaults and avoiding analyzer, gate, preview
  language, source-edit, generated-test, provider, and mutation-execution
  changes.
- Public product-copy cleanup: VS Code marketplace title restored to
  `ripr: Static Mutation Exposure`, plain-language first-hour copy in
  `README.md`, `editors/vscode/README.md`, `docs/QUICKSTART.md`, and
  `docs/EDITOR_EXTENSION.md`. Internal vocabulary (seams, discriminators,
  status IDs, schemas, commands) is unchanged.
- Added [docs/TERMINOLOGY.md](docs/TERMINOLOGY.md): plain-language -> internal
  vocabulary bridge linked from `README.md`, `docs/QUICKSTART.md`,
  `docs/EDITOR_EXTENSION.md`, and `editors/vscode/README.md`. No schema, JSON,
  status ID, or command renames.
- Generated CI advisory job summary now uses reviewer-friendly section
  headings: `PR review front panel` -> `PR review summary`, `First useful
  action` -> `Recommended next test`, `Report packet index` -> `Uploaded review
  artifacts`, `Assistant loop health` -> `Agent proof status`. The matching
  `... at a glance` subsection headings move with each section, and fallback
  "X was not generated" messages stay aligned. Artifact filenames, JSON
  fields, command names, status IDs, workflow step `name:` values, and
  schemas are unchanged.
- Generated CI now surfaces the Lane 2 policy operations stack as advisory
  packets: `policy-operations`, `policy-history`, `policy-promotion-*`, and
  configured preview-language `preview-promotion-*` artifacts are rendered,
  uploaded, indexed, and summarized without changing pass/fail authority,
  default blocking, comments, config, baselines, suppressions, history ledgers,
  workflows, branch protection, or preview eligibility.
- Closed the Lane 2 Policy Operations and Promotion Readiness tracker. The
  closeout records policy operations, policy history, promotion packets,
  preview-promotion packets, operator workflow docs, advisory generated-CI
  projection, capability metadata, traceability, and the boundary that stricter
  policy or preview-language promotion still requires explicit later review.
- Closed Campaign 27 Language Adapter Preview. TypeScript/JavaScript and Python
  preview adapters now have fixture-backed syntax-first facts, visible
  preview/advisory labels, editor projection, generated-CI language grouping,
  dogfood receipts, capability metadata, traceability, and a closeout boundary
  that keeps Rust defaults, gate authority, generated tests, provider calls,
  source edits, and mutation execution unchanged.
- Campaign manifests now accept top-level `status = "closed"` only when every
  work item is `done`, letting `.ripr/goals/active.toml` honestly record a
  closed campaign until the next campaign is selected. Campaign 26 and Campaign
  27 archived manifests were normalized to that closed state.
- Closed the generated-evidence discipline lane. Ordinary PRs now have
  generated-clean and badge endpoint ownership checks, worktree and PR-status
  operator reports, spec-numbering and campaign/source-of-truth guards,
  target-local receipts, critic reports, deterministic suggested-fixes patches,
  and contributor docs that separate authored truth from generated evidence and
  judgment-required decisions.
- Added `cargo xtask commands`, a target-local command mutability catalog that
  classifies xtask commands as mutating, non-mutating checks, report-only,
  external-state reads, external-state mutations, or argument-dependent, and
  flags commands that require explicit judgment before use.
- `cargo xtask pr-triage-report` now writes agent-readable JSON next to the
  Markdown queue report so open-board risks can be consumed without scraping
  prose.
- `cargo xtask gh-pr-status --pr <number>` now writes agent-readable JSON next
  to the Markdown merge-readiness packet so agents can consume merge state,
  outstanding checks, Droid status, reviews, and the safe next action without
  scraping prose.
- Lane 1 finding-alignment coverage now treats generic `static_unknown` and
  `unknown` limitation categories as unnamed, so static-unknown canonical items
  must carry a specific analyzer limitation category and repair route.
- Lane 1 finding-alignment coverage now requires actionable canonical items to
  carry a structured top-level `repair_route`, not just prose repair text, and
  its audit tests directly cover aligned `presentation_text` and
  `config_or_policy_constant` rows. The dogfood corpus also includes the opaque
  config lookup named-limitation receipt.
- `cargo xtask reports index` now includes repo-ops packet status in Markdown
  and JSON for command mutability, the repo cockpit, PR-ready, worktree doctor,
  PR triage, per-PR merge readiness, generated-clean, badge ownership, critic,
  receipts, suggested fixes, and `check-pr` artifacts.
- Added `cargo xtask pr-ready`, a target-local advisory cockpit that composes
  worktree doctor, command mutability, PR summary, critic, receipts check,
  suggested fixes, generated-clean, and badge ownership into
  `target/ripr/reports/pr-ready.md` and `.json`.
- Added `cargo xtask cockpit`, a repo-level advisory front panel that composes
  worktree doctor, command mutability, command-catalog coverage, spec
  numbering, campaign/source-of-truth checks, open PR triage, generated-clean,
  and badge ownership into `target/ripr/reports/cockpit.md` and `.json`.
- Added [docs/MERGE_WATCH_POLICY.md](docs/MERGE_WATCH_POLICY.md), documenting
  PR watcher cadence, branch freshness decisions, REST status fallback,
  Droid/advisory-check handling, merge execution limits, and task-worktree
  cleanup without changing branch protection or auto-merge behavior.
- `cargo xtask suggested-fixes` now suggests deterministic docs index table
  ordering for specs and ADRs in addition to allowlist ordering, while keeping
  badge values, goldens, baselines, suppressions, dependency exceptions, and
  schema changes out of generated repair patches.
- `cargo xtask suggested-fixes` now suggests deterministic
  `.ripr/traceability.toml` `[[behavior]]` block ordering by spec ID without
  re-rendering TOML or changing block bodies.
- `cargo xtask suggested-fixes` now suggests deterministic
  `metrics/capabilities.toml` `[[capability]]` block ordering by spec ID and
  capability ID without re-rendering TOML or changing block bodies.
- `cargo xtask suggested-fixes` now suggests deterministic command mutability
  catalog ordering by xtask help order, and `check-command-catalog` reports
  help/catalog order drift before agents rely on stale command sequencing.
- `cargo xtask pr-triage-report` now emits advisory queue dispositions so
  agents can distinguish merge candidates, stale/duplicate owner decisions,
  rebase needs, validation gaps, and wrong-lane PRs without mutating GitHub.
- Added `cargo xtask check-command-catalog`, a non-mutating guard that fails
  when xtask help entries and the command mutability catalog drift apart or
  omit write/judgment metadata.
- Added `RIPR-SPEC-0048` for Lane 1 config/policy constant evidence, defining
  how internal policy metadata, rendered config/report labels, behavior
  selectors, named limitations, repair routes, and must-not-claim guards should
  fit the raw-finding to canonical-item alignment model before analyzer work.
- Added config/policy constant cases to the Lane 1 evidence-quality benchmark,
  pinning internal no-action metadata, rendered output-observer repairs,
  observed schema labels, cross-file flow unknowns, and opaque lookup
  limitations before analyzer work.
- Config/policy constants now align into `ripr check --json` canonical
  evidence items for fixture-backed internal metadata, visible unobserved
  report/config labels, observed schema labels, cross-file flow unknowns, and
  opaque lookup limitations. Raw findings remain supporting evidence; no
  PR/CI rendering, gate, score, generated-test, provider, source-edit, or
  mutation-execution behavior changed.
- Added explicit config/policy behavior-selector proof for canonical
  `add_behavior_discriminator` repairs and already-observed
  `validation_behavior` discriminators, including benchmark cases, dogfood
  receipts, and unit assertions that declaration plus literal findings become
  one canonical item without recommending mutation execution first.
- Actionable finding-alignment items now expose a normalized top-level
  `repair_route` with `repair_kind`, `target_test_type`, and
  `suggested_assertion`, plus route-coverage counts, so downstream consumers
  can use one canonical repair contract instead of inferring actionability from
  raw static classes or class-specific fields.
- Finding-alignment summaries now include actionable verify-command coverage
  and missing-verify counts, keeping repair routes and verification routes
  explicit for canonical gaps without changing PR/CI rendering, gate policy,
  scores, generated tests, provider calls, source edits, or mutation execution.
- Evidence-quality scorecards now lead with actionable canonical gaps while
  keeping raw signals and canonical item counts as diagnostic context. This
  keeps the Lane 1 counting model visible without changing badges, gates,
  scores, PR/CI rendering, generated tests, provider calls, source edits, or
  mutation execution.
- `cargo xtask dogfood` now checks finding-alignment receipts for real RIPR PR
  examples, pinning actionable, already-observed, internal no-action, and named
  static-limitation outcomes without changing PR/CI rendering, gates, public
  scores, generated tests, provider calls, source edits, or mutation execution.
- Documented the canonical finding-alignment consumer contract v2 so downstream
  PR/CI, editor, report, and agent lanes render canonical evidence items first,
  keep raw findings as supporting evidence, and avoid inferring actionability
  from raw static classes.
- VS Code `ripr: Show Status` now includes first-run/no-output context:
  workspace root, resolved server source and command, editor selectors,
  enabled languages from the last server refresh, and the next safe action for
  disabled, no-workspace, unavailable-server, stale, language-off, no-seam,
  preview, and diagnostic states. LSP refresh logs now include enabled language
  names alongside the existing count.
- VS Code related-test opening now fails closed unless the target is a file in
  the current workspace with a current Rust or preview-language route, so stale,
  disabled, malformed, unsupported, or out-of-workspace command payloads cannot
  open arbitrary files.
- Added `docs/STATIC_LIMITS.md` so preview-language static-limit labels have a
  user-facing interpretation guide and downstream tools know not to parse
  prose into action semantics.
- Added Cargo feature gates for preview language adapters. Default builds still
  include TypeScript/JavaScript and Python preview support, Rust-only binaries
  can be built with `--no-default-features --features lang-rust`, and repo
  config now fails closed when it enables a language missing from the current
  binary.
- Presentation-text finding alignment now classifies fixture-backed help/report
  output text, supported golden/snapshot observers, internal-only labels, and
  visibility-unknown routes in `ripr check --json` canonical items while
  preserving raw findings and avoiding PR/CI rendering, gates, scores,
  generated tests, provider calls, or mutation execution.
- Presentation-text canonical items now include concrete repair guidance:
  `repair_kind`, `target_test_type`, and `suggested_assertion` fields distinguish
  output-observer repairs, already-observed no-action states, internal no-action
  labels, and visibility-inspection limitations without recommending mutation
  execution as the first action.
- Evidence-quality scorecard and trend reports now carry finding-alignment and
  presentation-text counts, including raw signals, canonical items,
  raw-to-canonical ratio, duplicate groups, actionable items, no-action items,
  static limitations, calibrated-support context, visibility unknowns, observed
  text, and output-observer repair counts. This keeps the Lane 1 quality loop
  repo-local and advisory without changing PR/CI rendering, gates, scores,
  generated tests, provider calls, or mutation execution.
- Lane 1 evidence audit now derives a `finding_alignment.summary` from
  `evidence_record.canonical_item`, so the scorecard can report
  raw-to-canonical, actionable, observed, static-limitation, and calibration
  counts even when there is no separate top-level finding-alignment projection.
  The reports remain repo-local and advisory.
- Lane 1 evidence audit now includes finding-alignment coverage by evidence
  class: aligned versus unaligned raw findings, top unaligned examples,
  same-line duplicate raw signals, static-unknown items missing named
  limitations, and canonical items missing repair or verification guidance.
  This remains repo-local and does not change PR/CI rendering, gates, scores,
  generated tests, provider calls, source edits, or mutation execution.
- Clarified the finding-to-gap alignment contract with explicit
  `primary_anchor` and `raw_spans[]` semantics plus class-scoped action,
  observed, no-action, limitation, and must-not-infer rules. This is a
  docs-only contract refinement for later projection/evidence-field work.
- Added the Lane 1 presentation-text consumer handoff. It tells downstream
  PR/CI, editor, agent, and report lanes to render canonical evidence items
  before raw findings, keep raw findings as supporting evidence, preserve the
  Lane 1 evidence-state versus policy-overlay boundary, and avoid inferring
  actionability, user test debt, or mutation-first repairs from raw
  `exposed`/`static_unknown` labels.
- Closed the Lane 1 User-Visible Output Evidence tracker in documented
  presentation-text scope. The closeout records the spec, benchmark,
  evidence-record, grouping, visibility, actionability, scorecard/trend, and
  consumer-handoff proof, adds the final observer-unknown benchmark guard, and
  keeps PR/CI rendering, LSP/editor behavior, gates, scores, generated tests,
  provider calls, source edits, and mutation execution out of scope.
- Python preview related-test matching now treats free-function calls more
  conservatively: module import aliases such as `pricing.apply_discount(...)`
  still relate to the owner, but unrelated object method calls no longer make a
  top-level function look related.
- Python preview static limits now treat common pytest `monkeypatch` runtime
  substitution calls as `mocked_module`, keeping related-test evidence advisory
  when the test changes module or attribute behavior at runtime.
- TypeScript preview related-test matching now uses the same conservative
  token-aware owner-call boundary, so string/comment mentions and arbitrary
  object method calls no longer make a top-level function look related or
  trigger mocked-module static limits.
- `ripr --help` and every `ripr <subcommand> --help` now lead with an
  action-oriented one-liner before the `Usage:` block (e.g.,
  `ripr pilot --help` opens with "Find the top test gap in this repo and
  write a packet you can act on."). The canonical `Usage: ripr <cmd>` syntax
  and all options remain in place. Command names, subcommand names, JSON
  fields, artifact filenames, schemas, and CLI behavior are unchanged.
- VS Code command palette title for `ripr.copyContext` renamed from
  `ripr: Copy Finding Context` to `ripr: Inspect Test Gap - Copy Context` so
  it groups alongside the existing workflow-step categories ("Write Targeted
  Test - ...", "Agent Handoff - ...", "Verify After Test - ...", "Review Result - ...").
  Other command palette titles are already action-oriented and stay
  unchanged. Command IDs, settings IDs, LSP requests, JSON fields, schemas,
  status IDs, report names, artifact paths, and behavior are unchanged.
- Added [docs/RELEASE_COPY_CHECKLIST.md](docs/RELEASE_COPY_CHECKLIST.md):
  the reusable public-surface checklist captured from the v0.5.0 release.
  Covers GitHub Release body vs. process narrative, marketplace metadata,
  install truth, README badge freshness disclosure, public vocabulary,
  VSIX rebuild before publish, and dependent-channel asset verification.
  Linked from [docs/RELEASE.md](docs/RELEASE.md),
  [docs/RELEASE_MARKETPLACE.md](docs/RELEASE_MARKETPLACE.md), and the root
  README docs table. No publish workflow, schema, JSON, or behavior change.
- Added `cargo xtask check-product-copy`: a lightweight guard that scans
  the public surfaces named in the release copy checklist and flags
  unbridged use of internal vocabulary (`test oracle`, `discriminator`,
  `seam-native`, `grip`, `evidence spine`, `canonical gap`,
  `no-actionable-seam`, `front panel`, `report packet`). A file is
  bridged if it links to `docs/TERMINOLOGY.md`. Specs, output schema,
  fixtures, metrics, implementation campaigns, and the CHANGELOG are
  allowlisted internal surfaces and are not scanned. The current
  baseline is clean; the `product_copy_baseline_is_clean` unit test
  catches regressions. The guard is not wired into `cargo xtask
  check-pr` yet — promote it to a gate after a release cycle confirms
  it stays low-noise. `crates/ripr/README.md` gains the bridge link so
  the published crates.io README is also covered.
- Opened the Lane 1 Evidence Accuracy Evaluation tracker after the v0.1
  evidence spine stabilized. The tracker names PR #697 as the final consumer
  closeout, keeps `.ripr/goals/active.toml` unchanged, and routes next work
  through a repo-local evidence-quality audit before analyzer or calibration
  changes.
- Closed the Lane 2 Policy Readiness and Preview Evidence Governance tracker.
  The closeout leaves Campaign 27 active while documenting the policy-readiness
  report, preview evidence boundary, waiver-aging report, suppression-health
  report, shrink-only baseline refresh guardrails, exception-ledger alignment,
  blocking readiness guide, and advisory generated CI projection. Preview
  TypeScript/Python evidence remains visible and advisory by default, with no
  gate eligibility, RIPR Zero blocking debt, calibrated-confidence authority,
  automatic baseline adoption, generated tests, mutation execution, or default
  CI blocking without later explicit promotion policy.
- Added additive `static_limit_kind` finding metadata for known preview static
  limits. The TypeScript mocked-module limit now emits
  `static_limit_kind = "mocked_module"` in JSON while keeping existing human
  evidence text advisory and leaving Rust/default behavior unchanged.
- Added `cargo xtask lane1-evidence-audit` with
  `cargo xtask evidence-quality-audit` as an alias. The repo-local report writes
  `target/ripr/reports/lane1-evidence-audit.{json,md}` from generated
  repo-exposure `evidence_record` data and summarizes headline gaps, canonical
  groups, duplicate-looking groups, missing discriminators, static limitations,
  oracle semantics, related-test ranking, movement availability, calibration
  availability, field health, and top files by unresolved evidence debt without
  changing analyzer behavior or gate policy.
- Added the Lane 1 evidence-quality failure fixture corpus, pinning the first
  audit-derived `evidence_record` subsets for duplicate canonical groups,
  equality-boundary misses, activation static limitations, mock-expectation
  observer semantics, and no-runtime-data calibration gaps before analyzer
  tuning begins.
- Reduced the first audit-pinned Lane 1 canonical overcount by emitting
  parser-backed match-arm discriminators such as `"kind" =>` instead of generic
  `=>` / `match` text. The repo-local audit now splits the suppressions
  match-arm case to group size `1` and reduces duplicate-looking groups from
  `1287` to `926` without changing gates, schemas, or public command surfaces.
- Folded durable Lane 1 audit fields into `ripr evidence-health`: canonical
  gap group totals, largest groups, duplicate-looking groups, actionability
  classes, static limitation distributions, evidence-record calibration
  coverage, movement availability, and top evidence-quality risks. The report
  remains advisory and does not change analyzer classifications, gate policy,
  CI behavior, mutation execution, or score definitions.
- Added checked `runtime-fixtures-v2` calibration reports for the Lane 1
  side-effect observer, mock expectation, snapshot oracle, and opaque dispatch
  runtime classes. The fixture maps imported outcomes to existing static seams
  where possible, keeps an ambiguous opaque dispatch file-line signal
  ambiguous, and keeps a runtime-only signal from creating a static gap. No
  CI mutation execution, gate behavior, schema, or score definition changes.
- Closed the Lane 1 Evidence Accuracy Evaluation campaign in documented scope.
  The closeout records the audit, fixture corpus, first analyzer improvement,
  evidence-health dashboard fields, runtime-fixtures-v2 calibration expansion,
  and future evidence-class boundary without changing `.ripr/goals/active.toml`.
- Added the first Lane 1 Evidence Quality Leadership repair: static
  limitations now carry normalized analyzer categories and suggested repair
  routes through `evidence_record`, evidence-health, the Lane 1 audit, and the
  evidence-quality scorecard. This keeps unknowns repairable for maintainers
  without changing grip classes, gates, CI behavior, mutation execution, or
  score definitions.
- Tightened the first Lane 1 oracle-semantics audit fix: clear custom
  assertion helpers such as `assert_total_matches(actual, expected)` remain
  strong exact-value evidence, while opaque custom helpers and duplicative
  equality assertions no longer overclaim exact-value grip. Benchmark guards
  now pin those must-not-claim cases without changing gates, CI behavior,
  mutation execution, generated tests, provider calls, or score definitions.
- Added checked `runtime-fixtures-v3` calibration reports for Lane 1
  static/runtime confidence expansion classes: custom assertion helper
  outcomes, table-driven boundaries, builder overrides, cross-file constants,
  snapshot field discriminators, and mock expectation mismatches. The fixture
  pins matched joins, ambiguous joins, runtime-only signal, and no-runtime-data
  guards without changing analyzer behavior, gates, CI behavior, mutation
  execution, generated tests, provider calls, or score definitions.
- Added `cargo xtask evidence-quality-trend`, a repo-local Lane 1 trend report
  over the current evidence-quality scorecard and optional previous scorecard
  or audit snapshot. It writes `evidence-quality-trend.{json,md}`, distinguishes
  improvement, regression, unchanged, mixed, and unknown trend states, and
  reports missing history explicitly without redefining RIPR scores or changing
  analyzer behavior, gates, CI behavior, mutation execution, generated tests,
  provider calls, or editor surfaces.
- Closed the Lane 1 Evidence Quality Leadership tracker in documented scope.
  The closeout records the scorecard, benchmark corpus, static limitation
  taxonomy, oracle-semantics audit fix, runtime-fixtures-v3, evidence-quality
  trend reporting, class-scoped capability metadata, traceability links, and
  future evidence-class boundary without changing `.ripr/goals/active.toml`.
- Opened the Lane 1 User-Visible Output Evidence tracker and proposal for
  presentation/help/report/table text evidence. The new lane keeps PR/CI
  rendering, LSP/editor polish, gates, generated tests, provider calls,
  mutation execution, and score definitions out of scope while defining the
  path toward visibility, observer, actionability, canonical grouping, and
  static-limitation evidence for changed presentation text.
- Added RIPR-SPEC-0043 for presentation text evidence. The spec defines planned
  Lane 1 behavior for visibility, observer shape, actionability, declaration
  plus literal grouping, static limitation categories, and must-not-claim guards
  before analyzer behavior changes begin.
- Added RIPR-SPEC-0045 for finding-to-gap alignment. The spec defines how raw
  line-local findings remain supporting evidence while rolling up into
  canonical evidence items with explicit state, actionability, reason, repair,
  verification, static limitations, confidence, counting rules, and downstream
  consumption boundaries.
- Expanded the Lane 1 evidence-quality benchmark with finding-alignment cases
  for presentation text: actionable visible-unobserved output, already-observed
  output, internal no-action labels, declaration/literal line movement, and
  different constants that must not collapse into one item.
- Added the first implemented finding-alignment fields to `evidence_record`.
  Repo exposure now carries supporting `raw_findings[]`, a canonical item with
  gap state, class-scoped actionability, repair, related test, verification, and
  confidence context, plus a nullable presentation-text projection reserved for
  the class-specific Lane 1 slices. This is additive and does not change
  rendering, gates, generated tests, provider calls, mutation execution, or
  score definitions.
- Added the first check-output finding-alignment projection for presentation
  text constants. `ripr check --json` now groups supported presentation-like
  `&str` constant declarations and adjacent string-literal raw findings into
  one visibility-unknown canonical limitation item, preserving the raw
  `findings[]` array as supporting evidence and avoiding mutation-first repair
  language for this class. The section is omitted when no supported alignment
  item exists, and PR/CI rendering, LSP/editor polish, gates, generated tests,
  provider calls, mutation execution, and score definitions are unchanged.
- Added a Lane 1 evidence-quality benchmark case for presentation text
  constants, pinning the claim boundary for changed help/label text: visibility
  and actionability must be explicit, declaration and literal lines should
  become one canonical evidence item, text alone must not become user test debt,
  and mutation testing must not be the first recommended action. The benchmark
  validator now also requires static limitation categories at the case level.
- Extended the Python preview fixture matrix with edge goldens for async owners,
  classmethod owners, no-projectable-owner changes, disabled Python config, and
  mixed-language no-cross-route related-test safety. This adds projection
  readiness evidence without adding editor selectors, LSP routing, source edits,
  generated tests, provider calls, mutation execution, policy gates, or default
  CI behavior.

See `docs/ci/rust-1.95-quality-rollout.md` for the full PR ladder and acceptance gates.

## 0.5.0 - 2026-05-10

`ripr` 0.5.0 is the review-surface release. It moves RIPR from a collection
of static evidence reports into a coordinated advisory workflow for
developers, reviewers, CI, editors, and coding agents. The core boundary is
unchanged: RIPR does not run mutation testing, call LLM providers, generate
tests, edit source, or make default CI blocking decisions.

### Highlights

- Lane 1 evidence spine is stable in scope: a seam-native `evidence_record`
  projection with canonical gap identity threads through agent packets, repo
  exposure, gate evaluation, baseline diff, RIPR Zero status, and assistant
  proof so headline gap classes group by behavior instead of by raw line.
- Campaigns 17 through 26 turn the read-only static-evidence loop into one
  reviewer-first surface: RIPR Zero adoption, PR evidence ledger, RIPR Zero
  reporting, test-oracle assistant proof and report producer, first useful
  action, assistant-loop health, PR review front panel, report packet index,
  and the optional PR inline comment publisher.
- Editor Evidence UX matches the agent loop: saved-workspace seam
  diagnostics, hovers, and intent-titled code actions; first-useful-action
  hover and status projection; LSP `ripr.collectEvidenceContext`; and a
  status-bar / `ripr: Show Status` surface that keeps stale buffers visible.

### Evidence spine and identity

- Added the shared `RIPR-SPEC-0021` `evidence_record` projection for
  seam-native evidence, giving Lane 1 an identity, evidence path, observed
  values, missing discriminators, related tests, recommendation, calibration
  placeholder, and static limits while preserving existing repo-exposure
  fields.
- Added generated canonical gap identity so headline-eligible raw seam gaps
  group by owner, seam kind, flow sink, missing discriminator, and assertion
  shape; line numbers remain locators but no longer act as durable identity.
- Routed baseline ledgers, PR evidence ledgers, RIPR Zero status repair
  routes, agent seam packets, targeted-test outcome, agent verify movement,
  and test-oracle assistant proof through the shared evidence spine.
- Routed calibrated gate baseline comparison through canonical evidence
  identity so reviewed baseline debt matches across line movement before
  falling back to legacy seam, source, and path/line/static-class identities.
- Promoted the Lane 1 `evidence_record` capability to stable within its
  documented v0.1 scope and added a dedicated Lane 1 evidence-spine tracker.
- Stabilized related-test ranking v2 (relation confidence, reason, oracle
  strength, activation overlap, file/name/line tie-breakers), oracle
  semantics v3 (structured `oracle_semantics` explanations on related
  tests), syntax-first side-effect propagation (event, state-write,
  persistence, log, config-change, call-effect, generic-call sinks), and
  fixture-backed activation/value modeling.
- Added advisory static/runtime confidence labels to mutation calibration
  rows so runtime data can support, contradict, remain ambiguous, or stay
  unavailable for a static claim without changing static classifications.
- Added the evidence-record contract corpus pinning representative v0.1
  shapes for predicate, error, exact-value, broad-error, field,
  whole-object, snapshot, side-effect, opaque static-limitation,
  canonical-gap, and calibration-placeholder cases.

### RIPR Zero adoption

- Added `ripr baseline create --from <gate-decision.json>` writing reviewed
  gate baselines from existing gate-decision evidence with `--dry-run`,
  `--force`, and skip-on-malformed semantics.
- Added `ripr baseline diff` writing advisory baseline-debt-delta JSON and
  Markdown over still-present, resolved, new policy-eligible, acknowledged,
  suppressed, stale, invalid, and missing-input identities without making
  gate decisions or rewriting baselines.
- Added `ripr baseline update --remove-resolved`, shrink-only refresh that
  preserves malformed or ambiguous records and refuses to auto-adopt new
  current debt.
- Added baseline metadata support: owner, reason, created, review-after,
  source fields preserved across baseline create / diff / shrink-only update
  without breaking Campaign 17 baseline files.
- Added `ripr zero status`, a read-only advisory report joining baseline
  debt deltas, reviewed baseline metadata, optional gate decisions, PR
  guidance, and recommendation calibration into repo-level RIPR Zero
  progress.
- Added generated CI baseline-debt-delta artifacts and RIPR Zero summary
  wiring guarded on `RIPR_GATE_BASELINE` without changing advisory defaults.
- Added `docs/BASELINE_LEDGER_WORKFLOW.md` and
  `docs/RIPR_ZERO_REPORTING_WORKFLOW.md`, framing RIPR 0 as configured-scope
  burn-down and documenting waiver / baseline / suppression boundaries.

### Agent and reviewer workflow

- Added `ripr agent start`, `ripr agent status`, `ripr agent verify`, and
  `ripr agent receipt` LLM work-loop commands: a source-edit-free workflow
  packet, read-only loop status, before/after comparison, and a
  provenance-backed receipt with bounded next-action guidance.
- Added `ripr assistant-loop proof` and `ripr assistant-loop health`, the
  test-oracle assistant proof report and the multi-proof health summary
  with proof completeness, missing inputs, static movement, recurring
  warnings, and bounded repair queues.
- Added `ripr first-action`, a read-only advisory report producer that
  writes `first-useful-action.{json,md}` from explicit PR guidance,
  assistant proof, PR evidence ledger, baseline delta, receipt, optional
  gate, optional coverage/grip frontier, and editor context inputs.
- Added `ripr pr-ledger record`, the per-PR evidence ledger joining PR
  guidance, gate decisions, baseline debt deltas, RIPR Zero status,
  recommendation calibration, agent receipts, optional coverage, and
  optional history.
- Added `ripr review-comments`, the bounded PR test guidance JSON and
  Markdown producer; `ripr coverage-grip frontier`, an advisory report that
  keeps coverage movement and behavioral grip movement visible as separate
  axes; `ripr pr-review front-panel`, the composed reviewer front panel
  over existing front-panel inputs; and `ripr reports index`, the reviewer
  packet index over explicit artifact directories.
- Added `ripr gate evaluate`, a read-only optional policy evaluator writing
  `gate-decision.{json,md}` from existing PR guidance, labels, baselines,
  and calibration without posting comments, editing source, or running
  mutation tests.
- Added `ripr pr-comments plan`, a read-only advisory publish plan from
  explicit PR guidance and optional existing comment metadata, plus
  generated CI wiring for `RIPR_COMMENT_MODE = off|plan|inline` that posts
  or updates only safe same-repository changed-line operations from the
  plan, capped, deduped, and default off.
- Added `cargo xtask recommendation-calibration`, the advisory
  PR-recommendation usefulness report (placement, suppression correctness,
  target-file correctness, before/after static movement).
- Generated GitHub CI now surfaces PR guidance, gate decisions, baseline
  debt deltas, RIPR Zero status, assistant proof, assistant-loop health,
  first useful action, PR review front panel, and the report packet index
  only when their explicit inputs already exist; defaults remain advisory.

### Editor evidence UX

- Hardened saved-workspace seam diagnostics, evidence hovers, intent-titled
  code actions (inspect seam, write targeted test, copy agent handoff,
  verify after test, review receipt, refresh analysis), and
  `ripr.collectEvidenceContext` handoff packet.
- Added the VS Code status bar item and `ripr: Show Status` command
  covering server, workspace, analysis, stale, failed, no-actionable-seam,
  and first-useful-action states; stale Rust buffers keep stale status
  visible until save or close.
- Added first-useful-action projection in VS Code status and in the LSP
  seam hover from existing workspace-matched reports without adding
  diagnostics, editing source, generating tests, or making gate decisions.
- Hardened LSP command payload contracts and first-action status edges so
  saved-workspace command smoke and saved-workspace status output stay
  pinned across analysis-queued, analysis-running, stale-buffer,
  missing-input, and no-actionable-seam transitions.
- Added the `fixtures/editor_lsp_workflow` canonical Lane 3 fixture and
  extended VS Code e2e + framed LSP smoke coverage.
- Pinned preview editor projection artifacts for TypeScript and Python
  preview diagnostics, bounded finding actions, hover/static-limit/status
  evidence, and disabled-preview no-diagnostic behavior without analyzer,
  schema, selector, or policy changes.
- Added `docs/EDITOR_EVIDENCE_WORKFLOW.md`, the saved-workspace editor
  guide from install and status through diagnostic, hover, related test,
  context packet, focused test, after snapshot, verify, receipt, and
  refresh with explicit static-evidence limits.

### CI, policy, and release hygiene

- Raised MSRV to Rust 1.95: workspace `rust-version`, pinned toolchain
  (`rust-toolchain.toml` -> `1.95.0`), CI MSRV job toolchain and cache keys,
  release-readiness preconditions, and doc/README references are aligned
  with the 0.5.0 / Rust 1.95 release line.
- Promoted clean Rust 1.94 / 1.95 Clippy ratchets into the active workspace
  lint table (`same_length_and_capacity`, `manual_ilog2`,
  `needless_type_cast`, `decimal_bitwise_operands`, `manual_checked_ops`,
  `manual_take`, `duration_suboptimal_units`, `unnecessary_trailing_comma`,
  plus `unsafe_op_in_unsafe_fn`, `undocumented_unsafe_blocks`,
  `multiple_unsafe_ops_per_block`, `repr_packed_without_abi`,
  `match_result_ok`); unsupported or config-dependent lints remain
  explicitly deferred with blockers in `policy/clippy-lints.toml`.
- Strengthened `cargo xtask check-no-panic-family` drift reporting (allowed
  / advisory-drift / stale / unallowed / warning sections) and added
  `--propose`, a review-only allowlist migration helper.
- Made `policy/no-panic-allowlist.toml` the canonical schema 0.3 no-panic
  allowlist with governed ids, owners, and expiry dates.
- Documented the CI verification economics policy (required, advisory,
  on-demand / release postures; LEM budget bands; label effects; artifact
  families; cheaper-signal-first rules; CI actuals; and rollback
  expectations) and added non-enforcing CI policy ledgers for LEM bands,
  target lane IDs, risk packs, artifact families, labels, and rollout
  exceptions.
- Prepared the 0.5.0 release surface: crate, VSIX, generated CI workflow
  artifacts, server archives and manifest, release-readiness flow, and the
  related-release docs.

### Release recovery (v0.5.0)

The initial `v0.5.0` tag push exposed a Windows-only bug in the new Rust
xtask release-server-archive path (PR #557 had moved the legacy
PowerShell-driven packaging into xtask, but the Windows zip branch relied
on `pwsh -Command` binding trailing positional args to `$args`, which
PowerShell only does for `-File`). The Linux and macOS targets succeeded;
the Windows target failed with a null `-Path` in `Compress-Archive`, and
the `manifest` job correctly skipped.

Recovery was fix-forward (#718): the zip branch was rewritten to use the
Rust `zip` crate (deflate-only, `default-features = false`), the Zlib
license used by the transitive `zlib-rs` dependency was added to
`deny.toml`, a `create_zip_archive_writes_flat_package_contents` test
exercises the new path on every platform including Windows, and
`release-server-binaries.yml` was rerun via `workflow_dispatch` from
`main` with `version=0.5.0`. The `v0.5.0` tag was kept at the release-prep
commit; the server archives, per-target `.sha256` files, `checksums.txt`,
and `ripr-server-manifest-v0.5.0.json` were attached to the existing
GitHub Release. The marketplace VSIX publish and crates.io publish run
separately once asset verification completes.

### Compatibility

- Raised the declared workspace MSRV and pinned repository toolchain from
  Rust 1.93 to Rust 1.95. CI MSRV jobs, release-readiness preconditions,
  README/AGENTS/CLAUDE MSRV references, and the active Clippy ratchet table
  are aligned with the 0.5.0 / Rust 1.95 release line; deferred Clippy
  promotions remain tracked in `policy/clippy-lints.toml`.

### Boundaries (unchanged)

- No LLM provider integration, no generated tests, no automatic edits, no
  runtime mutation execution, and no default CI blocking. RIPR remains a
  static, advisory evidence layer; calibrated gate, inline-comment publisher,
  and runtime calibration remain explicit opt-ins.

### Added

- Extended `cargo xtask dogfood` with checked report-packet index receipts for
  complete, sparse advisory, missing-front-panel, blocked-gate, missing-proof,
  missing-receipts, and coverage/grip-present packet cases, plus a handoff
  receipt documenting the validation boundary.
- Closed Campaign 25, Report Packet Index, with a prompt-to-artifact audit,
  validation plan, advisory boundary, and future-lane boundary in
  `docs/handoffs/2026-05-10-campaign-25-closeout.md`.
- Opened Campaign 26, PR Inline Comment Publisher, with
  `spec/pr-inline-comment-publisher-contract` as the first ready work item so
  optional durable PR comments can be planned, capped, deduped, and kept
  explicit opt-in before any GitHub posting behavior changes.
- Added `RIPR-SPEC-0025` for the PR inline comment publisher, pinning the
  read-only publish-plan schema, comment modes, permission boundary,
  summary-only exclusion, cap and dedupe behavior, and generated-CI default-off
  posture before producer or workflow changes.
- Added the PR inline comment publisher fixture corpus for publishable
  changed-line comments, summary-only exclusion, cap overflow, dedupe/upsert,
  stale-existing cleanup, fork or no-token blockers, and missing review-comments
  input before the publish-plan producer changes.
- Added read-only `ripr pr-comments plan` support, emitting advisory
  `comment-publish-plan.{json,md}` artifacts from explicit PR guidance and
  optional existing-comment metadata without posting to GitHub or changing gate
  authority.
- Added generated GitHub CI wiring for the optional PR inline comment publisher:
  `RIPR_COMMENT_MODE` defaults to `off`, `plan` mode uploads and summarizes the
  publish plan, and `inline` mode posts or updates only safe same-repository
  changed-line operations from that plan.
- Added the PR inline comment publisher workflow guide, documenting `off`,
  `plan`, and `inline` rollout, publish-plan review, fork and permission
  behavior, review-thread noise controls, dedupe/upsert, rollback, and the
  advisory gate boundary.
- Extended `cargo xtask dogfood` with checked PR inline comment publisher
  receipts for publishable, summary-only, capped, dedupe/upsert, stale-existing,
  fork or no-token, and missing-input publish plans without posting real PR
  comments.
- Closed Campaign 26, PR Inline Comment Publisher, with a prompt-to-artifact
  audit, validation plan, advisory/default-off boundary, and future-lane
  boundary in `docs/handoffs/2026-05-10-campaign-26-closeout.md`.
- Added the report-packet index fixture corpus under
  `fixtures/boundary_gap/expected/report-packet-index/`, pinning complete,
  sparse advisory, missing-front-panel, blocked-gate, missing-proof,
  missing-receipt, and coverage/grip-present packet states plus an
  `xtask check-fixture-contracts` guard before the producer changes.
- Added `fixtures/editor_lsp_workflow` as the canonical Lane 3 editor/LSP
  workflow fixture, pinning the saved-workspace diagnostic, hover, code action,
  first-useful-action status, stale-refresh guidance, and LSP cockpit surfaces
  without adding analyzer behavior or editor automation.
- Added RIPR-SPEC-0021 and the additive repo-exposure
  `seams[].evidence_record` projection, giving Lane 1 a seam-native evidence
  spine with identity, evidence path, observed values, missing discriminators,
  related tests, recommendation/actionability, calibration placeholder, and
  static limitations while preserving existing repo-exposure fields.
- Added generated canonical gap identity to evidence records so
  headline-eligible raw seam gaps group by owner, seam kind, flow sink, missing
  discriminator, and assertion shape while keeping line numbers as locators.
- Added advisory static/runtime confidence labels to mutation calibration
  JSON/Markdown rows so imported runtime data can support, contradict, remain
  ambiguous, or stay unavailable for a static gap/clean claim without changing
  static classifications, gate behavior, or mutation execution.
- Opened Campaign 23, Assistant Loop Health, with
  `spec/assistant-loop-health-report` as the first ready work item so existing
  assistant proof reports can be summarized into advisory health, missing-input,
  static-movement, warning, and repair-queue surfaces without changing analyzer,
  ranking, gate, editor, provider, mutation, generated-test, source-edit, or
  default-blocking behavior.
- Added RIPR-SPEC-0022 for the planned assistant-loop-health report, defining
  explicit proof inputs, complete/partial/missing proof states, static movement
  buckets, warning kinds, bounded repair queue entries, future multi-proof
  behavior, and advisory limits before fixtures or implementation.
- Routed zero-surface consumers through the shared evidence spine:
  agent seam packets now include additive `packets[].evidence_record`, and
  RIPR Zero status repair routes prefer supplied `evidence_record` guidance
  while preserving legacy top-level fallback fields and advisory boundaries.
- Added the assistant-loop-health fixture corpus under
  `fixtures/boundary_gap/expected/assistant-loop-health/`, pinning
  complete-improved, partial-missing-optional, missing-required-input,
  unchanged, regressed, warning-heavy, and multi-proof report states before the
  producer implementation.
- Routed targeted-test outcome and agent verify movement through the shared
  evidence spine: before/after comparison now prefers `seams[].evidence_record`
  stage, observed-value, missing-discriminator, oracle-strength, and
  related-test movement while preserving legacy repo-exposure fallback fields
  and existing advisory buckets.
- Routed test-oracle assistant proof through the shared evidence spine:
  selected seam identity, owner/location, missing discriminator, static limits,
  related test, assertion shape, verification command, and before/after classes
  now prefer supplied `evidence_record` fields while preserving legacy proof
  fallbacks and advisory boundaries.
- Added `ripr assistant-loop health`, a read-only advisory producer that writes
  `assistant-loop-health.{json,md}` from explicit proof artifacts, summarizes
  proof completeness, missing inputs, static movement, recurring warnings, and
  bounded repair queues, and leaves gate policy, analyzer behavior, provider
  calls, mutation execution, generated tests, source edits, and default CI
  blocking unchanged.
- Routed baseline and PR ledger identities through canonical gap identity:
  baseline create, diff, and shrink-only update now preserve supplied
  `canonical_gap_id` and match it before legacy selectors, while PR evidence
  ledger waiver, suppression, receipt, and top repair route records carry the
  same identity when supplied.
- Routed calibrated gate baseline comparison through canonical evidence
  identity so `ripr gate evaluate` can preserve supplied `canonical_gap_id`
  values and match reviewed baseline debt across line movement before falling
  back to legacy seam, source, and path/line/static-class identities.
- Added an evidence-record contract corpus that pins representative
  `evidence_record` v0.1 shapes for predicate, error, exact-value, broad-error,
  field, whole-object, snapshot, side-effect, opaque static-limitation,
  canonical-gap, and calibration-placeholder cases, with xtask validation for
  required cases, required fields, and schema-version drift.
- Stabilized related-test ranking v2 so capped related-test arrays preserve the
  full `related_tests_total` while ordering by relation confidence, relation
  reason, oracle strength, activation-value overlap, and stable file/name/line
  tie-breakers.
- Stabilized oracle-semantics v3 by adding structured
  `evidence_record.related_tests[].oracle_semantics` explanations that name what
  an oracle observes, what discriminator remains missing, and which assertion
  upgrade applies for broad, smoke-only, unknown, snapshot, and exact oracle
  shapes.
- Deepened local delta flow so syntax-first side-effect propagation now
  distinguishes event or outbound calls, state writes, persistence writes, log
  messages, configuration changes, and generic call-effect fallback sinks while
  preserving advisory static evidence semantics.
- Promoted activation/value modeling to stable within fixture-backed
  syntax-first scope, covering visible equality boundaries, exact error
  variants, direct literals, let bindings, same-file constants, table rows,
  rstest cases, builder or fixture overrides, enum variants, and one-level
  Option/Result constructor values while keeping unsupported value sources as
  explicit limitations.
- Promoted imported static/runtime calibration labels to calibrated for checked
  runtime-fixture classes, covering agreement, disagreement, runtime-only,
  ambiguous-join, unmatched, no-runtime-data, and seam-id/file-line join cases
  without running mutation tests or changing static classifications.
- Surfaced `assistant-loop-health.{json,md}` in generated GitHub CI when
  `test-oracle-assistant-proof.json` exists, uploads the reports with the
  normal `ripr-reports` packet, and appends a compact advisory health summary
  without changing pass/fail authority.
- Added `docs/ASSISTANT_LOOP_HEALTH_WORKFLOW.md`, explaining proof report versus
  health report, generated-CI summary use, complete/partial/missing states,
  static movement interpretation, repair routing, coding-agent handoff, and
  advisory limits for assistant-loop-health reports.
- Closed Campaign 23, Assistant Loop Health, with a prompt-to-artifact audit,
  validation plan, advisory boundary, and future-lane boundary in
  `docs/handoffs/2026-05-09-campaign-23-closeout.md`.
- Opened Campaign 24, PR Review Front Panel, with
  `spec/pr-review-front-panel-report` as the first ready work item so existing
  PR guidance, first useful action, assistant proof, assistant-loop health, PR
  ledger, baseline, gate, receipt, calibration, and coverage/grip artifacts can
  be composed into one advisory generated-CI first screen.
- Added RIPR-SPEC-0023 for the planned PR review front-panel report, including
  explicit input artifacts, bounded first-screen states, artifact groups,
  generated-CI projection limits, advisory boundaries, and the next
  fixture-first work item.
- Added the PR review front-panel fixture corpus for advisory-only,
  actionable, summary-only, acknowledged, suppressed, baseline-resolved,
  blocked, missing-proof, and coverage-flat-grip-improved cases, with an xtask
  guard to keep the producer fixture-first.
- Added `ripr pr-review front-panel`, a read-only advisory producer that writes
  `pr-review-front-panel.{json,md}` from explicit existing RIPR artifacts
  without rerunning analysis or changing gate authority.
- Updated generated GitHub CI to run `ripr pr-review front-panel` only when
  explicit input artifacts exist, upload `pr-review-front-panel.{json,md}` with
  the report packet, and append the advisory PR review front panel to the job
  summary while preserving `ripr gate evaluate` as pass/fail authority.
- Added `docs/PR_REVIEW_FRONT_PANEL_WORKFLOW.md`, documenting how reviewers,
  developers, maintainers, and coding agents read the front panel, follow
  repair routes, inspect receipts, and preserve the advisory gate boundary.
- Added dogfood validation for PR review front-panel receipts covering
  actionable, acknowledged, suppressed, baseline-resolved, blocked,
  missing-proof, no-actionable, and coverage-flat-grip-improved reviewer states
  without changing generated-CI blocking defaults.
- Closed Campaign 24, PR Review Front Panel, with a prompt-to-artifact audit,
  validation plan, advisory boundary, and future-lane boundary in
  `docs/handoffs/2026-05-10-campaign-24-closeout.md`.
- Opened Campaign 25, Report Packet Index, with
  `spec/report-packet-index-contract` as the first ready work item so the
  uploaded `ripr-reports` packet can become a reviewer-first index over
  explicit existing artifacts without changing analyzer, gate, editor,
  provider, mutation, source-edit, generated-test, inline-comment, or
  default-blocking behavior.
- Added RIPR-SPEC-0024 for the Report Packet Index contract and advanced
  Campaign 25 to the fixture-corpus slice so packet states are pinned before
  changing the index producer.
- Added `ripr reports index`, a read-only advisory producer that writes
  `target/ripr/reports/index.{json,md}` from explicit artifact directories,
  grouping reviewer-first packet surfaces while preserving gate-decision
  authority and avoiding hidden analysis reruns.
- Updated generated GitHub CI to run `ripr reports index` only when indexed
  artifacts exist, upload `index.{json,md}`, and append a compact packet-index
  section to the advisory summary without changing gate authority.
- Added `docs/REPORT_PACKET_INDEX_WORKFLOW.md`, documenting how reviewers,
  maintainers, developers, and coding agents use the grouped packet index,
  regenerate missing surfaces, and preserve the advisory gate boundary.
- Strengthened `cargo xtask check-no-panic-family` drift reporting with
  structured allowed, advisory-drift, stale, unallowed, and warning sections,
  plus exact selector-cardinality checks for ambiguous or duplicate no-panic
  allowlist entries.
- Added `cargo xtask check-no-panic-family --propose`, a review-only no-panic
  allowlist migration helper that writes Markdown/TOML selector proposals
  without rewriting policy files.
- Opened Campaign 22, First Useful Action, with
  `spec/first-useful-action-report` as the first ready work item so existing
  editor, PR guidance, ledger, proof, receipt, optional gate, coverage/grip,
  and staleness evidence can be compressed into one advisory next test action
  before adding another raw artifact surface.
- Added RIPR-SPEC-0020, defining the first-useful-action report contract,
  bounded status and action vocabularies, deterministic routing priorities,
  planned JSON/Markdown schema, traceability, and capability metrics before
  adding the producer, fixtures, CI projection, or editor projection.
- Added the Assistant Loop Health proposal, including the planned advisory
  `assistant-loop-health` JSON/Markdown surface, health buckets, repair queue,
  PR stack, and non-goals before promoting it into the active Campaign 23
  manifest.
- Added the first-useful-action routing corpus under
  `fixtures/boundary_gap/expected/first-useful-action/`, pinning actionable,
  stale, missing-required-artifact, baseline-only, acknowledged, waived,
  suppressed, no-actionable-seam, already-improved, and
  unchanged-after-attempt JSON/Markdown expectations before adding the report
  producer.
- Added `ripr first-action`, a read-only advisory report producer that writes
  `first-useful-action.{json,md}` from explicit PR guidance, assistant proof,
  PR evidence ledger, baseline delta, receipt, optional gate, optional
  coverage/grip frontier, and editor context inputs without hidden analysis,
  source edits, generated tests, provider calls, mutation execution, or default
  CI blocking.
- Generated GitHub CI now renders `ripr first-action` when explicit report
  inputs already exist, uploads `first-useful-action.{json,md}` with the normal
  report artifact packet, and appends a compact advisory first-action summary
  without changing gate authority or default blocking.
- VS Code status and `ripr: Show Status` now project an existing
  `target/ripr/reports/first-useful-action.json` report, including the selected
  action, seam location, missing discriminator, verify/receipt commands,
  warnings, fallback, and advisory limits without running hidden analysis,
  adding diagnostics, editing source, generating tests, or changing gate
  authority.
- Hardened the VS Code first-useful-action projection so reports from a
  different workspace root are ignored and stale saved-workspace evidence stays
  visible instead of being hidden behind the action report.
- Added `docs/FIRST_USEFUL_ACTION_WORKFLOW.md`, documenting how developers,
  reviewers, and coding agents read first-action reports from GitHub or the
  editor, act on the selected action, verify static movement, emit receipts,
  and interpret fallback states without treating the report as gate authority.
- Extended `cargo xtask dogfood` with checked first-useful-action receipts for
  actionable, baseline-only, stale, missing-required-artifact,
  unchanged-after-attempt, and no-actionable-seam routes while preserving
  advisory static-evidence limits and default non-blocking CI behavior.
- Closed Campaign 22 with
  `docs/handoffs/2026-05-09-campaign-22-closeout.md`, recording the
  first-useful-action prompt-to-artifact audit, validation plan, and boundary
  that future health, analyzer, policy, or editor lanes need explicit follow-up
  campaigns.
- Added `docs/ci/msrv-1.95-audit.md`, recording that `ripr` passes
  `cargo +1.95 check --workspace --all-targets`,
  `cargo +1.95 test --workspace`, and
  `cargo +1.95 clippy --workspace --all-targets -- -D warnings` before the
  follow-up MSRV bump.
- Opened Campaign 20, Test-Oracle Assistant Proof, with
  `spec/test-oracle-assistant-loop` as the first ready work item so the
  already-built PR guidance, editor/agent handoff, verification, receipts,
  ledgers, and advisory CI projection can be exercised as one end-to-end
  test-oracle assistant loop without changing analyzer, policy, editor, or CI
  defaults.
- Added RIPR-SPEC-0019, defining the end-to-end test-oracle assistant proof
  contract from changed Rust behavior through static evidence, PR/editor
  guidance, focused-test handoff, after-evidence verification, receipt, and
  advisory PR/CI projection while leaving analyzer semantics, recommendation
  ranking, gate policy, editor behavior, and default CI behavior unchanged.
- Added the canonical Campaign 20 replay corpus under
  `fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/`,
  pinning one boundary-gap seam across PR guidance, editor/agent handoff,
  before/after static evidence, a receipt, and PR ledger projection without
  adding analyzer, policy, editor, or CI behavior.
- Added the Campaign 20 dogfood receipt, tracing the canonical boundary-gap
  seam through PR guidance, editor/agent handoff, verification commands,
  after-evidence, receipt, PR ledger projection, and coverage/grip frontier
  availability while preserving advisory static-evidence limits.
- Added `docs/TEST_ORACLE_ASSISTANT_WORKFLOW.md`, documenting the Campaign 20
  user path from PR recommendation or editor diagnostic through bounded
  handoff, one focused test, after evidence, receipt, and advisory CI/ledger
  projection without source edits, generated tests, provider calls, mutation
  execution, or default CI blocking.
- Closed Campaign 20 with `docs/handoffs/2026-05-09-campaign-20-closeout.md`,
  recording the prompt-to-artifact audit, proof commands, and follow-up
  boundaries for future proof report, PR/CI polish, analyzer, and editor work.
- Opened Campaign 21, Test-Oracle Assistant Report Producer, with
  `report/test-oracle-assistant-proof` as the first ready work item so the
  Campaign 20 proof loop can become advisory `test-oracle-assistant-proof`
  JSON/Markdown artifacts from explicit existing inputs.
- Added `ripr assistant-loop proof`, a read-only advisory report producer that
  writes `test-oracle-assistant-proof.{json,md}` from explicit PR guidance,
  agent packet, before/after evidence, receipt, PR ledger, optional gate, and
  optional coverage/grip frontier inputs without rerunning analysis, editing
  source, generating tests, calling providers, running mutation testing, or
  changing default CI blocking.
- Generated GitHub CI now surfaces `test-oracle-assistant-proof.{json,md}` as
  advisory summary and artifact content only when the required PR guidance,
  agent brief, before/after evidence, agent receipt, and PR evidence ledger
  inputs already exist.
- Added `docs/TEST_ORACLE_ASSISTANT_PROOF_REPORT.md`, a reader-facing guide for
  proof report status, warnings, static movement, optional CI projection,
  coding-agent handoff, and advisory limits.
- Closed Campaign 21 with `docs/handoffs/2026-05-09-campaign-21-closeout.md`,
  recording the proof-report producer, generated-CI projection, user docs,
  validation, next-work boundary, and advisory non-goals.
- Opened Campaign 19, PR Evidence Ledger, with
  `spec/pr-evidence-ledger-surface` as the first ready work item so per-PR
  RIPR evidence can become an adoption ledger for movement history, waiver
  aging, baseline burn-down, repair receipts, and coverage/grip frontier
  signals without changing advisory defaults.
- Added RIPR-SPEC-0018, defining the PR evidence ledger contract for per-PR
  behavioral grip movement, waiver aging, baseline burn-down, repair receipts,
  optional coverage/grip frontier signals, and advisory-only CI projection
  without changing analyzer identity, gate policy, or default blocking.
- Added `ripr pr-ledger record`, a read-only advisory JSON/Markdown report that
  joins existing PR guidance, gate decisions, baseline debt deltas, RIPR Zero
  status, recommendation calibration, agent receipts, optional coverage, and
  optional history into per-PR evidence ledger records without changing gate
  authority or CI blocking defaults.
- Added generated GitHub CI projection for PR evidence ledgers: pull-request
  runs now render and upload `pr-evidence-ledger.{json,md}` when PR guidance is
  present, append a PR movement card to the job summary, and keep gate decisions
  as the only pass/fail authority.
- Added `ripr coverage-grip frontier`, an advisory JSON/Markdown report that
  keeps coverage movement and RIPR behavioral grip movement visible as separate
  axes without treating coverage as test adequacy.
- Added `docs/PR_EVIDENCE_LEDGER_WORKFLOW.md`, explaining how teams read PR
  evidence ledgers for waiver aging, baseline burn-down, repair receipts,
  coverage/grip frontier signals, and movement toward RIPR 0 without learning
  internal report topology.
- Closed Campaign 19, PR Evidence Ledger, after the spec, producer, generated
  CI projection, coverage/grip frontier report, user workflow docs, and
  closeout receipt landed while generated CI stayed advisory by default.
- Opened Campaign 18, RIPR Zero Reporting, with
  `spec/ripr-zero-reporting-surface` as the first ready work item so reviewed
  baselines and debt deltas can become repo-level RIPR 0 status, stale-debt,
  trend, and top-repair-area reporting without changing advisory defaults.
- Added RIPR-SPEC-0017, defining the RIPR Zero status report contract for
  repo-level status, baseline metadata health, stale warnings, trend summaries,
  top debt areas, and advisory repair routing without changing analyzer
  identity, gate policy, or default CI blocking.
- Added additive baseline review metadata support: new baseline ledgers record
  owner/reason/created/review-after/source fields, baseline delta reports
  preserve that metadata on baseline-derived items, and shrink-only updates keep
  existing metadata while remaining compatible with Campaign 17 baseline files.
- Added `ripr zero status`, a read-only advisory JSON/Markdown report that
  joins baseline debt deltas, reviewed baseline metadata, optional gate
  decisions, PR guidance, and recommendation calibration into repo-level RIPR
  Zero progress without changing gate authority or CI blocking defaults.
- Added generated-CI RIPR Zero summary wiring: when baseline debt delta exists,
  the workflow writes/uploads `ripr-zero-status.{json,md}` and appends a
  first-screen RIPR Zero summary without changing advisory defaults or gate
  pass/fail authority.
- Added `docs/RIPR_ZERO_REPORTING_WORKFLOW.md`, a user workflow for reading
  RIPR Zero status, aging and refreshing reviewed baselines, routing repair
  packets, and interpreting movement without treating RIPR 0 as perfect tests
  or 100 percent coverage.
- Closed Campaign 18, RIPR Zero Reporting, after the reporting spec, baseline
  metadata preservation, status report, generated-CI summary, and user workflow
  docs made RIPR 0 progress visible without changing advisory defaults.
- Added `ripr baseline create --from <gate-decision.json> --out .ripr/gate-baseline.json`,
  which writes reviewed gate baseline ledgers from existing gate-decision
  evidence, skips suppressed or malformed decisions, supports `--dry-run`, and
  refuses to overwrite without `--force`.
- Added LSP `ripr.collectEvidenceContext`, a saved-workspace seam handoff
  packet with seam identity, evidence path, missing discriminator, related
  test, suggested test, shared agent-loop commands, and static limits for
  editor or external-agent use.
- Added `ripr baseline diff --baseline <gate-baseline.json> --current <gate-decision.json>`,
  which writes advisory baseline-debt-delta JSON/Markdown showing
  still-present, resolved, new policy-eligible, acknowledged, suppressed,
  stale, invalid, and missing-input identities without making gate decisions or
  rewriting baselines.
- Added `ripr baseline update --remove-resolved`, which shrink-only refreshes a
  reviewed gate baseline ledger by removing resolved entries while preserving
  malformed or ambiguous records for review and refusing to auto-adopt new
  current debt.
- Added generated CI baseline debt delta artifacts and summary output: when a
  repository sets `RIPR_GATE_BASELINE` and gate evaluation writes
  `gate-decision.json`, the workflow runs `ripr baseline diff`, uploads
  `baseline-debt-delta.{json,md}`, and summarizes debt movement without making
  the delta report the pass/fail authority.
- Added `docs/BASELINE_LEDGER_WORKFLOW.md`, a command-by-command adoption guide
  for reviewed baseline creation, baseline debt deltas, `baseline-check`,
  shrink-only refresh, new debt review, waiver versus baseline versus
  suppression boundaries, and the path toward RIPR 0.
- Closed Campaign 17, RIPR Zero Adoption, after the baseline debt delta spec,
  baseline create/diff/update commands, generated CI delta artifacts, and
  baseline ledger workflow docs made historical behavioral test debt governable
  without changing advisory defaults.
- Extended framed LSP protocol smoke coverage through a real seam diagnostic,
  hover, code actions, `ripr.collectEvidenceContext`, and shutdown.
- Extended VS Code e2e smoke coverage so the real boundary-gap server path
  reaches a seam diagnostic, hover, seam actions, copied packet and verify
  payloads, and related-test opening.
- Added `ripr evidence-health` and `cargo xtask evidence-health`, which write
  advisory Lane 1 analyzer-health JSON/Markdown reports summarizing grip
  classes, stage states, missing discriminators, observed value contexts,
  related-test confidence, oracle evidence, top static limitations, and
  optional imported calibration availability without changing analyzer
  behavior.
- Added a VS Code status bar item and `ripr: Show Status` command for first-run
  server resolution, workspace detection, saved-workspace analysis
  disabled, queued/running/complete/stale/failed, server-unavailable, and
  no-actionable-seam states. Dirty Rust buffers now keep stale status visible
  until save or close so saved-workspace evidence is not presented as current
  for unsaved text.
- Added `docs/EDITOR_EVIDENCE_WORKFLOW.md`, a user-facing saved-workspace editor
  guide from install and status through diagnostic, hover, related test, context
  packet, one focused test, after snapshot, verify, receipt, and refresh with
  explicit static-evidence limits.
- Closed Editor Evidence UX with a prompt-to-artifact audit covering seam
  diagnostics, evidence hover, related-test actions, context packets, VS Code
  smoke, status/staleness, workflow docs, and the no-source-edit/no-runtime
  boundary.
- Added `ripr agent start --root . --seam-id <id> --out target/ripr/workflow`
  to write a source-edit-free workflow packet with `workflow.json`,
  `commands.md`, and `agent-brief.json` for one selected seam. The packet
  names artifact paths, shared commands, missing inputs, and explicit no-edit,
  no-generated-test, no-LLM-call boundaries.
- Added `ripr agent status --root . --json`, a read-only LLM work-loop status
  report that checks existing agent artifacts, recovers a seam id when
  possible, emits missing-step commands, and warns on stale-looking verify or
  receipt artifacts without rerunning analysis.
- Added `cargo xtask check-ci-lane-whitelist`, a structural advisory checker
  for the CI lane, risk-pack, budget, artifact-family, and rollout-exception
  ledgers.
- Added `ripr agent start --root . --seam-id <id> --out target/ripr/workflow`,
  which writes source-edit-free `agent-workflow.json` and `agent-workflow.md`
  checklists from the shared LLM work-loop command templates.
- Queued Campaign 12, First-Hour UX, as the post-LLM-work-loop lane for making
  the VS Code extension and generated CI workflow useful from their first
  screens without requiring users to learn RIPR's internal report topology.
- Opened Campaign 12, First-Hour UX, with `spec/pr-test-guidance-annotations`
  as the first ready contract item before editor or CI behavior changes.
- Added RIPR-SPEC-0012 for advisory PR test guidance annotations, defining the
  `ripr review-comments` JSON contract, changed-line placement rules,
  check-annotation default, opt-in inline review comments, and bounded LLM
  handoff guidance.
- Added `ripr review-comments --root . --base <sha> --head <sha> --out target/ripr/review/comments.json`
  to write bounded advisory PR guidance JSON plus Markdown without posting to
  GitHub, generating tests, editing source, running mutation testing, or making
  CI blocking.
- Added generated CI execution of `ripr review-comments` on pull requests so
  `target/ripr/review/comments.json` is written before the existing advisory
  summary and non-blocking check-annotation consumers run.
- Added PR guidance fixture outputs under
  `fixtures/boundary_gap/expected/pr-guidance` for exact-line,
  owner-function-line, same-file-line, summary-only, capped, configured-off,
  and changed-test-skip cases.
- Added `docs/PR_REVIEW_GUIDANCE.md` to document `ripr review-comments`,
  generated CI check annotations, summary-only fallback, pinned fixture cases,
  and the inline-comment opt-in boundary.
- Added the Campaign 13 closeout handoff after PR guidance renderer, generated
  CI consumption, placement fixtures, and user-facing docs aligned.
- Opened Campaign 14, Recommendation Calibration, with
  `spec/recommendation-calibration-report` as the first ready item so
  recommendation quality is measured before ranking or policy work.
- Queued Campaign 15, Calibrated Gate Policy, as a later optional-policy lane
  after recommendation calibration, preserving advisory defaults and keeping
  static evidence separate from runtime mutation vocabulary.
- Added RIPR-SPEC-0013 for recommendation calibration reports, defining the
  planned input artifacts, JSON/Markdown shape, usefulness metrics, false
  annotation tracking, summary-only correctness, suppression correctness,
  target-file correctness, latency fields, advisory posture, and non-goals.
- Added pinned review guidance outcome receipt examples for useful, noisy,
  wrong-line, already-covered, wrong-target, summary-only-correct, and
  suppressed-correctly recommendation calibration feedback.
- Added `cargo xtask recommendation-calibration`, which reads existing PR
  guidance, calibration expectations, optional outcome receipts, targeted-test
  outcome, and agent receipt artifacts, then writes advisory
  `recommendation-calibration.{json,md}` without telemetry, source edits,
  generated tests, runtime execution, or CI blocking.
- Added checked recommendation calibration report outputs under
  `fixtures/boundary_gap/expected/recommendation-calibration/`.
- Added `docs/RECOMMENDATION_CALIBRATION.md` to document how to run and read
  recommendation calibration reports, local outcome receipts, placement
  quality, suppression correctness, static movement buckets, and advisory
  limits.
- Added the Campaign 14 closeout handoff after recommendation calibration was
  specified, fixture-pinned, receipt-backed, reported, documented, and kept
  advisory-first for later ranking or policy work.
- Added RIPR-SPEC-0014 for calibrated gate policy, defining optional
  visible-only, acknowledgeable, baseline-check, and calibrated-gate modes plus
  the planned gate decision JSON/Markdown contract.
- Added `ripr gate evaluate`, a read-only optional policy evaluator that writes
  `gate-decision.{json,md}` from existing PR guidance, labels, baselines, and
  calibration inputs without posting comments, editing source, running mutation
  tests, uploading SARIF, or changing generated workflow defaults.
- Added `docs/CALIBRATED_GATE_POLICY.md` to document optional gate modes,
  waiver labels, generated CI behavior, calibration evidence, rollout stages,
  fixture cases, and static/runtime vocabulary boundaries.
- Added the Campaign 15 closeout handoff after optional calibrated gates were
  specified, implemented as read-only evaluation, fixture-pinned, optionally
  wired into generated CI, documented, and kept advisory by default.
- Opened Campaign 16, Gate Adoption UX, with `docs/gate-adoption-examples` as
  the first ready item before waiver workflows, baseline guidance, CI summary
  polish, dogfood receipts, and blocking-readiness docs.
- Added copyable generated-CI gate adoption examples for default advisory
  posture, `visible-only`, `acknowledgeable`, `baseline-check`, and
  `calibrated-gate` repository-variable settings.
- Queued Editor Evidence UX as a separate Lane 3 campaign proposal after Gate
  Adoption UX, preserving `gate-adoption-ux` as the active manifest while
  documenting the saved-workspace LSP loop from diagnostic to hover, related
  test, context packet, verify, and receipt.
- Added gate waiver workflow docs for `ripr-waive`, including label setup,
  acknowledgeable-mode review steps, audit artifacts, and the boundary between
  PR-time waivers, durable suppressions, and baselines.
- Added gate baseline workflow docs for creating, reviewing, and refreshing
  `.ripr/gate-baseline.json` as a visible historical-debt ledger rather than a
  suppression file, with RIPR 0 framed as a configured-scope burn-down target
  and `baseline-check` behavior documented for reviewed historical debt.
- Polished the generated CI gate summary so reviewers can see mode, status,
  decision counts, active and acknowledgement labels, applied waiver, baseline
  input, calibration inputs/effects, blocking reason, and gate artifact paths
  before opening JSON.
- Added checked repo-local gate adoption dogfood receipts to
  `cargo xtask dogfood`, covering `visible-only`, acknowledged waiver,
  baseline-existing, baseline-new, repair-oriented missing-baseline, and
  explicit calibrated-gate decisions from checked evidence while preserving
  non-blocking generated CI defaults.
- Added `docs/BLOCKING_READINESS.md` to explain when teams should stay
  advisory, require acknowledgement, use `baseline-check`, or enable
  `calibrated-gate` after local evidence is mature.
- Added the Campaign 16 closeout handoff after gate adoption examples, waiver
  workflows, baseline guidance, generated-CI gate summary polish, dogfood
  receipts, and blocking-readiness guidance were complete while generated CI
  stayed advisory by default.
- Opened Campaign 17, RIPR Zero Adoption, with
  `spec/baseline-debt-delta-report` as the first ready item before baseline
  create, diff, shrink-only update, and generated CI debt-delta artifacts.
- Added `docs/EDITOR_EVIDENCE_UX.md` and the Editor Evidence UX audit handoff
  to define the queued saved-workspace editor contract before behavior changes.
- Hardened seam evidence hover so saved-workspace seam diagnostics now show
  related test locations, suggested test shape, packet and brief handoff
  commands, verify and receipt commands, and static-evidence limits from the
  same classified seam state.
- Tightened seam code-action visibility so the focused test brief action is
  offered only when a related test or suggested assertion context exists, while
  packet, agent handoff, verify, receipt, and refresh commands remain
  available for stable seam diagnostics.
- Added RIPR-SPEC-0016 for the baseline debt delta report, defining the planned
  JSON/Markdown contract, identity matching order, debt movement buckets,
  advisory boundary, and future `ripr baseline create`, `diff`, and
  shrink-only `update --remove-resolved` command surfaces.
- Added a generated GitHub workflow advisory summary that combines the pilot
  recommendation, agent review packet, artifact paths, SARIF and badge status,
  known limits, and PR guidance annotation counts before artifact
  download.
- Added a generated workflow smoke fixture test that pins artifact paths,
  top-seam extraction, agent artifact generation, non-blocking posture,
  optional SARIF gates, badge output, advisory summary sections, and PR
  guidance annotation hooks.
- Added an LLM work-loop fixture matrix that pins happy, unchanged, regressed,
  missing-artifact, stale-artifact, configured-off, path-with-spaces, and
  Windows-separator review states.
- Added generated CI LLM work-loop packets: `ripr init --ci github` now uploads
  workflow manifest, commands Markdown, agent status JSON/Markdown, review
  summary JSON/Markdown, receipt, and operator cockpit artifacts as advisory
  evidence.
- Added `docs/LLM_OPERATOR_GUIDE.md`, a source-edit-free guide for humans and
  external LLM tools using RIPR status, workflow packet, verify, receipt, and
  reviewer-summary artifacts.
- Closed Campaign 11 after status, command templates, workflow manifests,
  provenance-backed receipts, bounded next-action guidance, reviewer summaries,
  fixtures, generated CI packets, and the LLM operator guide aligned around a
  source-edit-free static work loop.
- Extended the LSP seam evidence hover to project first-useful-action when an
  existing workspace-matched report selects the same seam, so the editor hover
  carries the same advisory next-action, target test, verify command, and
  receipt command surfaces as the status bar without rerunning analysis.

### Changed

- Promoted the Lane 1 `evidence_record` capability to stable within its
  documented v0.1 scope and added a dedicated Lane 1 evidence-spine tracker so
  future evidence work stays separate from active PR/CI, editor, policy, and
  release campaigns.
- Promoted local delta flow to stable within its fixture-backed syntax-first
  scope for visible return, error, field, match, and side-effect sink families
  while keeping unsupported propagation as explicit static limitations.
- Moved the declared workspace MSRV, pinned toolchain, `clippy.toml`, and
  `policy/clippy-lints.toml` MSRV ledger to Rust 1.95 after the compatibility
  audit passed; planned Clippy lint promotion remains deferred to the next
  rollout PR.
- Promoted the clean Rust 1.94/1.95 planned Clippy lints into the active
  workspace lint policy and retained unsupported or config-dependent lints with
  explicit blockers.
- Made `policy/no-panic-allowlist.toml` the canonical schema 0.3 no-panic
  allowlist, with governed ids, owners, expiry dates, and checker support while
  leaving `.ripr/no-panic-allowlist.toml` as a legacy compatibility mirror.
- Advanced Campaign 13, PR Review Guidance, after adding the read-only
  `ripr review-comments` producer and generated CI producer step;
  placement/suppression fixtures and PR guidance docs completed the lane before
  closeout.
- Closed Campaign 13 after PR guidance became produced, consumed by generated
  CI, fixture-pinned, documented, and still advisory/non-blocking by default.
- Advanced the active product lane to Campaign 14 so recommendation
  usefulness, placement, suppression/noise behavior, and before/after static
  movement can be calibrated before optional gates.
- Advanced Campaign 14 to `fixtures/pr-guidance-calibration-corpus` and
  `review-feedback/outcome-receipts` after pinning the recommendation
  calibration report contract.
- Advanced Campaign 14 to `report/recommendation-precision` after pinning
  outcome receipt fixtures for local recommendation feedback.
- Advanced Campaign 14 to `docs/calibration-workflow` after adding the
  advisory recommendation precision report command and checked outputs.
- Advanced Campaign 14 to `campaign/recommendation-calibration-closeout` after
  documenting the recommendation calibration workflow.
- Opened Campaign 15, Calibrated Gate Policy, with
  `spec/calibrated-gate-policy` as the next ready contract item.
- Advanced Campaign 15 to `gate/policy-evaluator` after pinning the calibrated
  gate policy contract.
- Advanced Campaign 15 to `fixtures/calibrated-gate-cases` after adding the
  read-only gate decision producer.
- Advanced Campaign 15 to `ci/generated-gate-wiring` after pinning calibrated
  gate decision fixtures for advisory, acknowledged, baseline-check,
  high-confidence blocking, suppression, missing-input, and calibration
  disagreement cases.
- Wired generated GitHub workflows to run `ripr gate evaluate` only when
  `RIPR_GATE_MODE` is explicitly configured, upload gate-decision artifacts,
  and keep default generated workflows advisory.
- Advanced Campaign 15 to `docs/calibrated-gate-policy` after optional
  generated CI gate wiring landed without changing default workflow blocking.
- Advanced Campaign 15 to `campaign/calibrated-gate-closeout` after documenting
  calibrated gates as optional policy over existing static evidence.
- Closed Campaign 15 after the optional calibrated gate layer was specified,
  implemented as a read-only evaluator, fixture-pinned, wired into generated CI
  only behind explicit configuration, documented, and kept advisory by default.
- Clarified agent merge ownership and replaced the old campaign-field guard
  with stale merge-boundary language detection.
- Pinned RIPR-SPEC-0012 as the PR test guidance annotation contract and
  advanced Campaign 12 to `vscode/first-run-status` as the next ready UX item.
- Advanced Campaign 12 to `vscode/action-discoverability` after pinning the
  extension first-run status surface.
- Grouped seam diagnostic code-action and VS Code command titles around user
  intent: inspect the seam, write the targeted test, copy agent handoff
  commands, verify after the test, review the receipt, and refresh analysis.
  Command IDs and payloads remain stable.
- Advanced Campaign 12 to `ci/pr-summary-surface` after pinning editor action
  discoverability.
- Advanced Campaign 12 to `ci/generated-workflow-smoke-fixture` after wiring
  the generated workflow advisory summary and PR guidance annotation hook.
- Advanced Campaign 12 to `docs/ux-by-user-type` after pinning the generated
  workflow smoke fixture.
- Reworked README and Quickstart first-hour docs around VS Code, CI, CLI,
  agent/reviewer, troubleshooting, and known-limit paths, and advanced Campaign
  12 to closeout.
- Closed Campaign 12 after the editor first-run status path, intent-titled
  actions, generated CI advisory summary, workflow smoke fixture, and
  user-type first-hour docs aligned the VS Code, CI, CLI, and agent/reviewer
  entry paths.
- Aligned public package and extension front-door metadata around Rust
  test-oracle gaps, targeted tests, and static RIPR evidence instead of
  internal mutation-exposure wording.
- Centralized LLM work-loop command templates for agent status next commands,
  agent brief follow-up commands, pilot follow-up commands, LSP copy-action
  payloads, generated CI artifact paths, and operator cockpit missing-input
  commands without changing the emitted command text.
- `ripr agent status --root .` now prints Markdown by default; `--json` keeps
  the machine-readable Agent Status schema.
- Documented the CI verification economics policy: required, advisory, and
  on-demand/release postures; LEM budget bands; label effects; artifact
  families; cheaper-signal-first rules; CI actuals; and rollback expectations.
  The PR template now asks CI-affecting PRs to record cost, affected workflows,
  branch-protection impact, cheaper signals considered, artifact families, and
  rollback path.
- Documented the completed `0.4.0` post-publish verification for crates.io,
  GitHub Release server assets, VS Marketplace, Open VSX, and installed
  editor-agent loop smoke checks.
- Added non-enforcing CI policy ledgers for LEM budget bands, target lane IDs,
  risk packs, artifact families, labels, and rollout exceptions. These seed
  files document the future PR planning surface without changing workflow
  behavior.
- Tightened LSP command payload contracts and first-useful-action status edges
  so saved-workspace command smoke and saved-workspace status output stay
  pinned across analysis-queued, analysis-running, stale-buffer, missing-input,
  and no-actionable-seam transitions.
- Hardened the VS Code saved-workspace status output and command smoke
  fixtures to keep the status bar item, `ripr: Show Status` text, and
  intent-titled action payloads stable across the report-projection,
  stale-buffer, and disabled-by-setting paths without changing user-visible
  behavior.

### Release prep

- Bumped crate, extension, lockfile, agent-receipt fixture, doc, and xtask
  release-readiness test-fixture references from 0.4.0 to 0.5.0; aligned the
  CI MSRV job toolchain pin and cache keys to Rust 1.95.0; refreshed the
  workspace and crate README MSRV badges, Distribution capability rows, and
  Rust-requirement statements; advanced version refs in `docs/RELEASE.md`,
  `docs/RELEASE_BINARIES.md`, `docs/RELEASE_MARKETPLACE.md`,
  `docs/SERVER_PROVISIONING.md`, `docs/EDITOR_EXTENSION.md`, `docs/OPENVSX.md`,
  `docs/specs/RIPR-SPEC-0011-llm-work-loop.md`, `docs/OUTPUT_SCHEMA.md`, and
  the VS Code extension changelog and resolver fallback. The 0.4.0 release
  receipts and historical rollout records are preserved unchanged.

## 0.4.0 - 2026-05-07

This release aligns RIPR's editor and CI evidence loop: saved-workspace
diagnostics, hover evidence, targeted briefs, agent command handoff, focused
test verification, receipts, cockpit artifacts, and non-blocking CI output now
tell the same conservative static-exposure story.

### Added

- Added `ripr pilot`, a zero-config first-run command that writes
  `target/ripr/pilot/repo-exposure.{json,md}`,
  `target/ripr/pilot/agent-seam-packets.json`, and
  `target/ripr/pilot/pilot-summary.{json,md}` while printing the top
  actionable seam and after-test commands.
- Added `ripr outcome` to compare before/after `repo-exposure-json` snapshots
  from the installed binary, printing an advisory targeted-test receipt by
  default with `--format json` and `--out` for tool/file output.
- Added `ripr calibrate cargo-mutants` to import already-produced
  cargo-mutants JSON from the installed binary, join it to a
  `repo-exposure-json` snapshot, and render advisory Markdown/JSON calibration
  output without running mutation testing.
- Added `ripr init --ci github` to generate a non-blocking GitHub Actions
  workflow that runs `ripr pilot`, uploads pilot/report artifacts, writes repo
  badge JSON, and keeps SARIF rendering/upload optional through
  `RIPR_UPLOAD_SARIF`.
- Added `ripr init` as an optional command that materializes built-in defaults
  into a repo-local `ripr.toml` so teams can commit, review, version, and tune
  policy; it does not unlock basic usefulness, and missing `ripr.toml` remains
  the normal first-run state. Includes `--dry-run` for previewing and `--force`
  for explicit overwrite.
- Added RIPR-SPEC-0009 to define defaults-first adoption behavior for `init`,
  `pilot`, `outcome`, calibration import, editor, SARIF, badge, and config
  work.
- Added focused defaults-first guardrails that pin the generated `ripr.toml`
  against `ripr.toml.example` and test default repo discovery exclusions for
  generated, policy-only, fixture-only, and package-manager directories.
- Added `cargo xtask operator-cockpit-report`, which writes
  `target/ripr/reports/operator-cockpit.{json,md}` by joining repo exposure,
  LSP cockpit, SARIF policy, badge status, targeted-test outcome, and optional
  mutation calibration artifacts into one next-action report.
- Added `docs/INSTALLATION_VERIFICATION.md` to pin the defaults-first release
  proof for local package install, public `cargo install`, GitHub Release server
  archives, VSIX packaging, and known limits.
- Added the initial JSON-only `ripr agent brief` command, which ranks
  working-set seams from existing repo exposure evidence and points agents to
  packet references, candidate discriminators, assertion shape, and static
  before/after verification commands.
- Added `ripr agent verify` and `ripr agent receipt` so agent workflows can
  compare before/after repo exposure snapshots and emit a focused review
  receipt for one seam.
- Added saved-workspace LSP/VS Code code actions that copy the agent loop
  command chain for a seam diagnostic: agent packet, agent brief, after
  snapshot, agent verify, and agent receipt.
- Added `cargo xtask release-readiness --version <version>`, which writes
  `target/ripr/reports/release-readiness.{json,md}` and checks the 0.4 CLI,
  agent verify/receipt, LSP cockpit, advisory CI, latency, install, VSIX, and
  known-limit surfaces from repo artifacts.
- Added operator cockpit status for the editor-agent loop artifacts:
  before/after snapshots, `agent verify`, `agent receipt`, movement counts,
  and missing-input commands aligned with the editor copy-command chain.
- Added a canonical boundary-gap editor-agent loop fixture that pins agent
  packet, agent brief, agent verify, agent receipt, and operator cockpit output
  against the LSP diagnostic/action seam identity.
- Expanded the generated `ripr init --ci github` workflow to upload the
  editor-agent loop artifacts: pilot output, agent packet, agent brief, agent
  verify, agent receipt, targeted-test outcome, optional operator cockpit,
  SARIF, and badge JSON.

### Changed

- Centered the first-hour installed-user docs on the full editor-agent evidence
  loop: `ripr pilot`, targeted brief, focused test, after snapshot,
  `ripr outcome`, `ripr agent verify`, `ripr agent receipt`, editor actions,
  generated CI artifacts, and the explicit `ripr init` policy-materialization
  boundary.
- Documented and test-pinned the LSP agent-loop copy-command payload contract:
  commands stay workspace-relative, preserve seam metadata, and fail closed for
  stale seam diagnostics.
- Restored Campaign 10 to `editor-agent-integration` after the brief
  release-surface pivot, carrying release readiness as a later gate and moving
  the lane from LSP command copy actions to operator cockpit verify/receipt
  status.
- Closed Campaign 10 after aligning editor, agent, cockpit, CI, fixture, docs,
  and release-readiness surfaces without adding analyzer families, runtime
  mutation execution, CI blocking, public crate splits, automatic edits, or
  speculative editor features.
- Routed `ripr agent brief` file and diff working sets through existing
  related-test evidence so edits to known related tests rank their seams before
  repo fallback, and added the related-test-confidence tie-breaker from
  RIPR-SPEC-0010.
- Added an advisory `ripr agent brief` warning when visible seams are omitted
  by the default or requested brief cap.
- Routed `ripr agent brief --diff` and `--base` changed lines through existing
  owner-function facts so same-owner seams can rank as
  `changed_owner_function` before broad file fallback.
- Normalized agent seam packet file paths to use stable `/` separators in
  checked JSON, including related-test and recommended-test paths on Windows.
- Made `ripr pilot` budget-aware with a default 30 second analysis timeout,
  `--timeout-ms` for explicit runs, and a versioned `pilot-summary.json` schema
  update that records complete versus partial timeout status.
- Prepared the `0.3.1` release line as the first defaults-first public install
  target. `0.3.0` remains published but predates `ripr pilot` and
  `ripr outcome`.
- Documented and test-pinned the defaults-first config profile, including
  missing-config/generated-config equivalence, repo-mode production exclusions,
  badge/report defaults, and fast/normal/deep operator mode vocabulary; Campaign
  7 now moves from `defaults/config-init` to the operator cockpit and editor
  install polish work items.
- Aligned the VS Code extension's default `ripr.check.mode` with the
  defaults-first posture by switching it to `draft` and exposing the full LSP
  mode enum.
- Ignored generated `.vscode-test` editor-host artifacts in repo file scans so
  local extension smoke runs do not pollute Rust policy gates.
- Split `xtask` command parsing, help/catalog, and execution dispatch into
  focused modules while preserving existing `cargo xtask` command behavior.
- Routed xtask policy checks through focused `xtask/src/policy/` checker modules
  while preserving existing `cargo xtask check-*` policy command behavior.
- Routed xtask report commands through focused `xtask/src/reports/` modules
  while preserving existing report command behavior.
- Closed Campaign 6 after the internal module SRP refactor chain landed through
  #405, confirmed stale forks #250, #253, and #352 are closed unmerged, and
  moved the active manifest to Campaign 7 defaults-first operator adoption.
- Completed the Campaign 7 `defaults/config-init` baseline and advanced the
  active manifest to `reports/operator-cockpit` as the next ready work item.
- Completed the Campaign 7 `reports/operator-cockpit` surface and advanced the
  active manifest to `ci/github-action-entrypoint` as the next ready work item.
- Completed the Campaign 7 `ci/github-action-entrypoint` surface and advanced
  the active manifest to `editor/install-polish` as the next ready work item.
- Completed the Campaign 7 `editor/install-polish` surface and advanced the
  active manifest to `fixtures/example-corpus` as the next ready work item.
- Aligned built-in defaults with the `ripr init` profile for LSP seam
  diagnostics: missing config now uses the same bounded saved-workspace default
  as the generated policy file, while explicit LSP options or `ripr.toml` can
  still disable seam diagnostics.
- Tightened RIPR-SPEC-0009 so missing `ripr.toml` means useful built-in
  defaults, while `ripr init` records repo policy instead of unlocking basic
  CLI or editor usefulness.
- Added a boundary-gap runtime calibration sample so the targeted-test case
  study can demonstrate a static-gap/runtime-clean join without running
  mutation testing.
- Closed Campaign 4B (Repo Seam Inventory and Test Grip) and made repo
  seam evidence first-class: `RepoSeam` / `SeamId` / `SeamKind` /
  `RequiredDiscriminator` /
  `ExpectedSink` / `SeamGripClass` data model with deterministic 16-char
  FNV-1a seam IDs (#229); production-file seam inventory walker writing
  `target/ripr/reports/repo-seams.{json,md}` (#235); `TestGripEvidence`
  + `RelatedTestGrip` attaching reach/activate/propagate/observe/
  discriminate evidence per seam (#236); seam classification mapping
  evidence to one of 11 spec classes with explicit headline-eligibility
  table (#237); repo exposure report at
  `target/ripr/reports/repo-exposure.{json,md}` with per-class metric
  buckets (#239); agent seam packets at
  `target/ripr/reports/agent-seam-packets.json` carrying
  `write_targeted_test` work orders for headline-eligible seams and
  `inspect_static_limitation` for opaque seams (#240); LSP seam
  diagnostics with stable `ripr-seam-{class}` codes behind
  `seamDiagnostics: true` opt-in (#241); seam-native LSP hover that
  looks up `ClassifiedSeam` via `data.seam_id` and renders the RIPR
  evidence path (#242); and `docs/AGENT_DISPATCH_WORKFLOW.md`
  documenting the practical loop (#248). Static output keeps the
  audit vocabulary; runtime mutation testing remains a separate
  confirmation step.
- Started Campaign 5 (Adoption and Calibration). `cache/repo-seam-facts-v1`
  and `calibration/cargo-mutants-v1` carry forward from Campaign 4B as
  ready items; `config/ripr-config-v1` and `ci/sarif-ci-policy` remain
  blocked on the cache and config respectively.
- Reframed Campaign 5 as Campaign 5A (Seam Evidence Usability and Precision)
  to focus the queue on four product axes — fast (cache), precise
  (related-test-precision-v1, value-extraction-v2, oracle-shape-v2),
  actionable (agent-seam-packets-v2, lsp/seam-code-actions-v1), and
  calibrated (cargo-mutants-v1). Operationalization items
  (`config/ripr-config-v1`, `ci/sarif-ci-policy`, future
  `badge/seam-native-count-mapping`) move to Campaign 5B and stay
  blocked behind 5A's cache and oracle-shape work. Cache
  serialization policy: never bincode; postcard if binary; fact
  layers only.
- Renamed durable Campaign 5A wording from "Voice B" to "seam
  evidence" across manifest, docs, README, and rendered report
  Markdown; marked `cache/repo-seam-facts-v1` done after #255 merged.
  State-only PR; no analyzer behavior, cache behavior, or output
  schema changes. The manifest campaign id is now
  `seam-evidence-usability-and-precision`.
- Added internal local flow sink facts for changed expressions, including
  return values, error variants, struct fields, call effects, and match-arm
  results.
- Added activation evidence facts for observed test values and missing
  discriminator values, including boundary equality gaps and exact error
  variant gaps tied to local flow sinks.
- Added evidence-first human and JSON finding output that promotes changed
  behavior evidence paths, local flow sinks, observed values, missing
  discriminators, oracle kind/strength, and suggested next actions.
- Added negative and metamorphic fixture coverage for whitespace/comment/import
  noise, unrelated token mentions, strong boundary/error oracles, and equivalent
  assertion/test-layout variants.
- Closed Campaign 3 and added the advisory Test Efficiency and Vacuity Signals
  lane for per-test evidence ledgers, likely-vacuity signals, and duplicate
  discriminator reports.
- Added `cargo xtask test-efficiency-report`, an advisory per-test evidence
  ledger that reports apparent owner calls, oracle kind/strength, observed
  literal values, and static limitations.
- Extended the test-efficiency report with advisory reason counts for
  smoke-only, broad-oracle, disconnected, opaque, circular, and likely-vacuous
  signals.
- Passed VS Code `ripr.check.mode` and `ripr.baseRef` settings into LSP
  workspace diagnostics.
- Stored the latest LSP analysis snapshot alongside diagnostics so future
  hover, code-action, and context paths can resolve findings without rerunning
  analysis.
- Scoped LSP diagnostic ranges to the probe source column and expression width
  instead of marking a fixed line prefix.
- Added a framed LSP protocol smoke test for initialize, didOpen, refresh,
  hover, codeAction, shutdown, and exit over the tower server.
- Added `cargo xtask mutation-calibration`, an advisory cargo-mutants import
  scaffold that joins runtime mutation records to static seam evidence by
  `seam_id` or unambiguous normalized file/line and writes
  `target/ripr/reports/mutation-calibration.{json,md}`. Span-based generated
  mutant locations are imported, and ambiguous file/line candidates remain
  unassigned. Runtime mutation vocabulary stays confined to calibration/runtime
  reports.
- Closed Campaign 5A (Seam Evidence Usability and Precision) after the cache,
  related-test precision, value extraction, oracle-shape, agent packet, LSP code
  action, and cargo-mutants calibration chain landed (#255, #310, #313, #314,
  #315, #316, #327). The active manifest now moves to Campaign 5B
  Operationalization with `config/ripr-config-v1` as the next ready item and
  SARIF / seam-native badge policy blocked behind config.
- Added repo-root `ripr.toml` configuration for Campaign 5B. Config can set
  analysis mode, oracle policy for snapshots/mocks/broad errors, finding and
  seam severity mapping, suppressions path, related-test report caps, and LSP
  seam-diagnostic defaults. Missing config preserves existing defaults, unknown
  keys fail loudly, and explicit CLI flags or LSP initialization options still
  win. SARIF and seam-native badge remapping remain out of scope for this PR.
- Added `ripr doctor` visibility for repository config. Doctor now reports
  whether `ripr.toml` was loaded, which effective defaults are active, and
  malformed config errors without printing config source text.
- Added RIPR-SPEC-0008 to define the Campaign 5B SARIF and CI policy contract:
  stable Finding and seam rule IDs, configured severity mapping, suppression
  visibility, advisory defaults, and opt-in baseline policy modes.
- Added SARIF output formats for Campaign 5B. `ripr check --format sarif`
  renders diff-scoped Finding SARIF and `--format repo-sarif` renders
  repo-scoped seam SARIF with configured severity, visible suppression metadata,
  stable rule IDs, and stable fingerprints.
- Added `cargo xtask sarif-policy` for opt-in SARIF baseline checks. The
  command compares current SARIF to a baseline using stable rule IDs and
  fingerprints, ignores suppressed results, writes
  `target/ripr/reports/sarif-policy.{json,md}`, and only exits non-zero for
  new warning-level results when `--mode fail-on-new-warning` is requested.
- Remapped public repo badges onto seam-native counts for Campaign 5B.
  Repo-scoped `ripr` and `ripr+` badges now count configured-visible
  headline-eligible `SeamGripClass` values, while diff-scoped badge artifacts
  remain legacy finding-exposure summaries for PRs. Native badge JSON is now
  schema `0.3` with `basis` and `counts.analyzed_seams`; the checked-in
  Shields endpoint artifacts in `badges/` were refreshed together.
- Closed Campaign 5B (Operationalization) after repository config, SARIF/CI
  policy, and seam-native badge count mapping landed (#331, #333, #336, #338,
  #342). The active manifest now moves to Campaign 6 with a draft-stack audit
  before structural refactors resume.
- Audited the Campaign 6 modularization draft stack against current `main` and
  recorded the canonical rebase path before structural refactors resume. The
  first ready item is the #244 summary/sort extraction; #249 stays in the
  sequence after the workspace split, while #250 is parked for close or rewrite
  after the facts/syntax/build-index path stabilizes.
- Started the Campaign 6 refactor stack by extracting summary/sort helpers,
  pipeline orchestration, diff load/model/parse modules, workspace
  classify/discover/select modules, and probe classify/config/diff/repo modules
  without output, schema, or public API drift.
- Moved neutral Rust analysis fact DTOs into `analysis/facts/model.rs` for the
  Campaign 6 facts model extraction while leaving syntax adapters, builders,
  extraction, and query logic in place. The next ready seam is syntax adapter
  type extraction.
- Moved syntax adapter traits and shared syntax facts into
  `analysis/syntax/adapter.rs` while keeping builders, parser-backed extraction,
  lexical fallback, and query logic in `analysis/rust_index.rs`. The next ready
  seam is build-index extraction.
- Moved Rust index construction into `analysis/facts/build.rs` while keeping
  parser-backed extraction, lexical fallback, and query helpers in
  `analysis/rust_index.rs`. The next ready seam is parser-backed RA syntax
  extraction.
- Moved parser-backed RA syntax adapter implementation into
  `analysis/syntax/ra.rs` while keeping lexical fallback and Rust index query
  helpers behavior-stable. The next ready seam is lexical syntax fallback
  extraction.
- Moved the lexical syntax fallback implementation into
  `analysis/syntax/lexical.rs` while keeping `analysis/rust_index.rs` as the
  compatibility facade for query and extractor helpers. The next ready seam is
  fact extraction helper modularization.
- Moved call, return, literal, oracle, and text extraction helpers plus
  probe-shape constants into `analysis/extract/*`, with `analysis/rust_index.rs`
  still re-exporting the compatibility helper surface. The next ready seam is
  probe family metadata extraction.
- Moved probe-family mapping, changed-line family heuristics, and delta metadata
  into `analysis/probes/family.rs` while preserving probe generation behavior.
  The next ready seam is probe expectation helper extraction.
- Moved probe expected-sink and required-oracle helpers into
  `analysis/probes/expectations.rs` while preserving probe generation behavior.
  The next ready seam is probe ID helper extraction.
- Moved probe ID construction and path sanitization helpers into
  `analysis/probes/ids.rs` while preserving diff and repo probe ID formats.
  The next ready seam is lexical probe fallback extraction.
- Moved lexical changed-line probe fallback helpers into
  `analysis/probes/lexical.rs` while preserving probe generation behavior.
  The next ready seam is diff/repo probe seeding split.
- Reconciled the Campaign 6 probe seeding manifest after confirming diff and
  repo probe seeding already lives in `analysis/probes/diff.rs` and
  `analysis/probes/repo.rs`. The next ready seam is classification context
  extraction.
- Added a private `analysis/classify/context.rs` `ProbeContext` carrier for
  the classifier's probe, owner, and related-test inputs, setting up later
  RIPR stage module extraction without changing classification behavior. The
  next ready seam is related-test discovery extraction.
- Moved related-test discovery into `analysis/classify/related_tests.rs` while
  preserving classification behavior. The next ready seam is reach evidence
  extraction.
- Moved reach evidence into `analysis/classify/reach.rs` while preserving
  classification behavior. The next ready seam is flow and propagation
  extraction.
- Moved local flow and propagation evidence into `analysis/classify/flow.rs`
  while preserving classification behavior. The next ready seam is activation
  evidence extraction.
- Moved activation evidence, observed-value extraction, and missing
  discriminator helpers into `analysis/classify/activation.rs` while preserving
  classification behavior. The next ready seam is remaining classifier stage
  extraction.
- Moved remaining classifier stage and decision helpers into
  `analysis/classify/{infection,reveal,decision}.rs` while preserving
  classification behavior. The next ready seam is app use-case splitting.
- Split check, explain, and context use-case orchestration into focused `app`
  modules while preserving public API and output behavior. The next ready seam
  is output format extraction.
- Moved `OutputFormat` into `output/format.rs` while preserving the
  `app::OutputFormat` public path and output behavior. The next ready seam is
  render dispatch extraction.
- Moved `render_check` dispatch into `output/render.rs` while preserving the
  `app::render_check` public facade and output behavior. The next ready seam is
  CLI command model extraction.
- Added a focused private `cli/command.rs` `CliCommand` enum for top-level CLI
  command shape while preserving CLI parsing and dispatch behavior. The next
  ready seam is parsed-command extraction.
- Updated CLI parsing so `cli::parse` returns the typed `CliCommand` shape
  before dispatch, while preserving command argument behavior. The next ready
  seam is CLI execution extraction.
- Moved CLI command execution dispatch into `cli/execute.rs` while preserving
  parsed argument and handler behavior. The next ready seam is context packet
  DTO extraction.
- Added the domain-owned `ContextPacket` DTO shape in `domain/context_packet.rs`
  without changing context packet JSON rendering. The next ready seam is wiring
  JSON context rendering through the DTO.
- Updated JSON context packet rendering to build from the domain `ContextPacket`
  DTO while preserving the emitted packet schema. The next ready seam is LSP
  context packet usage.
- Updated LSP context packet lookup to build finding packets through the domain
  `ContextPacket` DTO while preserving the emitted packet schema. The next
  ready seam is doc-hidden internal modules.
- Marked compatibility module exports as doc-hidden so generated Rust docs point
  new integrations at crate-root re-exports. The optional private-internals seam
  remains blocked behind an explicit breaking public API decision.
- Added `cargo xtask targeted-test-outcome` as an advisory receipt for comparing
  before/after `repo-exposure-json` artifacts. The report writes
  `target/ripr/reports/targeted-test-outcome.{json,md}`, matches seams by
  `seam_id`, summarizes grip-class movement, and keeps runtime mutation
  confirmation as a separate calibration step.
- Added `docs/TARGETED_TEST_WORKFLOW.md` to join repo exposure snapshots, LSP
  seam actions, targeted-test receipts, SARIF policy, badge artifacts, and
  mutation calibration into one operator loop for adding a focused test.
- Updated `ripr check --help` to list the repo seam, repo exposure, repo SARIF,
  and agent seam packet formats used by the targeted-test workflow.
- Extended `cargo xtask mutation-calibration` with advisory static/runtime
  agreement buckets, precision notes, static-only finding samples, and runtime
  gap signals that did not line up with a static gap.
- Added `fixtures/CALIBRATION_CORPUS.md` as a controlled-scenario index for
  targeted-test receipts, static/runtime calibration, SARIF, badges, and LSP
  alignment checks without changing fixture execution.
- Documented a copyable, non-blocking GitHub Actions recipe for rendering RIPR
  SARIF and uploading it to GitHub code scanning.
- Updated targeted-test outcome Markdown to show unchanged seams and their
  evidence deltas, so a receipt can show static evidence movement even when the
  grip class does not change.
- Added a boundary-gap targeted-test case study showing one focused test, the
  before/after receipt, and the current static evidence gap when the class stays
  `weakly_gripped`.

## 0.3.0 - 2026-05-02

### Added

- Added the syntax-backed analyzer foundation: `FileFacts`,
  `RustSyntaxAdapter`, parser-backed test/oracle extraction, stable owner
  symbols, and parser-backed predicate, return, error, field, match,
  side-effect, and call-change probes.
- Added the Evidence Quality foundation: unknown findings now carry explicit
  stop reasons, and oracle kind/strength is probe-relative for exact values,
  exact error variants, broad errors, snapshots, mock expectations, relational
  checks, smoke-only checks, and unknown oracles.
- Added fixture, golden, report, metrics, traceability, dogfood, test-oracle,
  report-index, receipt, golden-drift, critic, local-context, allow-attribute,
  supply-chain, and workflow-runtime automation for reviewable PR evidence.
- Added `tower-lsp-server` as the LSP framework and moved the sidecar to typed
  async handlers.
- Added LSP state and evidence surfaces: workspace-root selection from
  initialization, stale diagnostic clearing, refresh failure logging, document
  state tracking, saved-workspace refresh semantics, serialized refresh
  generations, stable diagnostic metadata, related test information,
  diagnostic-targeted context actions, and diagnostic hover details.
- Added CI and release hardening: coverage workflow, cargo-deny supply-chain
  checks, GitHub Dependency Review, Dependabot configuration, Node 24 workflow
  action/tooling updates, and Open VSX publishing through `OVSX_PAT`.

### Changed

- Reworked the README as a problem-first front door and moved detailed operating
  guidance into docs.
- Upgraded the Rust baseline to 1.93 and added high-signal Rust/Clippy lint
  gates.
- Split larger internal modules for CLI, domain, JSON output, and LSP sidecar
  responsibilities without changing the one-package public surface.

### Fixed

- Hardened unified diff parsing against multi-hunk, multi-file, malformed, and
  fuzz-like inputs.
- Expanded output, CLI, classifier, app mode, snapshot oracle, workspace
  selection, rustdoc, and LSP unit coverage.
- Improved golden snapshot drift diagnostics and normalized golden text
  comparison around trailing newlines.

## 0.2.0 - 2026-05-01

- First self-provisioning editor distribution path.
- Added `ripr lsp --stdio` and `ripr lsp --version`.
- Added VS Code/Open VSX server resolution:
  `ripr.server.path` -> bundled -> cached download -> verified first-run
  download -> PATH -> actionable error.
- Added GitHub Release server archives and a SHA-256 manifest used by the
  extension downloader.
- Published the universal VSIX and Open VSX extension.

## 0.1.0 - 2026-05-01

- First publishable alpha of `ripr`: static RIPR exposure analysis for
  Rust/Cargo workspaces.
