# PR body guidance

Explain the current contribution, not the process used to produce it. A reviewer
needs the concrete problem or requested capability, the change and its causal
reason, the acceptance boundary, and sufficient evidence to judge the result.
Preserve relevant existing behavior and state material compatibility, support,
policy, migration or non-obvious rollback effects. Keep linked authority where
it helps; retain any fields actually required by the destination.

## Choose the useful shape

There is no mandatory heading set or length gate. A small manifest correction
can use two paragraphs and its test command. A cross-cutting behavior change
may benefit from a compact verification table and a reading-order review map.
Use those structures when they help compare outcomes or navigate the change,
not to restate the same explanation three times. A review map describes where
the important decisions occur, not every file changed.

Start with the actual trigger and consequence, or the new capability and
preserved default. Include the mechanism when it supplies the shortest complete
causal explanation. Do not inflate the finding into generic reliability or
security language. Test-only work must not imply production behavior changed.

## Match evidence to the claim

Keep the decisive before/after result or negative control. A regression test offered
as repair evidence must distinguish the defect. Preservation tests may pass on
the base by design. Selection and discrimination are different obligations:
confirm both that the intended case ran and that the oracle notices a violation.
A source-level safety argument is not an observed crash. A passing helper test
is not end-to-end execution.

Inspectable worker or CI evidence can support direct result statements after
its subject, provenance, command, environment and applicability are checked.
A bare report is not sufficient. Do not claim personal execution that did not
occur or add repeated 'reported' qualifiers solely because work was delegated.
State a missing check only where it materially limits the claim. Preserve the
original identity of carried-forward evidence and actual exact-head requirements.

## Preserve the useful text

Edit inaccuracies, material ambiguity, repetition or stale claims. Preserve
effective author wording, causal order, referents, attribution, local uncertainty
and meaningful voice. Do not rewrite another contributor into a house voice,
add promotional framing or invent personality. A changed test count normally
requires a small patch, not a new description.

The body is durable explanation, not a rolling CI or investigation log. Keep
ordinary findings and replies in their threads and required structured review
records on their existing surfaces. Do not copy the review checklist into the
body. Delete redundant narration rather than moving it into more comments.
If the current candidate does not establish its advertised guarantee, correct
that representation and retain the repair requirement. Prose cannot substitute
for repair or an authorized change to acceptance.

Before writing, reload the body and head, reconcile intervening changes and
preserve newer valid text. Use supported conditional writes where available;
read back the saved body and head and check rendering. Separate reads and writes
are not atomic. Keep staging-only coordination out of an external contribution
and honor publication/reference boundaries. This guidance grants no new authority.

## Maintenance

The [contribution evaluation](../agent-context/pr-contribution-evaluation.md)
contains maintenance-only scenarios and scoring. It is not an additional gate
for ordinary PRs or a claim that model behavior has been independently tested.
