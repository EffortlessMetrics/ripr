//! Zed extension that starts `ripr lsp --stdio` from the user's PATH.
//!
//! Zed only launches language servers that Zed or an extension registers, so
//! `ripr lsp` needs this registration to run in Zed at all (#4460). The
//! extension never downloads or bundles ripr: it finds the binary the user
//! installed, and it passes the user's `lsp.ripr` settings through unchanged.
//!
//! When `lsp.ripr.binary.path` is set, Zed starts that binary itself and never
//! calls [`RiprExtension::language_server_command`].

use std::collections::HashMap;

use zed_extension_api::{
    self as zed, LanguageServerId, Result, serde_json::Value, settings::LspSettings,
};

/// Key of the `lsp` settings entry and of `[language_servers.*]` in
/// `extension.toml`.
const SERVER_ID: &str = "ripr";

/// Binary name looked up on the worktree's PATH.
const BINARY_NAME: &str = "ripr";

/// `ripr lsp` speaks LSP over stdio; Zed has no other transport for extensions.
const DEFAULT_ARGS: [&str; 2] = ["lsp", "--stdio"];

/// The `workspace/configuration` section ripr requests. Zed answers a
/// section request with that key of the object the extension returns.
const CONFIGURATION_SECTION: &str = "ripr";

/// Shown in Zed's language server status when ripr is not on PATH.
const MISSING_BINARY: &str = "ripr is not on PATH. Install it with \
    `cargo install ripr --locked` or a release binary, restart Zed, or set \
    lsp.ripr.binary.path together with lsp.ripr.binary.arguments \
    [\"lsp\", \"--stdio\"]. See editors/zed/README.md in the ripr repository.";

struct RiprExtension;

impl zed::Extension for RiprExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree(SERVER_ID, worktree)?.binary;
        let (arguments, extra_env) = match binary {
            Some(binary) => (binary.arguments, binary.env),
            None => (None, None),
        };
        server_command(
            worktree.which(BINARY_NAME),
            arguments,
            worktree.shell_env(),
            extra_env,
        )
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<Value>> {
        Ok(LspSettings::for_worktree(SERVER_ID, worktree)?.initialization_options)
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<Value>> {
        Ok(workspace_configuration(
            LspSettings::for_worktree(SERVER_ID, worktree)?.settings,
        ))
    }
}

/// Builds the launch command from what Zed reports about the worktree.
///
/// Configured arguments replace the defaults; configured environment entries
/// override the shell environment key by key, as Zed does for a configured
/// binary path.
fn server_command(
    found: Option<String>,
    arguments: Option<Vec<String>>,
    shell_env: Vec<(String, String)>,
    extra_env: Option<HashMap<String, String>>,
) -> Result<zed::Command> {
    let command = found.ok_or_else(|| MISSING_BINARY.to_string())?;
    let args = arguments.unwrap_or_else(|| DEFAULT_ARGS.map(String::from).to_vec());
    let mut env = shell_env;
    for (key, value) in extra_env.unwrap_or_default() {
        match env.iter_mut().find(|(existing, _)| *existing == key) {
            Some(entry) => entry.1 = value,
            None => env.push((key, value)),
        }
    }
    Ok(zed::Command { command, args, env })
}

/// Places the user's `lsp.ripr.settings` under the `ripr` section, so the keys
/// documented for other clients (`diagnosticProfile`, `baseRef`, ...) are
/// written flat in Zed's settings.
fn workspace_configuration(settings: Option<Value>) -> Option<Value> {
    let settings = settings?;
    let mut configuration = zed::serde_json::Map::new();
    configuration.insert(CONFIGURATION_SECTION.to_string(), settings);
    Some(Value::Object(configuration))
}

zed::register_extension!(RiprExtension);

#[cfg(test)]
mod tests {
    use super::*;
    use zed_extension_api::serde_json::json;

    fn pair(key: &str, value: &str) -> (String, String) {
        (key.to_string(), value.to_string())
    }

    #[test]
    fn starts_the_path_binary_with_stdio_lsp_arguments() -> Result<()> {
        let command = server_command(
            Some("/opt/cargo/bin/ripr".to_string()),
            None,
            vec![pair("PATH", "/opt/cargo/bin:/usr/bin")],
            None,
        )?;
        assert_eq!(command.command, "/opt/cargo/bin/ripr");
        assert_eq!(command.args, vec!["lsp".to_string(), "--stdio".to_string()]);
        assert_eq!(command.env, vec![pair("PATH", "/opt/cargo/bin:/usr/bin")]);
        Ok(())
    }

    #[test]
    fn missing_binary_names_the_install_and_override_routes() {
        // An `Ok` leaves the message empty and fails every assertion below.
        let error = server_command(None, None, Vec::new(), None)
            .err()
            .unwrap_or_default();
        assert!(error.contains("ripr is not on PATH"), "{error}");
        assert!(error.contains("cargo install ripr --locked"), "{error}");
        assert!(error.contains("lsp.ripr.binary.path"), "{error}");
    }

    #[test]
    fn configured_arguments_replace_the_defaults() -> Result<()> {
        let command = server_command(
            Some("ripr".to_string()),
            Some(vec!["lsp".to_string()]),
            Vec::new(),
            None,
        )?;
        assert_eq!(command.args, vec!["lsp".to_string()]);
        Ok(())
    }

    #[test]
    fn configured_env_overrides_and_extends_the_shell_env() -> Result<()> {
        let command = server_command(
            Some("ripr".to_string()),
            None,
            vec![pair("PATH", "/usr/bin"), pair("HOME", "/var/empty")],
            Some(HashMap::from([
                ("PATH".to_string(), "/opt/ripr/bin".to_string()),
                ("RUST_LOG".to_string(), "debug".to_string()),
            ])),
        )?;
        let mut env = command.env;
        env.sort();
        assert_eq!(
            env,
            vec![
                pair("HOME", "/var/empty"),
                pair("PATH", "/opt/ripr/bin"),
                pair("RUST_LOG", "debug"),
            ]
        );
        Ok(())
    }

    #[test]
    fn settings_answer_the_ripr_configuration_section() {
        let configuration = workspace_configuration(Some(json!({ "diagnosticProfile": "full" })));
        assert_eq!(
            configuration,
            Some(json!({ "ripr": { "diagnosticProfile": "full" } }))
        );
    }

    #[test]
    fn no_settings_leaves_the_configuration_unset() {
        assert_eq!(workspace_configuration(None), None);
    }
}
