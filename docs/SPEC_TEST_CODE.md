# Spec-Test-Code Traceability

`ripr` should be easy for humans and LLM agents to reason about. Every
meaningful behavior should have a three-way match:

```text
spec -> test -> code
```

The purpose is not bureaucracy. The purpose is to make expected behavior,
verification, and implementation discoverable without reconstructing intent from
chat history.

## Spec IDs

Specs live in [specs](specs/) and use stable IDs:

```text
RIPR-SPEC-0001
RIPR-SPEC-0002
```

Each spec should include:

- problem
- user-facing behavior
- non-goals
- data contract, when relevant
- acceptance examples
- test mapping
- implementation mapping

Machine-readable mapping starts in `.ripr/traceability.toml`. Keep it aligned
with spec files, fixtures, code modules, output contracts, and metrics. The
manifest is intentionally lightweight now and will become stricter as xtask
traceability checks land.

## Test Mapping

Tests should make the behavior visible in names or comments.

Preferred:

```rust
#[test]
fn given_changed_boundary_when_equal_value_is_missing_then_reports_weak_exposure() -> Result<(), String> {
    // Covers RIPR-SPEC-0001.
    Ok(())
}
```

Avoid generic names like:

```text
works
handles_case
test_output
```

Golden fixtures should map to specs in a small manifest or README beside the
fixture.

## Code Mapping

Implementation modules should map naturally to concepts. When the mapping is not
obvious, add a short module-level or function-level comment naming the spec or
behavior.

Do not add noisy comments that merely restate code.

## PR Checklist

For behavior changes, the PR should answer:

- [ ] Which spec changed or was added?
- [ ] Which tests prove the behavior?
- [ ] Which code module owns the behavior?
- [ ] Which golden outputs changed?
- [ ] Which conservative static-language rules were checked?
- [ ] Which metrics should move?

For long-context agent work, also answer:

- [ ] What is the narrow production delta?
- [ ] What is the supporting evidence delta?
- [ ] What is the single acceptance criterion?
- [ ] What is intentionally out of scope?

## Output Changes

Any output change should update:

- JSON schema docs, if JSON changed
- human golden output, if human output changed
- context-packet golden output, if agent context changed
- LSP diagnostic expectations, if editor output changed

Output changes must preserve the static-vs-mutation language boundary.
