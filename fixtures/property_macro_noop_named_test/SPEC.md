# Fixture: property_macro_noop_named_test

Spec: RIPR-SPEC-0001

## Given

A collected test shares the changed owner's name in its own test module. Its
only qualified owner occurrence is inside a no-op property assertion.

## When

The public analyzer reads the boundary diff. The runtime integration target
compiles these exact bytes and a real super-qualified ordinary-call positive.

## Then

The test declaration cannot count as an independently executed owner call.
Reach, infection and propagation stay unresolved with the existing macro-reach
limitation. Correct and wrong owner versions both collect one test and pass.
The ordinary-call positive fails the wrong version.

## Must Not

- Treat a function/type declaration as independent execution
- Report exposed or invent an assertion oracle from opaque property spelling
