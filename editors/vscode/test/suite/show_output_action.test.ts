import * as assert from 'assert';
import * as vscode from 'vscode';
import { RiprClientController, RiprClientRuntime } from '../../src/client';
import { RiprConfig } from '../../src/config';

const enabledConfig: RiprConfig = {
  enabled: true,
  serverPath: '/sentinel/trusted/ripr',
  serverArgs: ['lsp', '--stdio'],
  autoDownload: false,
  serverVersion: 'sentinel-trusted-version',
  downloadBaseUrl: 'https://sentinel.invalid/ripr',
  checkMode: 'draft',
  baseRef: 'origin/main',
  traceServer: 'off'
};

suite('Show Output Warning Action', () => {
  test('a failed copy warns with a Show Output button that reveals the channel', async () => {
    const warningCalls: Array<{ message: string; items: string[] }> = [];
    let outputShown = 0;
    const runtime: RiprClientRuntime = {
      getConfig: () => enabledConfig,
      workspaceRootState: () => ({
        kind: 'singleRoot',
        root: '/workspace',
        roots: ['/workspace']
      }),
      workspaceFolders: () => [],
      showQuickPick: async () => undefined,
      resolveServer: async () => ({
        command: '/sentinel/trusted/ripr',
        source: 'configured',
        detail: 'sentinel server'
      }),
      createLanguageClient: () => ({
        onNotification: () => ({ dispose: () => undefined }),
        sendRequest: async () => ({ kind: 'repair_packet', status: 'ready' }),
        setTrace: () => undefined,
        start: async () => undefined,
        stop: async () => undefined
      }),
      createFileSystemWatcher: () => ({} as unknown as vscode.FileSystemWatcher),
      readFile: async () => undefined,
      runRipr: async () => '',
      writeClipboard: async () => {
        throw new Error('sentinel clipboard failure');
      },
      isWorkspaceTrusted: () => true,
      showInformationMessage: async () => undefined,
      showWarningMessage: async (message: string, ...items: string[]) => {
        warningCalls.push({ message, items });
        return 'Show Output';
      },
      showErrorMessage: async () => undefined
    };
    const output = {
      appendLine: () => undefined,
      show: () => {
        outputShown += 1;
      }
    } as unknown as vscode.LogOutputChannel;
    const controller = new RiprClientController(
      {} as unknown as vscode.ExtensionContext,
      output,
      runtime
    );

    await controller.start();
    await controller.copyTopRepairPacket();
    // warnWithOutput resolves the action asynchronously.
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.strictEqual(warningCalls.length, 1, 'expected exactly one warning');
    assert.ok(
      warningCalls[0].items.includes('Show Output'),
      `warning must offer the Show Output action: ${JSON.stringify(warningCalls[0])}`
    );
    assert.strictEqual(outputShown, 1, 'choosing Show Output must reveal the output channel');
  });
});
