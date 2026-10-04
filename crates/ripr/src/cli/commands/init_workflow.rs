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

      # Pinned to the ripr that generated this workflow. The steps below use
      # that version's commands and flags; an unpinned install takes the
      # newest release, whose CLI may not match. To upgrade, install the
      # newer ripr and compare `ripr init --ci github --force --dry-run`
      # with this file.
      #
      # Downloads that release's prebuilt binary from GitHub Releases and
      # checks it against the release's published SHA-256: seconds, where
      # compiling ripr takes minutes. With no prebuilt binary for this
      # runner (Windows, or a download failure), it falls back to
      # `cargo install`. A checksum mismatch fails the step instead.
      - name: Install ripr
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
            echo "::notice::No prebuilt ripr $version downloaded for $RUNNER_OS-$RUNNER_ARCH; building it with cargo install"
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

      - name: Generate RIPR pilot packet
        continue-on-error: true
        run: |
          ripr pilot \
            --root . \
            --out target/ripr/pilot \
            --mode ready \
            --max-seams 5

      - name: Prepare RIPR editor-agent artifacts
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports target/ripr/agent target/ripr/workflow
          if [ -f target/ripr/pilot/repo-exposure.json ]; then
            cp target/ripr/pilot/repo-exposure.json target/ripr/reports/repo-exposure.json
            cp target/ripr/pilot/repo-exposure.json target/ripr/workflow/before.repo-exposure.json
          fi
          if [ -f target/ripr/pilot/agent-seam-packets.json ]; then
            cp target/ripr/pilot/agent-seam-packets.json target/ripr/workflow/agent-seam-packets.json
          fi
          if [ -f target/ripr/pilot/pilot-summary.json ]; then
            top_seam_id="$(jq -r '.top_actionable_seams[0].seam_id // empty' target/ripr/pilot/pilot-summary.json 2>/dev/null || true)"
            if [ -n "$top_seam_id" ] && [ "$top_seam_id" != "null" ]; then
              echo "RIPR_TOP_SEAM_ID=$top_seam_id" >> "$GITHUB_ENV"
            fi
          fi

      - name: Generate RIPR agent loop artifacts
        if: always() && env.RIPR_TOP_SEAM_ID != ''
        continue-on-error: true
        # CI writes the before side of the repair loop only: the workflow
        # manifest, brief, and packet the focused-test edit starts from.
        # The after snapshot, verify, and receipt need that edit between
        # the snapshots, so the repair's `--attempt ... --phase after`
        # command produces them where the edit happens (#3906). The packet
        # lands through a temporary file so a failed render never leaves an
        # empty JSON artifact for later steps or the upload.
        run: |
          ripr agent start \
            --root . \
            --seam-id "$RIPR_TOP_SEAM_ID" \
            --out target/ripr/workflow
          packet_tmp="$(mktemp)"
          ripr agent packet \
            --root . \
            --seam-id "$RIPR_TOP_SEAM_ID" \
            --json \
            > "$packet_tmp"
          mv "$packet_tmp" target/ripr/workflow/agent-packet.json
          cp target/ripr/workflow/agent-packet.json target/ripr/agent/agent-packet.json
          cp target/ripr/workflow/agent-brief.json target/ripr/agent/agent-brief.json

      - name: Render RIPR gap decision ledger
        if: always() && hashFiles('target/ripr/reports/repo-exposure.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ripr reports gap-ledger \
            --root . \
            --repo-exposure target/ripr/reports/repo-exposure.json \
            --out target/ripr/reports/gap-decision-ledger.json \
            --out-md target/ripr/reports/gap-decision-ledger.md

      - name: Capture pull request diff
        if: github.event_name == 'pull_request'
        run: |
          mkdir -p target/ripr/reports
          # Pinned diff contract (#4005): the same presentation pins as the
          # production loaders. Ambient external-diff, textconv, color,
          # context, path-quoting, and side-prefix configuration must not
          # change the bytes RIPR analyzes.
          base_ref="origin/${{ github.base_ref }}"
          base_sha="$(git rev-parse --verify "${base_ref}^{commit}")" || { echo "ripr: cannot resolve base ref $base_ref" >&2; exit 1; }
          head_sha="$(git rev-parse --verify "HEAD^{commit}")" || { echo "ripr: cannot resolve HEAD" >&2; exit 1; }
          git -c core.quotePath=true diff --binary --no-ext-diff --no-textconv --no-color --src-prefix=a/ --dst-prefix=b/ --unified=3 --inter-hunk-context=0 "${base_sha}...${head_sha}" > target/ripr/reports/pr.diff || { echo "ripr: git diff failed for ${base_sha}...${head_sha}" >&2; exit 1; }
          byte_count="$(wc -c < target/ripr/reports/pr.diff | tr -d ' ')"
          digest="$(sha256sum target/ripr/reports/pr.diff)" || {
            echo "ripr: failed to compute SHA-256 for patch" >&2
            exit 1
          }
          digest="${digest%% *}"
          jq -n --arg base_ref "$base_ref" --arg base_sha "$base_sha" --arg head_sha "$head_sha" --argjson byte_count "$byte_count" --arg digest "$digest" '{tool:"ripr",kind:"pr-diff-receipt",base_ref:$base_ref,base_sha:$base_sha,head_sha:$head_sha,byte_count:$byte_count,sha256:$digest}' > target/ripr/reports/pr-diff.receipt.json
          if [ "$byte_count" -eq 0 ]; then
            name_list="$(mktemp)" || { echo "ripr: cannot create temp file for path inventory" >&2; exit 1; }
            git -c core.quotePath=true diff --name-only -z "${base_sha}...${head_sha}" > "$name_list" || { echo "ripr: git diff --name-only failed for ${base_sha}...${head_sha}" >&2; exit 1; }
            changed_paths="$(tr -cd '\0' < "$name_list" | wc -c | tr -d ' ')"
            rm -f "$name_list"
            if [ "$changed_paths" -ne 0 ]; then
              echo "ripr: empty patch but $changed_paths changed path(s); refusing an absent result" >&2
              exit 1
            fi
          fi

      - name: Run RIPR PR guidance report
        if: github.event_name == 'pull_request'
        # Gate-critical producer (#2009): advisory by default, but a
        # blocking RIPR_GATE_MODE must not green-on-error past the gate's
        # own input.
        continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}
        run: |
          mkdir -p target/ripr/pr target/ripr/review
          check_status=0
          ripr check \
            --root . \
            --base "origin/${{ github.base_ref }}" \
            --format json > target/ripr/pr/check.json || check_status=$?
          if [ "$check_status" -ne 0 ]; then
            echo "RIPR check did not produce a complete result (exit $check_status); review-comments will fail closed on the named artifact."
          fi
          ripr review-comments \
            --root . \
            --base "origin/${{ github.base_ref }}" \
            --head HEAD \
            --check-output target/ripr/pr/check.json \
            --out target/ripr/review/comments.json

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

      - name: Plan RIPR inline comments
        if: always() && github.event_name == 'pull_request' && env.RIPR_COMMENT_MODE != 'off' && hashFiles('target/ripr/review/comments.json') != ''
        continue-on-error: true
        env:
          GH_TOKEN: ${{ github.token }}
          RIPR_ACTOR: ${{ github.actor }}
          RIPR_PR_AUTHOR: ${{ github.event.pull_request.user.login }}
        run: |
          mkdir -p target/ripr/review
          comment_args=(
            pr-comments plan
            --root .
            --pr-guidance target/ripr/review/comments.json
            --mode "$RIPR_COMMENT_MODE"
            --event-name "${{ github.event_name }}"
            --pull-request "${{ github.event.pull_request.number }}"
            --head-repo "${{ github.event.pull_request.head.repo.full_name }}"
            --base-repo "${{ github.repository }}"
            --out target/ripr/review/comment-publish-plan.json
            --out-md target/ripr/review/comment-publish-plan.md
          )
          if [ -f target/ripr/review/existing-comments.json ]; then
            comment_args+=(--existing-comments target/ripr/review/existing-comments.json)
          fi
          if [ -n "${GH_TOKEN:-}" ]; then
            comment_args+=(--token-available)
          else
            comment_args+=(--no-token)
          fi
          # GitHub gives Dependabot runs a read-only token whatever the
          # permissions block says, so the plan must not claim write. Check
          # the PR author too: a maintainer who reopens a Dependabot PR is the
          # event actor, and the run can still carry the read-only token.
          if [ "${RIPR_ACTOR:-}" = "dependabot[bot]" ] || [ "${RIPR_PR_AUTHOR:-}" = "dependabot[bot]" ]; then
            comment_args+=(--no-write-permission)
          else
            comment_args+=(--write-permission)
          fi
          ripr "${comment_args[@]}"

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

      - name: Capture RIPR gate labels
        if: always() && github.event_name == 'pull_request'
        continue-on-error: true
        run: |
          mkdir -p target/ci
          jq -c '{labels: [.pull_request.labels[]?.name]}' "$GITHUB_EVENT_PATH" > target/ci/labels.json

      - name: Render RIPR diff SARIF
        if: env.RIPR_UPLOAD_SARIF == 'true' && github.event_name == 'pull_request'
        continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}
        run: |
          ripr check \
            --root . \
            --diff target/ripr/reports/pr.diff \
            --format sarif \
            > target/ripr/reports/ripr-findings.sarif

      - name: Render RIPR repo seam SARIF
        if: env.RIPR_UPLOAD_SARIF == 'true'
        continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}
        run: |
          mkdir -p target/ripr/reports
          ripr check \
            --root . \
            --mode ready \
            --format repo-sarif \
            > target/ripr/reports/ripr-seams.sarif

      - name: Render RIPR repo badge artifacts
        # These files are uploaded with this PR run; they do not update a
        # README badge endpoint on the default branch. To publish a badge,
        # set up a separate reviewed badge-refresh workflow as described at
        # https://github.com/EffortlessMetrics/ripr/blob/main/docs/BADGE_ADOPTION.md
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ripr check \
            --root . \
            --mode ready \
            --format repo-badge-json \
            > target/ripr/reports/repo-ripr-badge.json
          ripr check \
            --root . \
            --mode ready \
            --format repo-badge-shields \
            > target/ripr/reports/repo-ripr-badge-shields.json

      - name: Evaluate RIPR gate decision
        if: always() && env.RIPR_GATE_MODE != '' && hashFiles('target/ripr/review/comments.json') != ''
        run: |
          mkdir -p target/ripr/reports
          gate_args=(
            gate evaluate
            --root .
            --pr-guidance target/ripr/review/comments.json
            --mode "$RIPR_GATE_MODE"
            --out target/ripr/reports/gate-decision.json
            --out-md target/ripr/reports/gate-decision.md
          )
          if [ -f target/ripr/reports/repo-exposure.json ]; then
            gate_args+=(--repo-exposure target/ripr/reports/repo-exposure.json)
          fi
          if [ -f target/ci/labels.json ]; then
            gate_args+=(--labels-json target/ci/labels.json)
          fi
          if [ -f target/ripr/reports/sarif-policy.json ]; then
            gate_args+=(--sarif-policy target/ripr/reports/sarif-policy.json)
          fi
          if [ -f target/ripr/workflow/agent-verify.json ]; then
            gate_args+=(--agent-verify target/ripr/workflow/agent-verify.json)
          fi
          if [ -f target/ripr/reports/agent-receipt.json ]; then
            gate_args+=(--agent-receipt target/ripr/reports/agent-receipt.json)
          fi
          if [ -f target/ripr/reports/recommendation-calibration.json ]; then
            gate_args+=(--recommendation-calibration target/ripr/reports/recommendation-calibration.json)
          fi
          if [ -f target/ripr/reports/mutation-calibration.json ]; then
            gate_args+=(--mutation-calibration target/ripr/reports/mutation-calibration.json)
          fi
          if [ -n "${RIPR_GATE_BASELINE:-}" ]; then
            gate_args+=(--baseline "$RIPR_GATE_BASELINE")
          fi
          ripr "${gate_args[@]}"

      - name: Render RIPR baseline debt delta
        if: always() && env.RIPR_GATE_BASELINE != '' && hashFiles('target/ripr/reports/gate-decision.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ripr baseline diff \
            --baseline "$RIPR_GATE_BASELINE" \
            --current target/ripr/reports/gate-decision.json \
            --out target/ripr/reports/baseline-debt-delta.json \
            --out-md target/ripr/reports/baseline-debt-delta.md

      - name: Render RIPR Zero status
        if: always() && hashFiles('target/ripr/reports/baseline-debt-delta.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          zero_args=(
            zero status
            --delta target/ripr/reports/baseline-debt-delta.json
            --out target/ripr/reports/ripr-zero-status.json
            --out-md target/ripr/reports/ripr-zero-status.md
          )
          if [ -n "${RIPR_GATE_BASELINE:-}" ]; then
            zero_args+=(--baseline "$RIPR_GATE_BASELINE")
          fi
          if [ -f target/ripr/reports/gate-decision.json ]; then
            zero_args+=(--gate target/ripr/reports/gate-decision.json)
          fi
          if [ -f target/ripr/review/comments.json ]; then
            zero_args+=(--pr-guidance target/ripr/review/comments.json)
          fi
          if [ -f target/ripr/reports/recommendation-calibration.json ]; then
            zero_args+=(--recommendation-calibration target/ripr/reports/recommendation-calibration.json)
          fi
          ripr "${zero_args[@]}"

      - name: Render RIPR PR evidence ledger
        if: always() && github.event_name == 'pull_request' && hashFiles('target/ripr/review/comments.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ledger_args=(
            pr-ledger record
            --pr-number "${{ github.event.pull_request.number }}"
            --base "origin/${{ github.base_ref }}"
            --head HEAD
            --pr-guidance target/ripr/review/comments.json
            --out target/ripr/reports/pr-evidence-ledger.json
            --out-md target/ripr/reports/pr-evidence-ledger.md
          )
          if [ -f target/ripr/reports/gate-decision.json ]; then
            ledger_args+=(--gate target/ripr/reports/gate-decision.json)
          fi
          if [ -f target/ripr/reports/baseline-debt-delta.json ]; then
            ledger_args+=(--baseline-delta target/ripr/reports/baseline-debt-delta.json)
          fi
          if [ -f target/ripr/reports/ripr-zero-status.json ]; then
            ledger_args+=(--zero-status target/ripr/reports/ripr-zero-status.json)
          fi
          if [ -f target/ripr/reports/recommendation-calibration.json ]; then
            ledger_args+=(--recommendation-calibration target/ripr/reports/recommendation-calibration.json)
          fi
          if [ -f target/ripr/reports/agent-receipt.json ]; then
            ledger_args+=(--agent-receipt target/ripr/reports/agent-receipt.json)
          fi
          if [ -f target/ripr/reports/coverage-summary.json ]; then
            ledger_args+=(--coverage target/ripr/reports/coverage-summary.json)
          fi
          if [ -f .ripr/pr-evidence-ledger.jsonl ]; then
            ledger_args+=(--history .ripr/pr-evidence-ledger.jsonl)
          fi
          if [ -f target/ci/labels.json ]; then
            while IFS= read -r label; do
              if [ -n "$label" ] && [ "$label" != "null" ]; then
                ledger_args+=(--label "$label")
              fi
            done < <(jq -r '.labels[]? // empty' target/ci/labels.json 2>/dev/null || true)
          fi
          ripr "${ledger_args[@]}"

      - name: Render RIPR waiver aging
        if: always() && hashFiles('target/ripr/reports/pr-evidence-ledger.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          waiver_args=(
            policy waiver-aging
            --root .
            --ledger target/ripr/reports/pr-evidence-ledger.json
            --out target/ripr/reports/waiver-aging.json
            --out-md target/ripr/reports/waiver-aging.md
          )
          if [ -f .ripr/pr-evidence-ledger.jsonl ]; then
            waiver_args+=(--history .ripr/pr-evidence-ledger.jsonl)
          fi
          ripr "${waiver_args[@]}"

      - name: Render RIPR suppression health
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          suppression_args=(
            policy suppression-health
            --root .
            --out target/ripr/reports/suppression-health.json
            --out-md target/ripr/reports/suppression-health.md
          )
          ripr "${suppression_args[@]}"

      - name: Render RIPR policy readiness
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          policy_args=(
            policy readiness
            --root .
            --out target/ripr/reports/policy-readiness.json
            --out-md target/ripr/reports/policy-readiness.md
          )
          if [ -f target/ripr/reports/gate-decision.json ]; then
            policy_args+=(--gate-decision target/ripr/reports/gate-decision.json)
          fi
          if [ -f target/ripr/reports/baseline-debt-delta.json ]; then
            policy_args+=(--baseline-delta target/ripr/reports/baseline-debt-delta.json)
          fi
          if [ -f target/ripr/reports/recommendation-calibration.json ]; then
            policy_args+=(--recommendation-calibration target/ripr/reports/recommendation-calibration.json)
          fi
          if [ -f target/ripr/reports/mutation-calibration.json ]; then
            policy_args+=(--mutation-calibration target/ripr/reports/mutation-calibration.json)
          fi
          if [ -f target/ripr/reports/waiver-aging.json ]; then
            policy_args+=(--waiver-aging target/ripr/reports/waiver-aging.json)
          fi
          if [ -f target/ripr/reports/suppression-health.json ]; then
            policy_args+=(--suppression-health target/ripr/reports/suppression-health.json)
          fi
          ripr "${policy_args[@]}"

      - name: Render RIPR policy operations
        if: always() && hashFiles('target/ripr/reports/policy-readiness.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          operations_args=(
            policy operations
            --root .
            --policy-readiness target/ripr/reports/policy-readiness.json
            --out target/ripr/reports/policy-operations.json
            --out-md target/ripr/reports/policy-operations.md
          )
          if [ -f target/ripr/reports/waiver-aging.json ]; then
            operations_args+=(--waiver-aging target/ripr/reports/waiver-aging.json)
          fi
          if [ -f target/ripr/reports/suppression-health.json ]; then
            operations_args+=(--suppression-health target/ripr/reports/suppression-health.json)
          fi
          if [ -f target/ripr/reports/baseline-debt-delta.json ]; then
            operations_args+=(--baseline-delta target/ripr/reports/baseline-debt-delta.json)
          fi
          if [ -f target/ripr/reports/gate-decision.json ]; then
            operations_args+=(--gate-decision target/ripr/reports/gate-decision.json)
          fi
          if [ -f target/ripr/reports/recommendation-calibration.json ]; then
            operations_args+=(--recommendation-calibration target/ripr/reports/recommendation-calibration.json)
          fi
          if [ -f target/ripr/reports/mutation-calibration.json ]; then
            operations_args+=(--mutation-calibration target/ripr/reports/mutation-calibration.json)
          fi
          if [ -f target/ripr/reports/repo-exposure.json ]; then
            operations_args+=(--preview-boundary target/ripr/reports/repo-exposure.json)
          fi
          ripr "${operations_args[@]}"

      - name: Render RIPR policy history
        if: always() && hashFiles('target/ripr/reports/policy-operations.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          # Record the analyzed commit. On a PR, GITHUB_SHA names the merge
          # commit, which this workflow does not check out.
          history_args=(
            policy history
            --root .
            --current target/ripr/reports/policy-operations.json
            --commit "$(git rev-parse HEAD)"
            --out target/ripr/reports/policy-history.json
            --out-md target/ripr/reports/policy-history.md
          )
          if [ -f .ripr/policy-history.jsonl ]; then
            history_args+=(--history .ripr/policy-history.jsonl)
          fi
          if [ "${{ github.event_name }}" = "pull_request" ]; then
            history_args+=(--pr-number "${{ github.event.number }}")
          fi
          ripr "${history_args[@]}"

      - name: Render RIPR policy promotion packets
        if: always() && hashFiles('target/ripr/reports/policy-operations.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          for target_mode in visible-only acknowledgeable baseline-check calibrated-gate; do
            promotion_args=(
              policy promote
              --to "$target_mode"
              --operations target/ripr/reports/policy-operations.json
              --out "target/ripr/reports/policy-promotion-${target_mode}.json"
              --out-md "target/ripr/reports/policy-promotion-${target_mode}.md"
            )
            if [ -f target/ripr/reports/policy-history.json ]; then
              promotion_args+=(--history target/ripr/reports/policy-history.json)
            fi
            ripr "${promotion_args[@]}"
          done

      - name: Render RIPR preview promotion packets
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          preview_languages="$(
            ripr doctor --root . --json 2>/dev/null \
              | jq -r '.languages[]?' 2>/dev/null \
              | sed -n '/^typescript$/p; /^python$/p' \
              | sort -u \
              | tr '\n' ' ' \
              | sed 's/ $//' \
              || true
          )"
          if [ -z "$preview_languages" ]; then
            # Empty can mean "none configured" OR "doctor --json failed"
            # (#2182 review): doctor always emits at least the default
            # language, so an empty result is a detection failure. Say so
            # instead of asserting none are configured.
            if ripr doctor --root . --json > /dev/null 2>&1; then
              echo 'No TypeScript or Python preview languages are configured; preview promotion packets were not generated.'
            else
              echo 'Language detection via `ripr doctor --json` failed; preview promotion packets were not generated. Run ripr doctor locally for the underlying error.'
            fi
            exit 0
          fi
          for language in $preview_languages; do
            class_label=boundary_gap
            preview_args=(
              policy preview-promote
              --language "$language"
              --class "$class_label"
              --out "target/ripr/reports/preview-promotion-${language}-${class_label//_/-}.json"
              --out-md "target/ripr/reports/preview-promotion-${language}-${class_label//_/-}.md"
            )
            if [ -f target/ripr/reports/preview-promotion-evidence.json ]; then
              preview_args+=(--evidence target/ripr/reports/preview-promotion-evidence.json)
            fi
            ripr "${preview_args[@]}"
          done

      - name: Render RIPR test-oracle assistant proof
        if: always() && hashFiles('target/ripr/review/comments.json') != '' && hashFiles('target/ripr/workflow/agent-brief.json') != '' && hashFiles('target/ripr/workflow/before.repo-exposure.json') != '' && hashFiles('target/ripr/workflow/after.repo-exposure.json') != '' && hashFiles('target/ripr/reports/agent-receipt.json') != '' && hashFiles('target/ripr/reports/pr-evidence-ledger.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          proof_args=(
            assistant-loop proof
            --root .
            --pr-guidance target/ripr/review/comments.json
            --agent-packet target/ripr/workflow/agent-brief.json
            --before target/ripr/workflow/before.repo-exposure.json
            --after target/ripr/workflow/after.repo-exposure.json
            --receipt target/ripr/reports/agent-receipt.json
            --ledger target/ripr/reports/pr-evidence-ledger.json
            --out target/ripr/reports/test-oracle-assistant-proof.json
            --out-md target/ripr/reports/test-oracle-assistant-proof.md
          )
          if [ -f target/ripr/reports/coverage-grip-frontier.json ]; then
            proof_args+=(--coverage-frontier target/ripr/reports/coverage-grip-frontier.json)
          fi
          if [ -f target/ripr/reports/gate-decision.json ]; then
            proof_args+=(--gate-decision target/ripr/reports/gate-decision.json)
          fi
          ripr "${proof_args[@]}"

      - name: Render RIPR assistant loop health
        if: always() && hashFiles('target/ripr/reports/test-oracle-assistant-proof.json') != ''
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ripr assistant-loop health \
            --root . \
            --proof target/ripr/reports/test-oracle-assistant-proof.json \
            --out target/ripr/reports/assistant-loop-health.json \
            --out-md target/ripr/reports/assistant-loop-health.md

      - name: Render RIPR first useful action
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          first_action_has_input=false
          first_action_args=(
            first-action
            --root .
            --out target/ripr/reports/first-useful-action.json
            --out-md target/ripr/reports/first-useful-action.md
          )
          if [ -f target/ripr/review/comments.json ]; then
            first_action_args+=(--pr-guidance target/ripr/review/comments.json)
            first_action_has_input=true
          fi
          if [ -f target/ripr/reports/test-oracle-assistant-proof.json ]; then
            first_action_args+=(--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json)
            first_action_has_input=true
          fi
          if [ -f target/ripr/reports/pr-evidence-ledger.json ]; then
            first_action_args+=(--ledger target/ripr/reports/pr-evidence-ledger.json)
            first_action_has_input=true
          fi
          if [ -f target/ripr/reports/baseline-debt-delta.json ]; then
            first_action_args+=(--baseline-delta target/ripr/reports/baseline-debt-delta.json)
            first_action_has_input=true
          fi
          if [ -f target/ripr/reports/agent-receipt.json ]; then
            first_action_args+=(--receipt target/ripr/reports/agent-receipt.json)
            first_action_has_input=true
          fi
          if [ -f target/ripr/reports/gate-decision.json ]; then
            first_action_args+=(--gate-decision target/ripr/reports/gate-decision.json)
            first_action_has_input=true
          fi
          if [ -f target/ripr/reports/coverage-grip-frontier.json ]; then
            first_action_args+=(--coverage-frontier target/ripr/reports/coverage-grip-frontier.json)
            first_action_has_input=true
          fi
          if [ -f target/ripr/workflow/evidence-context.json ]; then
            first_action_args+=(--editor-context target/ripr/workflow/evidence-context.json)
            first_action_has_input=true
          fi
          if [ "$first_action_has_input" = true ]; then
            ripr "${first_action_args[@]}"
          else
            echo 'No RIPR first-useful-action inputs were available.'
            echo 'Safe next action: run `ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md` after attaching at least one explicit input.'
          fi

      - name: Render RIPR PR review front panel
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          front_panel_has_input=false
          front_panel_args=(
            pr-review front-panel
            --root .
            --out target/ripr/reports/pr-review-front-panel.json
            --out-md target/ripr/reports/pr-review-front-panel.md
          )
          if [ -f target/ripr/review/comments.json ]; then
            front_panel_args+=(--pr-guidance target/ripr/review/comments.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/first-useful-action.json ]; then
            front_panel_args+=(--first-action target/ripr/reports/first-useful-action.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/test-oracle-assistant-proof.json ]; then
            front_panel_args+=(--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/assistant-loop-health.json ]; then
            front_panel_args+=(--assistant-health target/ripr/reports/assistant-loop-health.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/pr-evidence-ledger.json ]; then
            front_panel_args+=(--ledger target/ripr/reports/pr-evidence-ledger.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/baseline-debt-delta.json ]; then
            front_panel_args+=(--baseline-delta target/ripr/reports/baseline-debt-delta.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/ripr-zero-status.json ]; then
            front_panel_args+=(--zero-status target/ripr/reports/ripr-zero-status.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/gate-decision.json ]; then
            front_panel_args+=(--gate-decision target/ripr/reports/gate-decision.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/recommendation-calibration.json ]; then
            front_panel_args+=(--recommendation-calibration target/ripr/reports/recommendation-calibration.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/mutation-calibration.json ]; then
            front_panel_args+=(--mutation-calibration target/ripr/reports/mutation-calibration.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/coverage-grip-frontier.json ]; then
            front_panel_args+=(--coverage-frontier target/ripr/reports/coverage-grip-frontier.json)
            front_panel_has_input=true
          fi
          if [ -f target/ripr/reports/agent-receipt.json ]; then
            front_panel_args+=(--receipt target/ripr/reports/agent-receipt.json)
            front_panel_has_input=true
          fi
          if [ "$front_panel_has_input" = true ]; then
            ripr "${front_panel_args[@]}"
          else
            echo 'No RIPR PR review front-panel inputs were available.'
            echo 'Safe next action: run `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md` after attaching at least one explicit input.'
          fi

      - name: Render RIPR first-pr start-here
        if: always()
        continue-on-error: true
        # first-pr checks its base resolves and that the review cards were
        # built for the same base. Without --base it resolves the default
        # branch, which is not the base of a PR into another branch, so
        # pass the PR base. A manual run has no PR base; use the default branch.
        run: |
          mkdir -p target/ripr/reports
          ripr first-pr \
            --root . \
            --base "origin/${{ github.base_ref || github.event.repository.default_branch }}" \
            --head HEAD \
            --gap-ledger target/ripr/reports/gap-decision-ledger.json \
            --first-action target/ripr/reports/first-useful-action.json \
            --review-comments target/ripr/review/comments.json \
            --agent-packet target/ripr/workflow/agent-packet.json \
            --gate-decision target/ripr/reports/gate-decision.json \
            --receipts-dir target/ripr/receipts \
            --out-dir target/ripr/reports

      - name: Render RIPR report packet index
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          index_has_input=false
          for path in \
            target/ripr/reports/start-here.md \
            target/ripr/reports/pr-review-front-panel.md \
            target/ripr/reports/first-useful-action.md \
            target/ripr/review/comments.md \
            target/ripr/review/comments.json \
            target/ripr/review/comment-publish-plan.md \
            target/ripr/reports/test-oracle-assistant-proof.md \
            target/ripr/reports/assistant-loop-health.md \
            target/ripr/reports/pr-evidence-ledger.md \
            target/ripr/reports/waiver-aging.md \
            target/ripr/reports/suppression-health.md \
            target/ripr/reports/policy-readiness.md \
            target/ripr/reports/policy-operations.md \
            target/ripr/reports/policy-history.md \
            target/ripr/reports/policy-promotion-visible-only.md \
            target/ripr/reports/policy-promotion-acknowledgeable.md \
            target/ripr/reports/policy-promotion-baseline-check.md \
            target/ripr/reports/policy-promotion-calibrated-gate.md \
            target/ripr/reports/preview-promotion-typescript-boundary-gap.md \
            target/ripr/reports/preview-promotion-python-boundary-gap.md \
            target/ripr/reports/baseline-debt-delta.md \
            target/ripr/reports/ripr-zero-status.md \
            target/ripr/reports/gate-decision.md \
            target/ripr/reports/recommendation-calibration.md \
            target/ripr/reports/mutation-calibration.md \
            target/ripr/reports/coverage-grip-frontier.md \
            target/ripr/reports/agent-receipt.json \
            target/ripr/reports/pr-summary.md \
            target/ripr/reports/check-pr.md \
            target/ripr/reports/ripr.sarif.json \
            target/ripr/reports/ripr-badge.json; do
            if [ -f "$path" ]; then
              index_has_input=true
              break
            fi
          done
          if [ "$index_has_input" = true ]; then
            ripr reports index \
              --root . \
              --reports-dir target/ripr/reports \
              --review-dir target/ripr/review \
              --receipts-dir target/ripr/receipts \
              --workflow-dir target/ripr/workflow \
              --agent-dir target/ripr/agent \
              --pilot-dir target/ripr/pilot \
              --ci-dir target/ci \
              --out target/ripr/reports/index.json \
              --out-md target/ripr/reports/index.md
          else
            echo 'No RIPR report-packet index inputs were available.'
            echo 'Regenerate command: `ripr reports index --root . --reports-dir target/ripr/reports --review-dir target/ripr/review --receipts-dir target/ripr/receipts --workflow-dir target/ripr/workflow --agent-dir target/ripr/agent --pilot-dir target/ripr/pilot --ci-dir target/ci --out target/ripr/reports/index.json --out-md target/ripr/reports/index.md`.'
          fi

      - name: Render RIPR LLM work-loop summaries
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/workflow
          ripr agent status \
            --root . \
            --json \
            > target/ripr/workflow/agent-status.json
          ripr agent status \
            --root . \
            > target/ripr/workflow/agent-status.md
          ripr agent review-summary \
            --root . \
            --json \
            > target/ripr/workflow/agent-review-summary.json
          ripr agent review-summary \
            --root . \
            > target/ripr/workflow/agent-review-summary.md

      - name: Emit RIPR PR guidance annotations
        if: always() && hashFiles('target/ripr/review/comments.json') != ''
        continue-on-error: true
        run: |
          # Encode the workflow command in jq. Routing the fields through a
          # TSV round-trip rewrites backslash, tab, CR, and LF into transport
          # text before GitHub's encoder can see the original bytes (#4089). An annotation names the repair start only when
          # that field is present. Literal "null" stays absent. The brief
          # command is not interpolated: it points at this runner's checkout.
          jq -r '
            def escape_data:
              gsub("%"; "%25") | gsub("\r"; "%0D") | gsub("\n"; "%0A");
            def escape_property:
              escape_data | gsub(":"; "%3A") | gsub(","; "%2C");
            .comments[]?
            | select(.placement.path and .placement.line)
            | (.llm_guidance.repair_command // "") as $repair_start
            | ((.reason // "RIPR targeted test guidance")
                + (if $repair_start != "" and $repair_start != "null"
                   then " Start the repair: " + $repair_start
                   else "" end)) as $message
            | "::warning file=\(.placement.path | escape_property),line=\(.placement.line | tostring | escape_property),title=RIPR targeted test guidance::\($message | escape_data)"
          ' target/ripr/review/comments.json

"#;

/// The template from the `Check RIPR advisory artifacts` step through the
/// final upload step.
const TEMPLATE_TAIL: &str = r#"      - name: Check RIPR advisory artifacts
        if: always()
        continue-on-error: true
        run: |
          # Green-with-missing-artifacts is a real failure mode (#2009):
          # report it visibly without failing the advisory job.
          missing=()
          for artifact in target/ripr/reports/start-here.md target/ripr/reports/index.json; do
            if [ ! -f "$artifact" ]; then
              missing+=("$artifact")
            fi
          done
          if [ "$RIPR_GATE_MODE" != '' ] && [ ! -f target/ripr/reports/gate-decision.json ] && [ -f target/ripr/review/comments.json ]; then
            missing+=("target/ripr/reports/gate-decision.json (RIPR_GATE_MODE is set)")
          fi
          if [ ${#missing[@]} -gt 0 ]; then
            echo '::warning::Some RIPR advisory artifacts are missing (upstream step failed softly):'
            for artifact in "${missing[@]}"; do
              echo "  - $artifact"
            done
          fi

      - name: Upload RIPR report artifacts
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

pub(super) fn generated_github_actions_workflow() -> String {
    generated_workflow_template()
        .replace("@RIPR_VERSION@", env!("CARGO_PKG_VERSION"))
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
    /// shell with `ripr reports ci-summary`. The unrendered template is the
    /// stable identity: rendering additionally substitutes the crate version
    /// and artifact paths, which the `generated_workflow_*` tests pin at the
    /// rendered level.
    const TEMPLATE_SHA256: &str =
        "4cb034a82890f201628565cc0c1a2f744b8039d18c94d62425c7bc820095f45d";

    #[test]
    fn template_matches_the_pinned_bytes() {
        let hex: String = Sha256::digest(generated_workflow_template().as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(hex, TEMPLATE_SHA256);
    }
}
