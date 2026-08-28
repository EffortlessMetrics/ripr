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
  seamDiagnostics: boolean;
  diagnosticProfile: DiagnosticProfile;
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
    seamDiagnostics: config.get<boolean>('seamDiagnostics', true),
    diagnosticProfile: config.get<DiagnosticProfile>('diagnosticProfile', 'actionable'),
    traceServer: config.get<TraceSetting>('trace.server', 'off')
  };
}
