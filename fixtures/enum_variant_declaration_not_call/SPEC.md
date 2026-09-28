# Fixture: enum_variant_declaration_not_call

Spec: RIPR-SPEC-0046

Issue: #3740

## Given

A Rust library whose diff changes tuple enum variants, a tuple struct, a real
constructor expression, and an ordinary function call:

- `Invalid(std::string::String),` and `Ambiguous(std::string::String),` are
  declarations. They must not become `call_deletion` probes.
- `pub struct Wrap(std::string::String);` is the same declaration mechanism.
- `Subject::Invalid(message.clone());` and `record(message.trim());` are
  executable calls and must stay `call_deletion`.

## When

```bash
ripr check \
  --root fixtures/enum_variant_declaration_not_call/input \
  --diff fixtures/enum_variant_declaration_not_call/diff.patch \
  --mode fast
```

## Then

Declaration lines classify `static_unknown` only. The constructor expression
and the function call classify `call_deletion`.

## Must Not

- Mint `call_deletion` for a tuple variant or tuple struct declaration.
- Drop `call_deletion` from `Subject::Invalid(...)` or `record(...)`.
