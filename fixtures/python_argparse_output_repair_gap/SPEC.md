# Fixture: python_argparse_output_repair_gap

Spec: RIPR-SPEC-0028

## Given

A Python argparse-shaped command changes printed CLI output, and a pytest test
calls the command owner with `capsys` but only checks broad stdout presence.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_argparse_output_repair_gap
```

or:

```bash
ripr check \
  --root fixtures/python_argparse_output_repair_gap/input \
  --diff fixtures/python_argparse_output_repair_gap/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- keeps the argparse parser construction as ordinary static command setup,
- classifies the changed `print(...)` line as an output/call-effect probe,
- emits a concrete `output contains ...` missing discriminator,
- suggests strengthening the related pytest test with an exact output proof,
- provides a focused pytest verify command for that existing test.

## Must Not

- Execute pytest, import argparse, or require a virtualenv.
- Infer dynamic argparse runtime semantics beyond the changed static output.
- Generate tests or authorize production-code edits.
