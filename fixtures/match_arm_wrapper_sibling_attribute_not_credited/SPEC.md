# Fixture: match_arm_wrapper_sibling_attribute_not_credited

Spec: RIPR-SPEC-0094

Owner: analysis-fixtures

Issue: #6297

## Given

The diff changes the `Unit::Fortnight` arm of `seconds`.
A sibling derive generates a constant callable that shadows the real wrapper in the test module. Source-level call names do not establish its identity.
The local Rust-only macro is deliberately opaque to the analyzer.

## When

```bash
cargo xtask fixtures match_arm_wrapper_sibling_attribute_not_credited
```

## Then

The finding must not be promoted to `exposed`. The unchanged bridge oracle
passes for either arm value after actual expansion, and the independent
honesty corpus refuses promotion without its own verified binding.

## Must Not

- Treat attribute-generated namespace or enum identity as parser-established.
- Claim runtime adequacy.
