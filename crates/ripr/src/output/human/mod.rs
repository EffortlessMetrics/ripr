mod finding;
mod header;

use crate::app::CheckOutput;
use crate::domain::Finding;

pub fn render(output: &CheckOutput) -> String {
    let mut out = String::new();
    out.push_str(&header::render_header(output));

    if output.findings.is_empty() {
        out.push_str("No diff-derived mutation exposure probes found.\n");
        return out;
    }

    for finding in &output.findings {
        out.push_str(&render_finding(finding));
        out.push('\n');
    }
    out
}

pub fn render_finding(finding: &Finding) -> String {
    finding::render_finding(finding)
}

#[cfg(test)]
mod tests;
