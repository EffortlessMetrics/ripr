# Fixture: python_api_route_decorator_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python route handler uses a simple FastAPI/Flask-shaped route decorator, and
the changed behavior assigns a response status code.

The related pytest test calls the handler but only asserts broad truthiness.
The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_api_route_decorator_repair_gap
```

or:

```bash
ripr check \
  --root fixtures/python_api_route_decorator_repair_gap/input \
  --diff fixtures/python_api_route_decorator_repair_gap/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- treats the route decorator as static route metadata, not arbitrary decorator
  indirection,
- finds the `checkout` route owner,
- emits a field/object repair card for the changed status code,
- recommends strengthening the existing pytest test with a direct
  `assert response.status_code == 422` assertion shape.

## Must Not

- Treat arbitrary decorators such as retry wrappers as transparent.
- Execute imports, route registration, or pytest.
- Promote Python out of preview/advisory status.
