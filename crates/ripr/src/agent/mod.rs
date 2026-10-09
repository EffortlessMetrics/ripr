pub(crate) mod artifact;
pub(crate) mod command_specs;
pub(crate) mod loop_commands;
pub(crate) mod provenance;

/// Trust-bound repair continuations require a fresh, explicit authorization.
pub(crate) const PYTHON_REPAIR_AUTHORIZATION_SUFFIX: &str =
    " --edit-authorized --edit-authority <operator-or-agent-identity>";
