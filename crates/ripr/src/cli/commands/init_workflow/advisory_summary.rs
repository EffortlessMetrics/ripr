//! The generated workflow's `Add RIPR advisory summary` step.
//!
//! #4386 (slice 2, include form): the 1,266-line shell/jq body of this step
//! used to sit inline in the middle of `init.rs`'s workflow template string,
//! where every fix landed as a Rust-string diff that hid shell quoting and
//! jq errors. It now lives here as its own raw-string constant, and
//! `init_workflow` splices it between the template head and tail. The
//! spliced bytes are identical to the pre-extraction bytes, pinned by the
//! template hash in `init_workflow`; rendering is unchanged. Promoting this
//! step behind a real `ripr` subcommand is the issue's separate slice-2
//! follow-up (a new public CLI surface, per the #4386 claim boundary).

/// The complete step, from `      - name: Add RIPR advisory summary` through
/// the blank line before `      - name: Check RIPR advisory artifacts`.
pub(super) const ADVISORY_SUMMARY_STEP: &str = r#"      - name: Add RIPR advisory summary
        if: always()
        continue-on-error: true
        run: |
          {
            markdown_inline() {
              printf '%s' "$1" | tr '\r\n' '  ' | sed 's/`/\\`/g'
            }
            # Artifacts bind commands to this runner's absolute checkout
            # (#3999), but a summary reader copies them on another machine.
            # Rewrite the checkout path, where it is a whole path token, to
            # the repository root `.`, like the Agent review packet block.
            repo_relative() {
              RIPR_CHECKOUT_PHYSICAL="$(pwd -P)" RIPR_CHECKOUT_LOGICAL="$PWD" awk '
                function rel(s, root,   out, i, pre, rest, before, after) {
                  out = ""
                  while (root != "" && (i = index(s, root)) > 0) {
                    pre = substr(s, 1, i - 1)
                    rest = substr(s, i + length(root))
                    before = substr(pre, length(pre), 1)
                    after = substr(rest, 1, 1)
                    if ((before == "" || before ~ /[ \047"`=(]/) && (after == "" || after ~ /[\/ \047"`):]/)) {
                      out = out pre "."
                    } else {
                      out = out pre root
                    }
                    s = rest
                  }
                  return out s
                }
                { print rel(rel($0, ENVIRON["RIPR_CHECKOUT_PHYSICAL"]), ENVIRON["RIPR_CHECKOUT_LOGICAL"]) }
              '
            }

            echo '## RIPR advisory summary'
            echo
            echo "RIPR is advisory static evidence. It does not edit source, generate tests, or run mutation testing."
            echo
            echo '### Start here'
            echo '- Open `target/ripr/reports/start-here.md` first when it exists.'
            echo '- Then open `target/ripr/reports/index.md` to navigate deeper evidence artifacts.'
            echo '- Safe next action: repair one named gap, regenerate missing or malformed artifacts, refresh stale evidence, fix wrong-root setup, or stop on no-action.'
            echo '- Recovery states: missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, and preview-limited evidence are explicit stop or regeneration states.'
            echo '- Proof rail: the repair start, verify, receipt, and receipt path are static movement evidence only; verify and receipt run after the test edit.'
            echo '- Preview boundary: preview-limited evidence stays syntax-first and advisory, with static limits before repair language.'
            echo '- Gate authority: `ripr gate evaluate` remains the pass/fail source only when `RIPR_GATE_MODE` is configured.'
            if [ -f target/ripr/reports/start-here.md ]; then
              echo '- Start-here artifact: `target/ripr/reports/start-here.md`'
            elif [ -f target/ripr/reports/index.json ]; then
              start_here_path="$(jq -r '.summary.start_here // "not_available"' target/ripr/reports/index.json 2>/dev/null || echo not_available)"
              start_here_path="$(markdown_inline "$start_here_path")"
              echo "- Start-here artifact: \`$start_here_path\`"
            elif [ -f target/ripr/reports/pr-review-front-panel.md ]; then
              echo '- Start-here artifact: `target/ripr/reports/pr-review-front-panel.md`'
            elif [ -f target/ripr/pilot/pilot-summary.md ]; then
              echo '- Start-here artifact: `target/ripr/pilot/pilot-summary.md`'
            else
              echo '- Start-here artifact: not generated yet; inspect uploaded artifacts and job logs.'
            fi
            echo
            echo '#### First-run status'
            if [ -f target/ripr/reports/start-here.json ]; then
              start_json=target/ripr/reports/start-here.json
              start_status="$(jq -r '.status // "unknown"' "$start_json" 2>/dev/null || echo unknown)"
              start_state="$(jq -r '.selected.state // "unknown"' "$start_json" 2>/dev/null || echo unknown)"
              start_gap="$(jq -r '.selected.canonical_gap_id // .selected.gap_id // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_language="$(jq -r 'if .selected.language then (.selected.language + " (" + (.selected.language_status // "unknown") + ")") else "not_available" end' "$start_json" 2>/dev/null || echo unknown)"
              start_kind="$(jq -r '.selected.kind // "none"' "$start_json" 2>/dev/null || echo unknown)"
              start_changed="$(jq -r '.selected.changed_behavior // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_evidence="$(jq -r '.selected.current_evidence_strength // .selected.state // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_missing="$(jq -r '.selected.missing_discriminator // .selected.repair.suggested_assertion // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_repair="$(jq -r '.selected.repair.route // .selected.repair.suggested_assertion // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_focused="$(jq -r '.selected.focused_proof_intent // .selected.repair.suggested_assertion // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_boundary="$(jq -r '.selected.static_evidence_boundary // "static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval."' "$start_json" 2>/dev/null || echo unknown)"
              start_target="$(jq -r '.selected.repair.target_file // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_related="$(jq -r '.selected.repair.related_test // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_limit="$(jq -r 'if .selected.static_limit_kind then (.selected.static_limit_kind + (if .selected.static_limit_detail then ": " + .selected.static_limit_detail else "" end)) else "none" end' "$start_json" 2>/dev/null || echo unknown)"
              start_verify="$(jq -r '.selected.verify_command // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_receipt="$(jq -r '.selected.receipt_command // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_receipt_path="$(jq -r '.selected.receipt_path // "not_available"' "$start_json" 2>/dev/null || echo unknown)"
              start_receipt_state="$(jq -r '.selected.receipt_state // "receipt_missing"' "$start_json" 2>/dev/null || echo unknown)"
              start_repair_command="$(jq -r '.selected.repair_command // empty' "$start_json" 2>/dev/null || true)"
              start_next="$(jq -r '.selected.repair_command // .selected.next_command // .selected.regeneration_command // "none"' "$start_json" 2>/dev/null || echo unknown)"
              start_warnings="$(jq -r '(.warnings // [] | length)' "$start_json" 2>/dev/null || echo 0)"
              start_status="$(markdown_inline "$start_status")"
              start_state="$(markdown_inline "$start_state")"
              start_gap="$(markdown_inline "$start_gap")"
              start_language="$(markdown_inline "$start_language")"
              start_kind="$(markdown_inline "$start_kind")"
              start_changed="$(markdown_inline "$start_changed")"
              start_evidence="$(markdown_inline "$start_evidence")"
              start_missing="$(markdown_inline "$start_missing")"
              start_repair="$(markdown_inline "$start_repair")"
              start_focused="$(markdown_inline "$start_focused")"
              start_boundary="$(markdown_inline "$start_boundary")"
              start_target="$(markdown_inline "$start_target")"
              start_related="$(markdown_inline "$start_related")"
              start_limit="$(markdown_inline "$start_limit")"
              start_verify="$(markdown_inline "$start_verify")"
              start_receipt="$(markdown_inline "$start_receipt")"
              start_receipt_path="$(markdown_inline "$start_receipt_path")"
              start_receipt_state="$(markdown_inline "$start_receipt_state")"
              start_next="$(markdown_inline "$start_next")"
              start_warnings="$(markdown_inline "$start_warnings")"
              # A carried repair start leads the block (#3906): its after
              # phase runs verify and writes the receipt, so the low-level
              # verify and receipt commands below are the manual alternative.
              if [ -n "$start_repair_command" ]; then
                start_repair_command="$(markdown_inline "$start_repair_command")"
                echo "- Start repair: \`$start_repair_command\`"
                echo '- @RIPR_REPAIR_AFTER_PHASE@'
                start_verify_label='@RIPR_MANUAL_VERIFY_LABEL@'
                start_receipt_label='@RIPR_MANUAL_RECEIPT_LABEL@'
              else
                start_verify_label='@RIPR_VERIFY_AFTER_EDIT_LABEL@'
                start_receipt_label='@RIPR_RECEIPT_AFTER_VERIFY_LABEL@'
              fi
              echo "- Status: \`$start_status\`"
              echo "- Selected state: \`$start_state\`"
              echo "- Canonical gap: \`$start_gap\`"
              echo "- Language: \`$start_language\`"
              echo "- Top gap/no-action: \`$start_kind\`"
              echo "- Repair: \`$start_repair\`"
              echo "- Changed behavior: \`$start_changed\`"
              echo "- Current evidence strength: \`$start_evidence\`"
              echo "- Missing discriminator: \`$start_missing\`"
              echo "- Focused proof intent: \`$start_focused\`"
              echo "- Boundary: \`$start_boundary\`"
              echo "- Repair target: \`$start_target\`"
              echo "- Related test: \`$start_related\`"
              echo "- Static limit: \`$start_limit\`"
              echo "- $start_verify_label: \`$start_verify\`"
              echo "- $start_receipt_label: \`$start_receipt\`"
              echo "- Receipt path: \`$start_receipt_path\`"
              echo "- Receipt state: \`$start_receipt_state\`"
              echo "- Safe next action command: \`$start_next\`"
              echo "- Warnings: \`$start_warnings\`"
              echo "- Artifacts: \`target/ripr/reports/start-here.json\`, \`target/ripr/reports/start-here.md\`"
              echo "- Boundary: start-here is advisory first-run guidance only; gate decision remains separate pass/fail authority when configured."
              if [ -f target/ripr/reports/start-here.md ]; then
                echo
                echo '<details><summary>Full report: target/ripr/reports/start-here.md</summary>'
                echo
                cat target/ripr/reports/start-here.md
                echo
                echo '</details>'
              fi
            elif [ -f target/ripr/reports/first-useful-action.json ]; then
              first_json=target/ripr/reports/first-useful-action.json
              first_status="$(jq -r '.status // "unknown"' "$first_json" 2>/dev/null || echo unknown)"
              first_action_kind="$(jq -r '.action_kind // "unknown"' "$first_json" 2>/dev/null || echo unknown)"
              first_title="$(jq -r '.title // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_why="$(jq -r '.why // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_changed="$(jq -r '.selected.changed_behavior // .why // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_evidence="$(jq -r '.selected.current_evidence_strength // .selected.classification // .status // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_missing="$(jq -r '.selected.missing_discriminator // .target.suggested_assertion // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_proof="$(jq -r '.selected.focused_proof_intent // .target.suggested_assertion // .title // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_gap="$(jq -r 'if .selected == null then "none" else ((.selected.path // "unknown") + (if .selected.line then ":" + (.selected.line|tostring) else "" end) + " " + (.selected.missing_discriminator // .selected.classification // .selected.seam_id // "gap")) end' "$first_json" 2>/dev/null || echo unknown)"
              first_target="$(jq -r 'if .target == null then "none" else ((.target.file // "not_available") + (if .target.related_test then " related_test=" + .target.related_test else "" end) + (if .target.suggested_test_name then " suggested=" + .target.suggested_test_name else "" end)) end' "$first_json" 2>/dev/null || echo unknown)"
              first_repair="$(jq -r '.commands.repair // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_packet="$(jq -r '.commands.context_packet // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_verify="$(jq -r '.commands.verify // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_receipt="$(jq -r '.commands.receipt // "not_available"' "$first_json" 2>/dev/null || echo unknown)"
              first_fallback="$(jq -r '.fallback.summary // .fallback.kind // "none"' "$first_json" 2>/dev/null || echo unknown)"
              first_warnings="$(jq -r '(.warnings // [] | length)' "$first_json" 2>/dev/null || echo 0)"
              first_status="$(markdown_inline "$first_status")"
              first_action_kind="$(markdown_inline "$first_action_kind")"
              first_title="$(markdown_inline "$first_title")"
              first_why="$(markdown_inline "$first_why")"
              first_changed="$(markdown_inline "$first_changed")"
              first_evidence="$(markdown_inline "$first_evidence")"
              first_missing="$(markdown_inline "$first_missing")"
              first_proof="$(markdown_inline "$first_proof")"
              first_gap="$(markdown_inline "$first_gap")"
              first_target="$(markdown_inline "$first_target")"
              first_repair="$(markdown_inline "$first_repair")"
              first_packet="$(markdown_inline "$first_packet")"
              first_verify="$(markdown_inline "$first_verify")"
              first_receipt="$(markdown_inline "$first_receipt")"
              first_fallback="$(markdown_inline "$first_fallback")"
              first_warnings="$(markdown_inline "$first_warnings")"
              # #3906: a carried repair start leads the block; its after phase
              # runs verify and writes the receipt, so verify and receipt below are
              # the manual alternative. Without one they run after the test edit.
              if [ "$first_repair" != not_available ] && [ "$first_repair" != unknown ]; then
                echo "- Repair start: \`$first_repair\`"
                echo '- @RIPR_REPAIR_AFTER_PHASE@'
                first_verify_label='@RIPR_MANUAL_VERIFY_LABEL@'
                first_receipt_label='@RIPR_MANUAL_RECEIPT_LABEL@'
              else
                first_verify_label='@RIPR_VERIFY_AFTER_EDIT_LABEL@'
                first_receipt_label='@RIPR_RECEIPT_AFTER_VERIFY_LABEL@'
              fi
              echo "- Status: \`$first_status\`"
              echo "- Safe next action: \`$first_action_kind\`"
              echo "- Title: \`$first_title\`"
              echo "- Why: \`$first_why\`"
              echo "- Changed behavior: \`$first_changed\`"
              echo "- Current evidence strength: \`$first_evidence\`"
              echo "- Missing discriminator: \`$first_missing\`"
              echo "- Focused proof intent: \`$first_proof\`"
              echo "- Gap: \`$first_gap\`"
              echo "- Repair target: \`$first_target\`"
              echo "- Agent packet: \`$first_packet\`"
              echo "- $first_verify_label: \`$first_verify\`"
              echo "- $first_receipt_label: \`$first_receipt\`"
              echo "- Fallback/no-action: \`$first_fallback\`"
              echo "- Warnings: \`$first_warnings\`"
              echo "- Artifacts: \`target/ripr/reports/first-useful-action.json\`, \`target/ripr/reports/first-useful-action.md\`, \`target/ripr/workflow/agent-packet.json\`"
              echo "- Boundary: advisory first-run path only; gate decision remains separate pass/fail authority when configured."
            else
              echo "- Status: \`missing_start_here\`"
              echo "- State: \`missing_artifact\`"
              echo "- Safe next action: run \`ripr first-pr --root . --base origin/${{ github.base_ref || github.event.repository.default_branch }} --head HEAD --gap-ledger target/ripr/reports/gap-decision-ledger.json --first-action target/ripr/reports/first-useful-action.json --review-comments target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-packet.json --gate-decision target/ripr/reports/gate-decision.json --receipts-dir target/ripr/receipts --out-dir target/ripr/reports\`."
              echo "- Fallback safe next action: run \`ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md\` after attaching at least one explicit input."
              echo "- Boundary: missing start-here packet does not fail generated CI or create gate authority."
            fi
            echo
            configured_languages="$(
              ripr doctor --root . --json 2>/dev/null \
                | jq -r '.languages | join(",")' 2>/dev/null \
                || true
            )"
            if [ -z "$configured_languages" ]; then
              configured_languages="rust"
            fi
            preview_languages="$(
              printf '%s\n' "$configured_languages" \
                | tr ',' '\n' \
                | sed 's/^ *//; s/ *$//' \
                | sed -n '/^typescript$/p; /^python$/p' \
                | sort -u \
                | tr '\n' ' ' \
                | sed 's/ $//' \
                || true
            )"
            if [ -n "$preview_languages" ]; then
              grouped_preview_languages="$preview_languages"
              if printf '%s\n' "$preview_languages" | tr ' ' '\n' | grep -qx 'typescript'; then
                grouped_preview_languages="$grouped_preview_languages javascript"
              fi
              grouped_preview_languages="$(
                printf '%s\n' $grouped_preview_languages \
                  | sort -u \
                  | tr '\n' ' ' \
                  | sed 's/ $//' \
                  || true
              )"
              configured_inline="$(markdown_inline "$configured_languages")"
              grouped_inline="$(markdown_inline "$grouped_preview_languages")"
              echo '### Language preview grouping'
              echo "- Configured languages: \`$configured_inline\`"
              echo "- Grouped preview evidence languages: \`$grouped_inline\`"
              echo "- Boundary: preview-language groups are advisory presentation only; \`ripr gate evaluate\` remains pass/fail authority when explicitly configured."
              for language in $grouped_preview_languages; do
                language_inputs=()
                if [ -f target/ripr/reports/repo-exposure.json ]; then
                  language_inputs+=(target/ripr/reports/repo-exposure.json)
                elif [ -f target/ripr/pilot/repo-exposure.json ]; then
                  language_inputs+=(target/ripr/pilot/repo-exposure.json)
                fi
                for language_json in \
                  target/ripr/review/comments.json \
                  target/ripr/reports/gate-decision.json \
                  target/ripr/reports/pr-evidence-ledger.json; do
                  if [ -f "$language_json" ]; then
                    language_inputs+=("$language_json")
                  fi
                done

                artifact_entries=0
                preview_entries=0
                missing_preview_status=0
                static_limit_entries=0
                class_counts="none"
                static_limit_kinds="none"
                actionability_states="none"
                actionability_categories="none"
                repair_packet_ready_entries=0
                if [ "${#language_inputs[@]}" -gt 0 ]; then
                  artifact_entries="$(jq -s -r --arg language "$language" '[.[] | .. | objects | select(.language? == $language)] | length' "${language_inputs[@]}" 2>/dev/null || echo 0)"
                  preview_entries="$(jq -s -r --arg language "$language" '[.[] | .. | objects | select(.language? == $language and .language_status? == "preview")] | length' "${language_inputs[@]}" 2>/dev/null || echo 0)"
                  missing_preview_status="$(jq -s -r --arg language "$language" '[.[] | .. | objects | select(.language? == $language and .language_status? != "preview")] | length' "${language_inputs[@]}" 2>/dev/null || echo 0)"
                  static_limit_entries="$(jq -s -r --arg language "$language" '[.[] | .. | objects | select(.language? == $language and .static_limit_kind? != null)] | length' "${language_inputs[@]}" 2>/dev/null || echo 0)"
                  class_counts="$(jq -s -r --arg language "$language" '[.[] | .. | objects | select(.language? == $language and .classification? != null) | .classification] | sort | group_by(.) | map("\(.[0])=\(length)") | if length == 0 then "none" else join(", ") end' "${language_inputs[@]}" 2>/dev/null || echo none)"
                  static_limit_kinds="$(jq -s -r --arg language "$language" '[.[] | .. | objects | select(.language? == $language) | .static_limit_kind? | select(. != null)] | unique | if length == 0 then "none" else join(", ") end' "${language_inputs[@]}" 2>/dev/null || echo none)"
                  actionability_states="$(jq -s -r --arg language "$language" '[.[] | .. | objects | select(.language? == $language) | (.preview_actionability?.gap_state // .gap_state?) | select(. != null)] | sort | group_by(.) | map("\(.[0])=\(length)") | if length == 0 then "none" else join(", ") end' "${language_inputs[@]}" 2>/dev/null || echo none)"
                  actionability_categories="$(jq -s -r --arg language "$language" '[.[] | .. | objects | select(.language? == $language) | (.preview_actionability?.actionability_category // .actionability_category?) | select(. != null)] | sort | group_by(.) | map("\(.[0])=\(length)") | if length == 0 then "none" else join(", ") end' "${language_inputs[@]}" 2>/dev/null || echo none)"
                  repair_packet_ready_entries="$(jq -s -r --arg language "$language" '[.[] | .. | objects | select(.language? == $language and .preview_actionability?.repair_packet_ready == true)] | length' "${language_inputs[@]}" 2>/dev/null || echo 0)"
                fi
                language_inline="$(markdown_inline "$language")"
                artifact_entries="$(markdown_inline "$artifact_entries")"
                preview_entries="$(markdown_inline "$preview_entries")"
                missing_preview_status="$(markdown_inline "$missing_preview_status")"
                static_limit_entries="$(markdown_inline "$static_limit_entries")"
                class_counts="$(markdown_inline "$class_counts")"
                static_limit_kinds="$(markdown_inline "$static_limit_kinds")"
                actionability_states="$(markdown_inline "$actionability_states")"
                actionability_categories="$(markdown_inline "$actionability_categories")"
                repair_packet_ready_entries="$(markdown_inline "$repair_packet_ready_entries")"
                if [ "$artifact_entries" = "0" ]; then
                  echo "- \`$language_inline\`: configured preview/advisory; no language findings were emitted in this run; gate_impact=\`none\`."
                else
                  echo "- \`$language_inline\`: artifact_entries=\`$artifact_entries\`, preview_entries=\`$preview_entries\`, missing_preview_status=\`$missing_preview_status\`, static_limit_entries=\`$static_limit_entries\`, classifications=\`$class_counts\`, static_limit_kinds=\`$static_limit_kinds\`, actionability_states=\`$actionability_states\`, actionability_categories=\`$actionability_categories\`, repair_packet_ready=\`$repair_packet_ready_entries\`, gate_impact=\`none\`"
                fi
              done
              echo
            fi
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
                panel_repair="$(jq -r '.top_issue.repair_command // "not_available"' "$panel_json" 2>/dev/null || echo unknown)"
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
                panel_verify="$(markdown_inline "$(printf '%s\n' "$panel_verify" | repo_relative)")"
                panel_agent="$(markdown_inline "$(printf '%s\n' "$panel_agent" | repo_relative)")"
                panel_repair="$(markdown_inline "$(printf '%s\n' "$panel_repair" | repo_relative)")"
                panel_receipt="$(markdown_inline "$(printf '%s\n' "$panel_receipt" | repo_relative)")"
                panel_gate_mode="$(markdown_inline "$panel_gate_mode")"
                panel_gate_decision="$(markdown_inline "$panel_gate_decision")"
                panel_warning_count="$(markdown_inline "$panel_warning_count")"
                echo '#### PR review at a glance'
                # #3906: a carried repair start leads; its after phase runs verify,
                # so the verify command is the manual alternative.
                if [ "$panel_repair" != not_available ] && [ "$panel_repair" != unknown ]; then
                  echo "- Repair start: \`$panel_repair\`"
                  echo '- @RIPR_REPAIR_AFTER_PHASE@'
                  panel_verify_label='@RIPR_MANUAL_VERIFY_LABEL@'
                else
                  panel_verify_label='@RIPR_VERIFY_AFTER_EDIT_LABEL@'
                fi
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
                echo "- $panel_verify_label: \`$panel_verify\`"
                if [ "$panel_agent" != "$panel_repair" ]; then
                  echo "- Agent handoff: \`$panel_agent\`"
                fi
                echo "- Receipt: \`$panel_receipt\`"
                echo "- Gate: mode=\`$panel_gate_mode\`, decision=\`$panel_gate_decision\`"
                echo "- Warnings: \`$panel_warning_count\`"
                echo "- Front-panel artifacts: \`target/ripr/reports/pr-review-front-panel.json\`, \`target/ripr/reports/pr-review-front-panel.md\`"
                echo "- Pass/fail authority remains \`ripr gate evaluate\` when an explicit gate mode is configured."
                echo
              fi
              if [ -f target/ripr/reports/pr-review-front-panel.md ]; then
                echo '<details><summary>Full report: target/ripr/reports/pr-review-front-panel.md</summary>'
                echo
                repo_relative < target/ripr/reports/pr-review-front-panel.md
                echo
                echo '</details>'
              fi
            else
              echo 'PR review summary was not generated. It runs when existing PR guidance, first-useful-action, assistant proof, health, ledger, baseline, gate, calibration, coverage/grip, or receipt artifacts are available.'
              echo 'Safe next action: run `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md` after attaching at least one explicit input.'
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
                action_repair="$(jq -r '.commands.repair // "not_available"' "$action_json" 2>/dev/null || echo unknown)"
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
                action_repair="$(markdown_inline "$(printf '%s\n' "$action_repair" | repo_relative)")"
                action_verify="$(markdown_inline "$(printf '%s\n' "$action_verify" | repo_relative)")"
                action_receipt="$(markdown_inline "$(printf '%s\n' "$action_receipt" | repo_relative)")"
                action_fallback="$(markdown_inline "$action_fallback")"
                action_warning_count="$(markdown_inline "$action_warning_count")"
                echo '#### Recommended next test at a glance'
                # #3906: a carried repair start leads; its after phase runs verify
                # and writes the receipt, so verify and receipt are the manual
                # alternative. Without one they run after the focused test edit.
                if [ "$action_repair" != not_available ] && [ "$action_repair" != unknown ]; then
                  echo "- Repair start: \`$action_repair\`"
                  echo '- @RIPR_REPAIR_AFTER_PHASE@'
                  action_verify_label='@RIPR_MANUAL_VERIFY_LABEL@'
                  action_receipt_label='@RIPR_MANUAL_RECEIPT_LABEL@'
                else
                  action_verify_label='@RIPR_VERIFY_AFTER_EDIT_LABEL@'
                  action_receipt_label='@RIPR_RECEIPT_AFTER_VERIFY_LABEL@'
                fi
                echo "- Status: \`$action_status\`"
                echo "- Safe next action: \`$action_kind\`"
                echo "- Title: \`$action_title\`"
                echo "- Why: \`$action_why\`"
                echo "- Seam: \`$action_seam\`"
                echo "- Target: \`$action_target\`"
                echo "- $action_verify_label: \`$action_verify\`"
                echo "- $action_receipt_label: \`$action_receipt\`"
                echo "- Fallback: \`$action_fallback\`"
                echo "- Warnings: \`$action_warning_count\`"
                echo "- Action artifacts: \`target/ripr/reports/first-useful-action.json\`, \`target/ripr/reports/first-useful-action.md\`"
                echo "- Boundary: static evidence only; no runtime mutation execution."
                echo
              fi
              if [ -f target/ripr/reports/first-useful-action.md ]; then
                echo '<details><summary>Full report: target/ripr/reports/first-useful-action.md</summary>'
                echo
                repo_relative < target/ripr/reports/first-useful-action.md
                echo
                echo '</details>'
              fi
            else
              echo 'Recommended next test was not generated. It runs when existing PR guidance, assistant proof, ledger, baseline, receipt, gate, coverage/grip, or editor context artifacts are available.'
              echo 'Safe next action: run `ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md` after attaching at least one explicit input.'
            fi
            echo
            echo '### Top recommendation'
            if [ -f target/ripr/pilot/pilot-summary.md ]; then
              echo '<details><summary>Full report: target/ripr/pilot/pilot-summary.md</summary>'
              echo
              cat target/ripr/pilot/pilot-summary.md
              echo
              echo '</details>'
            else
              echo "Pilot summary was not generated. Inspect the uploaded artifact packet and job logs."
            fi
            echo
            echo '### Agent review packet'
            # CI runs before any test edit, so no receipt exists yet (#3906,
            # N5). The packet then leads with the carried repair start and
            # its after phase, like every other block, instead of the
            # low-level post-edit loop; the full summary stays an artifact.
            review_movement=''
            if [ -f target/ripr/workflow/agent-review-summary.json ]; then
              review_movement="$(jq -r '.static_movement.state // empty' target/ripr/workflow/agent-review-summary.json 2>/dev/null || true)"
            fi
            if [ "$review_movement" = missing_artifact ]; then
              echo '- Receipt: @RIPR_NO_RECEIPT_BEFORE_REPAIR@'
              review_repair_command=''
              if [ -f target/ripr/reports/start-here.json ]; then
                review_repair_command="$(jq -r '.selected.repair_command // empty' target/ripr/reports/start-here.json 2>/dev/null || true)"
              fi
              if [ -n "$review_repair_command" ]; then
                review_repair_command="$(markdown_inline "$review_repair_command")"
                echo "- Start repair: \`$review_repair_command\`"
                echo '- @RIPR_REPAIR_AFTER_PHASE@'
              else
                echo '- No repair start is available; `ripr agent status --root .` names the next step on a local checkout.'
              fi
              echo '- Full packet: `target/ripr/workflow/agent-review-summary.md` (workflow artifact).'
            elif [ -f target/ripr/workflow/agent-review-summary.md ]; then
              echo '<details><summary>Full report: target/ripr/workflow/agent-review-summary.md</summary>'
              echo
              cat target/ripr/workflow/agent-review-summary.md
              echo
              echo '</details>'
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
                echo '<details><summary>Full report: target/ripr/reports/index.md</summary>'
                echo
                cat target/ripr/reports/index.md
                echo
                echo '</details>'
              fi
            else
              echo 'Uploaded review artifacts summary was not generated. It runs when existing RIPR report, review, receipt, workflow, agent, pilot, or CI artifacts are available.'
              echo 'Regenerate command: `ripr reports index --root . --reports-dir target/ripr/reports --review-dir target/ripr/review --receipts-dir target/ripr/receipts --workflow-dir target/ripr/workflow --agent-dir target/ripr/agent --pilot-dir target/ripr/pilot --ci-dir target/ci --out target/ripr/reports/index.json --out-md target/ripr/reports/index.md`.'
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
              ledger_repair="$(jq -r '.top_repair_route.repair_command // "not_available"' "$ledger_json" 2>/dev/null || echo unknown)"
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
              ledger_repair="$(markdown_inline "$ledger_repair")"
              echo '#### PR movement at a glance'
              # #3906: a carried repair start leads; its after phase runs verify,
              # so the verify command is the manual alternative.
              if [ "$ledger_repair" != not_available ] && [ "$ledger_repair" != unknown ]; then
                echo "- Repair start: \`$ledger_repair\`"
                echo '- @RIPR_REPAIR_AFTER_PHASE@'
                ledger_verify_label='@RIPR_MANUAL_VERIFY_LABEL@'
              else
                ledger_verify_label='@RIPR_VERIFY_AFTER_EDIT_LABEL@'
              fi
              echo "- Status: \`$ledger_status\`"
              echo "- Gate: mode=\`$ledger_gate_mode\`, decision=\`$ledger_gate_decision\`"
              # F60-4: counts with no baseline delta or RIPR Zero status
              # behind them were never measured; do not print their zeros.
              ledger_count_source="$(jq -r '.movement.count_source // "unknown"' "$ledger_json" 2>/dev/null || echo unknown)"
              if [ "$ledger_count_source" = "not_measured" ]; then
                echo "- Counts: gap counts not measured (no baseline debt delta or RIPR Zero status); acknowledged=\`$ledger_acknowledged\`, suppressed=\`$ledger_suppressed\`, blocking_candidates=\`$ledger_blocking\`"
              else
                echo "- Counts: new_policy_eligible=\`$ledger_new_policy_eligible\`, baseline_still_present=\`$ledger_still_present\`, baseline_resolved=\`$ledger_resolved\`, acknowledged=\`$ledger_acknowledged\`, suppressed=\`$ledger_suppressed\`, blocking_candidates=\`$ledger_blocking\`, visible_unresolved=\`$ledger_visible\`"
              fi
              echo "- Top repair route: \`$ledger_route\`"
              echo "- $ledger_verify_label: \`$ledger_verify\`"
              if [ "$ledger_agent" != "$ledger_repair" ]; then
                echo "- Agent command: \`$ledger_agent\`"
              fi
              echo "- Coverage/grip frontier: \`$ledger_coverage_status\`"
              echo "- History trend: \`$ledger_trend\`"
              echo "- Ledger artifacts: \`target/ripr/reports/pr-evidence-ledger.json\`, \`target/ripr/reports/pr-evidence-ledger.md\`"
              echo "- Pass/fail authority remains \`ripr gate evaluate\` when an explicit gate mode is configured."
              echo
            fi
            if [ -f target/ripr/reports/pr-evidence-ledger.md ]; then
              echo '<details><summary>Full report: target/ripr/reports/pr-evidence-ledger.md</summary>'
              echo
              cat target/ripr/reports/pr-evidence-ledger.md
              echo
              echo '</details>'
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
                echo '<details><summary>Full report: target/ripr/reports/test-oracle-assistant-proof.md</summary>'
                echo
                cat target/ripr/reports/test-oracle-assistant-proof.md
                echo
                echo '</details>'
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
                echo '<details><summary>Full report: target/ripr/reports/assistant-loop-health.md</summary>'
                echo
                cat target/ripr/reports/assistant-loop-health.md
                echo
                echo '</details>'
              fi
              echo
            fi
            echo '### Policy readiness'
            if [ -f target/ripr/reports/policy-readiness.json ]; then
              readiness_json=target/ripr/reports/policy-readiness.json
              readiness_status="$(jq -r '.status // "unknown"' "$readiness_json" 2>/dev/null || echo unknown)"
              readiness_mode="$(jq -r '.recommended_mode // "unknown"' "$readiness_json" 2>/dev/null || echo unknown)"
              blocking_status="$(jq -r '.blocking_readiness.state // "unknown"' "$readiness_json" 2>/dev/null || echo unknown)"
              baseline_status="$(jq -r '.baseline_health.state // "unknown"' "$readiness_json" 2>/dev/null || echo unknown)"
              waiver_status="$(jq -r '.waiver_health.state // "unknown"' "$readiness_json" 2>/dev/null || echo unknown)"
              suppression_status="$(jq -r '.suppression_health.state // "unknown"' "$readiness_json" 2>/dev/null || echo unknown)"
              calibration_status="$(jq -r '.calibration_health.state // "unknown"' "$readiness_json" 2>/dev/null || echo unknown)"
              preview_status="$(jq -r '.preview_evidence_boundary.state // "unknown"' "$readiness_json" 2>/dev/null || echo unknown)"
              readiness_warnings="$(jq -r '(.warnings // [] | length)' "$readiness_json" 2>/dev/null || echo 0)"
              readiness_unknowns="$(jq -r '(.unknowns // [] | length)' "$readiness_json" 2>/dev/null || echo 0)"
              next_policy_action="$(jq -r '.next_policy_action // "not_available"' "$readiness_json" 2>/dev/null || echo unknown)"
              readiness_status="$(markdown_inline "$readiness_status")"
              readiness_mode="$(markdown_inline "$readiness_mode")"
              blocking_status="$(markdown_inline "$blocking_status")"
              baseline_status="$(markdown_inline "$baseline_status")"
              waiver_status="$(markdown_inline "$waiver_status")"
              suppression_status="$(markdown_inline "$suppression_status")"
              calibration_status="$(markdown_inline "$calibration_status")"
              preview_status="$(markdown_inline "$preview_status")"
              readiness_warnings="$(markdown_inline "$readiness_warnings")"
              readiness_unknowns="$(markdown_inline "$readiness_unknowns")"
              next_policy_action="$(markdown_inline "$next_policy_action")"
              echo '#### Policy readiness at a glance'
              echo "- Status: \`$readiness_status\`"
              echo "- Recommended mode: \`$readiness_mode\`"
              echo "- Axes: blocking=\`$blocking_status\`, baseline=\`$baseline_status\`, waiver=\`$waiver_status\`, suppression=\`$suppression_status\`, calibration=\`$calibration_status\`, preview=\`$preview_status\`"
              echo "- Warnings: \`$readiness_warnings\`; unknowns: \`$readiness_unknowns\`"
              echo "- Next policy action: \`$next_policy_action\`"
              echo "- Policy readiness artifacts: \`target/ripr/reports/policy-readiness.json\`, \`target/ripr/reports/policy-readiness.md\`"
              echo "- Boundary: advisory readiness projection only; \`ripr gate evaluate\` remains pass/fail authority when configured."
              echo
            fi
            if [ -f target/ripr/reports/policy-readiness.md ]; then
              echo '<details><summary>Full report: target/ripr/reports/policy-readiness.md</summary>'
              echo
              cat target/ripr/reports/policy-readiness.md
              echo
              echo '</details>'
            else
              echo 'Policy readiness was not generated. It is advisory and requires existing policy artifacts to be useful.'
            fi
            echo
            echo '### Policy operations'
            if [ -f target/ripr/reports/policy-operations.json ]; then
              operations_json=target/ripr/reports/policy-operations.json
              operations_ceiling="$(jq -r '.current_policy_ceiling // "unknown"' "$operations_json" 2>/dev/null || echo unknown)"
              operations_next="$(jq -r '.recommended_next_action // "not_available"' "$operations_json" 2>/dev/null || echo unknown)"
              operations_safe="$(jq -r '(.safe_to_promote_to // [] | length)' "$operations_json" 2>/dev/null || echo 0)"
              operations_blocked="$(jq -r '(.not_safe_to_promote_to // [] | length)' "$operations_json" 2>/dev/null || echo 0)"
              operations_blockers="$(jq -r '(.promotion_blockers // [] | length)' "$operations_json" 2>/dev/null || echo 0)"
              operations_top_blocker="$(jq -r '([.promotion_blockers[]?.repair_action] | first) // "none"' "$operations_json" 2>/dev/null || echo unknown)"
              operations_warnings="$(jq -r '(.warnings // [] | length)' "$operations_json" 2>/dev/null || echo 0)"
              operations_unknowns="$(jq -r '(.unknowns // [] | length)' "$operations_json" 2>/dev/null || echo 0)"
              operations_ceiling="$(markdown_inline "$operations_ceiling")"
              operations_next="$(markdown_inline "$operations_next")"
              operations_safe="$(markdown_inline "$operations_safe")"
              operations_blocked="$(markdown_inline "$operations_blocked")"
              operations_blockers="$(markdown_inline "$operations_blockers")"
              operations_top_blocker="$(markdown_inline "$operations_top_blocker")"
              operations_warnings="$(markdown_inline "$operations_warnings")"
              operations_unknowns="$(markdown_inline "$operations_unknowns")"
              echo '#### Policy operations at a glance'
              echo "- Current ceiling: \`$operations_ceiling\`"
              echo "- Next safe action: \`$operations_next\`"
              echo "- Promotion modes: allowed=\`$operations_safe\`, blocked=\`$operations_blocked\`"
              echo "- Blockers: total=\`$operations_blockers\`, first=\`$operations_top_blocker\`"
              echo "- Warnings: \`$operations_warnings\`; unknowns: \`$operations_unknowns\`"
              echo "- Policy operations artifacts: \`target/ripr/reports/policy-operations.json\`, \`target/ripr/reports/policy-operations.md\`"
              echo "- Boundary: advisory operations packet only; promotion requires manual review and separate configuration changes."
              echo
            fi
            if [ -f target/ripr/reports/policy-operations.md ]; then
              echo '<details><summary>Full report: target/ripr/reports/policy-operations.md</summary>'
              echo
              cat target/ripr/reports/policy-operations.md
              echo
              echo '</details>'
            else
              echo 'Policy operations was not generated. It requires policy-readiness and keeps promotion advisory until packet review.'
            fi
            echo
            echo '### Policy history'
            if [ -f target/ripr/reports/policy-history.json ]; then
              history_json=target/ripr/reports/policy-history.json
              history_ceiling="$(jq -r '.current.current_policy_ceiling // "unknown"' "$history_json" 2>/dev/null || echo unknown)"
              history_entries="$(jq -r '.history_summary.entries // 0' "$history_json" 2>/dev/null || echo 0)"
              history_readiness="$(jq -r '.trend.ceiling.direction // "unknown"' "$history_json" 2>/dev/null || echo unknown)"
              history_waiver="$(jq -r '.trend.waiver_count.direction // "unknown"' "$history_json" 2>/dev/null || echo unknown)"
              history_suppression="$(jq -r '.trend.stale_suppression_count.direction // "unknown"' "$history_json" 2>/dev/null || echo unknown)"
              history_baseline_present="$(jq -r '.trend.baseline_still_present.direction // "unknown"' "$history_json" 2>/dev/null || echo unknown)"
              history_baseline_resolved="$(jq -r '.trend.baseline_resolved.direction // "unknown"' "$history_json" 2>/dev/null || echo unknown)"
              history_preview="$(jq -r '.trend.preview_boundary_state.direction // "unknown"' "$history_json" 2>/dev/null || echo unknown)"
              history_warnings="$(jq -r '(.warnings // [] | length)' "$history_json" 2>/dev/null || echo 0)"
              history_unknowns="$(jq -r '(.unknowns // [] | length)' "$history_json" 2>/dev/null || echo 0)"
              history_ceiling="$(markdown_inline "$history_ceiling")"
              history_entries="$(markdown_inline "$history_entries")"
              history_readiness="$(markdown_inline "$history_readiness")"
              history_waiver="$(markdown_inline "$history_waiver")"
              history_suppression="$(markdown_inline "$history_suppression")"
              history_baseline_present="$(markdown_inline "$history_baseline_present")"
              history_baseline_resolved="$(markdown_inline "$history_baseline_resolved")"
              history_preview="$(markdown_inline "$history_preview")"
              history_warnings="$(markdown_inline "$history_warnings")"
              history_unknowns="$(markdown_inline "$history_unknowns")"
              echo '#### Policy history at a glance'
              echo "- Current ceiling: \`$history_ceiling\`; history entries: \`$history_entries\`"
              echo "- Trends: readiness=\`$history_readiness\`, waiver_pressure=\`$history_waiver\`, suppression_health=\`$history_suppression\`, baseline_still_present=\`$history_baseline_present\`, baseline_resolved=\`$history_baseline_resolved\`, preview_boundary=\`$history_preview\`"
              echo "- Warnings: \`$history_warnings\`; unknowns: \`$history_unknowns\`"
              echo "- Policy history artifacts: \`target/ripr/reports/policy-history.json\`, \`target/ripr/reports/policy-history.md\`"
              echo "- Boundary: history is read-only and never appends to \`.ripr/policy-history.jsonl\` automatically."
              echo
            fi
            if [ -f target/ripr/reports/policy-history.md ]; then
              echo '<details><summary>Full report: target/ripr/reports/policy-history.md</summary>'
              echo
              cat target/ripr/reports/policy-history.md
              echo
              echo '</details>'
            else
              echo 'Policy history was not generated. It requires policy-operations and never writes history automatically.'
            fi
            echo
            echo '### Policy promotion packets'
            promotion_found=false
            for promotion_json in \
              target/ripr/reports/policy-promotion-visible-only.json \
              target/ripr/reports/policy-promotion-acknowledgeable.json \
              target/ripr/reports/policy-promotion-baseline-check.json \
              target/ripr/reports/policy-promotion-calibrated-gate.json; do
              if [ -f "$promotion_json" ]; then
                promotion_found=true
                promotion_target="$(jq -r '.target_mode // "unknown"' "$promotion_json" 2>/dev/null || echo unknown)"
                promotion_allowed="$(jq -r '.allowed_now // false' "$promotion_json" 2>/dev/null || echo false)"
                promotion_repairs="$(jq -r '(.required_repairs // [] | length)' "$promotion_json" 2>/dev/null || echo 0)"
                promotion_receipts="$(jq -r '(.required_receipts // [] | length)' "$promotion_json" 2>/dev/null || echo 0)"
                promotion_warnings="$(jq -r '(.warnings // [] | length)' "$promotion_json" 2>/dev/null || echo 0)"
                promotion_unknowns="$(jq -r '(.unknowns // [] | length)' "$promotion_json" 2>/dev/null || echo 0)"
                promotion_reason="$(jq -r '.why_or_why_not // "not_available"' "$promotion_json" 2>/dev/null || echo unknown)"
                promotion_target="$(markdown_inline "$promotion_target")"
                promotion_allowed="$(markdown_inline "$promotion_allowed")"
                promotion_repairs="$(markdown_inline "$promotion_repairs")"
                promotion_receipts="$(markdown_inline "$promotion_receipts")"
                promotion_warnings="$(markdown_inline "$promotion_warnings")"
                promotion_unknowns="$(markdown_inline "$promotion_unknowns")"
                promotion_reason="$(markdown_inline "$promotion_reason")"
                echo "- \`$promotion_target\`: allowed_now=\`$promotion_allowed\`, repairs=\`$promotion_repairs\`, receipts=\`$promotion_receipts\`, warnings=\`$promotion_warnings\`, unknowns=\`$promotion_unknowns\`, why=\`$promotion_reason\`"
              fi
            done
            if [ "$promotion_found" = false ]; then
              echo 'Policy promotion packets were not generated. They require policy-operations and remain read-only manual review packets.'
            else
              echo "- Promotion packet artifacts: \`target/ripr/reports/policy-promotion-*.json\`, \`target/ripr/reports/policy-promotion-*.md\`"
              echo "- Boundary: packets do not edit \`ripr.toml\`, baselines, suppressions, workflows, branch protection, CI defaults, or preview eligibility."
            fi
            for promotion_md in \
              target/ripr/reports/policy-promotion-visible-only.md \
              target/ripr/reports/policy-promotion-acknowledgeable.md \
              target/ripr/reports/policy-promotion-baseline-check.md \
              target/ripr/reports/policy-promotion-calibrated-gate.md; do
              if [ -f "$promotion_md" ]; then
                echo
                echo "<details><summary>Full report: $promotion_md</summary>"
                echo
                cat "$promotion_md"
                echo
                echo '</details>'
              fi
            done
            echo
            echo '### Preview promotion packets'
            preview_found=false
            for preview_json in target/ripr/reports/preview-promotion-*-*.json; do
              if [ -f "$preview_json" ]; then
                preview_found=true
                preview_language="$(jq -r '.language // "unknown"' "$preview_json" 2>/dev/null || echo unknown)"
                preview_class="$(jq -r '.candidate_class // "unknown"' "$preview_json" 2>/dev/null || echo unknown)"
                preview_allowed="$(jq -r '.allowed_now // false' "$preview_json" 2>/dev/null || echo false)"
                preview_missing="$(jq -r '(.missing_evidence // [] | length)' "$preview_json" 2>/dev/null || echo 0)"
                preview_supplied="$(jq -r '(.supplied_evidence // [] | length)' "$preview_json" 2>/dev/null || echo 0)"
                preview_warnings="$(jq -r '(.warnings // [] | length)' "$preview_json" 2>/dev/null || echo 0)"
                preview_unknowns="$(jq -r '(.unknowns // [] | length)' "$preview_json" 2>/dev/null || echo 0)"
                preview_language="$(markdown_inline "$preview_language")"
                preview_class="$(markdown_inline "$preview_class")"
                preview_allowed="$(markdown_inline "$preview_allowed")"
                preview_missing="$(markdown_inline "$preview_missing")"
                preview_supplied="$(markdown_inline "$preview_supplied")"
                preview_warnings="$(markdown_inline "$preview_warnings")"
                preview_unknowns="$(markdown_inline "$preview_unknowns")"
                echo "- \`$preview_language\`/\`$preview_class\`: allowed_now=\`$preview_allowed\`, supplied_evidence=\`$preview_supplied\`, missing_evidence=\`$preview_missing\`, warnings=\`$preview_warnings\`, unknowns=\`$preview_unknowns\`"
              fi
            done
            if [ "$preview_found" = false ]; then
              echo 'Preview promotion packets were not generated. They are only surfaced when TypeScript or Python preview adapters are configured.'
            else
              echo "- Preview promotion artifacts: \`target/ripr/reports/preview-promotion-*.json\`, \`target/ripr/reports/preview-promotion-*.md\`"
              echo "- Boundary: preview evidence remains visible and non-gating unless a later explicit promotion policy is reviewed."
            fi
            for preview_md in target/ripr/reports/preview-promotion-*-*.md; do
              if [ -f "$preview_md" ]; then
                echo
                echo "<details><summary>Full report: $preview_md</summary>"
                echo
                cat "$preview_md"
                echo
                echo '</details>'
              fi
            done
            echo
            echo '### Waiver aging'
            if [ -f target/ripr/reports/waiver-aging.json ]; then
              waiver_json=target/ripr/reports/waiver-aging.json
              waiver_status="$(jq -r '.status // "unknown"' "$waiver_json" 2>/dev/null || echo unknown)"
              waiver_count="$(jq -r '.summary.waiver_count // 0' "$waiver_json" 2>/dev/null || echo 0)"
              waiver_identities="$(jq -r '.summary.identity_count // 0' "$waiver_json" 2>/dev/null || echo 0)"
              waiver_repeated_seams="$(jq -r '.summary.repeated_seam_count // 0' "$waiver_json" 2>/dev/null || echo 0)"
              waiver_repeated_files="$(jq -r '.summary.repeated_file_count // 0' "$waiver_json" 2>/dev/null || echo 0)"
              waiver_focused_candidates="$(jq -r '.summary.focused_test_candidates // 0' "$waiver_json" 2>/dev/null || echo 0)"
              waiver_suppression_candidates="$(jq -r '.summary.durable_suppression_candidates // 0' "$waiver_json" 2>/dev/null || echo 0)"
              waiver_warnings="$(jq -r '.summary.warnings // 0' "$waiver_json" 2>/dev/null || echo 0)"
              waiver_status="$(markdown_inline "$waiver_status")"
              waiver_count="$(markdown_inline "$waiver_count")"
              waiver_identities="$(markdown_inline "$waiver_identities")"
              waiver_repeated_seams="$(markdown_inline "$waiver_repeated_seams")"
              waiver_repeated_files="$(markdown_inline "$waiver_repeated_files")"
              waiver_focused_candidates="$(markdown_inline "$waiver_focused_candidates")"
              waiver_suppression_candidates="$(markdown_inline "$waiver_suppression_candidates")"
              waiver_warnings="$(markdown_inline "$waiver_warnings")"
              echo '#### Waiver aging at a glance'
              echo "- Status: \`$waiver_status\`"
              echo "- Counts: waivers=\`$waiver_count\`, identities=\`$waiver_identities\`, repeated_seams=\`$waiver_repeated_seams\`, repeated_files=\`$waiver_repeated_files\`"
              echo "- Review signals: focused_test_candidates=\`$waiver_focused_candidates\`, durable_suppression_candidates=\`$waiver_suppression_candidates\`, warnings=\`$waiver_warnings\`"
              echo "- Waiver-aging artifacts: \`target/ripr/reports/waiver-aging.json\`, \`target/ripr/reports/waiver-aging.md\`"
              echo "- Boundary: repeated waiver is a visible signal, not a failure or durable suppression."
              echo
            fi
            if [ -f target/ripr/reports/waiver-aging.md ]; then
              echo '<details><summary>Full report: target/ripr/reports/waiver-aging.md</summary>'
              echo
              cat target/ripr/reports/waiver-aging.md
              echo
              echo '</details>'
            elif [ -f target/ripr/reports/pr-evidence-ledger.json ]; then
              echo 'Waiver aging was not generated. Inspect `target/ripr/reports/pr-evidence-ledger.json` and rerun `ripr policy waiver-aging` locally.'
            else
              echo 'Waiver aging was not run. It requires a PR evidence ledger.'
            fi
            echo
            echo '### Suppression health'
            if [ -f target/ripr/reports/suppression-health.json ]; then
              suppression_json=target/ripr/reports/suppression-health.json
              suppression_status="$(jq -r '.status // "unknown"' "$suppression_json" 2>/dev/null || echo unknown)"
              suppression_total="$(jq -r '.summary.suppressions // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_healthy="$(jq -r '.summary.healthy // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_missing_owner="$(jq -r '.summary.missing_owner // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_missing_reason="$(jq -r '.summary.missing_reason // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_stale="$(jq -r '.summary.stale // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_overbroad="$(jq -r '.summary.overbroad_scope // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_unknown_selector="$(jq -r '.summary.unknown_selector // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_preview_gap="$(jq -r '.summary.preview_without_preview_label // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_warnings="$(jq -r '.summary.warnings // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_config_errors="$(jq -r '.summary.config_errors // 0' "$suppression_json" 2>/dev/null || echo 0)"
              suppression_status="$(markdown_inline "$suppression_status")"
              suppression_total="$(markdown_inline "$suppression_total")"
              suppression_healthy="$(markdown_inline "$suppression_healthy")"
              suppression_missing_owner="$(markdown_inline "$suppression_missing_owner")"
              suppression_missing_reason="$(markdown_inline "$suppression_missing_reason")"
              suppression_stale="$(markdown_inline "$suppression_stale")"
              suppression_overbroad="$(markdown_inline "$suppression_overbroad")"
              suppression_unknown_selector="$(markdown_inline "$suppression_unknown_selector")"
              suppression_preview_gap="$(markdown_inline "$suppression_preview_gap")"
              suppression_warnings="$(markdown_inline "$suppression_warnings")"
              suppression_config_errors="$(markdown_inline "$suppression_config_errors")"
              echo '#### Suppression health at a glance'
              echo "- Status: \`$suppression_status\`"
              echo "- Counts: suppressions=\`$suppression_total\`, healthy=\`$suppression_healthy\`, missing_owner=\`$suppression_missing_owner\`, missing_reason=\`$suppression_missing_reason\`"
              echo "- Review signals: stale=\`$suppression_stale\`, overbroad_scope=\`$suppression_overbroad\`, unknown_selector=\`$suppression_unknown_selector\`, preview_without_preview_label=\`$suppression_preview_gap\`"
              echo "- Warnings: \`$suppression_warnings\`; config_errors: \`$suppression_config_errors\`"
              echo "- Suppression-health artifacts: \`target/ripr/reports/suppression-health.json\`, \`target/ripr/reports/suppression-health.md\`"
              echo "- Boundary: suppressions remain visible durable exceptions; this report never applies or gates on suppressions."
              echo
            fi
            if [ -f target/ripr/reports/suppression-health.md ]; then
              echo '<details><summary>Full report: target/ripr/reports/suppression-health.md</summary>'
              echo
              cat target/ripr/reports/suppression-health.md
              echo
              echo '</details>'
            else
              echo 'Suppression health was not generated. It is advisory and reads the durable suppression manifest when present.'
            fi
            echo
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
              blocking_reason="$(jq -r '([.decisions[]? | select(.decision == "blocking") | .gate_reason]) as $reasons | if ($reasons | length) == 0 then "none" elif ($reasons | length) == 1 then $reasons[0] else "\($reasons[0]) (+\(($reasons | length) - 1) more, see gate-decision.md)" end' "$gate_json" 2>/dev/null || echo unknown)"
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
              echo "- Blocking reason (\`$blocking\`): \`$blocking_reason\`"
              echo "- Gate artifacts: \`target/ripr/reports/gate-decision.json\`, \`target/ripr/reports/gate-decision.md\`"
              echo "- Related inputs: \`target/ripr/review/comments.json\`, \`target/ci/labels.json\`"
              echo
            fi
            if [ -f target/ripr/reports/gate-decision.md ]; then
              echo '<details><summary>Full report: target/ripr/reports/gate-decision.md</summary>'
              echo
              cat target/ripr/reports/gate-decision.md
              echo
              echo '</details>'
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
              legacy_fallback_match="$(jq -r '.delta.legacy_fallback_match // 0' "$delta_json" 2>/dev/null || echo 0)"
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
              legacy_fallback_match="$(markdown_inline "$legacy_fallback_match")"
              limits_note="$(markdown_inline "$limits_note")"
              echo '#### Baseline debt movement'
              echo "- Baseline: \`$baseline_path\`"
              echo "- Counts: still_present=\`$still_present\`, resolved=\`$resolved\`, new_policy_eligible=\`$new_policy_eligible\`, acknowledged=\`$acknowledged_delta\`, suppressed=\`$suppressed_delta\`, stale=\`$stale_baseline_entry\`, invalid=\`$invalid_baseline_entry\`, missing_current_input=\`$missing_current_input\`, legacy_fallback=\`$legacy_fallback_match\`"
              echo "- Boundary: $limits_note"
              echo "- Baseline delta artifacts: \`target/ripr/reports/baseline-debt-delta.json\`, \`target/ripr/reports/baseline-debt-delta.md\`"
              echo
            fi
            if [ -f target/ripr/reports/baseline-debt-delta.md ]; then
              echo '<details><summary>Full report: target/ripr/reports/baseline-debt-delta.md</summary>'
              echo
              cat target/ripr/reports/baseline-debt-delta.md
              echo
              echo '</details>'
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
              echo '<details><summary>Full report: target/ripr/reports/ripr-zero-status.md</summary>'
              echo
              cat target/ripr/reports/ripr-zero-status.md
              echo
              echo '</details>'
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
                echo '<details><summary>Full report: target/ripr/review/comment-publish-plan.md</summary>'
                echo
                cat target/ripr/review/comment-publish-plan.md
                echo
                echo '</details>'
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

"#;
