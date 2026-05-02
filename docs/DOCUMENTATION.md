# Documentation System

`ripr` uses Diataxis so docs answer the reader's immediate problem instead of
mixing tutorials, references, and design arguments in one place.

## Tutorials

Tutorials help a new user succeed once.

Current and planned tutorial docs:

- README quick start
- future first-extension-install walkthrough
- future first-fixture walkthrough

## How-To Guides

How-to guides solve concrete tasks.

Current how-to docs:

- [Contributing](../CONTRIBUTING.md)
- [Testing](TESTING.md)
- [CI strategy](CI.md)
- [Fix CI shape failures](how-to/fix-ci-shape-failures.md)
- [Run Codex Goals](how-to/run-codex-goals.md)
- [PR automation](PR_AUTOMATION.md)
- [Dogfooding](DOGFOODING.md)
- [Agent workflows](AGENT_WORKFLOWS.md)
- [Release](RELEASE.md)
- [Publishing](PUBLISHING.md)
- [Editor extension](EDITOR_EXTENSION.md)
- [Server provisioning](SERVER_PROVISIONING.md)
- [Server binary release](RELEASE_BINARIES.md)
- [Marketplace release](RELEASE_MARKETPLACE.md)
- [Open VSX](OPENVSX.md)

## Reference

Reference docs define stable commands, schemas, config, and enum meanings.

Current reference docs:

- [Output schema](OUTPUT_SCHEMA.md)
- [Static exposure model](STATIC_EXPOSURE_MODEL.md)
- [Spec-test-code traceability](SPEC_TEST_CODE.md)
- [Spec format](SPEC_FORMAT.md)
- [Test taxonomy](TEST_TAXONOMY.md)
- [Engineering rules](ENGINEERING.md)
- [File policy](FILE_POLICY.md)
- [Changelog policy](CHANGELOG_POLICY.md)
- [Capability matrix](CAPABILITY_MATRIX.md)

Planned reference docs:

- `ripr.toml` reference
- SARIF output reference
- LSP diagnostic code reference

Templates:

- [ADR template](templates/ADR_TEMPLATE.md)
- [Spec template](templates/SPEC_TEMPLATE.md)

## Explanation

Explanation docs record why the product and architecture are shaped this way.

Current explanation docs:

- [Charter](CHARTER.md)
- [Architecture](ARCHITECTURE.md)
- [Roadmap](ROADMAP.md)
- [Implementation plan](IMPLEMENTATION_PLAN.md)
- [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md)
- [Codex Goals](CODEX_GOALS.md)
- [Scoped PR contract](SCOPED_PR_CONTRACT.md)
- [PR automation](PR_AUTOMATION.md)
- [Metrics](METRICS.md)
- [Capability matrix](CAPABILITY_MATRIX.md)
- [Learnings](LEARNINGS.md)
- [ADRs](adr/)
- [Specs](specs/)
- [Agent workflows](AGENT_WORKFLOWS.md)

## README Rule

The README is the front door. It should stay problem-first and include:

- what `ripr` is
- what question it answers
- where it fits against coverage and mutation testing
- quick start
- current capability state
- important metrics and engineering status
- links to the deeper docs

Avoid turning the README into the full roadmap or full schema reference.

## Index Check

Run:

```bash
cargo xtask check-doc-index
```

The check verifies that spec and ADR indexes list current files and that README
and this documentation map still point at the active planning, metrics, spec,
ADR, and PR automation docs.
