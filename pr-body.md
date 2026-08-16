<!-- source-promotion: true -->
<!-- source-promotion-control: 675636061680c2a7602e27960d7409fc3f6c83cf -->

## Exact live-source J5

This fresh direct two-parent object replaces stale J2/#1558 after the source-owned contract repair and all later source-main movement.

```text
J5             = 7fcb62a3433424dddadd1afb47025ee284c5755e
SOURCE_PARENT  = 6cc5d6135593d9fb9a745eb215c5b0f92cbd14d5
SWARM_PARENT   = 83217e97ec6847db41d757f57279a8b1ca433fe6
JOIN_TREE      = 7a915ae9827358aab88eca6ddad746720cbe92a4
CONTROL        = 675636061680c2a7602e27960d7409fc3f6c83cf
PREFLIGHT_SHA  = ada462b092397e9798a8229b94e54598a48b2f082244e9e6de29d2217597f911
RESOLUTION_SHA = c03b424c89fdfd5a29e6e576cf9c2e3ed673d60c30c424b4dc86523cf8d265d6
```

## Reviewed movement

The reviewed J2 product tree remains the baseline. J5 carries the complete live source delta from `a072b7efe80f1a32d7b5ba7342559a114edeb12e` to `6cc5d6135593d9fb9a745eb215c5b0f92cbd14d5` through exact source copies or conflict-free three-way integration. Process policy is reconciled by literal owner and cannot widen a maximum implicitly.

The J2-to-J5 delta, source-delta receipt, process-policy receipt, fresh W7 preflight, complete `kind:key` manifest, trusted verification, and patch are retained in the construction packet. Frozen W7 is unchanged.

- `M	.github/workflows/source-promotion-contract.yml`
- `M	docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md`
- `M	policy/process_allowlist.txt`
- `M	xtask/src/command.rs`
- `M	xtask/src/tests.rs`
- `M	xtask/tests/source_promotion_workflow_contract.rs`

## Required merge transport

Use Create a merge commit.
Do not use Squash and merge.
Do not use Rebase and merge.

```bash
gh pr merge 1569 --repo EffortlessMetrics/ripr --merge --match-head-commit 7fcb62a3433424dddadd1afb47025ee284c5755e
```

Do not append a repair descendant to J5. Any exact-head defect or source-main movement rejects this object and requires a fresh direct join.

## Boundary

No version bump, changelog release entry, public tag, publication, signing, marketplace mutation, release-secret use, merge, or back-sync is included or authorized here.
