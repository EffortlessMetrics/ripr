import * as assert from 'assert';
import * as vscode from 'vscode';
import { RiprClientController, RiprClientRuntime } from '../../src/client';
import { RiprConfig } from '../../src/config';
import { startAfterWorkspaceTrust } from '../../src/extension';

suite('Workspace Trust Transition', () => {
  test('concurrent trust-grant starts coalesce into one controller start', async () => {
    let releaseStart: (() => void) | undefined;
    const startGate = new Promise<void>((resolve) => {
      releaseStart = resolve;
    });
    let startCalls = 0;
    const controller = {
      start: async () => {
        startCalls += 1;
        await startGate;
      }
    } as Pick<RiprClientController, 'start'>;

    const first = startAfterWorkspaceTrust(controller);
    const second = startAfterWorkspaceTrust(controller);
    await Promise.resolve();

    assert.strictEqual(startCalls, 1);
    releaseStart?.();
    await Promise.all([first, second]);
  });

  test('a failed trust-grant start can be retried', async () => {
    let startCalls = 0;
    const controller = {
      start: async () => {
        startCalls += 1;
        if (startCalls === 1) {
          throw new Error('sentinel start failure');
        }
      }
    } as Pick<RiprClientController, 'start'>;

    await assert.rejects(
      startAfterWorkspaceTrust(controller),
      /sentinel start failure/
    );
    await startAfterWorkspaceTrust(controller);

    assert.strictEqual(startCalls, 2);
  });
});

suite('Workspace Trust Start Failure Recovery', () => {
  test('a rejected real controller start leaves no stale client and the retry re-initializes', async () => {
    let createClientCalls = 0;
    let clientStartCalls = 0;
    const outputLines: string[] = [];
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
    const runtime: RiprClientRuntime = {
      getConfig: () => enabledConfig,
      workspaceRootState: () => ({
        kind: 'singleRoot',
        root: '/workspace',
        roots: ['/workspace']
      }),
      resolveServer: async () => ({
        command: '/sentinel/trusted/ripr',
        source: 'configured',
        detail: 'sentinel server'
      }),
      createLanguageClient: () => {
        createClientCalls += 1;
        const shouldFail = createClientCalls === 1;
        return {
          onNotification: () => ({ dispose: () => undefined }),
          sendRequest: async () => undefined,
          setTrace: () => undefined,
          start: async () => {
            clientStartCalls += 1;
            if (shouldFail) {
              throw new Error('sentinel client start failure');
            }
          },
          stop: async () => undefined
        };
      },
      createFileSystemWatcher: () => ({} as unknown as vscode.FileSystemWatcher),
      readFile: async () => undefined,
      runRipr: async () => '',
      writeClipboard: async () => undefined,
      isWorkspaceTrusted: () => true,
      showInformationMessage: async () => undefined,
      showWarningMessage: async () => undefined,
      showErrorMessage: async () => undefined
    };
    const output = {
      appendLine: (line: string) => {
        outputLines.push(line);
      }
    } as unknown as vscode.OutputChannel;
    const controller = new RiprClientController(
      {} as unknown as vscode.ExtensionContext,
      output,
      runtime
    );

    await assert.rejects(controller.start(), /sentinel client start failure/);
    await controller.start();

    assert.strictEqual(createClientCalls, 2, 'retry must create a fresh client, not reuse stale state');
    assert.strictEqual(clientStartCalls, 2, 'retry must start the fresh client');
  });
});
