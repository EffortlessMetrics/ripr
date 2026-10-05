# Install channels

Status of every way a developer can get `ripr`, observed 2026-10-04 against
public registries, plus the reversible preparation for 0.11. Nothing here
publishes, tags or submits anything; each outward step names who must act.
The current source checkout identifies `0.11.0-alpha.2`; entries describing
planned `0.11.0` publication are future owner actions, not acceptance of this
engineering merge or proof that its package and archive bytes are public.

## Observed state

| Channel | Public today | 0.11 state | Gap |
| --- | --- | --- | --- |
| `cargo install ripr` | crates.io serves 0.10.0 | Publishes with the release | Compiles from source (needs Rust 1.95; minutes, not seconds). |
| GitHub Release server archives | 0.10.0 has all five targets plus `.sha256` | Rehearsed by `release-server-binaries.yml`; 0.11.0 assets do not exist yet | Release operator publishes from `EffortlessMetrics/ripr`. |
| `cargo binstall ripr` | Falls back to a source build (0.10.0 carries no binstall metadata) | Metadata added in this change; works once 0.11.0 assets exist | Verify with a real `cargo binstall ripr` after the release. |
| CI (`ripr init --ci github`) | Compiles ripr each run | Downloads the release archive and verifies its checksum (#5236) | Merge #5236. |
| PyPI `ripr-rs` | Only `0.11.0a1`, Linux x86-64 wheel | Wheel qualification exists | Single platform; no macOS, Windows or ARM wheel is public. |
| npm `@effortlessmetrics/ripr` | `0.11.0-alpha.2` on `next`, Linux x86-64 only, README says so | Multi-platform launcher is checked in, not published | Platform payload packages (`ripr-darwin-arm64` and the others) do not exist on npm. |
| VS Code Marketplace / Open VSX | Marketplace lists 0.10.0 (read back in a browser 2026-10-04; Open VSX not checked) | `publish-extension.yml` is source-owned | The listing's two continuation links omit `/blob/main/` and render "Not Found" (`github.com/EffortlessMetrics/ripr/docs/EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md` and `.../EDITOR_FIRST_PR_BRIDGE_WORKFLOW.md`); its description still says 0.8.x. A source docs merge does not change a published listing; the next authorized extension publication does. Installed-snippet and continuation work is owned by #4629. |
| Homebrew | None | Formula draft below | Needs a tap repository and Steven's go-ahead. |

## Install time by route

Measured on one Linux container with a cold target directory (#5311). The
last two rows were re-measured on a 4-core container in October 2026 from the
same tree:

| route | seconds |
| --- | --- |
| prebuilt 0.10.0 archive (download, checksum, extract) | about 1 |
| `cargo install ripr --locked` (0.10.0 from crates.io) | 128 |
| `cargo install --locked --path crates/ripr` (0.11.0 development build, workspace profile) | 640 |
| `cargo build --release --locked -p ripr` in the workspace (same profile, 4 cores) | 751 |
| `cargo build --release --locked` on the packaged 0.11.0 crate (what `cargo install ripr --locked` builds, 4 cores) | 275 |

The first three rows did not record a core count and the 640 s figure ran beside
light unrelated work, so read them as rough, not an exact ratio. The 751 s and 275 s rows came from `cargo build`,
not `cargo install`, and the packaged-crate row is a local build of the output of
`cargo package`, not a download from crates.io, which carries no 0.11.0 yet.

What dominates: the `ripr` crate itself, about 700 of 907 CPU-seconds in the
workspace build (library 462 s, binary 234 s), not its dependencies (about 210
s, the largest being `rmcp`, `oxc_parser`, `ra_ap_syntax` and
`rustpython-parser` at 9 to 24 s each). The workspace `[profile.release]` sets
`lto = true` and `codegen-units = 1`, and the published crate carries no
`[profile]` section, so a crates.io install compiles with cargo's default release
profile and takes about 2.7 times as little time as a workspace build (275 s against 751 s) such as the
git install in the README. As an experiment, thin LTO with 16 codegen units
built the workspace in 266 s; on a tiny crate `ripr check` took the same 7 ms,
and on the `ripr` workspace itself, a large Rust workspace, it took about 5% longer (10.5 s against
10.0 s) with an 11% larger binary. The release profile was not changed, since it
also builds the prebuilt archives. `cargo xtask first-run --install-published`
times the source route and the scoreboard records it as
`first_run.install_seconds`. The metric has a regression rule but no committed
baseline sample and no nightly ingest yet, so nothing fails on a slower install
until both exist; the prebuilt route is not timed by the walk.

## What blocks a developer who is not us

1. **Every fast path hangs on one event:** the 0.11.0 GitHub Release with its
   server archives. Binstall, the VS Code server download, the CI install and a
   Homebrew formula all read those assets.
2. **macOS and Windows users have no non-source path** in the package registries
   until npm and PyPI publish more than Linux x86-64.
3. **First-run trust:** the archive is unsigned beyond its SHA-256. Binstall
   verifies nothing further, and Homebrew verifies the pinned hash.

## cargo-binstall

`crates/ripr/Cargo.toml` now carries `[package.metadata.binstall]` pointing at
`ripr-server-v{version}-{target}.tar.gz` (`.zip` on `x86_64-pc-windows-msvc`).
The archive holds `ripr` at its root. The xtask test
`release_server::binstall_metadata` renders the templates for all five targets
and compares them with `server_asset_name`, the function the release archive
command names its assets with; a renamed asset fails the test.

Only packages published from source containing this change carry the metadata.
After an authorized package publication and matching archive publication, check:

```bash
cargo binstall ripr --no-confirm   # expect a download, not a compile
ripr --version
```

## Homebrew formula (draft, not submitted)

Target: a third-party tap (`EffortlessMetrics/homebrew-tap`, which does not
exist yet), not homebrew-core. The formula installs the prebuilt archives, so
`brew install` takes seconds and needs no Rust. Fill each `sha256` from the
release's `.sha256` assets once 0.11.0 is published; do not guess them.

```ruby
class Ripr < Formula
  desc "Static mutation-exposure analyzer for Rust workspaces"
  homepage "https://github.com/EffortlessMetrics/ripr"
  version "0.11.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/EffortlessMetrics/ripr/releases/download/v#{version}/ripr-server-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "FILL_FROM_RELEASE_SHA256_ASSET"
    end
    on_intel do
      url "https://github.com/EffortlessMetrics/ripr/releases/download/v#{version}/ripr-server-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "FILL_FROM_RELEASE_SHA256_ASSET"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EffortlessMetrics/ripr/releases/download/v#{version}/ripr-server-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "FILL_FROM_RELEASE_SHA256_ASSET"
    end
    on_intel do
      url "https://github.com/EffortlessMetrics/ripr/releases/download/v#{version}/ripr-server-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "FILL_FROM_RELEASE_SHA256_ASSET"
    end
  end

  def install
    bin.install "ripr"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ripr --version")
  end
end
```

Open points before this is real: the Linux archives are built on `ubuntu-22.04`
(glibc 2.35), and release qualification caps the required glibc symbol version at
2.34. Linux compatibility is therefore established only for glibc 2.34+ hosts;
older Homebrew Linux systems are unverified. The Windows archive has no
Homebrew analogue (Scoop or winget would be the equivalent, not drafted).

## Decisions that need the owner

- Publish 0.11.0 (unblocks binstall, the extension download, CI install).
- Create the tap repository and approve the formula.
- Decide whether npm and PyPI ship 0.11.0 on all five targets or stay
  Linux-only alphas; today's README text is honest about the alpha scope.
