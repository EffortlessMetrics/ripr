# Fixture: ts_repair_packet_wrong_family_oracle

Spec: RIPR-SPEC-0087

## Given

A TypeScript function `parseLimit` gains a new guard that throws
`TypeError('invalid limit')`. The only related test imports and calls the
owner directly, with an exact-value assertion on the success path:
`expect(parseLimit('10')).toBe(10)`.

That assertion is strong for return values but cannot observe the new throw:
it is a wrong-family oracle for the error-path probe (RIPR-SPEC-0104
`ts_oracle_kind_matches_seam`).

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_wrong_family_oracle/input \
  --diff fixtures/ts_repair_packet_wrong_family_oracle/diff.patch \
  --mode fast
```

## Then

- The error-path finding stays `weakly_exposed` with the missing
  discriminator `throws TypeError matching 'invalid limit'`.
- No `typescript_oracle_observed` / `typescript_oracle_expected` metadata is
  borrowed from the exact-value assertion for that finding, so the
  RIPR-SPEC-0087 G-C precondition fails and `repair_packet_ready` stays
  `false`.

## Must Not

- Emit `repair_packet_ready: true` for the error-path finding.
- Name `expect(parseLimit('10')).toBe(10)` as the oracle target or repair
  action for the error-path finding. Before this fixture, the packet flipped
  ready and told the user to "add or strengthen" that existing assertion,
  which cannot detect the throw.
