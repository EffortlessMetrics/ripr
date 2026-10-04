# Integrations

Which tools ripr should work with, what exists today, and copy-paste setups for
the gaps. Snippets here use only commands whose behavior was run locally
(`ripr check`, `--format sarif`, exit codes); none was run on the named hosted
service. Each says so.

## Shortlist and why

Chosen from the tools a Rust team already runs on a pull request, plus the
surfaces where a developer would feel ripr in the inner loop.

| Integration | Why | Today |
| --- | --- | --- |
| GitHub Actions and code scanning | Where most Rust PRs are reviewed | `ripr init --ci github`, SARIF upload ([CI quickstart](QUICKSTART.md#ci-first-hour)) |
| VS Code, Zed, Neovim and other LSP editors | Inner loop; diagnostics on save | [Editor quickstart](QUICKSTART.md#vs-code-first-hour), [Neovim](interop/neovim-lsp.md), [others](interop/other-editors-lsp.md) |
| Coding agents (MCP) | Agents write the tests ripr points at | [MCP status server](interop/mcp.md) |
| pre-commit framework | Catches the gap before the push | Snippet below; no hook ships for users |
| GitLab CI and other hosted CI | Not everyone is on GitHub | Snippet below; no generator |
| cargo-nextest | The standard Rust test runner | Ordering guidance below; ripr never runs tests |
| cargo-mutants | The runtime authority ripr is the cheap pre-filter for | Positioning below; no automated handoff |
| Dev containers and Docker | Reproducible onboarding | Snippet below |
| Sibling tools and `ub-review` | The org's own PR gate | [Sibling tools](interop/sibling-tools.md) |

Not pursued now: coverage services (Codecov, Coveralls) answer a different
question; SonarQube and similar platforms have no static-exposure concept to
map to.

## pre-commit framework

`.pre-commit-config.yaml`, using a local hook so nothing is downloaded beyond
ripr itself. `ripr check` exits 0 with or without findings and exits 2 only
when analysis could not run, so this hook is advisory unless the analysis fails.

```yaml
repos:
  - repo: local
    hooks:
      - id: ripr
        name: ripr (static test-gap check)
        entry: ripr check --worktree --base HEAD
        language: system
        types: [rust]
        pass_filenames: false
        verbose: true   # show findings even when the hook passes
```

`types: [rust]` runs the hook on commits that change Rust files only. Add
`python` or `javascript` and `ts` types if you enable those preview adapters.
`--worktree` needs a 0.11 build. On 0.10 use `ripr check` after committing.
Run locally against an uncommitted edit: exit 0, "no changed line is behavior
ripr checks" for a comment-only change. Not run through the pre-commit tool.

## GitLab CI and other hosted CI

Install the prebuilt archive, verify its checksum, run, keep the report.
Replace `0.11.0` with the release you pin. The archive name is the one the
release workflow publishes; 0.11.0 assets must exist first (see
[Install channels](INSTALL_CHANNELS.md)).

```yaml
ripr:
  image: rust:1
  variables:
    GIT_DEPTH: "0"          # ripr needs the base commit; a shallow clone fails with exit 2
    RIPR_VERSION: "0.11.0"
  script:
    - base="https://github.com/EffortlessMetrics/ripr/releases/download/v${RIPR_VERSION}"
    - file="ripr-server-v${RIPR_VERSION}-x86_64-unknown-linux-gnu.tar.gz"
    - curl -fsSLO "$base/$file" && curl -fsSLO "$base/$file.sha256"
    - echo "$(cat $file.sha256)  $file" | sha256sum -c -
    - tar xzf "$file"          # members are ./ripr, so extract everything
    - ./ripr check --base "origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME" --format sarif > ripr.sarif
    - ./ripr check --base "origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME"
  artifacts:
    paths: [ripr.sarif]
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  allow_failure: true
```

The example covers merge requests from branches in the same project; a fork's pipeline runs in the fork, so fetch the target project's branch (`git fetch <target-url> <branch>`) and pass that ref to `--base`. The same four steps (fetch full history, install, verify, `ripr check --base`)
apply to Buildkite, CircleCI and Jenkins. Not run on a GitLab runner. The
shallow-clone note comes from ripr's own exit-2 message, which names the fix.

## cargo-nextest

ripr reads source; it does not execute tests, so it does not depend on nextest.
Order the two so the cheap one speaks first:

```bash
ripr check --base origin/main     # seconds, static
cargo nextest run                 # then the real run
```

ripr's "related tests" are found by name and call graph, not by nextest
filter sets. A finding that recommends a test is a prompt to write one; it is
not a claim that the nextest run would pass or fail.

## cargo-mutants

ripr answers "does a test appear to contain a discriminator for this change"
without running anything. cargo-mutants answers it by running mutants. Use ripr
to see where a change has no discriminator, add the test, then let cargo-mutants
confirm on the files you changed:

```bash
ripr check --base origin/main
git diff origin/main > pr.diff
cargo mutants --in-diff pr.diff
```

There is no automated bridge today: ripr does not read cargo-mutants results
and does not claim any mutant outcome. `--in-diff` is cargo-mutants' own flag.

## Dev containers

```json
{
  "image": "mcr.microsoft.com/devcontainers/rust:1",
  "postCreateCommand": "cargo install cargo-binstall && cargo binstall -y ripr"
}
```

From 0.11.0 on, binstall downloads the prebuilt archive; before that it falls
back to compiling. Not run in a dev container.
