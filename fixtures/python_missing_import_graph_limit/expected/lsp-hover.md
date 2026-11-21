# RIPR Preview Finding Hover

## Preview Boundary

Language: python
Status: preview
Evidence: syntax-first
Static limit: missing_import_graph
Action: advisory only

## RIPR Evidence

- reach yes: 1 related Python test found for owner `total`
- infection unknown: Python preview adapter does not model infection
- propagation unknown: Python preview adapter does not model propagation
- observation yes: strongest extracted Python oracle kind is `exact_value`
- discriminator yes: related Python test uses a strong exact-value oracle

## Static Limit

RIPR saw a changed line that calls an imported symbol.

RIPR could not establish imported implementation semantics because Python
preview mode does not build an import graph.

## Suggested Action

Verify that the exact-value assertion targets the changed boundary value.
Copying a context packet or refreshing analysis is advisory only.
