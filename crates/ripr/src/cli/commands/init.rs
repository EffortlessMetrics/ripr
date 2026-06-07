use crate::agent::loop_commands;
use crate::cli::commands_options::{InitCi, InitOptions};
use crate::cli::help;
use crate::cli::parse::expect_value;
use crate::config::{CONFIG_FILE_NAME, generated_init_config};
use std::path::{Path, PathBuf};

pub(in crate::cli) fn init(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_init_help();
        return Ok(());
    }
    let options = parse_init_options(args)?;
    if options.dry_run {
        print_init_dry_run(&options);
        return Ok(());
    }
    if !options.root.is_dir() {
        return Err(format!(
            "init root {} is not a directory",
            options.root.display()
        ));
    }
    let config_path = options.root.join(CONFIG_FILE_NAME);
    let workflow_path = options
        .ci
        .as_ref()
        .map(|ci| init_ci_workflow_path(&options.root, ci));

    if config_path.exists() && !options.force && options.ci.is_none() {
        return Err(format!(
            "{} already exists; rerun `ripr init --force` to overwrite it",
            config_path.display()
        ));
    }
    if let Some(path) = workflow_path
        .as_ref()
        .filter(|path| path.exists())
        .filter(|_| !options.force)
    {
        return Err(format!(
            "{} already exists; rerun `ripr init --ci github --force` to overwrite it",
            path.display()
        ));
    }

    if config_path.exists() && !options.force {
        println!("Left existing {} unchanged", config_path.display());
    } else {
        std::fs::write(&config_path, generated_init_config())
            .map_err(|err| format!("write {} failed: {err}", config_path.display()))?;
        println!("Wrote {}", config_path.display());
    }

    if let Some(ci) = options.ci.as_ref() {
        write_init_ci_workflow(&options.root, ci)?;
    }
    Ok(())
}

pub(super) fn parse_init_options(args: &[String]) -> Result<InitOptions, String> {
    let mut options = InitOptions {
        root: PathBuf::from("."),
        dry_run: false,
        force: false,
        ci: None,
    };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                options.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--ci" => {
                i += 1;
                options.ci = Some(parse_init_ci(expect_value(args, i, "--ci")?)?);
            }
            "--dry-run" => options.dry_run = true,
            "--force" => options.force = true,
            other => return Err(format!("unknown init argument {other:?}")),
        }
        i += 1;
    }
    Ok(options)
}

fn parse_init_ci(value: &str) -> Result<InitCi, String> {
    match value {
        "github" => Ok(InitCi::Github),
        _ => Err(format!("unknown init --ci provider {value:?}")),
    }
}

fn print_init_dry_run(options: &InitOptions) {
    if let Some(ci) = options.ci.as_ref() {
        println!("# {}", CONFIG_FILE_NAME);
        print!("{}", generated_init_config());
        println!();
        println!("# {}", init_ci_workflow_path(&options.root, ci).display());
        print!("{}", generated_github_actions_workflow());
    } else {
        print!("{}", generated_init_config());
    }
}

fn init_ci_workflow_path(root: &Path, ci: &InitCi) -> PathBuf {
    match ci {
        InitCi::Github => root.join(".github/workflows/ripr.yml"),
    }
}

fn write_init_ci_workflow(root: &Path, ci: &InitCi) -> Result<(), String> {
    match ci {
        InitCi::Github => {
            let path = init_ci_workflow_path(root, ci);
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
            }
            std::fs::write(&path, generated_github_actions_workflow())
                .map_err(|err| format!("write {} failed: {err}", path.display()))?;
            println!("Wrote {}", path.display());
            Ok(())
        }
    }
}

pub(super) fn generated_github_actions_workflow() -> String {
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

      - name: Render RIPR diff SARIF
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
          history_args=(
            policy history
            --root .
            --current target/ripr/reports/policy-operations.json
            --commit "$GITHUB_SHA"
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
          configured_languages="$(
            ripr doctor --root . 2>/dev/null \
              | sed -n 's/^- Enabled languages: //p' \
              | tail -n 1 \
              || true
          )"
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
          if [ -z "$preview_languages" ]; then
            echo 'No TypeScript or Python preview languages are configured; preview promotion packets were not generated.'
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
        run: |
          mkdir -p target/ripr/reports
          ripr first-pr \
            --root . \
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

      - name: Add RIPR advisory summary
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
            echo '### Start here'
            echo '- Open `target/ripr/reports/start-here.md` first when it exists.'
            echo '- Then open `target/ripr/reports/index.md` to navigate deeper evidence artifacts.'
            echo '- Safe next action: repair one named gap, regenerate missing or malformed artifacts, refresh stale evidence, fix wrong-root setup, or stop on no-action.'
            echo '- Recovery states: missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, and preview-limited evidence are explicit stop or regeneration states.'
            echo '- Proof rail: verify command, receipt command, and receipt path are static movement evidence only.'
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
              start_next="$(jq -r '.selected.next_command // .selected.regeneration_command // "none"' "$start_json" 2>/dev/null || echo unknown)"
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
              echo "- Verify command: \`$start_verify\`"
              echo "- Receipt command: \`$start_receipt\`"
              echo "- Receipt path: \`$start_receipt_path\`"
              echo "- Receipt state: \`$start_receipt_state\`"
              echo "- Safe next action command: \`$start_next\`"
              echo "- Warnings: \`$start_warnings\`"
              echo "- Artifacts: \`target/ripr/reports/start-here.json\`, \`target/ripr/reports/start-here.md\`"
              echo "- Boundary: start-here is advisory first-run guidance only; gate decision remains separate pass/fail authority when configured."
              if [ -f target/ripr/reports/start-here.md ]; then
                echo
                cat target/ripr/reports/start-here.md
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
              first_packet="$(markdown_inline "$first_packet")"
              first_verify="$(markdown_inline "$first_verify")"
              first_receipt="$(markdown_inline "$first_receipt")"
              first_fallback="$(markdown_inline "$first_fallback")"
              first_warnings="$(markdown_inline "$first_warnings")"
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
              echo "- Verify command: \`$first_verify\`"
              echo "- Receipt command: \`$first_receipt\`"
              echo "- Fallback/no-action: \`$first_fallback\`"
              echo "- Warnings: \`$first_warnings\`"
              echo "- Artifacts: \`target/ripr/reports/first-useful-action.json\`, \`target/ripr/reports/first-useful-action.md\`, \`target/ripr/workflow/agent-packet.json\`"
              echo "- Boundary: advisory first-run path only; gate decision remains separate pass/fail authority when configured."
            else
              echo "- Status: \`missing_start_here\`"
              echo "- State: \`missing_artifact\`"
              echo "- Safe next action: run \`ripr first-pr --root . --gap-ledger target/ripr/reports/gap-decision-ledger.json --first-action target/ripr/reports/first-useful-action.json --review-comments target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-packet.json --gate-decision target/ripr/reports/gate-decision.json --receipts-dir target/ripr/receipts --out-dir target/ripr/reports\`."
              echo "- Fallback safe next action: run \`ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md\` after attaching at least one explicit input."
              echo "- Boundary: missing start-here packet does not fail generated CI or create gate authority."
            fi
            echo
            configured_languages="$(
              ripr doctor --root . 2>/dev/null \
                | sed -n 's/^- Enabled languages: //p' \
                | tail -n 1 \
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
                echo "- Safe next action: \`$action_kind\`"
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
              echo 'Safe next action: run `ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md` after attaching at least one explicit input.'
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
              cat target/ripr/reports/policy-readiness.md
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
              cat target/ripr/reports/policy-operations.md
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
              cat target/ripr/reports/policy-history.md
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
                cat "$promotion_md"
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
                cat "$preview_md"
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
              cat target/ripr/reports/waiver-aging.md
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
              cat target/ripr/reports/suppression-health.md
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
