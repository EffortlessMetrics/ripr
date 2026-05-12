use crate::agent::loop_commands;

pub(super) fn generated_github_actions_workflow() -> String {
    let mut workflow = String::new();
    for section in workflow_sections() {
        workflow.push_str(section);
    }
    canonicalize_agent_artifact_paths(workflow)
}

fn workflow_sections() -> [&'static str; 7] {
    [
        workflow_preamble_and_agent_loop(),
        pull_request_review_steps(),
        repo_report_steps(),
        decision_summary_steps(),
        report_packet_and_work_loop_steps(),
        job_summary_step(),
        artifact_upload_steps(),
    ]
}

fn canonicalize_agent_artifact_paths(workflow: String) -> String {
    workflow
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

fn workflow_preamble_and_agent_loop() -> &'static str {
    r#"name: RIPR

on:
  pull_request:
  workflow_dispatch:

permissions:
  contents: read
  pull-requests: write
  security-events: write

env:
  RIPR_UPLOAD_SARIF: "true"
  RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}
  RIPR_GATE_BASELINE: ${{ vars.RIPR_GATE_BASELINE || '' }}
  RIPR_COMMENT_MODE: ${{ vars.RIPR_COMMENT_MODE || 'off' }}

jobs:
  ripr:
    name: RIPR advisory reports
    runs-on: ubuntu-latest
    continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0

      - uses: dtolnay/rust-toolchain@stable

      - name: Install ripr
        run: cargo install ripr --locked

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
        run: |
          ripr agent start \
            --root . \
            --seam-id "$RIPR_TOP_SEAM_ID" \
            --out target/ripr/workflow
          ripr agent packet \
            --root . \
            --seam-id "$RIPR_TOP_SEAM_ID" \
            --json \
            > target/ripr/workflow/agent-packet.json
          cp target/ripr/workflow/agent-packet.json target/ripr/agent/agent-packet.json
          cp target/ripr/workflow/agent-brief.json target/ripr/agent/agent-brief.json
          ripr check \
            --root . \
            --mode ready \
            --format repo-exposure-json \
            > target/ripr/workflow/after.repo-exposure.json
          cp target/ripr/workflow/after.repo-exposure.json target/ripr/pilot/after.repo-exposure.json
          ripr agent verify \
            --root . \
            --before target/ripr/workflow/before.repo-exposure.json \
            --after target/ripr/workflow/after.repo-exposure.json \
            --json \
            > target/ripr/workflow/agent-verify.json
          cp target/ripr/workflow/agent-verify.json target/ripr/agent/agent-verify.json
          ripr agent receipt \
            --root . \
            --verify-json target/ripr/workflow/agent-verify.json \
            --seam-id "$RIPR_TOP_SEAM_ID" \
            --json \
            --out target/ripr/reports/agent-receipt.json
          cp target/ripr/reports/agent-receipt.json target/ripr/agent/agent-receipt.json
          ripr outcome \
            --before target/ripr/workflow/before.repo-exposure.json \
            --after target/ripr/workflow/after.repo-exposure.json \
            --format json \
            --out target/ripr/reports/targeted-test-outcome.json

"#
}

fn pull_request_review_steps() -> &'static str {
    r#"      - name: Capture pull request diff
        if: github.event_name == 'pull_request'
        run: |
          mkdir -p target/ripr/reports
          git diff --binary "origin/${{ github.base_ref }}...HEAD" > target/ripr/reports/pr.diff

      - name: Run RIPR PR guidance report
        if: github.event_name == 'pull_request'
        continue-on-error: true
        run: |
          mkdir -p target/ripr/review
          ripr review-comments \
            --root . \
            --base "origin/${{ github.base_ref }}" \
            --head HEAD \
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
              | select((.body // "") | contains("<!-- ripr:dedupe="))
              | {
                  comment_id: .id,
                  dedupe_key: ((.body // "") | capture("<!-- ripr:dedupe=(?<key>[^ ]+) -->").key),
                  path: .path,
                  line: (.line // .original_line),
                  side: (.side // "RIGHT"),
                  body: ((.body // "") | sub("\n\n<!-- ripr:dedupe=[^ ]+ -->\n?$"; "")),
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
          comment_args+=(--write-permission)
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
            jq -r '.blocked[]? | "- \(.blocked_reason): \(.message)"' "$plan" || true
            exit 0
          fi

          jq -c '.operations[]? | select(.safe_to_publish == true)' "$plan" \
            | while IFS= read -r operation; do
                op="$(jq -r '.operation' <<< "$operation")"
                dedupe_key="$(jq -r '.dedupe_key' <<< "$operation")"
                body="$(jq -r '.body // ""' <<< "$operation")"
                body_with_marker="$(printf '%s\n\n<!-- ripr:dedupe=%s -->\n' "$body" "$dedupe_key")"
                if [ "$op" = "keep" ]; then
                  echo "RIPR inline comment already current: $dedupe_key"
                  continue
                fi
                if [ "$op" = "create" ]; then
                  path="$(jq -r '.placement.path' <<< "$operation")"
                  line="$(jq -r '.placement.line' <<< "$operation")"
                  side="$(jq -r '.placement.side // "RIGHT"' <<< "$operation")"
                  payload="$(mktemp)"
                  jq -n \
                    --arg body "$body_with_marker" \
                    --arg commit_id "${{ github.event.pull_request.head.sha }}" \
                    --arg path "$path" \
                    --arg side "$side" \
                    --argjson line "$line" \
                    '{body: $body, commit_id: $commit_id, path: $path, side: $side, line: $line}' \
                    > "$payload"
                  gh api --method POST "repos/${{ github.repository }}/pulls/${{ github.event.pull_request.number }}/comments" --input "$payload" >/dev/null
                  echo "Created RIPR inline comment: $dedupe_key"
                elif [ "$op" = "update" ]; then
                  comment_id="$(jq -r '.existing_comment_id' <<< "$operation")"
                  payload="$(mktemp)"
                  jq -n --arg body "$body_with_marker" '{body: $body}' > "$payload"
                  gh api --method PATCH "repos/${{ github.repository }}/pulls/comments/$comment_id" --input "$payload" >/dev/null
                  echo "Updated RIPR inline comment: $dedupe_key"
                else
                  echo "RIPR inline comment operation $op is review-only: $dedupe_key"
                fi
              done

      - name: Capture RIPR gate labels
        if: always() && github.event_name == 'pull_request'
        continue-on-error: true
        run: |
          mkdir -p target/ci
          jq -c '{labels: [.pull_request.labels[]?.name]}' "$GITHUB_EVENT_PATH" > target/ci/labels.json

"#
}

fn repo_report_steps() -> &'static str {
    r#"      - name: Render RIPR diff SARIF
        if: env.RIPR_UPLOAD_SARIF == 'true' && github.event_name == 'pull_request'
        continue-on-error: true
        run: |
          ripr check \
            --root . \
            --diff target/ripr/reports/pr.diff \
            --format sarif \
            > target/ripr/reports/ripr-findings.sarif

      - name: Render RIPR repo seam SARIF
        if: env.RIPR_UPLOAD_SARIF == 'true'
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ripr check \
            --root . \
            --mode ready \
            --format repo-sarif \
            > target/ripr/reports/ripr-seams.sarif

      - name: Render RIPR repo badge artifacts
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

      - name: Render RIPR operator cockpit
        if: always() && hashFiles('crates/ripr/Cargo.toml') != '' && hashFiles('xtask/src/reports/operator.rs') != ''
        continue-on-error: true
        run: cargo xtask operator-cockpit

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

"#
}

fn decision_summary_steps() -> &'static str {
    r#"      - name: Render RIPR first useful action
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
          fi

"#
}

fn report_packet_and_work_loop_steps() -> &'static str {
    r#"      - name: Render RIPR report packet index
        if: always()
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          index_has_input=false
          for path in \
            target/ripr/reports/pr-review-front-panel.md \
            target/ripr/reports/first-useful-action.md \
            target/ripr/review/comments.md \
            target/ripr/review/comments.json \
            target/ripr/review/comment-publish-plan.md \
            target/ripr/reports/test-oracle-assistant-proof.md \
            target/ripr/reports/assistant-loop-health.md \
            target/ripr/reports/pr-evidence-ledger.md \
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
          escape_github_message() {
            local value="$1"
            value="${value//'%'/'%25'}"
            value="${value//$'\r'/'%0D'}"
            value="${value//$'\n'/'%0A'}"
            printf '%s' "$value"
          }

          escape_github_property() {
            local value="$1"
            value="${value//'%'/'%25'}"
            value="${value//$'\r'/'%0D'}"
            value="${value//$'\n'/'%0A'}"
            value="${value//':'/'%3A'}"
            value="${value//','/'%2C'}"
            printf '%s' "$value"
          }

          jq -r '.comments[]? | select(.placement.path and .placement.line) | [.placement.path, (.placement.line | tostring), (.reason // "RIPR targeted test guidance"), (.llm_guidance.command // "")] | @tsv' target/ripr/review/comments.json \
            | while IFS="$(printf '\t')" read -r path line reason command; do
                message="$reason"
                if [ -n "$command" ] && [ "$command" != "null" ]; then
                  message="$message Command: $command"
                fi
                annotation_path="$(escape_github_property "$path")"
                annotation_line="$(escape_github_property "$line")"
                annotation_title="$(escape_github_property "RIPR targeted test guidance")"
                message="$(escape_github_message "$message")"
                echo "::warning file=$annotation_path,line=$annotation_line,title=$annotation_title::$message"
              done

"#
}

fn job_summary_step() -> &'static str {
    r#"      - name: Add RIPR advisory summary
        if: always()
        continue-on-error: true
        run: |
          {
            markdown_inline() {
              printf '%s' "$1" | tr '\r\n' '  ' | sed 's/`/\\`/g'
            }

            echo '## RIPR advisory summary'
            echo
            echo "RIPR is advisory static evidence. It does not edit source, generate tests, or run mutation testing."
            echo
            echo '### PR review summary'
            if [ -f target/ripr/reports/pr-review-front-panel.json ] || [ -f target/ripr/reports/pr-review-front-panel.md ]; then
              if [ -f target/ripr/reports/pr-review-front-panel.json ]; then
                panel_json=target/ripr/reports/pr-review-front-panel.json
                panel_status="$(jq -r '.status // "unknown"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_headline="$(jq -r '.summary.headline // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_top_state="$(jq -r '.summary.top_issue_state // "unknown"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_policy_state="$(jq -r '.summary.policy_state // "none"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_placement="$(jq -r '.summary.placement // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_movement="$(jq -r '.summary.movement_state // "unknown"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_coverage_grip="$(jq -r '.summary.coverage_grip_state // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_new_policy_eligible="$(jq -r '.summary.new_policy_eligible // 0' "$panel_json" 2>/dev/null || echo 0)"
                panel_baseline_present="$(jq -r '.summary.baseline_still_present // 0' "$panel_json" 2>/dev/null || echo 0)"
                panel_baseline_resolved="$(jq -r '.summary.baseline_resolved // 0' "$panel_json" 2>/dev/null || echo 0)"
                panel_acknowledged="$(jq -r '.summary.acknowledged // 0' "$panel_json" 2>/dev/null || echo 0)"
                panel_suppressed="$(jq -r '.summary.suppressed // 0' "$panel_json" 2>/dev/null || echo 0)"
                panel_blocking="$(jq -r '.summary.blocking_candidates // 0' "$panel_json" 2>/dev/null || echo 0)"
                panel_issue="$(jq -r 'if .top_issue == null then "not_available" else ((.top_issue.path // "unknown") + (if .top_issue.line then ":" + (.top_issue.line|tostring) else "" end)) end' "$panel_json" 2>/dev/null || echo unknown)"
                panel_class="$(jq -r '.top_issue.classification // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_missing="$(jq -r '.top_issue.missing_discriminator // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_related="$(jq -r '.top_issue.related_test // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_suggested="$(jq -r '.top_issue.suggested_test // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_verify="$(jq -r '.top_issue.verify_command // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_agent="$(jq -r '.top_issue.agent_command // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_receipt="$(jq -r '.top_issue.receipt.artifact // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_gate_mode="$(jq -r '.policy.mode // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_gate_decision="$(jq -r '.policy.decision // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
                panel_warning_count="$(jq -r '(.warnings // [] | length)' "$panel_json" 2>/dev/null || echo 0)"
                panel_status="$(markdown_inline "$panel_status")"
                panel_headline="$(markdown_inline "$panel_headline")"
                panel_top_state="$(markdown_inline "$panel_top_state")"
                panel_policy_state="$(markdown_inline "$panel_policy_state")"
                panel_placement="$(markdown_inline "$panel_placement")"
                panel_movement="$(markdown_inline "$panel_movement")"
                panel_coverage_grip="$(markdown_inline "$panel_coverage_grip")"
                panel_new_policy_eligible="$(markdown_inline "$panel_new_policy_eligible")"
                panel_baseline_present="$(markdown_inline "$panel_baseline_present")"
                panel_baseline_resolved="$(markdown_inline "$panel_baseline_resolved")"
                panel_acknowledged="$(markdown_inline "$panel_acknowledged")"
                panel_suppressed="$(markdown_inline "$panel_suppressed")"
                panel_blocking="$(markdown_inline "$panel_blocking")"
                panel_issue="$(markdown_inline "$panel_issue")"
                panel_class="$(markdown_inline "$panel_class")"
                panel_missing="$(markdown_inline "$panel_missing")"
                panel_related="$(markdown_inline "$panel_related")"
                panel_suggested="$(markdown_inline "$panel_suggested")"
                panel_verify="$(markdown_inline "$panel_verify")"
                panel_agent="$(markdown_inline "$panel_agent")"
                panel_receipt="$(markdown_inline "$panel_receipt")"
                panel_gate_mode="$(markdown_inline "$panel_gate_mode")"
                panel_gate_decision="$(markdown_inline "$panel_gate_decision")"
                panel_warning_count="$(markdown_inline "$panel_warning_count")"
                echo '#### PR review at a glance'
                echo "- Status: \`$panel_status\`"
                echo "- Headline: \`$panel_headline\`"
                echo "- Top issue state: \`$panel_top_state\`"
                echo "- Policy state: \`$panel_policy_state\`"
                echo "- Placement: \`$panel_placement\`"
                echo "- Static movement: \`$panel_movement\`"
                echo "- Coverage/grip: \`$panel_coverage_grip\`"
                echo "- Counts: new_policy_eligible=\`$panel_new_policy_eligible\`, baseline_still_present=\`$panel_baseline_present\`, baseline_resolved=\`$panel_baseline_resolved\`, acknowledged=\`$panel_acknowledged\`, suppressed=\`$panel_suppressed\`, blocking_candidates=\`$panel_blocking\`"
                echo "- Top issue: \`$panel_issue\` class=\`$panel_class\`"
                echo "- Missing discriminator: \`$panel_missing\`"
                echo "- Suggested focused test: \`$panel_suggested\`"
                echo "- Related test: \`$panel_related\`"
                echo "- Verify command: \`$panel_verify\`"
                echo "- Agent handoff: \`$panel_agent\`"
                echo "- Receipt: \`$panel_receipt\`"
                echo "- Gate: mode=\`$panel_gate_mode\`, decision=\`$panel_gate_decision\`"
                echo "- Warnings: \`$panel_warning_count\`"
                echo "- Front-panel artifacts: \`target/ripr/reports/pr-review-front-panel.json\`, \`target/ripr/reports/pr-review-front-panel.md\`"
                echo "- Pass/fail authority remains \`ripr gate evaluate\` when an explicit gate mode is configured."
                echo
              fi
              if [ -f target/ripr/reports/pr-review-front-panel.md ]; then
                cat target/ripr/reports/pr-review-front-panel.md
              fi
            else
              echo 'PR review summary was not generated. It runs when existing PR guidance, first-useful-action, assistant proof, health, ledger, baseline, gate, calibration, coverage/grip, or receipt artifacts are available.'
            fi
            echo
            echo '### Recommended next test'
            if [ -f target/ripr/reports/first-useful-action.json ] || [ -f target/ripr/reports/first-useful-action.md ]; then
              if [ -f target/ripr/reports/first-useful-action.json ]; then
                action_json=target/ripr/reports/first-useful-action.json
                action_status="$(jq -r '.status // "unknown"' "$action_json" 2>/dev/null || echo unknown)"
                action_kind="$(jq -r '.action_kind // "unknown"' "$action_json" 2>/dev/null || echo unknown)"
                action_title="$(jq -r '.title // "not_available"' "$action_json" 2>/dev/null || echo unknown)"
                action_why="$(jq -r '.why // "not_available"' "$action_json" 2>/dev/null || echo unknown)"
                action_seam="$(jq -r '.selected.seam_id // "not_available"' "$action_json" 2>/dev/null || echo unknown)"
                action_target="$(jq -r '(.target.file // "not_available") + (if .target.related_test then " related_test=" + .target.related_test else "" end)' "$action_json" 2>/dev/null || echo unknown)"
                action_verify="$(jq -r '.commands.verify // "not_available"' "$action_json" 2>/dev/null || echo unknown)"
                action_receipt="$(jq -r '.commands.receipt // "not_available"' "$action_json" 2>/dev/null || echo unknown)"
                action_fallback="$(jq -r '.fallback.kind // "none"' "$action_json" 2>/dev/null || echo unknown)"
                action_warning_count="$(jq -r '(.warnings // [] | length)' "$action_json" 2>/dev/null || echo 0)"
                action_status="$(markdown_inline "$action_status")"
                action_kind="$(markdown_inline "$action_kind")"
                action_title="$(markdown_inline "$action_title")"
                action_why="$(markdown_inline "$action_why")"
                action_seam="$(markdown_inline "$action_seam")"
                action_target="$(markdown_inline "$action_target")"
                action_verify="$(markdown_inline "$action_verify")"
                action_receipt="$(markdown_inline "$action_receipt")"
                action_fallback="$(markdown_inline "$action_fallback")"
                action_warning_count="$(markdown_inline "$action_warning_count")"
                echo '#### Recommended next test at a glance'
                echo "- Status: \`$action_status\`"
                echo "- Action: \`$action_kind\`"
                echo "- Title: \`$action_title\`"
                echo "- Why: \`$action_why\`"
                echo "- Seam: \`$action_seam\`"
                echo "- Target: \`$action_target\`"
                echo "- Verify command: \`$action_verify\`"
                echo "- Receipt command: \`$action_receipt\`"
                echo "- Fallback: \`$action_fallback\`"
                echo "- Warnings: \`$action_warning_count\`"
                echo "- Action artifacts: \`target/ripr/reports/first-useful-action.json\`, \`target/ripr/reports/first-useful-action.md\`"
                echo "- Boundary: static evidence only; no runtime mutation execution."
                echo
              fi
              if [ -f target/ripr/reports/first-useful-action.md ]; then
                cat target/ripr/reports/first-useful-action.md
              fi
            else
              echo 'Recommended next test was not generated. It runs when existing PR guidance, assistant proof, ledger, baseline, receipt, gate, coverage/grip, or editor context artifacts are available.'
            fi
            echo
            echo '### Top recommendation'
            if [ -f target/ripr/pilot/pilot-summary.md ]; then
              cat target/ripr/pilot/pilot-summary.md
            else
              echo "Pilot summary was not generated. Inspect the uploaded artifact packet and job logs."
            fi
            echo
            echo '### Agent review packet'
            if [ -f target/ripr/workflow/agent-review-summary.md ]; then
              cat target/ripr/workflow/agent-review-summary.md
            else
              echo 'Agent review summary was not generated. Run `ripr agent status --root .` locally or inspect uploaded workflow artifacts.'
            fi
            echo
            echo '### Artifact packet'
            echo '- Pilot reports: `target/ripr/pilot/`'
            echo '- Agent workflow: `target/ripr/workflow/`'
            echo '- Agent compatibility copies: `target/ripr/agent/`'
            echo '- Repo reports, badges, SARIF, and receipts: `target/ripr/reports/`'
            echo '- CI labels and plan inputs: `target/ci/`'
            if [ -d target/ripr/review ]; then
              echo '- PR test guidance report: `target/ripr/review/`'
            else
              echo "- PR test guidance report: not generated yet"
            fi
            echo
            echo '### Uploaded review artifacts'
            if [ -f target/ripr/reports/index.json ] || [ -f target/ripr/reports/index.md ]; then
              if [ -f target/ripr/reports/index.json ]; then
                index_json=target/ripr/reports/index.json
                index_status="$(jq -r '.status // "unknown"' "$index_json" 2>/dev/null || echo unknown)"
                index_entries="$(jq -r '.summary.entries // 0' "$index_json" 2>/dev/null || echo 0)"
                index_available="$(jq -r '.summary.available // 0' "$index_json" 2>/dev/null || echo 0)"
                index_missing="$(jq -r '.summary.missing_expected // 0' "$index_json" 2>/dev/null || echo 0)"
                index_warnings="$(jq -r '.summary.warnings // 0' "$index_json" 2>/dev/null || echo 0)"
                index_failures="$(jq -r '.summary.failures // 0' "$index_json" 2>/dev/null || echo 0)"
                index_start="$(jq -r '.summary.start_here // "not_available"' "$index_json" 2>/dev/null || echo unknown)"
                index_gate="$(jq -r '.summary.gate_authority // "not_available"' "$index_json" 2>/dev/null || echo unknown)"
                index_missing_labels="$(jq -r '([.missing_expected[]?.label] | if length == 0 then "none" else join(", ") end)' "$index_json" 2>/dev/null || echo unknown)"
                index_warning_kinds="$(jq -r '([.warnings[]?.kind] | if length == 0 then "none" else join(", ") end)' "$index_json" 2>/dev/null || echo unknown)"
                index_status="$(markdown_inline "$index_status")"
                index_entries="$(markdown_inline "$index_entries")"
                index_available="$(markdown_inline "$index_available")"
                index_missing="$(markdown_inline "$index_missing")"
                index_warnings="$(markdown_inline "$index_warnings")"
                index_failures="$(markdown_inline "$index_failures")"
                index_start="$(markdown_inline "$index_start")"
                index_gate="$(markdown_inline "$index_gate")"
                index_missing_labels="$(markdown_inline "$index_missing_labels")"
                index_warning_kinds="$(markdown_inline "$index_warning_kinds")"
                echo '#### Uploaded artifacts at a glance'
                echo "- Status: \`$index_status\`"
                echo "- Entries: total=\`$index_entries\`, available=\`$index_available\`, missing_expected=\`$index_missing\`, warnings=\`$index_warnings\`, failures=\`$index_failures\`"
                echo "- Start here: \`$index_start\`"
                echo "- Gate authority: \`$index_gate\`"
                echo "- Missing expected: \`$index_missing_labels\`"
                echo "- Warning kinds: \`$index_warning_kinds\`"
                echo "- Index artifacts: \`target/ripr/reports/index.json\`, \`target/ripr/reports/index.md\`"
                echo "- Boundary: advisory artifact map only; gate-decision remains configured pass/fail authority."
                echo
              fi
              if [ -f target/ripr/reports/index.md ]; then
                cat target/ripr/reports/index.md
              fi
            else
              echo 'Uploaded review artifacts summary was not generated. It runs when existing RIPR report, review, receipt, workflow, agent, pilot, or CI artifacts are available.'
            fi
            echo
            echo '### PR evidence ledger'
            if [ -f target/ripr/reports/pr-evidence-ledger.json ]; then
              ledger_json=target/ripr/reports/pr-evidence-ledger.json
              ledger_status="$(jq -r '.status // "unknown"' "$ledger_json" 2>/dev/null || echo unknown)"
              ledger_gate_mode="$(jq -r '.gate.mode // "not_evaluated"' "$ledger_json" 2>/dev/null || echo unknown)"
              ledger_gate_decision="$(jq -r '.gate.decision // "not_evaluated"' "$ledger_json" 2>/dev/null || echo unknown)"
              ledger_new_policy_eligible="$(jq -r '.movement.new_policy_eligible // 0' "$ledger_json" 2>/dev/null || echo 0)"
              ledger_still_present="$(jq -r '.movement.baseline_still_present // 0' "$ledger_json" 2>/dev/null || echo 0)"
              ledger_resolved="$(jq -r '.movement.baseline_resolved // 0' "$ledger_json" 2>/dev/null || echo 0)"
              ledger_acknowledged="$(jq -r '.movement.acknowledged // 0' "$ledger_json" 2>/dev/null || echo 0)"
              ledger_suppressed="$(jq -r '.movement.suppressed // 0' "$ledger_json" 2>/dev/null || echo 0)"
              ledger_blocking="$(jq -r '.movement.blocking_candidates // 0' "$ledger_json" 2>/dev/null || echo 0)"
              ledger_visible="$(jq -r '.movement.visible_unresolved // 0' "$ledger_json" 2>/dev/null || echo 0)"
              ledger_coverage_status="$(jq -r '.coverage_grip_frontier.status // "not_available"' "$ledger_json" 2>/dev/null || echo unknown)"
              ledger_trend="$(jq -r '.history.trend // "not_available"' "$ledger_json" 2>/dev/null || echo unknown)"
              ledger_route="$(jq -r '(.top_repair_route | if . == null then "none" else ((.path // "unknown") + (if .line then ":" + (.line|tostring) else "" end) + " " + (.missing_discriminator // "missing discriminator unavailable")) end)' "$ledger_json" 2>/dev/null || echo unknown)"
              ledger_verify="$(jq -r '.top_repair_route.verify_command // "not_available"' "$ledger_json" 2>/dev/null || echo unknown)"
              ledger_agent="$(jq -r '.top_repair_route.agent_command // "not_available"' "$ledger_json" 2>/dev/null || echo unknown)"
              ledger_status="$(markdown_inline "$ledger_status")"
              ledger_gate_mode="$(markdown_inline "$ledger_gate_mode")"
              ledger_gate_decision="$(markdown_inline "$ledger_gate_decision")"
              ledger_new_policy_eligible="$(markdown_inline "$ledger_new_policy_eligible")"
              ledger_still_present="$(markdown_inline "$ledger_still_present")"
              ledger_resolved="$(markdown_inline "$ledger_resolved")"
              ledger_acknowledged="$(markdown_inline "$ledger_acknowledged")"
              ledger_suppressed="$(markdown_inline "$ledger_suppressed")"
              ledger_blocking="$(markdown_inline "$ledger_blocking")"
              ledger_visible="$(markdown_inline "$ledger_visible")"
              ledger_coverage_status="$(markdown_inline "$ledger_coverage_status")"
              ledger_trend="$(markdown_inline "$ledger_trend")"
              ledger_route="$(markdown_inline "$ledger_route")"
              ledger_verify="$(markdown_inline "$ledger_verify")"
              ledger_agent="$(markdown_inline "$ledger_agent")"
              echo '#### PR movement at a glance'
              echo "- Status: \`$ledger_status\`"
              echo "- Gate: mode=\`$ledger_gate_mode\`, decision=\`$ledger_gate_decision\`"
              echo "- Counts: new_policy_eligible=\`$ledger_new_policy_eligible\`, baseline_still_present=\`$ledger_still_present\`, baseline_resolved=\`$ledger_resolved\`, acknowledged=\`$ledger_acknowledged\`, suppressed=\`$ledger_suppressed\`, blocking_candidates=\`$ledger_blocking\`, visible_unresolved=\`$ledger_visible\`"
              echo "- Top repair route: \`$ledger_route\`"
              echo "- Verify command: \`$ledger_verify\`"
              echo "- Agent command: \`$ledger_agent\`"
              echo "- Coverage/grip frontier: \`$ledger_coverage_status\`"
              echo "- History trend: \`$ledger_trend\`"
              echo "- Ledger artifacts: \`target/ripr/reports/pr-evidence-ledger.json\`, \`target/ripr/reports/pr-evidence-ledger.md\`"
              echo "- Pass/fail authority remains \`ripr gate evaluate\` when an explicit gate mode is configured."
              echo
            fi
            if [ -f target/ripr/reports/pr-evidence-ledger.md ]; then
              cat target/ripr/reports/pr-evidence-ledger.md
            elif [ -f target/ripr/review/comments.json ]; then
              echo 'PR evidence ledger was not generated. Inspect `target/ripr/review/comments.json` and rerun `ripr pr-ledger record` locally.'
            else
              echo 'PR evidence ledger was not run. It requires pull-request guidance from `target/ripr/review/comments.json`.'
            fi
            echo
            if [ -f target/ripr/reports/test-oracle-assistant-proof.json ] || [ -f target/ripr/reports/test-oracle-assistant-proof.md ]; then
              echo '### Test-oracle assistant proof'
              if [ -f target/ripr/reports/test-oracle-assistant-proof.json ]; then
                proof_json=target/ripr/reports/test-oracle-assistant-proof.json
                proof_status="$(jq -r '.status // "unknown"' "$proof_json" 2>/dev/null || echo unknown)"
                proof_seam="$(jq -r '(.seam.path // "unknown") + (if .seam.line then ":" + (.seam.line|tostring) else "" end)' "$proof_json" 2>/dev/null || echo unknown)"
                proof_missing="$(jq -r '.seam.missing_discriminator // "not_available"' "$proof_json" 2>/dev/null || echo unknown)"
                proof_placement="$(jq -r '.recommendation.placement // "not_available"' "$proof_json" 2>/dev/null || echo unknown)"
                proof_movement="$(jq -r '.evidence_movement.state // "unknown"' "$proof_json" 2>/dev/null || echo unknown)"
                proof_receipt="$(jq -r '.evidence_movement.artifact // .inputs.receipt // "not_available"' "$proof_json" 2>/dev/null || echo unknown)"
                proof_gate="$(jq -r '.ci_projection.gate_decision // "not_supplied"' "$proof_json" 2>/dev/null || echo unknown)"
                proof_coverage="$(jq -r '.ci_projection.coverage_frontier // "not_supplied"' "$proof_json" 2>/dev/null || echo unknown)"
                proof_warning_count="$(jq -r '(.warnings // [] | length)' "$proof_json" 2>/dev/null || echo 0)"
                proof_status="$(markdown_inline "$proof_status")"
                proof_seam="$(markdown_inline "$proof_seam")"
                proof_missing="$(markdown_inline "$proof_missing")"
                proof_placement="$(markdown_inline "$proof_placement")"
                proof_movement="$(markdown_inline "$proof_movement")"
                proof_receipt="$(markdown_inline "$proof_receipt")"
                proof_gate="$(markdown_inline "$proof_gate")"
                proof_coverage="$(markdown_inline "$proof_coverage")"
                proof_warning_count="$(markdown_inline "$proof_warning_count")"
                echo '#### Assistant proof at a glance'
                echo "- Status: \`$proof_status\`"
                echo "- Seam: \`$proof_seam\`"
                echo "- Missing discriminator: \`$proof_missing\`"
                echo "- Placement: \`$proof_placement\`"
                echo "- Static movement: \`$proof_movement\`"
                echo "- Receipt: \`$proof_receipt\`"
                echo "- Gate input: \`$proof_gate\`"
                echo "- Coverage/grip frontier input: \`$proof_coverage\`"
                echo "- Warnings: \`$proof_warning_count\`"
                echo "- Proof artifacts: \`target/ripr/reports/test-oracle-assistant-proof.json\`, \`target/ripr/reports/test-oracle-assistant-proof.md\`"
                echo "- Pass/fail authority remains \`ripr gate evaluate\` when an explicit gate mode is configured."
                echo
              fi
              if [ -f target/ripr/reports/test-oracle-assistant-proof.md ]; then
                cat target/ripr/reports/test-oracle-assistant-proof.md
              fi
              echo
            fi
            if [ -f target/ripr/reports/assistant-loop-health.json ] || [ -f target/ripr/reports/assistant-loop-health.md ]; then
              echo '### Agent proof status'
              if [ -f target/ripr/reports/assistant-loop-health.json ]; then
                health_json=target/ripr/reports/assistant-loop-health.json
                health_status="$(jq -r '.status // "unknown"' "$health_json" 2>/dev/null || echo unknown)"
                health_proofs="$(jq -r '.summary.proofs // 0' "$health_json" 2>/dev/null || echo 0)"
                health_complete="$(jq -r '.summary.complete // 0' "$health_json" 2>/dev/null || echo 0)"
                health_partial="$(jq -r '.summary.partial // 0' "$health_json" 2>/dev/null || echo 0)"
                health_missing_required="$(jq -r '.summary.missing_required_input // 0' "$health_json" 2>/dev/null || echo 0)"
                health_missing_optional="$(jq -r '.summary.missing_optional_input // 0' "$health_json" 2>/dev/null || echo 0)"
                health_improved="$(jq -r '.summary.improved // 0' "$health_json" 2>/dev/null || echo 0)"
                health_unchanged="$(jq -r '.summary.unchanged // 0' "$health_json" 2>/dev/null || echo 0)"
                health_regressed="$(jq -r '.summary.regressed // 0' "$health_json" 2>/dev/null || echo 0)"
                health_unknown="$(jq -r '.summary.unknown_movement // 0' "$health_json" 2>/dev/null || echo 0)"
                health_warnings="$(jq -r '.summary.warnings // 0' "$health_json" 2>/dev/null || echo 0)"
                health_repairs="$(jq -r '.summary.repair_queue // 0' "$health_json" 2>/dev/null || echo 0)"
                health_top_warning="$(jq -r '([.warning_summary[]? | "\(.kind)=\(.count)"] | if length == 0 then "none" else join(", ") end)' "$health_json" 2>/dev/null || echo unknown)"
                health_top_repair="$(jq -r '([.repair_queue[]?.repair_kind] | first) // "none"' "$health_json" 2>/dev/null || echo unknown)"
                health_status="$(markdown_inline "$health_status")"
                health_proofs="$(markdown_inline "$health_proofs")"
                health_complete="$(markdown_inline "$health_complete")"
                health_partial="$(markdown_inline "$health_partial")"
                health_missing_required="$(markdown_inline "$health_missing_required")"
                health_missing_optional="$(markdown_inline "$health_missing_optional")"
                health_improved="$(markdown_inline "$health_improved")"
                health_unchanged="$(markdown_inline "$health_unchanged")"
                health_regressed="$(markdown_inline "$health_regressed")"
                health_unknown="$(markdown_inline "$health_unknown")"
                health_warnings="$(markdown_inline "$health_warnings")"
                health_repairs="$(markdown_inline "$health_repairs")"
                health_top_warning="$(markdown_inline "$health_top_warning")"
                health_top_repair="$(markdown_inline "$health_top_repair")"
                echo '#### Agent proof status at a glance'
                echo "- Status: \`$health_status\`"
                echo "- Proof packets: total=\`$health_proofs\`, complete=\`$health_complete\`, partial=\`$health_partial\`, missing_required=\`$health_missing_required\`, missing_optional=\`$health_missing_optional\`"
                echo "- Evidence movement: improved=\`$health_improved\`, unchanged=\`$health_unchanged\`, regressed=\`$health_regressed\`, unknown=\`$health_unknown\`"
                echo "- Warnings: total=\`$health_warnings\`, top=\`$health_top_warning\`"
                echo "- Repair queue: total=\`$health_repairs\`, first=\`$health_top_repair\`"
                echo "- Health artifacts: \`target/ripr/reports/assistant-loop-health.json\`, \`target/ripr/reports/assistant-loop-health.md\`"
                echo "- Boundary: advisory static health over proof artifacts; gate evaluator remains pass/fail authority."
                echo
              fi
              if [ -f target/ripr/reports/assistant-loop-health.md ]; then
                cat target/ripr/reports/assistant-loop-health.md
              fi
              echo
            fi
            echo '### Gate decision'
            if [ -f target/ripr/reports/gate-decision.json ]; then
              gate_json=target/ripr/reports/gate-decision.json
              gate_status="$(jq -r '.status // "unknown"' "$gate_json" 2>/dev/null || echo unknown)"
              gate_mode="$(jq -r '.mode // "unknown"' "$gate_json" 2>/dev/null || echo unknown)"
              blocking="$(jq -r '.summary.blocking // 0' "$gate_json" 2>/dev/null || echo 0)"
              acknowledged="$(jq -r '.summary.acknowledged // 0' "$gate_json" 2>/dev/null || echo 0)"
              advisory="$(jq -r '.summary.advisory // 0' "$gate_json" 2>/dev/null || echo 0)"
              suppressed="$(jq -r '.summary.suppressed // 0' "$gate_json" 2>/dev/null || echo 0)"
              not_applicable="$(jq -r '.summary.not_applicable // 0' "$gate_json" 2>/dev/null || echo 0)"
              unknown_confidence="$(jq -r '.summary.unknown_confidence // 0' "$gate_json" 2>/dev/null || echo 0)"
              active_labels="$(jq -r 'if ((.inputs.labels // []) | length) == 0 then "none" else (.inputs.labels // [] | join(", ")) end' "$gate_json" 2>/dev/null || echo unknown)"
              acknowledgement_labels="$(jq -r 'if ((.policy.acknowledgement_labels // []) | length) == 0 then "none" else (.policy.acknowledgement_labels // [] | join(", ")) end' "$gate_json" 2>/dev/null || echo unknown)"
              applied_waiver="$(jq -r '([.decisions[]? | select(.decision == "acknowledged") | .policy.acknowledgement_label | select(. != null)] | first) // "none"' "$gate_json" 2>/dev/null || echo unknown)"
              baseline_artifact="$(jq -r '.inputs.baseline // "not supplied"' "$gate_json" 2>/dev/null || echo unknown)"
              recommendation_calibration="$(jq -r '.inputs.recommendation_calibration // "not supplied"' "$gate_json" 2>/dev/null || echo unknown)"
              mutation_calibration="$(jq -r '.inputs.mutation_calibration // "not supplied"' "$gate_json" 2>/dev/null || echo unknown)"
              recommendation_effects="$(jq -r '([.decisions[]?.evidence.recommendation_calibration.confidence_effect | select(. != null)] | unique | if length == 0 then "none" else join(", ") end)' "$gate_json" 2>/dev/null || echo unknown)"
              mutation_effects="$(jq -r '([.decisions[]?.evidence.mutation_calibration.confidence_effect | select(. != null)] | unique | if length == 0 then "none" else join(", ") end)' "$gate_json" 2>/dev/null || echo unknown)"
              blocking_reason="$(jq -r '([.decisions[]? | select(.decision == "blocking") | .gate_reason] | first) // "none"' "$gate_json" 2>/dev/null || echo unknown)"
              gate_status="$(markdown_inline "$gate_status")"
              gate_mode="$(markdown_inline "$gate_mode")"
              blocking="$(markdown_inline "$blocking")"
              acknowledged="$(markdown_inline "$acknowledged")"
              advisory="$(markdown_inline "$advisory")"
              suppressed="$(markdown_inline "$suppressed")"
              not_applicable="$(markdown_inline "$not_applicable")"
              unknown_confidence="$(markdown_inline "$unknown_confidence")"
              active_labels="$(markdown_inline "$active_labels")"
              acknowledgement_labels="$(markdown_inline "$acknowledgement_labels")"
              applied_waiver="$(markdown_inline "$applied_waiver")"
              baseline_artifact="$(markdown_inline "$baseline_artifact")"
              recommendation_calibration="$(markdown_inline "$recommendation_calibration")"
              mutation_calibration="$(markdown_inline "$mutation_calibration")"
              recommendation_effects="$(markdown_inline "$recommendation_effects")"
              mutation_effects="$(markdown_inline "$mutation_effects")"
              blocking_reason="$(markdown_inline "$blocking_reason")"
              echo '#### Gate decision at a glance'
              echo "- Mode: \`$gate_mode\`"
              echo "- Status: \`$gate_status\`"
              echo "- Counts: blocking=\`$blocking\`, acknowledged=\`$acknowledged\`, advisory=\`$advisory\`, suppressed=\`$suppressed\`, not_applicable=\`$not_applicable\`, unknown_confidence=\`$unknown_confidence\`"
              echo "- Active PR labels: \`$active_labels\`"
              echo "- Acknowledgement labels: \`$acknowledgement_labels\`"
              echo "- Applied waiver label: \`$applied_waiver\`"
              echo "- Baseline artifact: \`$baseline_artifact\`"
              echo "- Recommendation calibration: \`$recommendation_calibration\` (effects: $recommendation_effects)"
              echo "- Mutation calibration: \`$mutation_calibration\` (effects: $mutation_effects)"
              echo "- Blocking reason: \`$blocking_reason\`"
              echo "- Gate artifacts: \`target/ripr/reports/gate-decision.json\`, \`target/ripr/reports/gate-decision.md\`"
              echo "- Related inputs: \`target/ripr/review/comments.json\`, \`target/ci/labels.json\`"
              echo
            fi
            if [ -f target/ripr/reports/gate-decision.md ]; then
              cat target/ripr/reports/gate-decision.md
            else
              echo 'Gate decision was not run. Set `RIPR_GATE_MODE` to `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate` to opt in.'
            fi
            echo
            echo '### Baseline debt delta'
            if [ -f target/ripr/reports/baseline-debt-delta.json ]; then
              delta_json=target/ripr/reports/baseline-debt-delta.json
              baseline_path="$(jq -r '.baseline.path // .inputs.baseline // "unknown"' "$delta_json" 2>/dev/null || echo unknown)"
              still_present="$(jq -r '.delta.still_present // 0' "$delta_json" 2>/dev/null || echo 0)"
              resolved="$(jq -r '.delta.resolved // 0' "$delta_json" 2>/dev/null || echo 0)"
              new_policy_eligible="$(jq -r '.delta.new_policy_eligible // 0' "$delta_json" 2>/dev/null || echo 0)"
              acknowledged_delta="$(jq -r '.delta.acknowledged // 0' "$delta_json" 2>/dev/null || echo 0)"
              suppressed_delta="$(jq -r '.delta.suppressed // 0' "$delta_json" 2>/dev/null || echo 0)"
              stale_baseline_entry="$(jq -r '.delta.stale_baseline_entry // 0' "$delta_json" 2>/dev/null || echo 0)"
              invalid_baseline_entry="$(jq -r '.delta.invalid_baseline_entry // 0' "$delta_json" 2>/dev/null || echo 0)"
              missing_current_input="$(jq -r '.delta.missing_current_input // 0' "$delta_json" 2>/dev/null || echo 0)"
              limits_note="$(jq -r '.limits_note // "Advisory baseline debt movement; gate decision owns pass or fail."' "$delta_json" 2>/dev/null || echo unknown)"
              baseline_path="$(markdown_inline "$baseline_path")"
              still_present="$(markdown_inline "$still_present")"
              resolved="$(markdown_inline "$resolved")"
              new_policy_eligible="$(markdown_inline "$new_policy_eligible")"
              acknowledged_delta="$(markdown_inline "$acknowledged_delta")"
              suppressed_delta="$(markdown_inline "$suppressed_delta")"
              stale_baseline_entry="$(markdown_inline "$stale_baseline_entry")"
              invalid_baseline_entry="$(markdown_inline "$invalid_baseline_entry")"
              missing_current_input="$(markdown_inline "$missing_current_input")"
              limits_note="$(markdown_inline "$limits_note")"
              echo '#### Baseline debt movement'
              echo "- Baseline: \`$baseline_path\`"
              echo "- Counts: still_present=\`$still_present\`, resolved=\`$resolved\`, new_policy_eligible=\`$new_policy_eligible\`, acknowledged=\`$acknowledged_delta\`, suppressed=\`$suppressed_delta\`, stale=\`$stale_baseline_entry\`, invalid=\`$invalid_baseline_entry\`, missing_current_input=\`$missing_current_input\`"
              echo "- Boundary: $limits_note"
              echo "- Baseline delta artifacts: \`target/ripr/reports/baseline-debt-delta.json\`, \`target/ripr/reports/baseline-debt-delta.md\`"
              echo
            fi
            if [ -f target/ripr/reports/baseline-debt-delta.md ]; then
              cat target/ripr/reports/baseline-debt-delta.md
            elif [ -n "${RIPR_GATE_BASELINE:-}" ]; then
              echo 'Baseline debt delta was not generated. Check that `RIPR_GATE_MODE` produced `target/ripr/reports/gate-decision.json` and that `RIPR_GATE_BASELINE` points at a readable baseline.'
            else
              echo 'Baseline debt delta was not run. Set `RIPR_GATE_BASELINE` with an explicit gate mode to compare current evidence against reviewed baseline debt.'
            fi
            echo
            echo '### RIPR Zero status'
            if [ -f target/ripr/reports/ripr-zero-status.json ]; then
              zero_json=target/ripr/reports/ripr-zero-status.json
              zero_state="$(jq -r '.ripr_zero.state // "unknown"' "$zero_json" 2>/dev/null || echo unknown)"
              visible_unresolved="$(jq -r '.ripr_zero.visible_unresolved // 0' "$zero_json" 2>/dev/null || echo 0)"
              zero_new_policy_eligible="$(jq -r '.ripr_zero.new_policy_eligible // 0' "$zero_json" 2>/dev/null || echo 0)"
              zero_blocking_candidates="$(jq -r '.ripr_zero.blocking_candidates // 0' "$zero_json" 2>/dev/null || echo 0)"
              zero_acknowledged="$(jq -r '.ripr_zero.acknowledged // 0' "$zero_json" 2>/dev/null || echo 0)"
              zero_suppressed="$(jq -r '.ripr_zero.suppressed // 0' "$zero_json" 2>/dev/null || echo 0)"
              zero_still_present="$(jq -r '.baseline.still_present // 0' "$zero_json" 2>/dev/null || echo 0)"
              zero_resolved="$(jq -r '.baseline.resolved // 0' "$zero_json" 2>/dev/null || echo 0)"
              zero_metadata_stale="$(jq -r '.baseline.metadata.stale // 0' "$zero_json" 2>/dev/null || echo 0)"
              zero_metadata_missing="$(jq -r '.baseline.metadata.missing_metadata // 0' "$zero_json" 2>/dev/null || echo 0)"
              top_area="$(jq -r '(.top_debt_areas[0].area // "none")' "$zero_json" 2>/dev/null || echo unknown)"
              top_route="$(jq -r '(.repair_routes[0] | if . == null then "none" else ((.path // "unknown") + (if .line then ":" + (.line|tostring) else "" end) + " " + (.missing_discriminator // "missing discriminator unavailable")) end)' "$zero_json" 2>/dev/null || echo unknown)"
              trend_source="$(jq -r '.trend.source // "not_available"' "$zero_json" 2>/dev/null || echo unknown)"
              zero_state="$(markdown_inline "$zero_state")"
              visible_unresolved="$(markdown_inline "$visible_unresolved")"
              zero_new_policy_eligible="$(markdown_inline "$zero_new_policy_eligible")"
              zero_blocking_candidates="$(markdown_inline "$zero_blocking_candidates")"
              zero_acknowledged="$(markdown_inline "$zero_acknowledged")"
              zero_suppressed="$(markdown_inline "$zero_suppressed")"
              zero_still_present="$(markdown_inline "$zero_still_present")"
              zero_resolved="$(markdown_inline "$zero_resolved")"
              zero_metadata_stale="$(markdown_inline "$zero_metadata_stale")"
              zero_metadata_missing="$(markdown_inline "$zero_metadata_missing")"
              top_area="$(markdown_inline "$top_area")"
              top_route="$(markdown_inline "$top_route")"
              trend_source="$(markdown_inline "$trend_source")"
              echo '#### RIPR Zero at a glance'
              echo "- State: \`$zero_state\`"
              echo "- Visible unresolved: \`$visible_unresolved\`"
              echo "- New policy-eligible: \`$zero_new_policy_eligible\`"
              echo "- Blocking candidates: \`$zero_blocking_candidates\`"
              echo "- Acknowledged: \`$zero_acknowledged\`"
              echo "- Suppressed: \`$zero_suppressed\`"
              echo "- Baseline still present: \`$zero_still_present\`"
              echo "- Baseline resolved: \`$zero_resolved\`"
              echo "- Baseline metadata: stale=\`$zero_metadata_stale\`, missing=\`$zero_metadata_missing\`"
              echo "- Top debt area: \`$top_area\`"
              echo "- Top repair route: \`$top_route\`"
              echo "- Trend source: \`$trend_source\`"
              echo "- RIPR Zero artifacts: \`target/ripr/reports/ripr-zero-status.json\`, \`target/ripr/reports/ripr-zero-status.md\`"
              echo
            fi
            if [ -f target/ripr/reports/ripr-zero-status.md ]; then
              cat target/ripr/reports/ripr-zero-status.md
            elif [ -f target/ripr/reports/baseline-debt-delta.json ]; then
              echo 'RIPR Zero status was not generated. Inspect `target/ripr/reports/baseline-debt-delta.json` and rerun `ripr zero status` locally.'
            else
              echo 'RIPR Zero status was not run. It requires `baseline-debt-delta.json`, which is produced only after an explicit gate mode and reviewed baseline are configured.'
            fi
            echo
            echo '### SARIF and badge status'
            if [ "${RIPR_UPLOAD_SARIF:-}" = "true" ]; then
              if [ -f target/ripr/reports/ripr-findings.sarif ]; then echo "- Diff SARIF: generated"; else echo "- Diff SARIF: missing or skipped"; fi
              if [ -f target/ripr/reports/ripr-seams.sarif ]; then echo "- Repo seam SARIF: generated"; else echo "- Repo seam SARIF: missing or skipped"; fi
            else
              echo '- SARIF upload: disabled by `RIPR_UPLOAD_SARIF`'
            fi
            if [ -f target/ripr/reports/repo-ripr-badge.json ]; then echo "- Badge JSON: generated"; else echo "- Badge JSON: missing or skipped"; fi
            if [ -f target/ripr/reports/repo-ripr-badge-shields.json ]; then echo "- Badge Shields JSON: generated"; else echo "- Badge Shields JSON: missing or skipped"; fi
            echo
            echo '### PR guidance annotations'
            if [ -f target/ripr/review/comments.json ]; then
              comments="$(jq -r '.summary.comments // 0' target/ripr/review/comments.json 2>/dev/null || echo 0)"
              summary_only="$(jq -r '.summary.summary_only // 0' target/ripr/review/comments.json 2>/dev/null || echo 0)"
              suppressed="$(jq -r '.summary.suppressed // 0' target/ripr/review/comments.json 2>/dev/null || echo 0)"
              echo "- Changed-line annotations emitted: $comments"
              echo "- Summary-only recommendations: $summary_only"
              echo "- Suppressed recommendations: $suppressed"
            else
              echo 'No PR test guidance report was generated. When `ripr review-comments` writes `target/ripr/review/comments.json`, this workflow emits changed-line check annotations by default.'
            fi
            echo
            echo '### PR inline comments'
            comment_mode="$(markdown_inline "${RIPR_COMMENT_MODE:-off}")"
            echo "- Mode: \`$comment_mode\`"
            if [ -f target/ripr/review/comment-publish-plan.json ]; then
              comment_plan=target/ripr/review/comment-publish-plan.json
              comment_status="$(jq -r '.status // "unknown"' "$comment_plan" 2>/dev/null || echo unknown)"
              comment_publishable="$(jq -r '.summary.publishable // 0' "$comment_plan" 2>/dev/null || echo 0)"
              comment_skipped="$(jq -r '.summary.skipped // 0' "$comment_plan" 2>/dev/null || echo 0)"
              comment_blocked="$(jq -r '.summary.blocked // 0' "$comment_plan" 2>/dev/null || echo 0)"
              comment_safe="$(jq -r '.summary.safe_to_publish // false' "$comment_plan" 2>/dev/null || echo false)"
              comment_status="$(markdown_inline "$comment_status")"
              comment_publishable="$(markdown_inline "$comment_publishable")"
              comment_skipped="$(markdown_inline "$comment_skipped")"
              comment_blocked="$(markdown_inline "$comment_blocked")"
              comment_safe="$(markdown_inline "$comment_safe")"
              echo "- Status: \`$comment_status\`"
              echo "- Counts: publishable=\`$comment_publishable\`, skipped=\`$comment_skipped\`, blocked=\`$comment_blocked\`"
              echo "- Safe to publish: \`$comment_safe\`"
              echo "- Plan artifacts: \`target/ripr/review/comment-publish-plan.json\`, \`target/ripr/review/comment-publish-plan.md\`"
              echo "- Boundary: inline comments remain opt-in; gate decisions remain separate pass/fail authority."
              echo
              if [ -f target/ripr/review/comment-publish-plan.md ]; then
                cat target/ripr/review/comment-publish-plan.md
              fi
            else
              echo '- Inline comments are disabled by default. Set `RIPR_COMMENT_MODE` to `plan` to inspect a publish plan or `inline` to publish same-repo changed-line comments when permissions are safe.'
            fi
            echo
            echo '### Known limits'
            echo "- Advisory static evidence only; review the named seam and write one focused test."
            echo "- No automatic source edits or generated tests."
            echo "- No runtime mutation execution is performed by this workflow."
          } >> "$GITHUB_STEP_SUMMARY"

"#
}

fn artifact_upload_steps() -> &'static str {
    r#"      - name: Upload RIPR report artifacts
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
"#
}
