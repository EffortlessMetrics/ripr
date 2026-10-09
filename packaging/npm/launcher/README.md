# ripr

This package is the npm distribution adapter for the native **ripr** CLI.
It is not the VS Code extension and does not implement a JavaScript analyzer.

> Status: source package under development. No npm release is claimed by this
> repository state.

The intended installed command is:

```bash
npm install --save-dev --save-exact @effortlessmetrics/ripr
npm exec -- ripr check --base origin/main
```

The launcher selects one exact-version, platform-specific package and starts its
native executable. It does not run lifecycle installation scripts, download a
binary from GitHub, compile Rust, invoke a shell, or fall back to an unrelated
`ripr` on `PATH`.

Supported platform packages are currently planned for Linux glibc x64/ARM64,
macOS x64/ARM64, and Windows x64 MSVC. Public support begins only after the
corresponding native payload and installed-product qualification is accepted.

The product is licensed under MIT OR Apache-2.0.
