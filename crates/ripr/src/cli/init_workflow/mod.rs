// Keep the generated `ripr init --ci github` workflow assembled from
// responsibility-focused fragments so each CI concern can be reviewed without
// navigating a single thousand-line raw string.
mod agent_loop;
mod bootstrap;
mod evidence_reports;
mod pr_guidance;
mod publishing;

pub(super) fn github_actions_workflow() -> String {
    [
        bootstrap::WORKFLOW,
        agent_loop::WORKFLOW,
        pr_guidance::WORKFLOW,
        evidence_reports::WORKFLOW,
        publishing::WORKFLOW,
    ]
    .concat()
}
