# Provisional candidate exposes 24 Rust false-actionable rows

Subject: provisional 62234ac13b0f7a8ff7e30d239f59215b808775b3, tree ef7221a5c23a73670d23bc9998cfe1496b4e38f0; parent frozen b51aebc7e4735529befd677ebf14071c7f09e477. The only provisional delta is eight scope metadata files. Analyzer production source and all 1,698 scoring definitions are identical to b51. Thus this is an analyzer behavior observed on the provisional build, not a claimed executed b51 corpus result or a defect caused by the metadata repair.

The first full `xtask verdict-corpus check-all` scored all four corpora, then exited 1. Rust: 24/846 false verdicts, all false_actionable (24/433 discriminated truth); false_exposed and false_silent 0/413; contradictions 0/1035. Rust abstention 328/846. TypeScript 0/373, Python 0/385, Perl 0/94 false verdicts. No expectations, labels, baselines or ledgers have been blessed. Repeat/independent controls were still running when this first finding was published.

All 846 Rust definitions match prior main-tip 984c69a290486b1da9d91700a6f771045ab079e1 exactly. The historical zero-false result belongs to that prior source, not this candidate. The full measured report and all 24 rows are attached separately.

## Independent public-CLI reproductions

For each of `cell-0005-same-name-owner-tested`, `forwarder-hop-forwarded-hit`, and `spec0106-unwrap-err-exact-variant`, an independent direct CLI invocation after the full producer returned native status 0 and reproduced weakly_exposed/gap. Retained raw outputs and exact argument vectors are under reproductions/. CLI SHA256 is 84b87f91226965dbc4e91c7b09d880c0398b4154e474e3a7baec6d254ff8f0c4, version `ripr 0.11.0 (62234ac13b0f7a8ff7e30d239f59215b808775b3)`.

Example command from the existing materialized case:

```
/workspace/ripr-swarm/target/debug/ripr check --root /workspace/ripr-swarm/target/ripr/verdict-corpus/rust/cell-0005-same-name-owner-tested --diff /workspace/ripr-swarm/fixtures/rust-verdict-corpus/cases/cell-0005-same-name-owner-tested.diff --json
```

The retained test asserts `assert_eq!(price_quote(3).total, 36)` on retail::price_quote. Both independently labeled mutants of the changed constructed total fail that test. The current finding reports reach=yes, infect=yes, propagate=yes, observe=yes, but discriminate=weak: "Discriminator uncredited: strongest oracle reaches only through direct_owner_call linkage without sink-level evidence (identity_unresolved)". The forwarder and exact unwrap_err reproductions report the same demotion family. Fresh runtime replay is planned in this lane; retained runtime labels are not being described as fresh execution here.

## Bounded repair handoff to existing integration/identity owner

The common final demotion is in crates/ripr/src/analysis/classify/reveal.rs::reveal_outcome (near line 512): a Yes discriminator becomes Weak when `analysis.strongest_identity` is false. Its same-assertion identity construction is near lines 3150-3235; production admission receipts arrive from crates/ripr/src/analysis/classifier/evidence.rs. The direct lexical route requires a whole owner-call operand, so projected fields, forwarded results and bound exact-error results need the existing producer-authenticated identity routes to remain complete. These are leads from current source plus three executed reproductions, not a proven single-cause fix for all 24 rows.

Next repair should preserve the same-strongest-assertion identity rule and its proximity, shadowing, overridden method, sibling and self-computed negatives, while restoring each independently established semantic observer route. Do not remove the identity gate wholesale, credit every direct_owner_call relation, relabel discriminated cases, add accepted false rows, or turn them into silent/limited success by expectation changes. The existing full corpus is the 24-case regression denominator; use the listed three reproductions as the initial focused discriminator, plus their existing unreached/shadowed/nonliteral siblings as negative controls. #12010 and the integration owner own analyzer repair; this measurement lane leaves production source unchanged.

Disposition: unresolved_release_hold for these observed wrong-advice rows pending repair or explicit existing #2766 adjudication. No release-selection mutation, merge, remote CI, public source promotion or release action was performed. All five selection rows remain pending and hard-cut inactive.
