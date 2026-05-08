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
- `windows-separators`: root display and recovery commands normalize
  separators.

