import * as assert from 'assert';
import * as vscode from 'vscode';
import { RiprClientController, RiprClientRuntime } from '../../src/client';
import { RiprConfig } from '../../src/config';

const enabledConfig: RiprConfig = {
  enabled: true,
  serverPath: '/sentinel/untrusted/ripr',
  serverArgs: ['lsp', '--stdio'],
  autoDownload: true,
  serverVersion: 'sentinel-untrusted-version',
  downloadBaseUrl: 'https://sentinel.invalid/ripr',
  checkMode: 'draft',
  baseRef: 'origin/main',
  traceServer: 'off'
};

suite('Workspace Trust Runtime', () => {
  test('untrusted start performs zero resolver, process, watcher, or client work', async () => {
    let resolverCalls = 0;
    let processCalls = 0;
    let watcherCalls = 0;
    let clientCalls = 0;
    const outputLines: string[] = [];

    const runtime: RiprClientRuntime = {
      getConfig: () => enabledConfig,
      workspaceRootState: () => ({
        kind: 'singleRoot',
        root: '/workspace',
        roots: ['/workspace']
      }),
      workspaceFolders: () => [],
      showQuickPick: async () => undefined,
      resolveServer: async () => {
        resolverCalls += 1;
        throw new Error('untrusted workspace must not resolve or download a server');
      },
      createLanguageClient: () => {
        clientCalls += 1;
        return {
          onNotification: () => ({ dispose: () => undefined }),
          sendRequest: async () => undefined,
          setTrace: () => undefined,
          start: async () => undefined,
          stop: async () => undefined
        };
      },
      createFileSystemWatcher: () => {
        watcherCalls += 1;
        return {} as unknown as vscode.FileSystemWatcher;
      },
      readFile: async () => undefined,
      runRipr: async () => {
        processCalls += 1;
        throw new Error('untrusted workspace must not run ripr');
      },
      writeClipboard: async () => undefined,
      isWorkspaceTrusted: () => false,
      showInformationMessage: async () => undefined,
      showWarningMessage: async () => undefined,
      showErrorMessage: async () => undefined
    };

    const output = {
      appendLine: (line: string) => {
        outputLines.push(line);
      }
    } as unknown as vscode.LogOutputChannel;
    const controller = new RiprClientController(
      {} as unknown as vscode.ExtensionContext,
      output,
      runtime
    );

    await controller.start();

    assert.strictEqual(resolverCalls, 0, 'server resolution/download must remain unreachable');
    assert.strictEqual(processCalls, 0, 'ripr subprocess execution must remain unreachable');
    assert.strictEqual(watcherCalls, 0, 'trusted-session file watchers must not be created');
    assert.strictEqual(clientCalls, 0, 'language client creation must remain unreachable');
    assert.ok(
      outputLines.some((line) => line.includes('workspace is not trusted; refusing to start server or download')),
      outputLines.join('\n')
    );
  });
});
