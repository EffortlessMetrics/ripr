# Fixture: python_dynamic_route_registration_limit

Spec: RIPR-SPEC-0028

## Given

A Python route handler registers its route with a dynamic expression such as
`@api.post(route_path())`.

The related pytest test calls `client.post("/checkout")`, but the preview
adapter cannot safely prove that the dynamic route expression resolves to that
path without executing code. The fixture workspace enables Python preview
explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_dynamic_route_registration_limit
```

or:

```bash
ripr check \
  --root fixtures/python_dynamic_route_registration_limit/input \
  --diff fixtures/python_dynamic_route_registration_limit/diff.patch \
  --mode fast
```

## Then

The Python preview adapter emits a named static limitation using
`decorator_indirection` evidence with a `dynamic_route_registration` reason.

## Must Not

- Execute `route_path()`.
- Link the client call to the route as actionable evidence.
- Emit a Python repair card or agent-packet-eligible canonical gap.
