use crate::app::CheckOutput;

pub(super) fn render_header(output: &CheckOutput) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "ripr static RIPR exposure analysis\nmode: {}\nroot: {}\n\n",
        output.mode.as_str(),
        output.root.display()
    ));
    out.push_str(&format!(
        "Summary: {} probe(s), {} exposed, {} weak, {} unrevealed, {} no path, {} unknown\n\n",
        output.summary.probes,
        output.summary.exposed,
        output.summary.weakly_exposed,
        output.summary.reachable_unrevealed,
        output.summary.no_static_path,
        output.summary.static_unknown
            + output.summary.infection_unknown
            + output.summary.propagation_unknown
    ));
    out
}
