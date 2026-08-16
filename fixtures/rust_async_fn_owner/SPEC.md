# Fixture: rust_async_fn_owner

Spec: RIPR-SPEC-0001

## Given

Production code changes the predicate boundary in an `async fn` from `>=` to
`>`. The related test is a `#[tokio::test]` that calls `.await` on the async
function and asserts the exact return value with `assert_eq!`.

## When

ripr analyzes the diff against the workspace.

## Then

The analyzer resolves the `async fn` owner (`fetch_limit`), reaches it from
the `#[tokio::test]` oracle, and identifies the strong `exact_value` oracle.
Classification is `propagation_unknown`.

Note: the `propagation_unknown` here is not specific to `.await` — the
single-line predicate produces no `flow_sink` for the propagation analyzer
to trace, so a synchronous version of the same code produces the identical
result. This fixture probes **async owner/test discovery** (does ripr find
the `async fn` owner and the `#[tokio::test]` oracle?), not async-flow
propagation. A future fixture with a multi-line async body that produces a
traceable sink would be needed to probe `.await` propagation specifically.

## Must Not

- Must not over-credit as `exposed` — propagation is unknown.
- Must not fail to resolve the async owner or miss the test.
