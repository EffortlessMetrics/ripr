// Pull request diff capture, review guidance, and inline-comment planning.
pub(super) const WORKFLOW: &str = r####"      - name: Capture pull request diff
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

"####;
