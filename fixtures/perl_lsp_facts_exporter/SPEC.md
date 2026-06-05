# Fixture Corpus: perl_lsp_facts_exporter

Spec: RIPR-SPEC-0064

## Given

This fixture pins the first `perl-lsp` batch exporter handoff for the Perl
repair-routing lane.

It is exporter evidence only. RIPR consumes the expected
`ripr-perl-facts-v1` JSON packet without launching `perl-lsp`, running Perl,
installing modules, opening a live LSP session, or creating Perl repair
packets.

## When

The fixture-level Perl adapter consumes:

```text
fixtures/perl_lsp_facts_exporter/expected/ripr-perl-facts-v1.json
fixtures/perl_lsp_facts_exporter/expected/ripr-perl-source-test-oracle-facts-v1.json
```

The corpus validator reads:

```text
fixtures/perl_lsp_facts_exporter/corpus.json
```

## Then

The expected packet must keep:

- `producer.name = "perl-lsp"`;
- `schema_version = "ripr-perl-facts-v1"`;
- repo-relative paths with `/` separators;
- source, test, oracle, runner, limitation, and provenance facts as packet
  facts;
- strong exact Perl oracle shapes separate from `ok(...)`, mention-only,
  dies-only, unknown-helper, and dynamic-framework advisory evidence;
- verify commands as facts, not executed results;
- no RIPR-derived `canonical_gap_id` or `gap_state`.

## Must Not

This corpus must not imply a RIPR-owned Perl parser, live LSP dependency,
Perl runtime execution, package installation, test execution, repair card,
public badge contribution, default gate authority, RIPR Zero authority, or
stable Perl support-tier claim.
