# ripr

`ripr` finds static mutation-exposure gaps before expensive mutation testing.
The npm package is **@effortlessmetrics/ripr**; the installed command remains
**ripr**.

## Linux x86-64 prerelease

This npm prerelease contains one native executable for **Linux x86-64
with glibc 2.34 or newer**. macOS, Windows, ARM64 and musl/Alpine are not included.
Use npm 10 or newer. npm checks OS, architecture and libc family; it cannot
check the minimum glibc version. Older glibc loaders reject this executable.
Do not override platform checks with `--force`.

Install this exact candidate version when it is available from your registry:

```console
npm install --global @effortlessmetrics/ripr@0.11.0-alpha.2
ripr --version
ripr check
```

For a one-shot invocation, keep the package and command explicit:

```console
npx --yes --package=@effortlessmetrics/ripr@0.11.0-alpha.2 ripr check
```

Project-local installation also works:

```console
npm install --save-dev @effortlessmetrics/ripr@0.11.0-alpha.2
npm exec -- ripr check
```

Installation works with `--ignore-scripts`: there are no lifecycle scripts,
Rust compilation, installation-time downloads or first-run network requests.
The npm bin link starts the native executable directly, retaining its normal
argument, stdio, signal and exit-status behavior. This is a CLI distribution;
it does not provide a JavaScript import API. npm is needed for installation,
not for native execution. Avoid overwriting an unrelated `ripr` command.

## Exact native provenance

This package retains the exact audited native bytes from a fresh source build
qualified through both Python and npm consumers in the same workflow run.
`provenance.json` records the source commit/tree, native/wheel digests, toolchain
and immutable qualification artifact. The wheel is a build-time container;
installation and execution do not require Python or a public PyPI package.
The package includes its original licenses and native SBOM. Registry OIDC
provenance, when present, is separate from the prepared tar's provenance record.

This prerelease has a bounded single-platform scope. A future version may
use platform-specific optional dependencies and a launcher to add platforms.
It will not modify this immutable version. No other platform, alternate npm
client or stable release is claimed here.

Source and documentation: https://github.com/EffortlessMetrics/ripr
