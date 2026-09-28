# Perl CPAN Alpha Fixture

Spec: RIPR-SPEC-0064

Campaign 31 E1 convergence proof (ripr-swarm#1379).

## Given

A CPAN-style Perl project with a package, a test file using Test::More, and
a diff that changes a predicate boundary from `>=` to `>`.

Fixture:
- `input/lib/Pricing.pm`: package with `calculate_discount` + `dynamic_method`
- `input/t/pricing.t`: Test::More with `ok()` (weak) + `is()` (exact)
- `input/diff.patch`: changes `>=` to `>` in `calculate_discount`

## When

The producer (`perllsp ripr-facts`) emits a fact packet from the project.
The consumer (`ripr check --perl-facts`) reads it and produces honest results.

## Then

### Outcome 1: Actionable

The `ok(calculate_discount(100))` oracle reaches the changed boundary but
does not pin it. ripr produces a `WeaklyExposed` Finding with a concrete
missing discriminator (`$amount > 100`).

### Outcome 2: Already observed

The `is(calculate_discount(100), 90)` oracle pins the exact boundary. No
repair packet is needed.

### Outcome 3: Limited

The dynamic-dispatch pattern (`$obj->$method()`) produces a named limitation
(`dynamic_dispatch` boundary), not a repair packet.

## Must Not

- Emit a public repair packet for the dynamic-dispatch case.
- Emit a public repair packet without the shared validator passing.
- Crash or abort the whole report on any of the three outcomes.
- Credit a weak oracle as `exposed` without the shared validator.
