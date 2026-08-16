# Source-promotion verification

- Schema: ripr.source_promotion_verification.v2
- Status: **verified**
- J: `7fcb62a3433424dddadd1afb47025ee284c5755e`
- SOURCE_PARENT: `6cc5d6135593d9fb9a745eb215c5b0f92cbd14d5`
- Parents (ordered): `6cc5d6135593d9fb9a745eb215c5b0f92cbd14d5` then `83217e97ec6847db41d757f57279a8b1ca433fe6`
- Tree: `7a915ae9827358aab88eca6ddad746720cbe92a4`
- Preflight SHA-256: `sha256:ada462b092397e9798a8229b94e54598a48b2f082244e9e6de29d2217597f911`
- Resolution manifest SHA-256: `sha256:c03b424c89fdfd5a29e6e576cf9c2e3ed673d60c30c424b4dc86523cf8d265d6`

## Claim boundary

The receipt proves the exact two-parent Git graph, reviewed tree identity, ancestry denominators/digests, release-version identity and source-authoritative changelog bytes, and optional merged-main reachability. It does not adjudicate semantic conflict resolutions, product correctness, release readiness, publication, or K back-sync.

- MAIN_HEAD: `not supplied (post-merge reachability not_run)`
- MERGE_BASE: `36909460db013ed3a3238ee8b2fc3ccda1135c15`

## Swarm reachability

```json
{"all_reachable_count":859,"all_reachable_sha256":"sha256:d92e4a218aedae9937627a5d5cdae1ac5335caec0a67841cfe3762f8591d38a1","first_parent_count":859,"first_parent_ordered_sha256":"sha256:d92e4a218aedae9937627a5d5cdae1ac5335caec0a67841cfe3762f8591d38a1","verified_through_parent_2":true}
```

## Checks

```json
{"ancestry_and_digest":true,"caller_state_mutated":false,"head_is_declared_join":true,"main_reachability":"not_run","ordered_parents":true,"release_version_identity":true,"reviewed_tree":true}
```

## Failure reasons

```json
[]
```

## Invalidation rules

```json
["Changing the preflight bytes, resolution manifest, exact join, parent identities, reviewed tree, governed release-version identity, source-authoritative CHANGELOG.md bytes, or verified main invalidates this receipt.","A descendant repair commit is not the declared join and must be verified with a fresh exact head.","This receipt proves the exact Git graph, reviewed-tree identity, governed release-version identity, and source-authoritative CHANGELOG.md bytes only; it does not adjudicate conflicts, product correctness, release readiness, or publication."]
```

## Non-claims

```json
["No semantic conflict ruling or artifact adequacy claim.","No join construction, ref mutation, publication, release, or K back-sync verification."]
```

## Structured receipt

```json
{
  "checks": {
    "ancestry_and_digest": true,
    "caller_state_mutated": false,
    "head_is_declared_join": true,
    "main_reachability": "not_run",
    "ordered_parents": true,
    "release_version_identity": true,
    "reviewed_tree": true
  },
  "failure_reasons": [],
  "invalidation_rules": [
    "Changing the preflight bytes, resolution manifest, exact join, parent identities, reviewed tree, governed release-version identity, source-authoritative CHANGELOG.md bytes, or verified main invalidates this receipt.",
    "A descendant repair commit is not the declared join and must be verified with a fresh exact head.",
    "This receipt proves the exact Git graph, reviewed-tree identity, governed release-version identity, and source-authoritative CHANGELOG.md bytes only; it does not adjudicate conflicts, product correctness, release readiness, or publication."
  ],
  "join_head": "7fcb62a3433424dddadd1afb47025ee284c5755e",
  "main_head": null,
  "merge_base": "36909460db013ed3a3238ee8b2fc3ccda1135c15",
  "non_claims": [
    "No semantic conflict ruling or artifact adequacy claim.",
    "No join construction, ref mutation, publication, release, or K back-sync verification."
  ],
  "parents": [
    "6cc5d6135593d9fb9a745eb215c5b0f92cbd14d5",
    "83217e97ec6847db41d757f57279a8b1ca433fe6"
  ],
  "preflight_sha256": "sha256:ada462b092397e9798a8229b94e54598a48b2f082244e9e6de29d2217597f911",
  "release_metadata_surfaces": [
    "Cargo.toml",
    "crates/ripr/Cargo.toml",
    "Cargo.lock",
    "editors/vscode/package.json",
    "editors/vscode/package-lock.json",
    "CHANGELOG.md"
  ],
  "resolution_manifest_sha256": "sha256:c03b424c89fdfd5a29e6e576cf9c2e3ed673d60c30c424b4dc86523cf8d265d6",
  "schema": "ripr.source_promotion_verification.v2",
  "source_main": "6cc5d6135593d9fb9a745eb215c5b0f92cbd14d5",
  "status": "verified",
  "swarm_reachability": {
    "all_reachable_count": 859,
    "all_reachable_sha256": "sha256:d92e4a218aedae9937627a5d5cdae1ac5335caec0a67841cfe3762f8591d38a1",
    "first_parent_count": 859,
    "first_parent_ordered_sha256": "sha256:d92e4a218aedae9937627a5d5cdae1ac5335caec0a67841cfe3762f8591d38a1",
    "verified_through_parent_2": true
  },
  "tree": "7a915ae9827358aab88eca6ddad746720cbe92a4"
}
```
