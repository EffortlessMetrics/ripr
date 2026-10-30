# Proposed source-promotion evidence contract

This is a proposed repository-owned policy, not a description of GitHub branch
protection or a live admission rule. Its complete proposed check denominator is:

| Check name | Source workflow and job | Proposed role |
| --- | --- | --- |
| `Ripr Rust Small Result` | `.github/workflows/routed-rust.yml`, `result` | Ready-triggered routed proof for the exact PR subject. |
| `rust` | `.github/workflows/ci.yml`, `rust` | Full CI proof. The current job requires `full-ci` or `release-check` on PRs. |
| `cargo-deny` | `.github/workflows/security.yml`, `cargo-deny` | Dependency and advisory proof. |
| `dependency-review` | `.github/workflows/security.yml`, `dependency-review` | Pull-request dependency review. |
| `Source Promotion Contract` | `.github/workflows/source-promotion-contract.yml`, `promotion-contract` | Source promotion contract result, including its exact-head verifier. |

These five entries are a proposed complete denominator for the *new promotion
policy*. `docs/CI.md` names the routed result and says other required checks
include `rust`, but does not establish an exhaustive list. The security jobs
and promotion contract are additional proposed requirements based on their
checked-in workflows, not a claim that GitHub requires them. No repository or
branch settings query is needed to define or use this policy. A maintainer must
review the roster, triggering rules and independent reviewer policy, and the
approved contract must be present in the trusted current source base, before a
consumer can call it authoritative. A missing, skipped, cancelled, neutral,
wrong-subject, ambiguous, or unavailable required result is not a pass.

The promotion-contract job cannot require its own completion to finish: its
receipt must identify the four prerequisite results and a later external
admission check must also establish the `Source Promotion Contract` result.
Neither a green job nor matching workflow filenames proves the run used an
approved workflow revision. Current check/run APIs expose head, run, job and
workflow path; the hosted job also receives `github.workflow_sha` and
`job.workflow_sha`. A value written by an untrusted workflow is only a claim.
An actual authority needs an authenticated provenance receipt bound to the
exact repository, PR, base, head, workflow path and revision, run ID, job ID,
run attempt, subject and result, plus an approved producer at the current
source base. GitHub artifact attestations are one possible mechanism, but the
repository does not currently emit such a receipt for these jobs; signer,
predicate and artifact identity would have to be verified. Do not infer a
workflow revision from an equal base/head file alone or from the ordinary run
path.

An independent review must name the exact promotion head, be from someone other
than the PR author, and satisfy an explicit repository-owned eligibility policy.
Read all bounded review and timeline pages twice. A dismissal event cannot be
silently treated as a comment or as a fresh approval. Until an approved policy
and authenticated event reconciliation establishes a final eligible approval
without unresolved changes, reject. Missing eligibility, review, dismissal or
currentness evidence is unknown. Two reads can detect changes but are not an
atomic authorization; recheck the exact head and base at the protected merge
boundary and use the normal expected-head guard. This proposal grants no merge,
release, publication, or credential authority.
