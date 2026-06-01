# Python Real-Repo Eval Corpus

Spec: RIPR-SPEC-0028

Related: RIPR-SPEC-0057

## Given

Python repair-routing dogfood runs can come from scratch repos or real repos
that are outside analyzer fixture goldens.

## When

An eval case records a Python check run, top repair card, bounded agent
packet, focused verify command, after check run, and before/after outcome
receipt.

## Then

The corpus records the repo shape, source kind, commands, runtime, canonical
gap ID, missing discriminator, suggested test target, agent packet command,
allowed files, forbidden files, stop conditions, verify result, receipt result,
gap movement, usability notes, false-positive notes, limitation notes,
structured unsupported limitation kinds, ranked top-3 repair-card findings, and
claim boundaries.

The checked corpus must include at least one tiny controlled Python repo, one
normal pytest app repo, one CLI/output-style pytest repo, pytest and unittest
exception-path repos, API-style status-code, JSON-field, and exception-response
pytest repos, one mixed Rust/Python pytest repo, and one decorated route pytest
repo before metrics can support promotion discussion.

Dogfood quality metrics must report top-1 actionable usefulness, top-3
actionable precision over captured ranked repair-card findings, verify-command
validity, agent-packet boundary validity, concrete-discriminator coverage,
suggested test-location coverage, false-actionable rate, crash rate, receipt
closure rate, and unsupported limitation distribution. Eval cases with fewer
than three user-facing repair cards must include a ranked top-3 limit reason
instead of silently passing the metric.

## Must Not

- Do not treat these records as support-tier promotion.
- Do not claim correctness, mutation adequacy, generated tests, provider calls,
  CI gate eligibility, or arbitrary runtime import execution.
- Do not hide unsupported dynamic routing or missing metrics evidence.
