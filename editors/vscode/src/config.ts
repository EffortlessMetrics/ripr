import * as vscode from 'vscode';

export type TraceSetting = 'off' | 'messages' | 'verbose';
export type DiagnosticProfile = 'actionable' | 'full';

export interface RiprConfig {
  enabled: boolean;
  serverPath: string;
  serverArgs: string[];
  autoDownload: boolean;
  serverVersion: string;
  downloadBaseUrl: string;
  checkMode: 'instant' | 'draft' | 'fast' | 'deep' | 'ready';
  baseRef: string;
  includeUnchangedTests: boolean;
  // Undefined unless the user set the VS Code setting, so `ripr.toml`
  // `[lsp]` values still apply. Forwarding the manifest default made the
  // server treat it as an explicit override (#4717).
  seamDiagnostics: boolean | undefined;
  diagnosticProfile: DiagnosticProfile | undefined;
  traceServer: TraceSetting;
}

export function getConfig(resource?: vscode.Uri): RiprConfig {
  // Resource-scoped settings must be read against the same workspace root
  // that the language server receives through workspace/configuration. A
  // resource-less lookup can fall back to the user layer when a restart is
  // initiated without an active editor, which makes profile transitions
  // disagree between the extension and server.
  const config = vscode.workspace.getConfiguration('ripr', resource);
  return {
    enabled: config.get<boolean>('enabled', true),
    serverPath: config.get<string>('server.path', ''),
    serverArgs: config.get<string[]>('server.args', ['lsp', '--stdio']),
    autoDownload: config.get<boolean>('server.autoDownload', true),
    serverVersion: config.get<string>('server.version', ''),
    downloadBaseUrl: config.get<string>('server.downloadBaseUrl', ''),
    checkMode: config.get<'instant' | 'draft' | 'fast' | 'deep' | 'ready'>('check.mode', 'draft'),
    baseRef: config.get<string>('baseRef', 'origin/main'),
    includeUnchangedTests: config.get<boolean>('includeUnchangedTests', true),
    seamDiagnostics: explicitSetting<boolean>(config, 'seamDiagnostics'),
    diagnosticProfile: explicitSetting<DiagnosticProfile>(config, 'diagnosticProfile'),
    traceServer: config.get<TraceSetting>('trace.server', 'off')
  };
}

/**
 * Returns a setting only when some settings layer sets it explicitly.
 * `WorkspaceConfiguration.get` falls back to the package.json default, which
 * the server cannot tell apart from a user choice.
 */
export function explicitSetting<T>(
  config: vscode.WorkspaceConfiguration,
  section: string
): T | undefined {
  const inspected = config.inspect<T>(section);
  if (!inspected) {
    return undefined;
  }
  return (
    inspected.workspaceFolderLanguageValue ??
    inspected.workspaceLanguageValue ??
    inspected.globalLanguageValue ??
    inspected.workspaceFolderValue ??
    inspected.workspaceValue ??
    inspected.globalValue
  );
}
