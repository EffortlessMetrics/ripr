# Fixture: owner_return_pin_cfg_attr_module

Spec: RIPR-SPEC-0197

## Given

The return of `weight(input)` changes from `input * 2` to `input * 3`.
The `cfg_attr_module` control exercises assertion admission reviewed in PR #5020.

## When

The public analyzer checks this diff and the exact test source is compiled
against correct and deliberately wrong libraries.

## Then

One return-value finding reads `reachable_unrevealed`, Observe `no`,
Discriminate `no`, with no credited oracle. The actual harness executes
one test and passes for either library; an uncollected assertion or
unrelated unknown helper does not discriminate the changed return.

## Must Not

- Treat a nested or unconditionally cfg-disabled test item as collected.
- Manufacture a singleton fallback by removing a refused assertion.
- Accept a compile failure or zero executed tests as a runtime witness.

Raw configuration attribute identifiers are intentionally non-confirming in the
availability query; Rust accepts these spellings and excludes the gated test.
