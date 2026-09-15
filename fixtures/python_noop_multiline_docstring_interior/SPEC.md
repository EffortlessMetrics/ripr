# Fixture: python_noop_multiline_docstring_interior

Spec: RIPR-SPEC-0028

## Given

A Python diff changes only an interior prose line of `discount`'s multi-line
docstring. The surrounding triple quotes, function body, and strong exact-value
test remain unchanged.

## When

`ripr check` analyzes the diff through the Python preview adapter.

## Then

RIPR parses both source versions, establishes that the changed line belongs to
a real docstring in each version, and emits no behavior probe. The test oracle
cannot discriminate a runtime behavior change because the edit has none.

## Must Not

- Classify the interior prose edit `exposed`.
- Suppress a behavioral line replaced by a newly introduced multi-line
  docstring.
- Treat an assigned triple-quoted string or an f-string as a docstring.
- Infer the old-side docstring context from the new file alone.
