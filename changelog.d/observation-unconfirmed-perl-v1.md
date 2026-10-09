<!-- section: Fixed -->
- A related test whose observation ripr could not confirm now reads "ripr
  could not confirm that this assertion observes the changed behavior"
  instead of claiming the assertion does not mention the change, and human,
  explain and LSP output label it `unconfirmed:` rather than `misses:` (#5508).
  Perl gap findings say this for a direct, strong exact test whose packet
  establishes no sink alignment; other Perl rows still carry no reason
  (#5498). No verdict changes.
