# Public command hierarchy

This is the current human-facing task map for RIPR. It keeps the public entry
points distinct while the full typed command and workflow catalog is completed
under #1613.

| User task | Primary command | Boundary |
| --- | --- | --- |
| Diagnose setup | `ripr doctor` | Checks whether the workspace can produce evidence and gives bounded recovery. It is not required before every run. |
| Inspect one change | `ripr check --base origin/main` | Ordinary first value: analyze the selected diff and name the top gap or an honest no-action/limited state. |
| Adopt RIPR in a repository | `ripr pilot --root .` | Guided repository analysis and materialization. It is broader than the ordinary one-change check. |
| Repair one named gap | `ripr agent repair --seam-id <id> --phase before`, then `--phase after` | RIPR owns the before/after evidence plumbing. A human or external agent owns the focused test edit. |
| Compose PR evidence | `ripr first-pr --root . --base origin/main --head HEAD` | Composes existing artifacts into the start-here packet. It does not run analysis or repair a gap. |
| Adopt advisory CI | `ripr init --ci github` | Writes the non-blocking GitHub workflow. Blocking policy remains a later explicit repository decision. |
| Inspect advanced commands | `ripr help --all` | Complete reference for policy, reports, compatibility, and operator surfaces. |

## Repair transaction

The ordinary repair sequence is:

```bash
ripr agent repair --root . --seam-id <id> --phase before
# edit one focused test outside RIPR
ripr agent repair --root . --seam-id <id> --phase after
```

The lower-level `agent start`, `brief`, `packet`, `verify`, `receipt`, `status`,
and `review-summary` commands remain available for explicit control,
compatibility, and debugging. They are not the first-hour repair path.

## Drift rule

Top-level help, exhaustive help, the root README, Quickstart, agent help, and
editor onboarding should preserve the task boundaries above. Detailed flags
belong in per-command help rather than being copied into every document.

This document is descriptive guidance, not execution authority. #1613 will
replace prose-only coordination with a typed, schema-versioned command and
workflow catalog.

## Non-claims

This hierarchy does not rename or remove commands, add automatic edits, execute
mutation testing, strengthen gate authority, or prove real-repository usability.
RIPR remains static and advisory.
