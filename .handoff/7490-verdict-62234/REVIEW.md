# Scope-metadata repair review

Reviewed provisional commit: 62234ac13b0f7a8ff7e30d239f59215b808775b3. Tree: ef7221a5c23a73670d23bc9998cfe1496b4e38f0. Parent: b51aebc7e4735529befd677ebf14071c7f09e477.

Disposition: source change acceptable for provisional measurement; final integration/qualification remains with the central owner. This is not a merge or release clearance.

## What is repaired

Eight TOML files change, exactly the cross product of four language verdict corpora and RIPR-SPEC-0374/0375. Each changes `scope = "out"` to `unmeasured = true` and gives the factual reason that the current acceptance examples are unnumbered prose. No case, label, retained subject, expected row, classifier, producer, threshold or acceptance text changes. git diff --check passes. Four existing language validate commands passed on these exact patched bytes before commit; fresh committed measurement is in progress.

The semantic owner is xtask/src/reports/verdict_corpus_coverage.rs. `read_spec_scopes` accepts the mutually exclusive scope/unmeasured variants. `coverage_violations` rejects an out/in scope spec with zero numbered examples, and rejects an unmeasured spec once numbered examples exist. `SpecExampleCoverage.unmeasured_specs` and render_markdown explicitly disclose the spec IDs. RIPR-SPEC-0219's "Spec-example coverage" section documents this route. Existing precedents include RIPR-SPEC-0395 (normative prose contract) and RIPR-SPEC-0420 (xtask validator contract with separate tests).

## Why this does not waive accepted obligations

Neither spec is an analyzer-verdict truth case: 0374 governs native npm bootstrap packaging/admission, installed routes and staging; 0375 governs source-promotion verification identities, ordered parents, metadata and deterministic rejection. Their normative Behavior, Required Evidence, Acceptance Examples and Test Mapping remain byte-identical. The packaging/admission controls in .github/scripts/test_npm_package.py and npm_consumer.py, workflow dist-tag tests, source_promotion_verify.rs tests and command entrypoint tests remain their required proof owners. This patch supplies no evidence that those controls passed and grants no publication or source-promotion permission.

On frozen b51 the specs contain eight and four bullet examples, respectively, and zero machine-numbered example IDs. Thus the eight invalid `scope = "out"` entries could not represent measured numbered examples. Reporting both names as unmeasured is an explicit admission of corpus nonmeasurement; it cannot count them covered or silently satisfy their obligations. Existing numbered verdict-example covered/total counts and runtime-case denominators stay unchanged. The out-of-scope spec count falls by two and the named unmeasured list grows by two per language; these category changes must be disclosed rather than treated as increased coverage. The whole-corpus validity failure itself remains retained against b51.

## Alternative: number the real examples

Replacing the two specs' bullet markers with stable ordered example numbers is also valid. It would create 12 addressable example IDs and allow keeping the eight out-of-scope entries for their original domain-specific reasons. Those examples would remain excluded from verdict coverage; numbering alone would neither add a runtime-mutant case nor prove the packaging/promotion obligations. That alternative changes normative citation identities in two source-authoritative specs and would need their owners to maintain the numbering and references. The chosen patch records the specs as they currently exist using the documented prose representation, without rewriting them as part of a measurement repair. No requirement prevents the spec owners numbering them later. If they do, the current validator rejects `unmeasured` and requires a renewed explicit scope decision; this cannot permanently conceal future numbered acceptance debt.

## Strongest counterexamples and limits

- Deleting the metadata entries would fail when numbered examples are later introduced and could omit today's visible nonmeasurement; we retain named entries instead.
- Relabeling cases, removing adverse rows or changing accepted verdicts would change scoring populations; the exact eight-file diff does none of these. Definition audit and inventory checks bind this independently.
- Treating the validator's accepted metadata as acceptance of either release contract would be false. Packaging, trusted source-promotion checks and public #1893/#1889 holds remain independent and unchanged.
- Replacing existing in-scope numbered coverage with unmeasured would weaken coverage and be rejected by the validator. Here there are no numbered examples, and prior intent was already outside analyzer verdict scope.

Remaining proof: full fresh corpora, stable repeat, comparable/origin/provenance/abstention figures, integrity challenges and trust gate. Provisional results may be carried forward only after the integration owner reconciles actual source, producer, corpus and semantic identities. All five selection rows remain pending and hard-cut inactive.

## Public-source evidence transport

Transport is an evidence-only .handoff branch on EffortlessMetrics/ripr, parent d731a19bf13e5a5c2257d3c320f734f65ccd9894. All 25 checked-in workflow event mappings were inspected before push (public-workflow-events.txt). Push workflows restrict branches to main/master or tags to v*. No workflow declares create. This handoff branch matches none; no PR, dispatch, tag or release is requested. No workflow/settings/security file changes. This is a handoff carrier, not public source integration, candidate selection or a publication claim.
