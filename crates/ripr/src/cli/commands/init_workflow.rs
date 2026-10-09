//! The generated `ripr init --ci github` workflow template.
//!
//! #4386 (slice 1): the GitHub Actions workflow `ripr init` writes used to be
//! a ~2,400-line raw string inline in `init.rs`, dwarfing the command logic
//! around it. This module owns the generated workflow bytes; `init.rs` keeps
//! only the init command surface. The template is stored as a head, the
//! development advisory-summary step and a tail. Their compact command
//! wiring remains pinned by hash. Adopter rendering instead selects the
//! command template qualified for the installed release, currently 0.10.0.
//! `generated_github_actions_workflow` substitutes `@RIPR_...@` placeholders
//! at render time; rendered behavior stays pinned by the
//! `generated_workflow_*` tests in `init.rs` and `commands.rs`, the
//! `tests/generated_review_workflow.rs` replay, and `cargo xtask
//! check-workflows`.

#[cfg(test)]
#[path = "init_workflow/advisory_summary.rs"]
mod advisory_summary;

#[cfg(test)]
use crate::agent::loop_commands;

#[cfg(test)]
use advisory_summary::ADVISORY_SUMMARY_STEP;

/// The template up to the `Add RIPR advisory summary` step, ending with the
/// blank line before it.
const TEMPLATE_HEAD: &str = r#"name: RIPR

# Each setting and step is explained in docs/CI.md in the ripr repository.
on:
  pull_request:
    types: [opened, synchronize, reopened, labeled, unlabeled]
  workflow_dispatch:

permissions:
  contents: read
  pull-requests: write # inline comments only (RIPR_COMMENT_MODE=inline)
  security-events: write # SARIF upload only (RIPR_UPLOAD_SARIF=true)

env:
  RIPR_UPLOAD_SARIF: "true"
  # Repository variables. RIPR_GATE_MODE: empty (advisory), visible-only,
  # acknowledgeable, baseline-check or calibrated-gate.
  RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}
  RIPR_GATE_BASELINE: ${{ vars.RIPR_GATE_BASELINE || '' }}
  # RIPR_COMMENT_MODE: off, plan or inline.
  RIPR_COMMENT_MODE: ${{ vars.RIPR_COMMENT_MODE || 'off' }}

defaults:
  run:
    shell: bash

concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true

jobs:
  ripr:
    name: RIPR advisory reports
    runs-on: ubuntu-latest
    # Advisory unless RIPR_GATE_MODE names a blocking mode.
    continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}
    steps:
      - uses: actions/checkout@v6
        with:
          ref: ${{ github.event.pull_request.head.sha || github.sha }} # the PR head, where comments are placed
          fetch-depth: 0
          persist-credentials: false # leaves no token in .git/config

      - name: Remove checked-in RIPR artifacts
        run: rm -rf target/ripr target/ci # a pull request must not supply its own RIPR inputs

@RIPR_PIN_FIRST_LINE@
      # its commands. Downloads the prebuilt release and checks its SHA-256;
      # with none for this runner, `cargo install` (needs Rust).
      - name: Install ripr
        id: install
        run: |
          version=@RIPR_VERSION@
          case "$RUNNER_OS-$RUNNER_ARCH" in
            Linux-X64) target=x86_64-unknown-linux-gnu ;;
            Linux-ARM64) target=aarch64-unknown-linux-gnu ;;
            macOS-X64) target=x86_64-apple-darwin ;;
            macOS-ARM64) target=aarch64-apple-darwin ;;
            *) target="" ;;
          esac
          asset="ripr-server-v$version-$target.tar.gz"
          url="https://github.com/EffortlessMetrics/ripr/releases/download/v$version/$asset"
          cd "$RUNNER_TEMP" && mkdir -p ripr-bin
          if [ -n "$target" ] && curl -fsSL --retry 3 -O "$url" && curl -fsSL --retry 3 -O "$url.sha256"; then
            expected="$(awk 'NR == 1 { print $1 }' "$asset.sha256")"
            actual="$( { sha256sum "$asset" 2>/dev/null || shasum -a 256 "$asset"; } | awk '{ print $1 }')"
            if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
              echo "::error::$asset does not match its published SHA-256 (expected ${expected:-nothing}, got $actual)"; exit 1
            fi
            tar -xzf "$asset" -C ripr-bin; echo "$RUNNER_TEMP/ripr-bin" >> "$GITHUB_PATH"
          else
            why="no prebuilt ripr $version for $RUNNER_OS-$RUNNER_ARCH"; [ -z "$target" ] || why="downloading $url failed"
            if ! command -v cargo >/dev/null 2>&1; then
              echo "::error::Cannot install ripr: $why, and this runner has no cargo to build it. Install Rust on the runner (https://rustup.rs) or add a Rust toolchain step before Install ripr."; exit 1
            fi
            echo "::notice::$why; building it with cargo install"
            cargo install ripr --version @RIPR_VERSION@ --locked
          fi
          PATH="$RUNNER_TEMP/ripr-bin:$PATH" ripr --version
          echo "RIPR_CACHE_DIR=$RUNNER_TEMP/ripr-cache" >> "$GITHUB_ENV"

      - uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0; reuses unchanged files' analysis
        with:
          path: ${{ runner.temp }}/ripr-cache
          key: ripr-cache-@RIPR_VERSION@-${{ runner.os }}-${{ github.event.pull_request.head.sha || github.sha }}
          restore-keys: ripr-cache-@RIPR_VERSION@-${{ runner.os }}-

      - name: Capture existing RIPR inline comments
        if: always() && github.event_name == 'pull_request' && env.RIPR_COMMENT_MODE != 'off'
        continue-on-error: true
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          gh api --paginate --slurp "repos/${{ github.repository }}/pulls/${{ github.event.pull_request.number }}/comments" \
            | ripr pr-comments existing --root . --raw -

      - name: Run RIPR
        run: ripr reports ci-packet --root .

      - name: Publish RIPR inline comments
        if: always() && github.event_name == 'pull_request' && env.RIPR_COMMENT_MODE == 'inline' && hashFiles('target/ripr/review/comment-publish-plan.json') != ''
        continue-on-error: true
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          ripr pr-comments requests --root . --pull-request "${{ github.event.pull_request.number }}" --head-sha "${{ github.event.pull_request.head.sha }}"
          while IFS=$'\t' read -r method endpoint request message; do
            gh api --method "$method" "repos/${{ github.repository }}/$endpoint" --input "$request" </dev/null >/dev/null
            echo "$message"
          done < target/ripr/review/publish/requests.tsv

"#;

/// The template from the first upload step through the last.
const TEMPLATE_TAIL: &str = r#"      - name: Upload RIPR report artifacts
        if: always()
        continue-on-error: true
        uses: actions/upload-artifact@v7
        with:
          name: ripr-reports
          path: |
            target/ripr
            target/ci
          if-no-files-found: ignore
          retention-days: 14

      - name: Upload RIPR diff findings
        if: always() && env.RIPR_UPLOAD_SARIF == 'true' && github.event_name == 'pull_request' && hashFiles('target/ripr/reports/ripr-findings.sarif') != ''
        continue-on-error: true # a code-scanning outage must not fail the gate (#2009)
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: target/ripr/reports/ripr-findings.sarif
          category: ripr-findings

      - name: Upload RIPR repo seams
        if: always() && env.RIPR_UPLOAD_SARIF == 'true' && hashFiles('target/ripr/reports/ripr-seams.sarif') != ''
        continue-on-error: true
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: target/ripr/reports/ripr-seams.sarif
          category: ripr-seams
"#;

/// The unrendered workflow template, pinned by hash below. The only
/// substitutions between this and the written file are the render-time
/// `@RIPR_...@` placeholder replacements below.
#[cfg(test)]
fn generated_workflow_template() -> String {
    TEMPLATE_HEAD.to_owned() + ADVISORY_SUMMARY_STEP + TEMPLATE_TAIL
}

/// Retained development command wiring, never selected for an installed
/// 0.10.0 adopter workflow. Tests replay it with the binary under test.
#[cfg(test)]
pub(super) fn generated_development_workflow() -> String {
    render_development_workflow(env!("CARGO_PKG_VERSION"))
}

/// Published 0.10.0 has no ci-packet/ci-summary or comment JSON helper
/// commands. Keep its executable steps and artifact paths together, while
/// retaining current checkout, installation, cache and upload protections.
fn released_0_10_workflow_template() -> String {
    let prefix = TEMPLATE_HEAD
        .split("      - name: Capture existing RIPR inline comments\n")
        .next()
        .unwrap_or(TEMPLATE_HEAD);
    prefix.to_owned()
        + "      - name: Verify installed RIPR compatibility\n        run: |\n          installed=\"$(ripr --version)\"\n          if [ \"$installed\" != 'ripr 0.10.0' ]; then\n            echo \"::error::Expected ripr 0.10.0 for these workflow commands; got $installed\"; exit 1\n          fi\n\n"
        + include_str!("init_workflow/released_0_10_producer.yml")
        + "\n"
        + include_str!("init_workflow/released_0_10_summary.yml")
        + "\n"
        + TEMPLATE_TAIL
}

/// Pin selection alone does not establish command compatibility. Historical
/// simulated pins without a qualified template must fail visibly before the
/// install or any product command, never fall through to development APIs.
fn unsupported_workflow_template() -> String {
    let prefix = TEMPLATE_HEAD
        .split("      - name: Capture existing RIPR inline comments\n")
        .next()
        .unwrap_or(TEMPLATE_HEAD);
    prefix.replace(
        "@RIPR_PIN_FIRST_LINE@",
        "      - name: Reject unsupported RIPR command contract\n        run: |\n          echo '::error::No qualified workflow command template for installed ripr @RIPR_VERSION@; use the supported 0.10.0 generator contract.'\n          exit 1\n\n@RIPR_PIN_FIRST_LINE@",
    ) + TEMPLATE_TAIL
}

/// Newest ripr release on crates.io. `init --ci github` pins this in the
/// generated workflow when the generating binary is NEWER (unreleased), so
/// the install step always resolves (#5208). Released generators pin
/// themselves; command compatibility is selected separately below.
///
/// Bump together with the package version in the release commit, and
/// publish from that commit — never for a release candidate (#5208, #5244
/// review). A constant bumped only after publication would travel behind
/// the version it names, so the just-published generator would warn and
/// pin its predecessor on every release. A stale constant (bump forgotten)
/// degrades the same loud way: warn and pin the older release, always
/// resolvable, never an unresolvable pin. See docs/RELEASE.md Post-Publish
/// for the procedure and the release-commit-to-publication window.
const LATEST_RELEASED_VERSION: &str = "0.10.0";

/// Parse `major.minor.patch`; `None` for anything else. Unknown shapes fail
/// toward "unreleased": an unrecognized generator version must never become
/// a `--version` pin CI cannot resolve.
fn parse_release_version(text: &str) -> Option<(u64, u64, u64)> {
    let (major, rest) = text.split_once('.')?;
    let (minor, patch) = rest.split_once('.')?;
    if patch.contains('.') {
        return None;
    }
    Some((
        major.parse().ok()?,
        minor.parse().ok()?,
        patch.parse().ok()?,
    ))
}

/// Version the generated workflow's install step pins for a generator
/// reporting `generator_version`: the generator itself when it names a
/// released version (at most the latest release), else the latest release.
/// The caller warns on stderr when the two differ (#5208).
pub(super) fn workflow_install_version(generator_version: &str) -> String {
    let latest = parse_release_version(LATEST_RELEASED_VERSION);
    let own = parse_release_version(generator_version);
    match (latest, own) {
        (Some(latest), Some(own)) if own <= latest => generator_version.to_string(),
        _ => LATEST_RELEASED_VERSION.to_string(),
    }
}

/// Historical pin-comment first line, byte-for-byte: released generators
/// retain this pin description (#5208). Only the first line is
/// substituted — the rest of the install-step comment (prebuilt download,
/// cache, upgrade route) is version-independent (#5236).
const RELEASED_PIN_FIRST_LINE: &str =
    "      # Pinned to the ripr that generated this workflow. The steps below use";

/// Install-step comment first line for a workflow pinning `pinned`,
/// generated by `generator_version` (#5208). A fallback pin must not keep
/// the "generated this workflow" claim: that version did not generate it.
fn install_pin_first_line(generator_version: &str, pinned: &str) -> String {
    if pinned == generator_version {
        RELEASED_PIN_FIRST_LINE.to_string()
    } else {
        format!(
            "      # Pinned to released ripr {pinned} ({generator_version} is unreleased). The steps below use"
        )
    }
}

pub(super) fn generated_github_actions_workflow() -> String {
    generated_workflow_for_version(env!("CARGO_PKG_VERSION"))
}

/// Render the workflow as a generator reporting `version` would.
/// Parameterized so tests pin released and unreleased renderings without
/// rebuilding the binary (#5208).
pub(super) fn generated_workflow_for_version(version: &str) -> String {
    let pinned = workflow_install_version(version);
    let first_line = install_pin_first_line(version, &pinned);
    if pinned == "0.10.0" {
        // Do not canonicalize release artifact paths with development
        // loop_commands constants: the installed release owns this contract.
        let mut workflow = released_0_10_workflow_template()
            .replace("@RIPR_VERSION@", &pinned)
            .replace("@RIPR_PIN_FIRST_LINE@", &first_line);
        // Repair-attempt orchestration is also absent from 0.10.0. Its
        // proof rail uses manual snapshots/verify/receipt around a test edit.
        for (placeholder, label) in [
            (
                "@RIPR_REPAIR_AFTER_PHASE@",
                "After the test edit: take an after snapshot, then run the printed manual verify and receipt commands.",
            ),
            (
                "@RIPR_MANUAL_VERIFY_LABEL@",
                "Manual verify after the test edit (needs snapshots taken around that edit)",
            ),
            ("@RIPR_MANUAL_RECEIPT_LABEL@", "Manual receipt after verify"),
            (
                "@RIPR_VERIFY_AFTER_EDIT_LABEL@",
                "Verify after the test edit",
            ),
            ("@RIPR_RECEIPT_AFTER_VERIFY_LABEL@", "Receipt after verify"),
            (
                "@RIPR_NO_RECEIPT_BEFORE_REPAIR@",
                "No agent receipt yet. None is expected before the focused test edit; take an after snapshot, then run manual verify and receipt.",
            ),
        ] {
            workflow = workflow.replace(placeholder, label);
        }
        return workflow;
    }
    unsupported_workflow_template()
        .replace("@RIPR_VERSION@", &pinned)
        .replace("@RIPR_PIN_FIRST_LINE@", &first_line)
}

#[cfg(test)]
fn render_development_workflow(version: &str) -> String {
    let pinned = workflow_install_version(version);
    let first_line = install_pin_first_line(version, &pinned);
    generated_workflow_template()
        .replace("@RIPR_VERSION@", &pinned)
        .replace("@RIPR_PIN_FIRST_LINE@", &first_line)
        .replace(
            "target/ripr/pilot/repo-exposure.json",
            loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
        )
        .replace(
            "target/ripr/pilot/after.repo-exposure.json",
            loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        )
        .replace(
            "target/ripr/agent/agent-packet.json",
            loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
        )
        .replace(
            "target/ripr/agent/agent-brief.json",
            loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
        )
        .replace(
            "target/ripr/agent/agent-verify.json",
            loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
        )
        .replace(
            "target/ripr/agent/agent-receipt.json",
            loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/before.repo-exposure.json",
            loop_commands::WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/after.repo-exposure.json",
            loop_commands::WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/workflow.json",
            loop_commands::WORKFLOW_MANIFEST_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/agent-seam-packets.json",
            loop_commands::WORKFLOW_AGENT_SEAM_PACKETS_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/agent-packet.json",
            loop_commands::WORKFLOW_AGENT_PACKET_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/agent-brief.json",
            loop_commands::WORKFLOW_AGENT_BRIEF_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/agent-verify.json",
            loop_commands::WORKFLOW_AGENT_VERIFY_ARTIFACT,
        )
        .replace(
            "target/ripr/reports/agent-receipt.json",
            loop_commands::WORKFLOW_AGENT_RECEIPT_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/agent-status.json",
            loop_commands::WORKFLOW_AGENT_STATUS_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/agent-status.md",
            loop_commands::WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/agent-review-summary.json",
            loop_commands::WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT,
        )
        .replace(
            "target/ripr/workflow/agent-review-summary.md",
            loop_commands::WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT,
        )
}

#[cfg(test)]
mod template_pin_tests {
    use super::{generated_development_workflow, generated_workflow_template};
    use sha2::{Digest, Sha256};

    /// #4386: the extraction had to be byte-preserving, so this pinned the
    /// SHA-256 of the pre-extraction inline raw string
    /// (`9a9779116f239c59173b929cebb044c5de3685577c8218ff89cb8f7c7c9a4315`,
    /// base commit 08de7c3bf). The pin now guards against unintended template
    /// edits; the intended changes since extraction replaced the toolchain,
    /// rust-cache, and `cargo install` steps with the prebuilt release
    /// download and the analysis-cache restore, and the advisory summary's
    /// shell with `ripr reports ci-summary`. #5208 replaced the pin-comment
    /// first line with the `@RIPR_PIN_FIRST_LINE@` placeholder (the only
    /// template-bytes change on top of #5236; see the diff), merged with the
    /// #5428 ci-packet step replacement. #5409 moved the comment JSON work
    /// into `ripr pr-comments existing|requests`, the summary flags into the
    /// workflow env, and the setting docs into docs/CI.md (385 → 150 rendered
    /// lines), and re-measured the hash below.
    /// The unrendered template is the stable identity:
    /// rendering additionally substitutes the install version, the pin
    /// first line, and artifact paths, which the `generated_workflow_*` and
    /// `install_version_*` tests pin at the rendered level.
    const TEMPLATE_SHA256: &str =
        "1126e0e4b728332ff969abb54c422f65ef238e87d8aa0bfa104a8247e5de91b8";

    #[test]
    fn template_matches_the_pinned_bytes() {
        let hex: String = Sha256::digest(generated_workflow_template().as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(hex, TEMPLATE_SHA256);
    }

    #[test]
    fn development_replay_fixture_preserves_the_compact_command_contract() {
        let fixture = include_str!("../../../tests/fixtures/development_ci_workflow.yml");
        let workflow = fixture.lines().skip(2).collect::<Vec<_>>().join("\n") + "\n";
        assert_eq!(workflow, generated_development_workflow());
    }
}

#[cfg(test)]
mod install_version_tests {
    use super::{
        LATEST_RELEASED_VERSION, generated_workflow_for_version, parse_release_version,
        workflow_install_version,
    };

    #[test]
    fn installed_release_selects_only_qualified_command_surfaces() {
        for generator in ["0.10.0", "0.11.0-alpha.2", "unknown"] {
            let workflow = generated_workflow_for_version(generator);
            assert!(workflow.contains("          version=0.10.0\n"));
            assert!(workflow.contains("name: Verify installed RIPR compatibility"));
            assert!(workflow.contains("name: Evaluate RIPR gate decision"));
            assert!(!workflow.contains("@RIPR_"));
            for unsupported in [
                "ripr reports ci-packet",
                "ripr reports ci-summary",
                "ripr pr-comments existing",
                "ripr pr-comments requests",
                "--check-output",
                "--attempt",
            ] {
                assert!(
                    !workflow.contains(unsupported),
                    "{generator}: {unsupported}"
                );
            }
            assert!(!workflow.contains("ripr agent receipt"));
            assert!(!workflow.contains("ripr outcome"));
            assert!(!workflow.contains("cargo xtask"));
        }
    }

    #[test]
    fn unqualified_historical_install_versions_fail_before_product_commands() -> Result<(), String>
    {
        for generator in ["0.9.0", "0.1.0"] {
            let workflow = generated_workflow_for_version(generator);
            let reject = workflow
                .find("name: Reject unsupported RIPR command contract")
                .ok_or("missing explicit unsupported-version boundary")?;
            let install = workflow
                .find("name: Install ripr")
                .ok_or("missing install")?;
            let cleanup = workflow
                .find("name: Remove checked-in RIPR artifacts")
                .ok_or("missing untrusted artifact cleanup")?;
            assert!(cleanup < reject && reject < install);
            assert!(workflow.contains("No qualified workflow command template"));
            assert!(!workflow.contains("ripr reports ci-packet"));
            assert!(!workflow.contains("ripr reports ci-summary"));
            assert!(!workflow.contains("name: Evaluate RIPR gate decision"));
        }
        Ok(())
    }

    /// Released generators pin themselves, whatever the release (#5208).
    /// The constant itself is in the loop: the release commit carries
    /// `own == latest`, so a published generator must self-pin rather than
    /// fall back to its predecessor (#5244 review).
    #[test]
    fn install_version_pins_the_generator_when_released() {
        for version in ["0.10.0", "0.9.0", "0.1.0", LATEST_RELEASED_VERSION] {
            assert_eq!(workflow_install_version(version), version, "{version}");
        }
    }

    /// A version that always exceeds the release constant, so this test
    /// stays unreleased no matter how far the constant advances (a hardcoded
    /// `0.11.0` would rot into a released version at the next bump).
    fn version_beyond_latest_release() -> Result<String, String> {
        let (major, minor, _) = parse_release_version(LATEST_RELEASED_VERSION)
            .ok_or_else(|| "LATEST_RELEASED_VERSION must parse".to_string())?;
        Ok(format!("{major}.{}.0", minor + 1))
    }

    /// Unreleased generators pin the latest release, never themselves (#5208).
    /// Only the derived version: a hardcoded future version (however far
    /// off) rots into a released version when the constant reaches it
    /// (#5244 review).
    #[test]
    fn install_version_pins_the_latest_release_when_unreleased() -> Result<(), String> {
        let version = version_beyond_latest_release()?;
        assert_eq!(
            workflow_install_version(&version),
            LATEST_RELEASED_VERSION,
            "{version}"
        );
        Ok(())
    }

    /// Unknown shapes fail toward the resolvable pin (#5208).
    #[test]
    fn install_version_treats_unparseable_versions_as_unreleased() {
        for version in ["", "garbage", "0.10", "v0.9.0", "0.10.0-rc1", "1.2.3.4"] {
            assert_eq!(
                workflow_install_version(version),
                LATEST_RELEASED_VERSION,
                "{version:?}"
            );
        }
    }

    /// Compare a stable release with Cargo's admitted numeric core. A release
    /// equal to a prerelease core still leads that prerelease; this helper does
    /// not admit prerelease strings into the production stable-pin parser.
    fn release_is_not_ahead_of_cargo_package(
        released: &str,
        package_release: &str,
        package_has_prerelease: bool,
    ) -> Option<bool> {
        let released = parse_release_version(released)?;
        let package = parse_release_version(package_release)?;
        Some(released < package || (released == package && !package_has_prerelease))
    }

    #[test]
    fn release_ordering_distinguishes_stable_and_prerelease_packages() {
        for (released, package, prerelease, expected) in [
            ("0.10.0", "0.10.0", false, true),
            ("0.10.0", "0.10.0", true, false),
            ("0.10.0", "0.11.0", true, true),
            ("0.10.0", "0.11.0", false, true),
            ("0.10.0", "0.9.0", true, false),
            ("0.10.0", "0.9.0", false, false),
        ] {
            assert_eq!(
                release_is_not_ahead_of_cargo_package(released, package, prerelease),
                Some(expected),
                "released={released} package={package} prerelease={prerelease}"
            );
        }
        for invalid in [
            "",
            "garbage",
            "0.10",
            "v0.10.0",
            "0.10.0-alpha.2",
            "1.2.3.4",
        ] {
            assert_eq!(
                release_is_not_ahead_of_cargo_package(invalid, "0.11.0", true),
                None,
                "invalid release {invalid:?}"
            );
            assert_eq!(
                release_is_not_ahead_of_cargo_package("0.10.0", invalid, true),
                None,
                "invalid numeric package core {invalid:?}"
            );
        }
    }

    /// The constant cannot lead Cargo's admitted package identity. Equality is
    /// valid for a stable package, while the same-core prerelease remains below
    /// the stable release. Cargo owns the core and prerelease components.
    #[test]
    fn latest_released_constant_is_ordered_behind_the_package() -> Result<(), String> {
        let package_release = concat!(
            env!("CARGO_PKG_VERSION_MAJOR"),
            ".",
            env!("CARGO_PKG_VERSION_MINOR"),
            ".",
            env!("CARGO_PKG_VERSION_PATCH")
        );
        let ordered = release_is_not_ahead_of_cargo_package(
            LATEST_RELEASED_VERSION,
            package_release,
            !env!("CARGO_PKG_VERSION_PRE").is_empty(),
        )
        .ok_or_else(|| "release constant and Cargo numeric package core must parse".to_string())?;
        assert!(
            ordered,
            "LATEST_RELEASED_VERSION ({LATEST_RELEASED_VERSION}) leads the package ({})",
            env!("CARGO_PKG_VERSION")
        );
        Ok(())
    }

    #[test]
    fn prerelease_rendering_retains_only_the_released_install_pin() {
        for version in ["0.11.0-alpha.2", "0.10.0-alpha.2", "0.9.0-alpha.2"] {
            assert_eq!(parse_release_version(version), None, "{version}");
            assert_eq!(
                workflow_install_version(version),
                LATEST_RELEASED_VERSION,
                "{version}"
            );
            let workflow = generated_workflow_for_version(version);
            assert!(workflow.contains(&format!("          version={LATEST_RELEASED_VERSION}\n")));
            assert!(workflow.contains(&format!(
                "cargo install ripr --version {LATEST_RELEASED_VERSION} --locked"
            )));
            assert!(workflow.contains(&format!(
                "      # Pinned to released ripr {LATEST_RELEASED_VERSION} ({version} is unreleased). The steps below use\n"
            )));
            assert!(!workflow.contains(&format!("          version={version}\n")));
            assert!(
                !workflow.contains(&format!("cargo install ripr --version {version} --locked"))
            );
        }
    }

    /// Released rendering keeps the historical pin comment first line and
    /// self-pin byte-for-byte, on both install routes (#5208, #5236).
    #[test]
    fn released_rendering_pins_itself_with_the_historical_comment() {
        let workflow = generated_workflow_for_version("0.10.0");
        assert!(
            workflow.contains("          version=0.10.0\n"),
            "missing prebuilt self pin"
        );
        assert!(
            workflow.contains("cargo install ripr --version 0.10.0 --locked"),
            "missing fallback self pin"
        );
        assert!(
            workflow.contains(
                "      # Pinned to the ripr that generated this workflow. The steps below use\n"
            ),
            "missing historical comment"
        );
        assert!(!workflow.contains("@RIPR_"), "unsubstituted placeholder");
        let installs: Vec<&str> = workflow
            .lines()
            .filter(|line| line.contains("cargo install ripr"))
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect();
        assert_eq!(installs.len(), 1, "{installs:?}");
    }

    /// Unreleased rendering pins the latest release on both install routes
    /// and says why (#5208, #5236).
    #[test]
    fn unreleased_rendering_pins_the_latest_release_with_an_honest_comment() -> Result<(), String> {
        let future = version_beyond_latest_release()?;
        let workflow = generated_workflow_for_version(&future);
        assert!(
            workflow.contains(&format!("          version={LATEST_RELEASED_VERSION}\n")),
            "must pin the latest release for the prebuilt download"
        );
        assert!(
            workflow.contains(&format!(
                "cargo install ripr --version {LATEST_RELEASED_VERSION} --locked"
            )),
            "must pin the latest release for the cargo fallback"
        );
        assert!(
            !workflow.contains(&format!("version={future}"))
                && !workflow.contains(&format!("--version {future}")),
            "must not name the unreleased version"
        );
        assert!(
            workflow.contains(&format!(
                "      # Pinned to released ripr {LATEST_RELEASED_VERSION} ({future} is unreleased). The steps below use\n"
            )),
            "missing honest comment"
        );
        assert!(
            !workflow.contains("Pinned to the ripr that generated this workflow"),
            "must not keep the self-pin claim"
        );
        assert!(!workflow.contains("@RIPR_"), "unsubstituted placeholder");
        let installs: Vec<&str> = workflow
            .lines()
            .filter(|line| line.contains("cargo install ripr"))
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect();
        assert_eq!(installs.len(), 1, "{installs:?}");
        Ok(())
    }
}
