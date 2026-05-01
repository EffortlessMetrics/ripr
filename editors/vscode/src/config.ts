import * as vscode from 'vscode';

export type TraceSetting = 'off' | 'messages' | 'verbose';

export interface RiprConfig {
  serverPath: string;
  serverArgs: string[];
  checkMode: 'instant' | 'fast' | 'deep';
  baseRef: string;
  traceServer: TraceSetting;
}

export function getConfig(): RiprConfig {
  const config = vscode.workspace.getConfiguration('ripr');
  return {
    serverPath: config.get<string>('server.path', 'ripr'),
    serverArgs: config.get<string[]>('server.args', ['lsp']),
    checkMode: config.get<'instant' | 'fast' | 'deep'>('check.mode', 'instant'),
    baseRef: config.get<string>('baseRef', 'origin/main'),
    traceServer: config.get<TraceSetting>('trace.server', 'off')
  };
}
