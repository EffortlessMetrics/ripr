# Candidate execution-scope fixture

Spec: RIPR-SPEC-0145

## Given

The accepted 0.11 release scope excludes the merged verification-execution
surface from the candidate while development `main` remains unchanged.

## When

`cargo xtask release-scope --input fixtures/release_scope/accepted-outcome-a.json`
reconciles the named commit, candidate parent, and path inventory.

## Then

The report is `ready` only when the complete #2396 path set is excluded,
preserved static-assurance paths remain outside that exclusion, and #2332 stays
open for undelivered execution acceptance.

## Must Not

- claim that the candidate tree was constructed by this report;
- claim verification execution, receipt issuance, qualification, or publication;
- remove or rewrite development `main` history;
- treat a partial or stale path inventory as ready.
