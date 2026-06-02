# Python Real-Repo Eval Corpus

Spec: RIPR-SPEC-0028

Related: RIPR-SPEC-0057

## Given

Python repair-routing dogfood runs can come from scratch repos or real repos
that are outside analyzer fixture goldens.

## When

An eval case records a Python repair-routing receipt, a checked static-limit
no-action result, or a checked ordinary no-action result. Repair cases record a
Python check run, top repair card, bounded agent packet, focused verify command,
after check run, and before/after outcome receipt. Static-limit cases record
the changed owner, typed limitation, stop reason, related test context when
available, and the reason no repair card, packet, verify command, or receipt
movement is emitted. Ordinary no-action cases record non-limitation routing
stops such as no related static test path, already-observed exact evidence, or
heuristic-only related-test proximity.

## Then

The corpus records the repo shape, source kind, commands, runtime, canonical
gap ID, missing discriminator, suggested test target, agent packet command,
allowed files, forbidden files, stop conditions, verify result, receipt result,
gap movement, usability notes, false-positive notes, limitation notes,
structured unsupported limitation kinds, ranked top-3 repair-card findings, and
claim boundaries for repair cases. Static-limit cases live under
`static_limit_cases` and ordinary no-action cases live under `no_action_cases`.
Both buckets must prove the fail-closed side of the lane: no repair card, no
agent packet, `not_applicable` verify and receipt results, explicit stop
reasons, `no_receipt` gap movement, and preview/advisory claim boundaries.

The checked corpus must include at least one tiny controlled Python repo, one
normal pytest app repo, one parameterized-boundary pytest repo, one
CLI/output-style pytest repo, one Click-shaped CLI output pytest repo, one
Typer-shaped CLI output pytest repo, one CLI exit-code pytest repo, pytest and
unittest exception-path repos,
API-style status-code, JSON-field, Flask-style JSON-field, and
exception-response pytest repos, one mixed Rust/Python pytest repo, one
decorated route pytest repo, and one data/model field pytest repo before
metrics can support promotion discussion.

Dogfood quality metrics must report top-1 actionable usefulness, top-3
actionable precision over captured ranked repair-card findings, verify-command
validity, agent-packet boundary validity, concrete-discriminator coverage,
suggested test-location coverage, false-actionable rate, crash rate, receipt
closure rate, unsupported limitation distribution, and no-action static-limit
distribution plus ordinary no-action distribution. Eval cases with fewer than
three user-facing repair cards must include a ranked top-3 limit reason instead
of silently passing the metric.

Static-limit no-action cases are not counted as successful repair
recommendations. They exist to keep unsupported Python shapes visible without
routing unsafe human or agent work. The checked corpus must include
dynamic-dispatch, decorator-indirection, missing-import-graph, metaprogramming,
mocked-module, opaque-custom-helper, property-based, and unresolved-fixture
no-action examples before any broader support-tier discussion treats these
limits as measured instead of anecdotal. Post-promotion stability evals also
record unsupported-syntax no-action cases so syntax gaps stay fail-closed
instead of becoming implied repair work.

Ordinary no-action cases are also not counted as successful repair
recommendations. The checked corpus must include `no_related_test`,
`already_observed`, and `heuristic_only` examples so no-path evidence,
already-strong evidence, and uncertain related-test proximity stay visible
without inflating repair-card or packet success metrics.

## Must Not

- Do not treat these records as support-tier promotion.
- Do not claim correctness, mutation adequacy, generated tests, provider calls,
  CI gate eligibility, or arbitrary runtime import execution.
- Do not hide unsupported dynamic routing or missing metrics evidence.
- Do not count static-limit no-action cases as closed repair cards or queueable
  packets.
- Do not count ordinary no-action cases as closed repair cards or queueable
  packets.
