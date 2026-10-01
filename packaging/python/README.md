# ripr

`ripr` finds static mutation-exposure gaps before expensive mutation testing.
The **PyPI distribution name is `ripr-rs`**, while the installed executable and
product name remain `ripr`.

> **Qualification status:** this package adapter is under no-publish rehearsal.
> A local wheel does not mean that a public PyPI release exists.
> Use published installation commands only after a release has been independently verified from PyPI.

## Intended installation

Persistent tool installation:

```console
uv tool install ripr-rs
ripr --version
ripr check
```

One-shot execution with the distribution named explicitly:

```console
uvx --from ripr-rs ripr check
```

The explicit `--from ripr-rs` matters because another PyPI project uses the
unscoped distribution name `ripr` and also installs a command with that name.
Use an isolated tool environment rather than replacing an unrelated ambient
executable.

For a reviewed local wheelhouse during qualification:

```console
python -m pip install --no-index --find-links ./wheelhouse ripr-rs
ripr --version
```

## Package contract

- The wheel contains the native `ripr` executable; it does not compile Rust on
  the consumer machine and does not download a binary at installation or first
  run.
- This is a CLI distribution. It does not provide `import ripr` or
  `python -m ripr`.
- Initial delivery is wheel-only. There is no implicit source-distribution
  fallback that compiles the project when a compatible wheel is unavailable.
- Platform and minimum-OS compatibility claims apply only to wheel files whose
  installed payloads completed the corresponding qualification.
- Packaging does not change language activation, analysis behavior, output
  schemas, or the product's advisory claim boundary.

Source, documentation, and issue tracking remain under the
[`EffortlessMetrics/ripr`](https://github.com/EffortlessMetrics/ripr)
repository.
