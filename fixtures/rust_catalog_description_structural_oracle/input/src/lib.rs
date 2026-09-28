pub struct CommandEntry {
    pub name: &'static str,
    pub description: String,
}

pub struct Catalog {
    pub entries: Vec<CommandEntry>,
}

pub fn default_check_description() -> String {
    "verifies catalog consistency".to_string()
}

pub fn build_catalog() -> Catalog {
    Catalog {
        entries: vec![CommandEntry {
            name: "check",
            description: default_check_description(),
        }],
    }
}

// Structure-only consistency: duplicate/empty checks. Never inspects the
// wording, so a wording-only change passes it — even though the rendered
// description flows into the asserted catalog.
pub fn entry_description_violations(catalog: &Catalog) -> Vec<String> {
    let mut violations = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in &catalog.entries {
        if entry.name.is_empty() {
            violations.push("empty name".to_string());
        }
        if !seen.insert(entry.name) {
            violations.push(format!("duplicate {}", entry.name));
        }
        if entry.description.is_empty() {
            violations.push(format!("empty description for {}", entry.name));
        }
    }
    violations
}
