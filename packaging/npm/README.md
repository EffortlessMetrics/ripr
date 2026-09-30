# ripr

`ripr` finds static mutation-exposure gaps before expensive mutation testing.
The npm package is **@effortlessmetrics/ripr**; the installed command remains
**ripr**.

## Linux x86-64 alpha

This initial npm prerelease contains one native executable for **Linux x86-64
with glibc 2.34 or newer**. macOS, Windows, ARM64 and musl/Alpine are not included.
Use npm 10 or newer. npm checks OS, architecture and libc family; it cannot
check the minimum glibc version. Older glibc loaders reject this executable.
Do not override platform checks with `--force`.

```console
npm install --global @effortlessmetrics/ripr@0.11.0-alpha.1
ripr --version
ripr check
```

For a one-shot invocation, keep the package and command explicit:

```console
npx --yes --package=@effortlessmetrics/ripr@0.11.0-alpha.1 ripr check
```

Project-local installation also works:

```console
npm install --save-dev @effortlessmetrics/ripr@0.11.0-alpha.1
npm exec -- ripr check
```

Installation works with `--ignore-scripts`: there are no lifecycle scripts,
Rust compilation, installation-time downloads or first-run network requests.
The npm bin link starts the native executable directly, retaining its normal
argument, stdio, signal and exit-status behavior. This is a CLI distribution;
it does not provide a JavaScript import API. npm is needed for installation,
not for native execution. Avoid overwriting an unrelated `ripr` command.

## Exact native provenance

This package retains the exact audited native bytes from `ripr-rs 0.11.0a1` on
PyPI, built from source commit `b1955d098cb628925801976bf1642dbf9329e07e`.
`provenance.json` distinguishes that product source from the later npm packaging
source. The PyPI wheel is an authenticated build-time input, not a runtime
Python dependency. The package includes its original licenses and native SBOM.
Manual first publication does not imply automatic npm OIDC provenance.

The first alpha is a bounded single-platform bootstrap. A future version may
use platform-specific optional dependencies and a launcher to add platforms.
It will not modify this immutable version. No other platform, alternate npm
client or stable release is claimed here.

Source and documentation: https://github.com/EffortlessMetrics/ripr
