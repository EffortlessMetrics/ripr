# RIPR-SPEC-0003: Agent Context Packet

Status: planned

## Problem

A diagnostic is not enough for a coding agent. The agent needs a compact,
grounded test-writing brief that states what behavior changed, what evidence was
found, what discriminator is missing, and what test shape should be added.

## Behavior

`ripr context --json` should emit a packet optimized for writing one targeted
test.

The packet should include:

- task kind
- finding identity
- changed expression or behavior
- owner symbol
- recommended test location
- related existing tests
- fixture or builder hints
- missing input values
- missing oracle shape
- suggested assertion shapes
- confidence
- stop reasons

## Required Evidence

The context packet should be grounded in a selected finding and include enough
evidence for a human or agent to write one targeted test without broad repo
guesswork.

## Acceptance Examples

```json
{
  "task": "write_targeted_test",
  "finding": {
    "kind": "boundary_gap",
    "changed_expression": "amount >= discount_threshold"
  },
  "recommended_test_location": "tests/pricing.rs",
  "related_existing_tests": ["premium_customer_gets_discount"],
  "missing_input_values": ["discount_threshold"],
  "missing_oracle_shape": ["exact assertion on Quote.total"],
  "suggested_assertions": ["assert_eq!(quote.total, expected_total)"],
  "confidence": "medium",
  "stop_reasons": []
}
```

## Non-Goals

The context packet should not:

- generate complete tests by itself
- claim real mutation outcomes
- include broad unrelated repository context
- omit stop reasons when static evidence is incomplete

## Test Mapping

Planned tests:

- golden context packet for boundary gap
- golden context packet for weak error oracle
- golden context packet for unknown propagation
- LSP copy-context packet equality with CLI context packet

## Implementation Mapping

Planned modules:

- `domain` context data types
- `app` context use case
- `output::json`
- `lsp` code action bridge

## Metrics

- context packets with related tests
- context packets with missing input values
- context packets with suggested assertion shapes
- context packets with stop reasons when unknown
