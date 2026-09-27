# First Useful Action Fixture Corpus

These files pin the Campaign 22 first-useful-action routing corpus for
`RIPR-SPEC-0020`.

They are static fixture artifacts. They do not implement `ripr first-action`,
edit source, generate tests, call a provider, run mutation testing, rerun
hidden analysis, invent policy, or change CI blocking behavior.

Files:

- `corpus.json` records PR-shaped input states and expected first-action
  routing results for the bounded statuses in RIPR-SPEC-0020.
- `<case>/first-useful-action.json` and `<case>/first-useful-action.md` pin the
  expected report output for each route.
- `unchanged-after-attempt/{before,after}.repo-exposure.json`,
  `unchanged-after-attempt/agent-verify.json`,
  `unchanged-after-attempt/assistant-proof.json`, and
  `unchanged-after-attempt/agent-receipt.json` are dedicated negative-control
  inputs. The repo-exposure files are portable normalized goldens of the real
  `ripr check --root fixtures/boundary_gap/input --format repo-exposure-json`
  output: the corpus test requires the producer to emit the canonical absolute
  fixture root before normalizing only root, revision, worktree currentness,
  and the corresponding content commitment for checkout-independent storage.
  The full semantic `evidence_record` is retained and protected by the
  canonical placeholder-hash algorithm. The verify artifact and receipt bind
  the exact normalized snapshot bytes and their empty evidence delta. They intentionally
  remain weak/unchanged when the canonical boundary-gap journey advances to
  improved evidence. Because the receipt is portable-normalized, it carries no
  complete analysis outcome, so `ripr first-action` fails closed on it with
  `missing_required_artifact` rather than routing `unchanged` movement.
- `<case>/inputs/` holds the committed artifacts a case renders from when the
  report names a `target/ripr/...` input path (`stale`, `baseline-only`,
  `no-actionable-seam`). `cargo xtask dogfood` copies each input to the path
  the report names, runs `ripr first-action`, and compares the produced JSON
  and Markdown byte for byte to the goldens here.

The corpus intentionally covers:

- actionable PR-local weak seam;
- fresh PR whose review card carries the repair start (`repair-start/`): the
  report leads with the card's `llm_guidance.repair_command` verbatim instead
  of asking for post-repair assistant proof. The card names that command only
  past the fail-closed repair-packet flip; first-action never derives it, and
  the same card without it keeps the missing-required-artifact route;
- stale evidence;
- missing required artifact;
- baseline-only debt;
- acknowledged item;
- waived item;
- suppressed item;
- no actionable seam;
- already-improved receipt state;
- unchanged-after-attempt receipt that is not promotable, so the route fails
  closed.

Case directories:

- `actionable/`
- `repair-start/`
- `stale/`
- `missing-required-artifact/`
- `baseline-only/`
- `acknowledged/`
- `waived/`
- `suppressed/`
- `no-actionable-seam/`
- `already-improved/`
- `unchanged-after-attempt/`

Each case pins the expected status, action kind, audience, selected seam,
target, routing reason, fallback state, command expectations, and static
limits. The report producer, generated CI projection, editor status projection,
and dogfood receipt checks all use this corpus as the regression contract.

`cargo xtask dogfood` also treats the following cases as repo-local first-action
receipts:

- `actionable/`
- `baseline-only/`
- `stale/`
- `missing-required-artifact/`
- `unchanged-after-attempt/`
- `no-actionable-seam/`

Each of those cases renders through the shipped `ripr first-action` route, so
a router change that alters any byte of these goldens fails the gate. They remain advisory and do not rerun hidden
analysis, edit source, generate tests, call providers, run mutation testing,
invent policy, or change CI blocking.
