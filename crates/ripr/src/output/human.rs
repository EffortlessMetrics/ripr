use crate::app::CheckOutput;
use crate::domain::Finding;

pub fn render(output: &CheckOutput) -> String {
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
    let mut out = String::new();
    out.push_str(&format!(
        "{} {}:{}\n",
        finding.class.severity().to_ascii_uppercase(),
        finding.probe.location.file.display(),
        finding.probe.location.line
    ));
    out.push_str(&format!(
        "\nStatic exposure: {} ({}, {})\n",
        finding.class.as_str(),
        finding.probe.family.as_str(),
        finding.probe.delta.as_str()
    ));
    out.push_str("\nChanged behavior:\n");
    if let Some(before) = &finding.probe.before {
        out.push_str(&format!("  before: {before}\n"));
    }
    if let Some(after) = &finding.probe.after {
        out.push_str(&format!("  after:  {after}\n"));
    } else {
        out.push_str(&format!("  expr:   {}\n", finding.probe.expression));
    }

    out.push_str("\nRIPR:\n");
    out.push_str(&format!(
        "  Reach:       {} — {}\n",
        finding.ripr.reach.state.as_str(),
        finding.ripr.reach.summary
    ));
    out.push_str(&format!(
        "  Infect:      {} — {}\n",
        finding.ripr.infect.state.as_str(),
        finding.ripr.infect.summary
    ));
    out.push_str(&format!(
        "  Propagate:   {} — {}\n",
        finding.ripr.propagate.state.as_str(),
        finding.ripr.propagate.summary
    ));
    out.push_str(&format!(
        "  Observe:     {} — {}\n",
        finding.ripr.reveal.observe.state.as_str(),
        finding.ripr.reveal.observe.summary
    ));
    out.push_str(&format!(
        "  Discriminate:{} — {}\n",
        finding.ripr.reveal.discriminate.state.as_str(),
        finding.ripr.reveal.discriminate.summary
    ));

    if !finding.related_tests.is_empty() {
        out.push_str("\nRelated tests / oracles:\n");
        for test in finding.related_tests.iter().take(5) {
            out.push_str(&format!(
                "  - {}:{} {} [{}]",
                test.file.display(),
                test.line,
                test.name,
                test.oracle_strength.as_str()
            ));
            if let Some(oracle) = &test.oracle {
                out.push_str(&format!(" — {oracle}"));
            }
            out.push('\n');
        }
    }

    if !finding.missing.is_empty() {
        out.push_str("\nGap:\n");
        for missing in &finding.missing {
            out.push_str(&format!("  - {missing}\n"));
        }
    }

    if let Some(step) = &finding.recommended_next_step {
        out.push_str("\nRecommended next step:\n");
        out.push_str(&format!("  {step}\n"));
    }

    out
}
