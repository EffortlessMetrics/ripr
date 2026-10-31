# ripr

`ripr` finds static mutation-exposure gaps before expensive mutation testing.
The **PyPI distribution name is `ripr-rs`**, while the installed executable and
product name remain `ripr`.

> **Release status:** alpha prerelease for Linux x86-64 with glibc 2.34 or newer.
> This is preparation for the stable 0.11 release. Verify the published wheel's
> filename and SHA-256 on PyPI before relying on its provenance.

## Initial platform scope

The first source-owned qualification selects Linux x86-64 with glibc 2.34 or
newer only. The wheel is checked by auditwheel against manylinux_2_34; the
installed journey runs on Ubuntu 22.04. macOS, Windows, ARM64 and musl/Alpine
are not included in this initial package. A compatibility tag is not a claim
that every supported operating system has been exercised.

## Intended installation

Persistent tool installation:

```console
uv tool install --prerelease allow ripr-rs
ripr --version
ripr check
```

One-shot execution with the distribution named explicitly:

```console
uvx --prerelease allow --from ripr-rs ripr check
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

## Retained consumer diagnostics

The source wheel qualification workflow retains separate `pypi-consumer-pip`
and `pypi-consumer-uv` artifacts, suffixed with the run ID and attempt, for five
days. Each records the selected install, native-identity, preview, explain,
disable, reinstall, and uninstall command groups. A group passes only after its
existing shell assertions succeed. An interrupted active group is failed;
later groups remain `not_run`.

The diagnostic receipt binds the existing producer JSON digests, source, run,
attempt, wheel digest, and native digest. Check JSON, explanation text, and
disabled-language JSON are retained with digests, up to 1 MiB each; truncation
is explicit and cannot complete the diagnostic proof. These artifacts grant
no release eligibility, including when every group passes. Existing publisher
admission remains authoritative and still requires a successful qualification
run. Failures before recorder initialization and runner loss may leave no
consumer artifact; absence is never a pass.
Integrity refusals still bound the permitted outputs and persist a rejected
terminal diagnostic. Original producer and output bindings are preserved;
changed output bytes are identified separately as `observed_output`. A rejected
receipt cannot become complete on a later finalization, even if bytes are restored.

This retains evidence from the existing checkout-free, offline pip/uv installs;
it adds no network namespace isolation or broader platform qualification.
Local recorder fixtures do not qualify an installed release candidate.
