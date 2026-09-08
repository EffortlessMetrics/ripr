# #1672 PowerShell rendering evidence

This document records the bounded source slice for issue #1672. It is an
evidence packet for review and release qualification; it is not a release
approval.

## Source pair and ancestry

- Source base: `971ac1a85ac76e60003c2c915761a2c13e854c2b`.
- Final candidate: `ce4ad79d8c57cfa215033a7d2274efe52604e95f` (`fix(output): bound PowerShell translation eligibility`).
- Source candidate proof subject: `d3011ff036583cd751e2c8395bcee2f26c1a188a`
  (the standalone 16-test subject before the final eligibility controls and
  evidence update).
- Donor ancestry was carried and adapted from merged PRs in the
  `EffortlessMetrics/ripr-swarm` repository: #3617 at
  `140b2de76a7b43462e2212b005b6ac84142e08d`, #3625 at
  `58ff4e2368f0bfa9e7b95958f8cda7109072ba77`, #3661 at
  `25dad053cc6596bb3dcbca0b0f0245d5f7dad680`, and #3662 at
  `6ffb8c9ea4391ea07edf73674ca762233ebc8acd`. Those are donor references,
  not source-repository authority. The current candidate adds native proof and
  repairs donor defects found on the current source.

## Supported command subset

The shared Rust renderer emits a PowerShell form for a single simple command,
with or without one spaced trailing `>` redirect. Bash single-quote escapes
are decoded into PowerShell literals. Empty arguments, spaces, Unicode,
apostrophes, double quotes, dollar signs, backticks, and Windows path text are
covered by the native fixture. Redirected commands use an invocation-owned
staging file and publish the artifact only after a zero native exit status.

The renderer withholds compound, malformed, variable, unspaced, append, and
multiple redirect forms. Quoted metacharacters remain argument data. A
withheld form is an explicit unavailable disposition, not a PowerShell claim.

The supported native route is scoped to PowerShell 7.6, with the current
native proof run on PowerShell 7.6.5. Windows PowerShell 5.1 and cmd.exe are
unsupported. The disclosure does not establish support for other PowerShell
versions.

## Native proof

The Windows-only test in `crates/ripr/src/output/markdown.rs` compiles a tiny
Rust executable with `rustc`, then runs the generated command through
`pwsh`. It establishes:

- independently expected argv, including empty, Unicode, apostrophe,
  special-character, Windows-path, and literal leading/trailing apostrophe
  values;
- exact stdout bytes without text normalization;
- nonzero native failure preserving an existing valid artifact;
- preservation of an unrelated preexisting staging file;
- rejection of an output path that is a directory;
- cleanup of invocation-owned staging after failure.

The focused standalone receipt compiles and executes the test executable:

```text
rustc --edition=2024 --test crates/ripr/src/output/markdown.rs -o <temp>\\markdown-1672-test.exe
& <temp>\\markdown-1672-test.exe
```

The current receipt is 16/16 tests passed on the Windows host. The native
child route is invoked as `pwsh -NoProfile -Command <generated command>` and
the proof host reports `$PSVersionTable.PSVersion` as `7.6.5` and
`$PSNativeCommandArgumentPassing` as `Windows`; this packet therefore scopes
the claim to that tested PowerShell 7.6 mode and version. Cargo workspace
proof for the focused selector also passed 14/14 with
`cargo test -p ripr powershell_command --lib`; hosted CI and release
qualification remain separate gates.

## Remaining issue scope

The TypeScript editor copy consumers remain outside this Rust shared-renderer
slice. They need an existing-issue follow-up that defines the editor DTO and
render owner before adding a second quoting implementation. This packet does
not claim editor coverage, full #1672 closure, or release readiness.
