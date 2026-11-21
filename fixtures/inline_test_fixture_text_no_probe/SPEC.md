# Fixture: inline_test_fixture_text_no_probe

Spec: RIPR-SPEC-0002

## Given

A production file with an inline `#[cfg(test)]` module holding fixture
data: a multi-line raw JSON string constant. A test reads the constant.
The changed line is raw JSON text inside the test module, owned by no
function.

## When

The diff changes one JSON value (`"abc"` to `"abd"`) on a line inside
the `#[cfg(test)]` module.

## Then

No production probe is emitted for the fixture-text line: changing
fixture data is not a production behavior change (#3718). Zero probes,
zero findings.

## Must Not

- Emit `field_construction` (or any family) for the JSON fixture line.
- Suppress probes for the same JSON shape in production position (see
  the `NoneType`-style negative: production lines still probe).
- Grant evidence from the module name alone: a bare `mod tests`
  without `cfg(test)` must still probe (pinned at unit level).
