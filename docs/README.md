# ripr documentation

Start with the [Quickstart](QUICKSTART.md). It covers the CLI, VS Code,
GitHub Actions, and coding-agent paths, including what a no-action or limited
result means.

## Choose a task

| Goal | Start here | What it covers |
| --- | --- | --- |
| Inspect one change | [CLI first hour](QUICKSTART.md#cli-first-hour) | Run `ripr check`, choose the diff, and read the bounded next action. |
| Understand a finding | [Static exposure model](STATIC_EXPOSURE_MODEL.md) · [Terminology](TERMINOLOGY.md) · [Finding triage](how-to/triage-a-finding.md) | How RIPR connects changed behavior, related tests, observations, and assertions — and where static analysis stops. |
| Add one focused test | [Targeted test workflow](TARGETED_TEST_WORKFLOW.md) | Turn one supported gap into a focused test and compare before/after evidence. |
| Try RIPR on one pull request | [First successful PR workflow](FIRST_PR_WORKFLOW.md) | Run one bounded adoption loop and retain reviewer-facing evidence. |
| Work in VS Code | [Editor extension](EDITOR_EXTENSION.md) · [First run to first receipt](EDITOR_FIRST_RUN_TO_FIRST_RECEIPT.md) | Install the extension, inspect saved-workspace diagnostics, and complete one repair receipt. |
| Add advisory CI or review a PR | [CI strategy](CI.md) · [PR review guidance](PR_REVIEW_GUIDANCE.md) | Generate advisory GitHub Actions, read summaries and artifacts, and keep gate authority explicit. |
| Hand work to a coding agent | [LLM operator guide](LLM_OPERATOR_GUIDE.md) · [Agent workflows](AGENT_WORKFLOWS.md) | Give an agent a bounded packet with evidence, edit limits, verification, and stop conditions. |
| Configure RIPR or consume its output | [Configuration](CONFIGURATION.md) · [Output schema](OUTPUT_SCHEMA.md) | Repository policy, CLI and editor settings, JSON contracts, and machine-readable states. |
| Check language and workflow maturity | [Support tiers](status/SUPPORT_TIERS.md) · [Language adapter preview](LANGUAGE_ADAPTER_PREVIEW.md) | What is usable, preview, advisory, unavailable, or explicitly limited. |
| Connect another client | [MCP workspace status](interop/mcp.md) · [Other LSP editors](interop/other-editors-lsp.md) · [Neovim LSP recipe](interop/neovim-lsp.md) | Read-only MCP status, Helix/Neovim/Zed setup, and a portable standard-LSP client path. |

## Product and engineering reference

- [Command hierarchy](COMMAND_HIERARCHY.md) — which public command owns each task.
- [Architecture](ARCHITECTURE.md) — system boundaries and major components.
- [Behavioral specifications](specs/README.md) — normative product contracts.
- [Architecture decisions](adr/README.md) — durable design choices and rationale.
- [Verification](VERIFICATION.md) — evidence, badges, and non-claim boundaries.
- [LSP repair workflow](LSP_AGENT_REPAIR_WORKFLOW.md) — deeper editor/server repair orchestration after the first-use path.
- [Capability matrix](CAPABILITY_MATRIX.md) and [metrics](METRICS.md) — detailed implementation and evidence state.
- [Sibling-tool contracts](interop/sibling-tools.md) — interoperability boundaries and shared learning.

## Contributing and repository operation

- [Contributing](../CONTRIBUTING.md) — development setup and validation.
- [Scoped PR contract](SCOPED_PR_CONTRACT.md) — keep one coherent change and evidence denominator.
- [PR automation](PR_AUTOMATION.md) — repository review and integration mechanics.
- Agent context: [repository map](agent-context/repo-map.md), [review invariants](agent-context/review-invariants.md), and [validation](agent-context/validation.md).
- [Source-of-truth doctrine](source-of-truth/README.md) — proposals, specs, ADRs, plans, policy, proof, and closeout.
- [Spec/proposal system](SPEC_PROPOSAL_SYSTEM.md) — how proposals become reviewed specifications and implementation work.
- [Documentation system](DOCUMENTATION.md) — document roles and maintenance rules.
- [Knowledge library](LIBRARY.md) — curated reusable learnings.
