# Python Real-Repo Eval Corpus

Spec: RIPR-SPEC-0028

Related: RIPR-SPEC-0057

## Given

Python repair-routing dogfood runs can come from scratch repos or real repos
that are outside analyzer fixture goldens.

## When

An eval case records either a Python repair-routing receipt or a checked
static-limit no-action result. Repair cases record a Python check run, top
repair card, bounded agent packet, focused verify command, after check run, and
before/after outcome receipt. Static-limit cases record the changed owner,
typed limitation, stop reason, related test context when available, and the
reason no repair card, packet, verify command, or receipt movement is emitted.

## Then

The corpus records the repo shape, source kind, commands, runtime, canonical
gap ID, missing discriminator, suggested test target, agent packet command,
allowed files, forbidden files, stop conditions, verify result, receipt result,
gap movement, usability notes, false-positive notes, limitation notes,
structured unsupported limitation kinds, ranked top-3 repair-card findings, and
claim boundaries for repair cases. Static-limit cases live under
`static_limit_cases` and must prove the fail-closed side of the lane: no repair
card, no agent packet, `not_applicable` verify and receipt results, explicit
stop reasons, `no_receipt` gap movement, and preview/advisory claim boundaries.

The checked corpus must include at least one tiny controlled Python repo, one
normal pytest app repo, one parameterized-boundary pytest repo, one
CLI/output-style pytest repo, one Click-shaped CLI output pytest repo, one
CLI exit-code pytest repo, pytest and
unittest exception-path repos,
API-style status-code, JSON-field, and exception-response pytest repos, one
mixed Rust/Python pytest repo, one decorated route pytest repo, and one
data/model field pytest repo before metrics can support promotion discussion.

Dogfood quality metrics must report top-1 actionable usefulness, top-3
actionable precision over captured ranked repair-card findings, verify-command
validity, agent-packet boundary validity, concrete-discriminator coverage,
suggested test-location coverage, false-actionable rate, crash rate, receipt
closure rate, unsupported limitation distribution, and no-action static-limit
distribution. Eval cases with fewer than three user-facing repair cards must
include a ranked top-3 limit reason instead of silently passing the metric.

Static-limit no-action cases are not counted as successful repair
recommendations. They exist to keep unsupported Python shapes visible without
routing unsafe human or agent work. The checked corpus must include
dynamic-dispatch, decorator-indirection, missing-import-graph, metaprogramming,
mocked-module, opaque-custom-helper, property-based, and unresolved-fixture
no-action examples before any broader support-tier discussion treats these
limits as measured instead of anecdotal. Post-promotion stability evals also
record unsupported-syntax no-action cases so syntax gaps stay fail-closed
instead of becoming implied repair work.

## Must Not

- Do not treat these records as support-tier promotion.
- Do not claim correctness, mutation adequacy, generated tests, provider calls,
  CI gate eligibility, or arbitrary runtime import execution.
- Do not hide unsupported dynamic routing or missing metrics evidence.
- Do not count static-limit no-action cases as closed repair cards or queueable
  packets.
