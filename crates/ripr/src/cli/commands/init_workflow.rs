//! The generated `ripr init --ci github` workflow template.
//!
//! #4386 (slice 1): the GitHub Actions workflow `ripr init` writes used to be
//! a ~2,400-line raw string inline in `init.rs`, dwarfing the command logic
//! around it. This module owns the generated workflow bytes; `init.rs` keeps
//! only the init command surface. The template is stored as a head, the
//! advisory-summary step ([`advisory_summary`], now one `ripr reports
//! ci-summary` call), and a tail, spliced in
//! source order; the assembled bytes are pinned by hash in the test below.
//! `generated_github_actions_workflow` substitutes `@RIPR_...@` placeholders
//! at render time; rendered behavior stays pinned by the
//! `generated_workflow_*` tests in `init.rs` and `commands.rs`, the
//! `tests/generated_review_workflow.rs` replay, and `cargo xtask
//! check-workflows`.

#[path = "init_workflow/advisory_summary.rs"]
mod advisory_summary;

use crate::agent::loop_commands;

use advisory_summary::ADVISORY_SUMMARY_STEP;

/// The template up to the `Add RIPR advisory summary` step, ending with the
/// blank line before it.
const TEMPLATE_HEAD: &str = r#"name: RIPR

on:
  pull_request:
    # `labeled` and `unlabeled` re-run the gate when a waiver label such as
    # `ripr-waive` is added or removed, since labels are read from the event.
    # Any label change re-runs the job; the concurrency group below cancels
    # the superseded run.
    types: [opened, synchronize, reopened, labeled, unlabeled]
  workflow_dispatch:

permissions:
  contents: read
  # Used only when RIPR_COMMENT_MODE is `inline`, to post review comments.
  # With the default `off`, nothing writes to the pull request. Set this to
  # `read` if you keep RIPR_COMMENT_MODE at `off`.
  pull-requests: write
  # Used only to upload SARIF to code scanning while RIPR_UPLOAD_SARIF is
  # "true" (the default). Remove this line and set RIPR_UPLOAD_SARIF to
  # "false" if the repository does not use code scanning.
  security-events: write

env:
  # Upload SARIF to GitHub Security tab when true. Disable with
  # RIPR_UPLOAD_SARIF=false if your repo does not use code scanning.
  RIPR_UPLOAD_SARIF: "true"
  # Gate authority for this workflow. Configure as a GitHub Actions
  # repository variable (Settings > Secrets and variables > Actions >
  # Variables). Empty (default) = advisory only, the job never fails.
  # Allowed values:
  #   visible-only     gate runs and prints, but does not block the job
  #   acknowledgeable  gate runs; PR author can acknowledge to merge
  #   baseline-check   gate fails if exposure is worse than the baseline
  #   calibrated-gate  gate fails only on new, high-confidence,
  #                    policy-eligible gaps; needs baseline and
  #                    calibration inputs
  # See docs/CALIBRATED_GATE_POLICY.md for the full policy.
  RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}
  # Optional path to a reviewed baseline ledger file, such as
  # .ripr/gate-baseline.json, that baseline-check and calibrated-gate
  # compare current evidence against. Empty by default.
  RIPR_GATE_BASELINE: ${{ vars.RIPR_GATE_BASELINE || '' }}
  # PR review-comment publishing. Configure as a repository variable.
  # Allowed values:
  #   off     (default) no PR comments; findings only in artifacts
  #   plan    compute and publish a comment plan; do not post inline
  #   inline  publish inline review comments on changed lines (needs
  #           pull-requests: write, which this workflow grants)
  RIPR_COMMENT_MODE: ${{ vars.RIPR_COMMENT_MODE || 'off' }}

# Every run step is bash (arrays, mktemp, [ -f ]). Pin the shell so the
# steps still parse if a job is moved to windows-latest, whose default run
# shell is PowerShell.
defaults:
  run:
    shell: bash

# One run per PR: a newer push cancels the older run. Only the newest head's
# placements are valid, and two overlapping runs would each snapshot the
# existing inline comments before either publishes, then both create the
# same cards.
concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true

jobs:
  ripr:
    name: RIPR advisory reports
    runs-on: ubuntu-latest
    # The whole job is advisory (continue-on-error) unless RIPR_GATE_MODE
    # is set to a blocking value. With the default empty/visible-only mode
    # a failure here never fails the PR — set RIPR_GATE_MODE to opt in to
    # blocking behaviour. See docs/CALIBRATED_GATE_POLICY.md.
    continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}
    steps:
      # Analyze the PR head, not the default `refs/pull/N/merge` commit.
      # Review comments and `::warning` annotations are placed on the PR
      # head's lines; when the base branch has moved lines in a changed
      # file, merge-commit line numbers point at the wrong line, and GitHub
      # rejects the whole review when a line falls outside the PR diff.
      # upload-sarif detects the head checkout and reports it as
      # refs/pull/N/head. A manual run keeps the dispatched commit.
      # No step pushes or fetches after checkout, so the job token is not
      # left in .git/config where PR-controlled code (build scripts run by
      # `cargo`, analyzed sources) could read it.
      - uses: actions/checkout@v6
        with:
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
          fetch-depth: 0
          persist-credentials: false

      # Every RIPR input under target/ripr and target/ci must come from this
      # run. The gate, ledger, and policy steps read several files there only
      # when present (sarif-policy, agent-verify, agent-receipt, calibration,
      # coverage), and nothing in this workflow writes some of them, so a
      # pull request could commit forged copies (`git add -f`). Remove both
      # directories before the first RIPR step; steps you add later that
      # write there still work. ripr's analysis cache lives outside the
      # checkout (RIPR_CACHE_DIR, below), so this never discards it.
      - name: Remove checked-in RIPR artifacts
        run: rm -rf target/ripr target/ci

@RIPR_PIN_FIRST_LINE@
      # that version's commands and flags; an unpinned install takes the
      # newest release, whose CLI may not match. To upgrade, install the
      # newer ripr and compare `ripr init --ci github --force --dry-run`
      # with this file.
      #
      # Downloads that release's prebuilt binary from GitHub Releases and
      # checks it against the release's published SHA-256: seconds, where
      # compiling ripr takes minutes. With no prebuilt binary for this
      # runner (Windows, or a download failure), it falls back to
      # `cargo install`, which needs Rust on the runner; without cargo the
      # step fails and says how to fix it. A checksum mismatch fails the step
      # instead. The summary step reads this step's outcome by its id.
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
          bin_dir="$RUNNER_TEMP/ripr-bin"
          mkdir -p "$bin_dir"
          if [ -n "$target" ] &&
            curl -fsSL --retry 3 -o "$RUNNER_TEMP/$asset" "$url" &&
            curl -fsSL --retry 3 -o "$RUNNER_TEMP/$asset.sha256" "$url.sha256"; then
            expected="$(awk 'NR == 1 { print $1 }' "$RUNNER_TEMP/$asset.sha256")"
            actual="$( { sha256sum "$RUNNER_TEMP/$asset" 2>/dev/null || shasum -a 256 "$RUNNER_TEMP/$asset"; } | awk '{ print $1 }')"
            if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
              echo "::error::$asset does not match its published SHA-256 (expected ${expected:-nothing}, got $actual)"
              exit 1
            fi
            tar -xzf "$RUNNER_TEMP/$asset" -C "$bin_dir"
            echo "$bin_dir" >> "$GITHUB_PATH"
          else
            why="no prebuilt ripr $version for $RUNNER_OS-$RUNNER_ARCH"
            if [ -n "$target" ]; then why="downloading $url failed"; fi
            if ! command -v cargo >/dev/null 2>&1; then
              echo "::error::Cannot install ripr: $why, and this runner has no cargo to build it. Install Rust on the runner (https://rustup.rs) or add a Rust toolchain step before Install ripr."
              exit 1
            fi
            echo "::notice::$why; building it with cargo install"
            cargo install ripr --version @RIPR_VERSION@ --locked
          fi
          PATH="$bin_dir:$PATH" ripr --version
          echo "RIPR_CACHE_DIR=$RUNNER_TEMP/ripr-cache" >> "$GITHUB_ENV"

      # Restores ripr's analysis cache, so a new push to a pull request
      # reuses the facts of files it did not change. Entries are keyed on
      # file contents, configuration, and the ripr version: an entry that no
      # longer matches is a miss, never stale evidence. GitHub scopes a
      # cache a pull request saves to that pull request, and the cache lives
      # outside the checkout, so a pull request cannot commit one. Pinned to
      # a commit SHA: this job holds a token with write scopes.
      # actions/cache v6.1.0 = 55cc8345863c7cc4c66a329aec7e433d2d1c52a9.
      - uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9
        with:
          path: ${{ runner.temp }}/ripr-cache
          key: ripr-cache-@RIPR_VERSION@-${{ runner.os }}-${{ github.event.pull_request.head.sha || github.sha }}
          restore-keys: |
            ripr-cache-@RIPR_VERSION@-${{ runner.os }}-

      - name: Capture existing RIPR inline comments
        if: always() && github.event_name == 'pull_request' && env.RIPR_COMMENT_MODE != 'off'
        continue-on-error: true
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          mkdir -p target/ripr/review
          gh api --paginate --slurp "repos/${{ github.repository }}/pulls/${{ github.event.pull_request.number }}/comments" \
            > target/ripr/review/existing-comments.raw.json
          jq '{
            schema_version: "0.1",
            tool: "ripr",
            kind: "pr_inline_comment_existing_comments",
            comments: [
              .[]?[]?
              # Only comments this workflow posted: it publishes with
              # github.token, whose author is github-actions[bot]. Anyone can
              # write the marker; a marked comment from another author must
              # not suppress a RIPR card or be PATCHed by this job.
              | select(.user.login == "github-actions[bot]" and .user.type == "Bot")
              | select((.body // "") | contains("<!-- ripr:dedupe="))
              | (.body // "") as $body
              | {
                  comment_id: .id,
                  dedupe_key: ($body | capture("<!-- ripr:dedupe=(?<key>.*?)(?: presentation=[^ ]+)? -->").key),
                  path: .path,
                  line: (.line // .original_line),
                  side: (.side // "RIGHT"),
                  body: (
                    if ($body | contains(" presentation=compact-v1 -->")) then
                      (($body | [capture("<details><summary>Full RIPR repair card</summary>\n\n(?<card>.*)\n\n</details>"; "m").card][0]) // "__ripr_compact_presentation_unreadable__")
                    else
                      "__ripr_legacy_presentation__"
                    end
                  ),
                  outdated: (.position == null and .line == null)
                }
            ]
          }' target/ripr/review/existing-comments.raw.json \
            > target/ripr/review/existing-comments.json

      # One command runs the RIPR steps in order: the pilot packet, the
      # agent-loop start, the PR diff capture and guidance, the inline
      # comment plan, SARIF and badge renders, the gate when RIPR_GATE_MODE
      # is set, the policy and PR ledgers, start-here, the report index, and
      # changed-line annotations. Each step prints as a log group, and an
      # advisory step's failure is logged without stopping the rest. The
      # command fails when the diff capture or the gate fails, or when a gate
      # input fails under a blocking RIPR_GATE_MODE. It reads no token; the
      # comment steps around it hold that. `ripr help reports` has the details.
      - name: Run RIPR
        run: ripr reports ci-packet --root .

      - name: Publish RIPR inline comments
        if: always() && github.event_name == 'pull_request' && env.RIPR_COMMENT_MODE == 'inline' && hashFiles('target/ripr/review/comment-publish-plan.json') != ''
        continue-on-error: true
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          plan=target/ripr/review/comment-publish-plan.json
          if ! jq -e '.summary.safe_to_publish == true' "$plan" >/dev/null; then
            echo "RIPR inline comments were not published because the publish plan is not safe."
            # Messages can quote repository paths; fold CR/LF so a path
            # cannot start a new line that GitHub reads as a workflow command.
            jq -r '.blocked[]? | "- \(.blocked_reason): \(.message)" | gsub("[\r\n]"; " ")' "$plan" || true
            exit 0
          fi

          publishable="$(mktemp)"
          jq '
            def captured($regex; $flags): [capture($regex; $flags).value][0] // null;
            def code_span_line($label): captured("\n" + $label + ":\n(?<value>(?<fence>`+)[^`\n](?:[^\n]*[^`\n])?\\k<fence>)(?:\n|$)"; "");
            def compact_body:
              .body as $full
              | ($full | captured("^### ripr gap: (?<value>[^\n]+)"; "") // "repairable gap") as $gap
              | ($full | captured("\nRepair:\n(?<value>[^\n]+)"; "") // "Follow the bounded repair route in the RIPR artifact.") as $repair
              | ($full | code_span_line("Start the repair")) as $start
              | ($full | code_span_line("Verify") // "`ripr agent verify`") as $verify
              | (if $start then "Start the repair: \($start)" else "Verify: \($verify)" end) as $next
              | "**ripr: \($gap)** — \($repair)\n\n\($next)\n\n<details><summary>Full RIPR repair card</summary>\n\n\($full)\n\n</details>\n\n<!-- ripr:dedupe=\(.dedupe_key) presentation=compact-v1 -->";
            [
              .operations[]?
              | select(.safe_to_publish == true)
              | select(.operation == "create" or .operation == "update" or .operation == "keep")
              | . + {published_body: compact_body}
            ]
          ' "$plan" > "$publishable"

          review_body="$(jq -r '
            (.summary.publishable // 0) as $inline
            | ((.summary.summary_only // 0) + ([.skipped[]? | select(.skip_reason == "inline_comment_cap_reached" or .skip_reason == "comment_body_too_large")] | length)) as $additional
            | (.summary.suppressed // 0) as $suppressed
            | (if $inline == 1 then "" else "s" end) as $inline_suffix
            | (if $additional == 1 then "" else "s" end) as $additional_suffix
            | (if $suppressed == 1 then "" else "s" end) as $suppressed_suffix
            | "RIPR surfaced \($inline) line-placed recommendation\($inline_suffix)."
              + (if $additional > 0 then "\n\n\($additional) additional recommendation\($additional_suffix) remain in the generated `target/ripr/review/comments.json` and `target/ripr/review/comments.md` artifacts." else "" end)
              + (if $suppressed > 0 then "\n\n\($suppressed) suppressed recommendation\($suppressed_suffix) remain visible there with reasons." else "" end)
              + "\n\nAdvisory static evidence only; gate authority remains separate."
          ' "$plan")"

          create_count="$(jq '[.[] | select(.operation == "create")] | length' "$publishable")"
          update_count="$(jq '[.[] | select(.operation == "update")] | length' "$publishable")"
          additional_count="$(jq '(.summary.summary_only // 0) + ([.skipped[]? | select(.skip_reason == "inline_comment_cap_reached" or .skip_reason == "comment_body_too_large")] | length)' "$plan")"
          suppressed_count="$(jq '.summary.suppressed // 0' "$plan")"

          jq -c '.[] | select(.operation == "update")' "$publishable" \
            | while IFS= read -r operation; do
                comment_id="$(jq -r '.existing_comment_id' <<< "$operation")"
                dedupe_key="$(jq -r '.dedupe_key | tostring | gsub("[\r\n]"; " ")' <<< "$operation")"
                body="$(jq -r '.published_body' <<< "$operation")"
                payload="$(mktemp)"
                jq -n --arg body "$body" '{body: $body}' > "$payload"
                gh api --method PATCH "repos/${{ github.repository }}/pulls/comments/$comment_id" --input "$payload" >/dev/null
                echo "Updated RIPR inline comment: $dedupe_key"
              done

          review_required=false
          if [ "$create_count" -gt 0 ] || { [ "$update_count" -gt 0 ] && { [ "$additional_count" -gt 0 ] || [ "$suppressed_count" -gt 0 ]; }; }; then
            review_required=true
          fi
          if [ "$review_required" = true ]; then
            payload="$(mktemp)"
            jq -n \
              --arg body "$review_body" \
              --arg commit_id "${{ github.event.pull_request.head.sha }}" \
              --argjson create_count "$create_count" \
              --slurpfile operations "$publishable" \
              '({
                body: $body,
                event: "COMMENT",
                commit_id: $commit_id
              } + if $create_count > 0 then {
                comments: [
                  $operations[0][]
                  | select(.operation == "create")
                  | {
                      path: .placement.path,
                      line: .placement.line,
                      side: (.placement.side // "RIGHT"),
                      body: .published_body
                    }
                ]
              } else {} end)' > "$payload"
            gh api --method POST "repos/${{ github.repository }}/pulls/${{ github.event.pull_request.number }}/reviews" --input "$payload" >/dev/null
            if [ "$create_count" -gt 0 ]; then
              echo "Created one RIPR review with $create_count inline comment(s)."
            else
              echo "Created one RIPR review summary after $update_count inline comment update(s)."
            fi
          fi

          jq -r '.[] | select(.operation == "keep") | .dedupe_key | tostring | gsub("[\r\n]"; " ")' "$publishable" \
            | while IFS= read -r dedupe_key; do
                echo "RIPR inline comment already current: $dedupe_key"
              done

"#;

/// The template from the first upload step through the last.
const TEMPLATE_TAIL: &str = r#"      - name: Upload RIPR report artifacts
        if: always()
        continue-on-error: true
        uses: actions/upload-artifact@v7
        with:
          name: ripr-reports
          path: |
            target/ripr/pilot
            target/ripr/agent
            target/ripr/workflow
            target/ripr/reports
            target/ripr/review
            target/ci
          if-no-files-found: ignore
          retention-days: 14

      - name: Upload RIPR diff findings
        if: always() && env.RIPR_UPLOAD_SARIF == 'true' && github.event_name == 'pull_request' && hashFiles('target/ripr/reports/ripr-findings.sarif') != ''
        # Upload infra is not analysis authority (#2009 review): a CodeQL
        # flake must not fail a gate the analysis passed. Renders (the
        # analysis) stay gate-conditional; uploads stay advisory.
        continue-on-error: true
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
fn generated_workflow_template() -> String {
    TEMPLATE_HEAD.to_owned() + ADVISORY_SUMMARY_STEP + TEMPLATE_TAIL
}

/// Newest ripr release on crates.io. `init --ci github` pins this in the
/// generated workflow when the generating binary is NEWER (unreleased), so
/// the install step always resolves (#5208). Released generators pin
/// themselves and render byte-identical output to before.
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
/// keep it so their output is unchanged (#5208). Only the first line is
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
            "      # Pinned to released ripr {pinned}: the generating ripr ({generator_version}) is unreleased."
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
    use super::generated_workflow_template;
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
    /// #5428 ci-packet step replacement, and re-measured the hash below.
    /// The unrendered template is the stable identity:
    /// rendering additionally substitutes the install version, the pin
    /// first line, and artifact paths, which the `generated_workflow_*` and
    /// `install_version_*` tests pin at the rendered level.
    const TEMPLATE_SHA256: &str =
        "5c7deac5ccfe5b7c29a127108eb536d19f77f14d6905a62e26c88ca4c9eff44b";

    #[test]
    fn template_matches_the_pinned_bytes() {
        let hex: String = Sha256::digest(generated_workflow_template().as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(hex, TEMPLATE_SHA256);
    }
}

#[cfg(test)]
mod install_version_tests {
    use super::{
        LATEST_RELEASED_VERSION, generated_workflow_for_version, parse_release_version,
        workflow_install_version,
    };

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

    /// The constant must parse, and must never lead the package version: a
    /// constant ahead of the package would self-pin unreleased generators
    /// and silently defeat #5208. It travels with the package version in
    /// the release commit (see docs/RELEASE.md Post-Publish); the equality
    /// case is the published generator self-pinning, not a violation.
    #[test]
    fn latest_released_constant_is_ordered_behind_the_package() -> Result<(), String> {
        let latest = parse_release_version(LATEST_RELEASED_VERSION)
            .ok_or_else(|| "LATEST_RELEASED_VERSION must parse".to_string())?;
        let package = parse_release_version(env!("CARGO_PKG_VERSION"))
            .ok_or_else(|| "CARGO_PKG_VERSION must parse".to_string())?;
        assert!(
            latest <= package,
            "LATEST_RELEASED_VERSION ({LATEST_RELEASED_VERSION}) leads the package ({})",
            env!("CARGO_PKG_VERSION")
        );
        Ok(())
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
                "      # Pinned to released ripr {LATEST_RELEASED_VERSION}: the generating ripr ({future}) is unreleased.\n"
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
