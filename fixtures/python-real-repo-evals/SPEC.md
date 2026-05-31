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
and claim boundaries.

## Must Not

- Do not treat these records as support-tier promotion.
- Do not claim correctness, mutation adequacy, generated tests, provider calls,
  CI gate eligibility, or arbitrary runtime import execution.
- Do not hide missing normal pytest, API, CLI/tooling, or mixed-repo dogfood.
