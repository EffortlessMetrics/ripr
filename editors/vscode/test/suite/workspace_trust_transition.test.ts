import * as assert from 'assert';
import * as vscode from 'vscode';
import {
  RiprClientController,
  RiprClientLifecycleTimeoutError,
  RiprClientLifecycleWait,
  RiprClientRuntime
} from '../../src/client';
import { RiprConfig } from '../../src/config';
import {
  resetLifecycleCoordinatorForTests,
  startAfterWorkspaceTrust,
  startServerOnce
} from '../../src/extension';

interface Deferred {
  promise: Promise<void>;
  resolve: () => void;
  reject: (error: Error) => void;
}

function deferred(): Deferred {
  let resolve: (() => void) | undefined;
  let reject: ((error: Error) => void) | undefined;
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return {
    promise,
    resolve: () => resolve?.(),
    reject: (error: Error) => reject?.(error)
  };
}

function trustedConfig(): RiprConfig {
  return {
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
}

function trustedRuntime(
  createLanguageClient: RiprClientRuntime['createLanguageClient'],
  waitForLifecycle?: RiprClientLifecycleWait
): RiprClientRuntime {
  return {
    getConfig: trustedConfig,
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
    createLanguageClient,
    createFileSystemWatcher: () => ({} as unknown as vscode.FileSystemWatcher),
    readFile: async () => undefined,
    runRipr: async () => '',
    writeClipboard: async () => undefined,
    isWorkspaceTrusted: () => true,
    showInformationMessage: async () => undefined,
    showWarningMessage: async () => undefined,
    showErrorMessage: async () => undefined,
    waitForLifecycle
  };
}

function outputChannel(lines: string[]): vscode.LogOutputChannel {
  return {
    appendLine: (line: string) => {
      lines.push(line);
    }
  } as unknown as vscode.LogOutputChannel;
}

suite('Workspace Trust Transition', () => {
  setup(() => {
    resetLifecycleCoordinatorForTests();
  });

  test('concurrent trust-grant starts coalesce into one controller start', async () => {
    const startGate = deferred();
    const startEntered = deferred();
    let startCalls = 0;
    const controller = {
      start: async () => {
        startCalls += 1;
        startEntered.resolve();
        await startGate.promise;
      }
    } as Pick<RiprClientController, 'start'>;

    const first = startAfterWorkspaceTrust(controller);
    const second = startAfterWorkspaceTrust(controller);
    await startEntered.promise;

    assert.strictEqual(startCalls, 1);
    startGate.resolve();
    await Promise.all([first, second]);
  });

  test('startServerOnce coalesces concurrent starts and retries after failure', async () => {
    let startCalls = 0;
    const controller = {
      start: async () => {
        startCalls += 1;
        if (startCalls === 1) {
          throw new Error('sentinel start failure');
        }
      }
    } as Pick<RiprClientController, 'start'>;

    await assert.rejects(startServerOnce(controller), /sentinel start failure/);
    const first = startServerOnce(controller);
    const second = startServerOnce(controller);
    await Promise.all([first, second]);

    assert.strictEqual(startCalls, 2, 'failure resets the in-flight slot; concurrent retries coalesce');
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

suite('RiprClientController Lifecycle', () => {
  test('direct stop waits for a paused client start before calling client.stop', async () => {
    const startGate = deferred();
    const startEntered = deferred();
    let stopCalls = 0;
    const runtime = trustedRuntime(() => ({
      onNotification: () => ({ dispose: () => undefined }),
      sendRequest: async () => undefined,
      setTrace: () => undefined,
      start: async () => {
        startEntered.resolve();
        await startGate.promise;
      },
      stop: async () => {
        stopCalls += 1;
      }
    }));
    const controller = new RiprClientController(
      {} as unknown as vscode.ExtensionContext,
      outputChannel([]),
      runtime
    );

    const start = controller.start();
    await startEntered.promise;
    const stop = controller.stop();

    assert.strictEqual(stopCalls, 0, 'Starting client must not receive stop()');
    startGate.resolve();
    await Promise.all([start, stop]);

    assert.strictEqual(stopCalls, 1);
    assert.strictEqual(controller.isRunning(), false);
  });

  test('restart during paused startup performs one stop and one fresh start', async () => {
    const firstStartGate = deferred();
    const firstStartEntered = deferred();
    let createClientCalls = 0;
    let startCalls = 0;
    let stopCalls = 0;
    const runtime = trustedRuntime(() => {
      createClientCalls += 1;
      const firstClient = createClientCalls === 1;
      return {
        onNotification: () => ({ dispose: () => undefined }),
        sendRequest: async () => undefined,
        setTrace: () => undefined,
        start: async () => {
          startCalls += 1;
          if (firstClient) {
            firstStartEntered.resolve();
            await firstStartGate.promise;
          }
        },
        stop: async () => {
          stopCalls += 1;
        }
      };
    });
    const controller = new RiprClientController(
      {} as unknown as vscode.ExtensionContext,
      outputChannel([]),
      runtime
    );

    const initialStart = controller.start();
    await firstStartEntered.promise;
    const restart = controller.restart();

    assert.strictEqual(stopCalls, 0, 'restart must wait for the Starting client');
    firstStartGate.resolve();
    await Promise.all([initialStart, restart]);

    assert.strictEqual(createClientCalls, 2);
    assert.strictEqual(startCalls, 2);
    assert.strictEqual(stopCalls, 1);
    assert.strictEqual(controller.isRunning(), true);
    await controller.stop();
  });

  test('bounded startup-settle timeout retains ownership and permits a later stop retry', async () => {
    const startGate = deferred();
    const startEntered = deferred();
    let waitCalls = 0;
    let stopCalls = 0;
    const waitForLifecycle: RiprClientLifecycleWait = async (operation, budgetMs, description) => {
      waitCalls += 1;
      if (waitCalls === 1) {
        throw new RiprClientLifecycleTimeoutError(description, budgetMs);
      }
      await operation;
    };
    const runtime = trustedRuntime(() => ({
      onNotification: () => ({ dispose: () => undefined }),
      sendRequest: async () => undefined,
      setTrace: () => undefined,
      start: async () => {
        startEntered.resolve();
        await startGate.promise;
      },
      stop: async () => {
        stopCalls += 1;
      }
    }), waitForLifecycle);
    const controller = new RiprClientController(
      {} as unknown as vscode.ExtensionContext,
      outputChannel([]),
      runtime
    );

    const start = controller.start();
    await startEntered.promise;
    await assert.rejects(controller.stop(), /did not settle within 30000ms/);

    assert.strictEqual(stopCalls, 0);
    assert.strictEqual(controller.isRunning(), true, 'timeout retains the possibly-running client');

    startGate.resolve();
    await start;
    await controller.stop();

    assert.strictEqual(stopCalls, 1);
    assert.strictEqual(controller.isRunning(), false);
  });

  test('a rejected real controller start leaves no stale client and the retry re-initializes', async () => {
    let createClientCalls = 0;
    let clientStartCalls = 0;
    const outputLines: string[] = [];
    const runtime = trustedRuntime(() => {
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
    });
    const controller = new RiprClientController(
      {} as unknown as vscode.ExtensionContext,
      outputChannel(outputLines),
      runtime
    );

    await assert.rejects(controller.start(), /sentinel client start failure/);
    await controller.start();

    assert.strictEqual(createClientCalls, 2, 'retry must create a fresh client, not reuse stale state');
    assert.strictEqual(clientStartCalls, 2, 'retry must start the fresh client');
    await controller.stop();
  });
});
