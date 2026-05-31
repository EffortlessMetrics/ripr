# Python Real-Repo Eval Corpus

Spec: RIPR-SPEC-0028

Related: RIPR-SPEC-0057

## Given

Python repair-routing dogfood runs can come from scratch repos or real repos
that are outside analyzer fixture goldens.

## When

An eval case records a Python check run, top repair card, focused verify
command, after check run, and before/after outcome receipt.

## Then

The corpus records the repo shape, source kind, commands, runtime, canonical
gap ID, missing discriminator, suggested test target, verify result, receipt
result, gap movement, usability notes, false-positive notes, limitation notes,
structured unsupported limitation kinds, and claim boundaries.

The checked corpus must include at least one tiny controlled Python repo, one
normal pytest app repo, one CLI/output-style pytest repo, one API-style
status-code pytest repo, one mixed Rust/Python pytest repo, and one decorated
route pytest repo before metrics can support promotion discussion.

Dogfood quality metrics must report top-1 actionable usefulness, verify-command
validity, concrete-discriminator coverage, suggested test-location coverage,
false-actionable rate, crash rate, receipt closure rate, unsupported limitation
distribution, and an explicit `not_measured` state for top-3 precision until
ranked top-3 eval capture exists.

## Must Not

- Do not treat these records as support-tier promotion.
- Do not claim correctness, mutation adequacy, generated tests, provider calls,
  CI gate eligibility, or arbitrary runtime import execution.
- Do not hide unsupported dynamic routing or missing metrics evidence.
