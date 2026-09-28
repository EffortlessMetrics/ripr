# Fixture: editor_lsp_workflow

Spec: RIPR-SPEC-0020

## Given

Production code has the same predicate-boundary gap as the boundary-gap fixture:
related tests reach and observe `discounted_total`, but they do not exercise the
equality-boundary discriminator.

## When

```bash
cargo xtask fixtures editor_lsp_workflow
cargo xtask lsp-cockpit-report
```

or:

```bash
ripr check --root fixtures/editor_lsp_workflow/input --diff fixtures/editor_lsp_workflow/diff.patch --mode fast
```

## Then

The fixture pins the saved-workspace editor loop as one contract:

```text
diagnostic -> hover -> code action -> status -> refresh guidance
```

Expected artifacts record the seam diagnostic, evidence-aware actions, hover
summary, VS Code status projection, and first-useful-action status behavior for
the same seam.

## Must Not

- Rerun hidden analysis from editor status.
- Add diagnostics from first-useful-action reports.
- Generate tests or edit source.
- Claim runtime adequacy or mutation execution.
