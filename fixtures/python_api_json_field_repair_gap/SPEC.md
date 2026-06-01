# Fixture: python_api_json_field_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python route handler uses a simple FastAPI/Flask-shaped route decorator and
returns a JSON-shaped dict field from a changed branch.

The related pytest test reaches the route through `client.post("/checkout")`
but only asserts broad truthiness. The fixture workspace enables the Python
preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_api_json_field_repair_gap
```

or:

```bash
ripr check \
  --root fixtures/python_api_json_field_repair_gap/input \
  --diff fixtures/python_api_json_field_repair_gap/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- treats the literal route decorator as static route metadata,
- links the route owner to the pytest client call,
- emits a field/object repair card for the changed JSON response field,
- recommends strengthening the existing pytest test with
  `response.json()["detail"] == "coupon expired"`.

## Must Not

- Execute imports, route registration, pytest, or the route handler.
- Emit a repair card when the route path is registered dynamically.
- Promote Python out of preview/advisory status.
