# RIPR Preview Finding Hover

## Preview Boundary

Language: typescript
Status: preview
Evidence: syntax-first
Static limit: mocked_module
Action: advisory only

## RIPR Evidence

- reach yes: 1 related TypeScript test found for owner `applyDiscount`
- infection unknown: TypeScript preview adapter does not model infection
- propagation unknown: TypeScript preview adapter does not model propagation
- observation yes: strongest extracted oracle kind is `exact_value`
- discriminator yes: related TypeScript test uses a strong exact-value oracle

## Static Limit

RIPR saw a related test file that mocks `./api`.

RIPR could not establish mocked module runtime semantics because TypeScript
preview mode does not resolve the substituted implementation.

## Suggested Action

Verify that the exact-value assertion targets the changed boundary value.
Copying a context packet or refreshing analysis is advisory only.
