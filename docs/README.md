# ripr documentation

Start with the [Quickstart](QUICKSTART.md) and choose the CLI, editor, CI, or
agent path that matches how you work.

## Use RIPR

| Goal | Start here |
| --- | --- |
| Install the CLI and inspect a change | [CLI quickstart](QUICKSTART.md#cli-first-hour) |
| Use RIPR in VS Code | [Editor quickstart](QUICKSTART.md#vs-code-first-hour) · [Editor extension](EDITOR_EXTENSION.md) |
| Add advisory GitHub CI | [CI quickstart](QUICKSTART.md#ci-first-hour) |
| Work with a coding agent | [Agent quickstart](QUICKSTART.md#agent-or-reviewer-first-hour) · [LLM operator guide](LLM_OPERATOR_GUIDE.md) |
| Investigate a finding and choose a focused test | [Finding triage](how-to/triage-a-finding.md) · [Targeted test workflow](TARGETED_TEST_WORKFLOW.md) |
| Continue a prepared repair attempt | [Repair attempt identity and recovery](REPAIR_ATTEMPT.md) |
| Prepare evidence for one PR | [First successful PR workflow](FIRST_PR_WORKFLOW.md) |
| Use the editor repair workflow | [LSP repair workflow](LSP_AGENT_REPAIR_WORKFLOW.md) |

## Understand the results

| Question | Reference |
| --- | --- |
| What does RIPR infer, and how does that differ from coverage or mutation testing? | [Static exposure model](STATIC_EXPOSURE_MODEL.md) |
| What do gap, discriminator, oracle, and seam mean? | [Terminology](TERMINOLOGY.md) |
| What is usable, preview, or advisory? | [Support tiers](status/SUPPORT_TIERS.md) |
| What could the analysis not determine? | [Static limits](STATIC_LIMITS.md) |
| How do I enable and interpret preview languages? | [Language adapter preview](LANGUAGE_ADAPTER_PREVIEW.md) |
| What do verification records and badges establish? | [Verification contract](VERIFICATION.md) · [Badge policy](BADGE_POLICY.md) |

## Command and integration reference

| Need | Reference |
| --- | --- |
| Choose the command for a task | [Public command hierarchy](COMMAND_HIERARCHY.md) |
| Configure a repository | [Configuration](CONFIGURATION.md) |
| Interpret command success and failure | [Exit codes](EXIT_CODES.md) |
| Consume machine-readable output | [Output schema](OUTPUT_SCHEMA.md) |
| Use the broader agent workflows | [Agent workflows](AGENT_WORKFLOWS.md) |
| Connect a standard LSP client | [Neovim standard-LSP recipe](interop/neovim-lsp.md) |
| Integrate sibling tools | [Interop and learning ledger](interop/sibling-tools.md) |

## Contribute and maintain

- [Contributing](../CONTRIBUTING.md) — prepare and validate a scoped PR.
- [Architecture](ARCHITECTURE.md) — understand the implementation boundaries.
- [Specs](specs/README.md) and [ADRs](adr/README.md) — find behavioral contracts
  and architecture decisions.
- [Agent context](agent-context/) — repository map, review invariants, and
  validation guidance.
- [Source-of-truth doctrine](source-of-truth/README.md) and
  [spec/proposal system](SPEC_PROPOSAL_SYSTEM.md) — connect proposals, specs,
  plans, proof, and closeouts.
- [Knowledge library](LIBRARY.md) — read curated learnings.
- [Documentation map](DOCUMENTATION.md) — browse the full documentation system.

Distribution preparation has separate source-owned guides:

- [npm bootstrap](NPM_BOOTSTRAP.md) — native Linux tarball qualification and
  independently authorized publication.
- [PyPI bootstrap](PYPI_BOOTSTRAP.md) — Linux wheel qualification and separately
  authorized first prerelease.
