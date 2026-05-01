import * as vscode from 'vscode';

export type TraceSetting = 'off' | 'messages' | 'verbose';

export interface RiprConfig {
  serverPath: string;
  serverArgs: string[];
  autoDownload: boolean;
  serverVersion: string;
  downloadBaseUrl: string;
  checkMode: 'instant' | 'fast' | 'deep';
  baseRef: string;
  traceServer: TraceSetting;
}

export function getConfig(): RiprConfig {
  const config = vscode.workspace.getConfiguration('ripr');
  return {
    serverPath: config.get<string>('server.path', ''),
    serverArgs: config.get<string[]>('server.args', ['lsp', '--stdio']),
    autoDownload: config.get<boolean>('server.autoDownload', true),
    serverVersion: config.get<string>('server.version', ''),
    downloadBaseUrl: config.get<string>('server.downloadBaseUrl', ''),
    checkMode: config.get<'instant' | 'fast' | 'deep'>('check.mode', 'instant'),
    baseRef: config.get<string>('baseRef', 'origin/main'),
    traceServer: config.get<TraceSetting>('trace.server', 'off')
  };
}
