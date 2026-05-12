mod artifact_paths;
mod template;

pub(super) fn generated_github_actions_workflow() -> String {
    artifact_paths::apply_to(template::GITHUB_ACTIONS_WORKFLOW)
}
