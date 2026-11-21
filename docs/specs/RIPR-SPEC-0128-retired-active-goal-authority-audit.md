# RIPR-SPEC-0128: Retired active-goal authority audit placeholder

Status: deprecated

Owner: repository maintainers

Created: 2026-07-28

Linked issues:

- #2638
- #1701

## Problem

RIPR-SPEC-0128 was previously associated with the active-goal-authority-audit
framework removed by #1701. The framework is no longer a repository contract,
but the identifier remains part of the historical specification sequence.

Without an explicit record, the missing number can look like an accidental
omission or an available identifier for unrelated work.

## Behavior

This document reserves RIPR-SPEC-0128 as a deprecated historical identifier.
It defines no current product behavior, implementation surface, or acceptance
contract. Future specifications must use the next available identifier rather
than reusing 0128.

## Required Evidence

- The deprecated identifier is present in the specification index.
- The traceability ledger records this document as a deprecated artifact.
- The retired status and historical reason remain visible to future authors.

## Non-Goals

- Do not restore the removed active-goal-authority-audit framework.
- Do not add a runtime validator for contiguous specification numbering.
- Do not treat this tombstone as evidence of a supported product capability.

## Acceptance Examples

1. `docs/specs/README.md` lists RIPR-SPEC-0128 between 0127 and 0129 with a
   deprecated status.
2. A reader can identify #1701 as the reason the identifier has no active
   contract.
3. The next specification allocation remains above 0143; 0128 is not reused.

## Test Mapping

No executable test applies to a deprecated documentation-only identifier.
`check-spec-format` validates the document shape, `check-spec-numbering`
validates the index link, and `check-traceability` validates the manifest and
paths. These gates do not enforce that a future edit retains the `deprecated`
lifecycle status; it is intentionally recorded as human source truth here
rather than claimed as an enforced metric.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `docs/specs/RIPR-SPEC-0128-retired-active-goal-authority-audit.md` | historical tombstone and reservation explanation |
| `docs/specs/README.md` | indexed specification identifier and status |
| `.ripr/traceability.toml` | source-of-truth registration for the deprecated artifact |

## Metrics

No runtime metrics apply to this deprecated documentation-only identifier.
