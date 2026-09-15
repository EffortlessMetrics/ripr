# Fixture: decimal_field_observer_non_member_access

Spec: RIPR-SPEC-0108

Issue: #2691

## Given

The changed Rust expression is a struct field initializer. A related test
reaches the constructor but its custom assertion helpers only inspect a
decimal literal and the whole value; neither assertion names the changed
field.

## When

The initializer changes from `retries: 1` to `retries`, while the test remains
unchanged.

## Then

The field-construction finding must remain non-promoted because the decimal
literal is not a member access and does not identify the constructed field.

## Must Not

- Do not treat `3.14_f64` as a field observer merely because it contains a dot.
- Do not claim `exposed` for the changed `retries` field.
- Do not use mutation-runtime outcome vocabulary.
