# Exact reviewed-tree qualification evidence (ripr#1507)

Immutable evidence that one exact reviewed source/W7 tree behaves as the
intended 0.11.0 source-integrated product. This directory is **evidence only**.
It constructs no join, moves no ref, and authorizes no release.

## Exact qualified identities

```text
SOURCE_PARENT       = ad291d1bc936d00847d9712d2adf9ea56ca19533
SWARM_PARENT        = 83217e97ec6847db41d757f57279a8b1ca433fe6
SWARM_REF           = refs/tags/ripr-release-0.11.0-83217e97ec6847db41d757f57279a8b1ca433fe6
MERGE_BASE          = 36909460db013ed3a3238ee8b2fc3ccda1135c15
JOIN_TREE           = 7d592c00142f850cf08d52baa4c04398870d50a2
PREFLIGHT_SHA256    = 293b0b1b441068ba84f780500527743b5a7965e150b307be5850846bd518a631
MANIFEST_SHA256     = 1a490e51b35173bf39240c20cce44fb316cbe7c1d61ed30df8e4ada6fdb54ea3
VALIDATION_SHA256   = 1381eeceb276b29f46cb027286e335daaf40b818ad6e897961dcbfa62b227c6b
ADMISSION_INDEX     = 41cec008e2460690d710d738caae2e8525a667f8a14c2bef0fc7bf2c625849eb
ADMISSION_RECEIPT   = 0cfbec5d94ac0542e5f81fc69d2355b0d3941a4dca8c2e76a51f76e3bb57f07f
QUALIFICATION_SHA256= d5b3bddd25ea2bf8b47ef59b4e31c0fb3684710b44352f1acf5652c3f2bf2856
```

## The eight required lanes

`construct-exact-join` validates `ripr.source_promotion_tree_qualification.v1`
for terminal `qualified` status, exact identity, `promotion_ref_mutation_attempted
== false`, empty `failure_reasons`, binding to the admission evidence, and
exactly these eight lanes in this order, each `passed` with a 64-hex
`evidence_sha256`. Every digest below is the SHA-256 of the retained lane file in
`lanes/`, so no lane can be marked passed without bytes behind it.

| lane | evidence |
| --- | --- |
| `editor_package_linux` | `0f8ada8ac07aab25df2b6f813230d762ecb46cf09bf918ee7f9514e8a3bd0011` |
| `editor_package_windows` | `fab588e50699e78f973c669706c3afaf6b78084a3b1d1f46bcc5825639856b08` |
| `rust_product` | `b3a623383b47d4e72967b3eb0088e24724b8d18ed5992c28eeb3759ea7979e6c` |
| `source_governance` | `1c955e45cbe714364f521598e511eff1e6cd10666b61909344de776f2b3b9947` |
| `source_survivors` | `cc605208f4fb3b840ccb62e10765f5bcb0d1613166b5eaa153b94db938d42dc0` |
| `trusted_product_journeys` | `082c3d3a15c3e09e41a00789fd78ec40a2063656b61725417ccc95f7c1b2ac0c` |
| `untrusted_workspace_contract` | `a3bb30c0142052b16f3fd8bb1f0e24a878ecb6ab5b14280a44e72e7af84ac1cb` |
| `w7_product` | `87ca20435545ebc5b60a833b11a606d78a918885106cf6bd821638f2398f0260` |

## Hosted proof

Run `33257751087`, both `ubuntu-latest` and `windows-latest` terminal success.
The tree is materialized from `refs/qualify/join-tree-7d592c00`, a ref pointing
directly at the tree object, under a runner-local carrier commit that is never
pushed and never tagged. Every job re-derives the tree id after checkout and
again after every gate.

```text
VSIX    editors/vscode/dist/ripr-0.10.1.vsix   769070 bytes on both platforms
        linux  d4ca97d1167fe0c9a450c8e7eedfbb3d05f81efd8752f5acb4d4d09398c7fdd6
        win    f7d23cfca616ca4a6e4e79495066efe6ef5ca9b2b4c1e0210cfc34a5d9bc7eed
server  ripr 0.10.1
        linux  a6aaa2b8c35757cc2861d3b80b1b2543f9944594f45c997453815fc03beef96c
        win    e9875237c219ca8b88b77d4726fd36d27029b49c1758469a7adb773a0c9c9e62
```

## Trusted controller chain

Admission cross-checks the trusted builder's `executable_sha256` against the
validation receipt's `trusted_checker.executable_sha256`, so the controller was
built **before** validation and never rebuilt afterwards:

```text
cargo build --locked -p xtask     CARGO_TARGET_DIR outside the source checkout
executable  2240a3753a6028c4b4455771dccca0d6b7716393abf2c4090aa259bee0ef6675
Cargo.lock  fadd84304d7936ca6c611228892face090e5d9d63b820e5e245055865f645241
toolchain   rustc 1.95.0 (59807616e 2026-04-14)

trusted-builder        status = built
validate-resolved-tree status = validated, 13/13 required commands passed
admit-resolved-tree    status = admitted
                       constructor_eligible_after_tree_qualification = true
                       all_required_typed_integration_receipts_present = true
```

## Two evidence defects this qualification found and fixed

1. **`npm run test` and `npm run test:e2e` are the same command.** `vscode_test()`
   delegates straight to `vscode_test_e2e()`, so running both executed the
   identical 131-test suite twice per platform. Reporting those as two lanes
   would have double-counted one run.

2. **The untrusted contract was the skipped test.** The single `pending` test in
   the trusted run is `untrusted host keeps an actionable repair packet out of
   the clipboard`, which self-skips unless `RIPR_TEST_WORKSPACE_TRUST=untrusted`.
   The packaged untrusted contract this issue requires was therefore never
   exercised, and would have been claimed from a test that did not run.

   The duplicate run is now an untrusted-host run, guarded by a step that fails
   if that journey is absent from the log or self-skips again. Both platforms
   now record it executing:

   ```text
   ✔ untrusted host keeps an actionable repair packet out of the clipboard
   124 passing, 8 pending          (untrusted host)
   131 passing, 1 pending          (trusted host)
   ```

   The complements are exact: the 8 pending in untrusted mode are the
   trusted-only journeys, and the 1 pending in trusted mode is this untrusted
   journey. Together the two runs cover both hosts; neither alone does.

## Non-claims

This evidence proves product, editor, package and source-governance behavior for
one exact reviewed tree, and that the admission chain accepts it. It does not
construct the exact direct J6 (ripr#1508), transport it (ripr#1465), change
release metadata, or authorize any publication. No ref, tag, branch, or remote
was mutated: `promotion_ref_mutation_attempted` is `false` and the hosted jobs
prove no remote ref points at their runner-local carrier.
