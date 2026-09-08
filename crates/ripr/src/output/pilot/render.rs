mod complete;
mod render_helpers;
mod timeout;

pub(crate) use complete::{
    render_pilot_summary_json, render_pilot_summary_md, render_pilot_terminal,
};
pub(crate) use timeout::{
    render_pilot_timeout_summary_json, render_pilot_timeout_summary_md,
    render_pilot_timeout_terminal,
};

/// Shell disclosure shown before generated command blocks in pilot markdown.
///
/// The wording lives beside the `powershell_command` translation it describes
/// (`output::markdown::COMMAND_SHELL_DISCLOSURE`) so every generated-command
/// surface shares one disclosure; the pilot re-exports it for its render
/// modules (#2628).
pub(super) use crate::output::markdown::COMMAND_SHELL_DISCLOSURE;

pub(super) fn why_line(entry: &crate::analysis::ClassifiedSeam) -> String {
    render_helpers::why_line(entry)
}
