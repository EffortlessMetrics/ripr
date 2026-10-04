use super::server_asset_name;
use std::fs;
use std::path::Path;

const TARGETS: [(&str, &str); 5] = [
    ("x86_64-unknown-linux-gnu", "tar.gz"),
    ("aarch64-unknown-linux-gnu", "tar.gz"),
    ("x86_64-apple-darwin", "tar.gz"),
    ("aarch64-apple-darwin", "tar.gz"),
    ("x86_64-pc-windows-msvc", "zip"),
];

fn ripr_manifest() -> toml::Table {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/ripr/Cargo.toml");
    let text = fs::read_to_string(&path).unwrap_or_default();
    assert!(!text.is_empty(), "read {}", path.display());
    toml::from_str::<toml::Table>(&text).unwrap_or_default()
}

fn render(template: &str, version: &str, target: &str) -> String {
    template
        .replace("{ version }", version)
        .replace("{ target }", target)
}

#[test]
fn binstall_urls_name_the_assets_the_release_workflow_publishes() {
    let manifest = ripr_manifest();
    let binstall = &manifest["package"]["metadata"]["binstall"];
    let default_url = binstall["pkg-url"].as_str().unwrap_or_default();
    assert!(!default_url.is_empty(), "binstall pkg-url must be present");
    for (target, archive) in TARGETS {
        let overridden = binstall
            .get("overrides")
            .and_then(|overrides| overrides.get(target))
            .and_then(|row| row.get("pkg-url"))
            .and_then(toml::Value::as_str);
        let template = overridden.unwrap_or(default_url);
        let expected = format!(
            "https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0/{}",
            server_asset_name("0.11.0", target, archive)
        );
        assert_eq!(render(template, "0.11.0", target), expected, "{target}");
    }
}

#[test]
fn binstall_formats_match_archive_kinds() {
    let manifest = ripr_manifest();
    let binstall = &manifest["package"]["metadata"]["binstall"];
    assert_eq!(binstall["pkg-fmt"].as_str(), Some("tgz"));
    assert_eq!(
        binstall["overrides"]["x86_64-pc-windows-msvc"]["pkg-fmt"].as_str(),
        Some("zip")
    );
    assert_eq!(binstall["bin-dir"].as_str(), Some("{ bin }{ binary-ext }"));
    // Without this, binstall tries the third-party QuickInstall service before
    // compiling, contradicting the documented release-archive-or-source path.
    let disabled: Vec<&str> = binstall["disabled-strategies"]
        .as_array()
        .map(|items| items.iter().filter_map(|item| item.as_str()).collect())
        .unwrap_or_default();
    assert_eq!(disabled, ["quick-install"]);
}
