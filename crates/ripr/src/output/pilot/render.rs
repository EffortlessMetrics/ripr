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

pub(super) fn why_line(entry: &crate::analysis::ClassifiedSeam) -> String {
    render_helpers::why_line(entry)
}
