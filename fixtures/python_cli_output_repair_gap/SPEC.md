# Fixture: python_cli_output_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python Click-style command changes the CLI output text, and a pytest test
calls the command owner with `capsys` but only checks broad stdout presence.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_cli_output_repair_gap
```

or:

```bash
ripr check \
  --root fixtures/python_cli_output_repair_gap/input \
  --diff fixtures/python_cli_output_repair_gap/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- treats known static Click CLI decorators as transparent preview metadata,
- classifies the changed `click.echo(...)` line as an output/call-effect probe,
- emits a concrete `output contains ...` missing discriminator,
- suggests strengthening the related pytest test with the exact output proof,
- provides a focused pytest verify command for that existing test.

## Must Not

- Execute pytest, import Click, or require a virtualenv.
- Treat arbitrary `app.command` decorators as transparent without Typer context.
- Generate tests or authorize production-code edits.
