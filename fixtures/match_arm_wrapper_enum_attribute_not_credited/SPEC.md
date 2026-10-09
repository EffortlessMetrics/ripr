# Fixture: match_arm_wrapper_enum_attribute_not_credited

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #6297

## Given

The diff changes the `Unit::Fortnight` arm of `seconds`.
An item attribute replaces the owner enum and makes Fortnight an associated constant naming Week. The source-level path therefore cannot establish actual arm selection.
The local Rust-only macro is deliberately opaque to the analyzer.

## When

```bash
cargo xtask fixtures match_arm_wrapper_enum_attribute_not_credited
```

## Then

The finding must not be promoted to `exposed`. The unchanged bridge oracle
passes for either arm value after actual expansion, and the independent
honesty corpus refuses promotion without its own verified binding.

## Must Not

- Treat attribute-generated namespace or enum identity as parser-established.
- Claim runtime adequacy.
