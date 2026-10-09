# LLM Work Loop Fixture Matrix

These fixtures pin the artifact-only LLM work-loop states for the boundary-gap
scenario. They are checked projections over existing agent status, workflow,
receipt, and review-summary behavior; they are not a new executable fixture
runner surface.

Cases:

- `happy`: complete loop artifacts with improved static movement.
- `unchanged`: complete loop artifacts where static evidence did not move.
- `regressed`: complete loop artifacts where static evidence weakened.
- `missing-artifact`: missing work-loop artifacts and the first recovery
  command.
- `stale-artifact`: complete artifacts with stale-looking verify/receipt
  timestamps.
- `configured-off`: policy-hidden seam rejection text for agent handoff.
- `path-with-spaces`: missing-artifact recovery commands quote a spaced root.
- `windows-separators`: native Windows recovery commands normalize separators.
- `unix-literal-backslash`: Unix commands retain literal filename backslashes
  in roots and redirect targets; report-only root display keeps its presentation.

The Unix summary command test also captures actual shell argv and stdout
redirection from a foreign working directory into a real literal-backslash
directory, with a separate slash-path decoy and immutable marker. The fixtures
alone do not establish shell execution or filesystem identity.
