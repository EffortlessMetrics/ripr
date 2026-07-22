//
// Workspace-root picker runtime tests (#2077).
//
// The @vscode/test-electron harness runs a real VS Code instance, so
// `vscode.window.showQuickPick` cannot be stubbed globally. Instead the
// controller consumes the picker through the injected `RiprClientRuntime`
// (the same seam used by the workspace-trust runtime tests), and the
// pick-list construction is a pure exported function
// (`workspaceRootPickItems`) that is asserted directly.
//

import * as assert from 'assert';
import * as vscode from 'vscode';
import {
  RiprClientController,
  RiprClientRuntime,
  WorkspaceRootPickItem,
  workspaceRootPickItems
} from '../../src/client';
import { RiprConfig } from '../../src/config';

const enabledConfig: RiprConfig = {
  enabled: true,
  serverPath: '/sentinel/multiroot/ripr',
  serverArgs: ['lsp', '--stdio'],
  autoDownload: true,
  serverVersion: 'sentinel-multiroot-version',
  downloadBaseUrl: 'https://sentinel.invalid/ripr',
  checkMode: 'draft',
  baseRef: 'origin/main',
  traceServer: 'off'
};

function fakeFolder(name: string, fsPath: string, index: number): vscode.WorkspaceFolder {
  return { name, index, uri: { fsPath } as vscode.Uri };
}

interface FakeRuntimeHarness {
  runtime: RiprClientRuntime;
  outputLines: string[];
  warningMessages: string[];
  warningItems: string[][];
  quickPickItems: WorkspaceRootPickItem[][];
  serverCwds: string[];
  startedClients: number;
  resolverCalls: number;
  setRoots(roots: string[]): void;
  setFolders(folders: vscode.WorkspaceFolder[]): void;
  pickResponse?: WorkspaceRootPickItem;
  warningResponse?: string;
}

function makeHarness(roots: string[], folders: vscode.WorkspaceFolder[]): FakeRuntimeHarness {
  let currentRoots = roots;
  let currentFolders = folders;
  const harness: FakeRuntimeHarness = {
    runtime: undefined as unknown as RiprClientRuntime,
    outputLines: [],
    warningMessages: [],
    warningItems: [],
    quickPickItems: [],
    serverCwds: [],
    startedClients: 0,
    resolverCalls: 0,
    pickResponse: undefined,
    warningResponse: undefined,
    setRoots(next: string[]) {
      currentRoots = next;
    },
    setFolders(next: vscode.WorkspaceFolder[]) {
      currentFolders = next;
    }
  };
  harness.runtime = {
    getConfig: () => enabledConfig,
    workspaceRootState: () => ({
      kind: 'ambiguousMultiRoot',
      roots: currentRoots,
      detail: 'multiple workspace folders are open and no active editor selected a safe root'
    }),
    workspaceFolders: () => currentFolders,
    showQuickPick: async <T extends vscode.QuickPickItem>(items: T[]) => {
      harness.quickPickItems.push(items as unknown as WorkspaceRootPickItem[]);
      return harness.pickResponse as T | undefined;
    },
    resolveServer: async () => {
      harness.resolverCalls += 1;
      return {
        command: '/sentinel/multiroot/ripr',
        source: 'configured' as const,
        detail: 'sentinel multi-root server'
      };
    },
    createLanguageClient: (serverOptions) => {
      const cwd = (serverOptions as { options?: { cwd?: string } }).options?.cwd;
      harness.serverCwds.push(String(cwd));
      return {
        onNotification: () => ({ dispose: () => undefined }),
        sendRequest: async () => undefined,
        setTrace: () => undefined,
        start: async () => {
          harness.startedClients += 1;
        },
        stop: async () => undefined
      };
    },
    createFileSystemWatcher: () => ({} as unknown as vscode.FileSystemWatcher),
    readFile: async () => undefined,
    runRipr: async () => {
      throw new Error('multi-root picker tests must not run ripr');
    },
    writeClipboard: async () => undefined,
    isWorkspaceTrusted: () => true,
    showInformationMessage: async () => undefined,
    showWarningMessage: async (message: string, ...items: string[]) => {
      harness.warningMessages.push(message);
      harness.warningItems.push(items);
      return harness.warningResponse;
    },
    showErrorMessage: async () => undefined
  };
  return harness;
}

function makeController(harness: FakeRuntimeHarness): RiprClientController {
  const output = {
    appendLine: (line: string) => {
      harness.outputLines.push(line);
    }
  } as unknown as vscode.OutputChannel;
  return new RiprClientController(
    {} as unknown as vscode.ExtensionContext,
    output,
    harness.runtime
  );
}

const alpha = fakeFolder('alpha', '/ws/alpha', 0);
const beta = fakeFolder('beta', '/ws/beta', 1);

suite('Workspace Root Picker', () => {
  test('pick list shows every workspace folder with its name and path', () => {
    const items = workspaceRootPickItems([alpha, beta]);
    assert.deepStrictEqual(items.map((item) => item.label), ['alpha', 'beta']);
    assert.deepStrictEqual(items.map((item) => item.description), ['/ws/alpha', '/ws/beta']);
    assert.deepStrictEqual(items.map((item) => item.root), ['/ws/alpha', '/ws/beta']);
  });

  test('ripr.selectWorkspaceRoot offers every folder and starts the server for the picked root', async () => {
    const harness = makeHarness(['/ws/alpha', '/ws/beta'], [alpha, beta]);
    harness.pickResponse = workspaceRootPickItems([alpha, beta])[1];
    const controller = makeController(harness);

    await controller.selectWorkspaceRoot();

    assert.strictEqual(harness.quickPickItems.length, 1, 'the folder quick pick should be shown once');
    assert.deepStrictEqual(
      harness.quickPickItems[0].map((item) => item.root),
      ['/ws/alpha', '/ws/beta'],
      'the quick pick should offer every workspace folder'
    );
    assert.strictEqual(harness.resolverCalls, 1, 'picking a folder should start the server');
    assert.deepStrictEqual(harness.serverCwds, ['/ws/beta'], 'the server should start for the picked root');
    assert.strictEqual(harness.startedClients, 1);
  });

  test('ambiguous start warns with a picker action, and accepting it starts the server for the picked root', async () => {
    const harness = makeHarness(['/ws/alpha', '/ws/beta'], [alpha, beta]);
    harness.warningResponse = 'Select Workspace Root';
    harness.pickResponse = workspaceRootPickItems([alpha, beta])[0];
    const controller = makeController(harness);

    await controller.start();
    // The ambiguous-start offer is intentionally fire-and-forget (it must
    // not block start()); flush the microtask queue so the warning/picker
    // flow has completed before asserting.
    await new Promise((resolve) => setImmediate(resolve));

    assert.strictEqual(harness.warningMessages.length, 1, 'the ambiguous start should surface one warning');
    assert.deepStrictEqual(
      harness.warningItems[0],
      ['Select Workspace Root'],
      'the warning should offer the workspace-root picker'
    );
    assert.strictEqual(harness.quickPickItems.length, 1, 'accepting the warning should show the picker');
    assert.deepStrictEqual(harness.serverCwds, ['/ws/alpha'], 'the server should start for the picked root');
  });

  test('dismissing the ambiguous-start warning keeps the server stopped', async () => {
    const harness = makeHarness(['/ws/alpha', '/ws/beta'], [alpha, beta]);
    const controller = makeController(harness);

    await controller.start();
    // Same fire-and-forget flush as above: the warning lands async.
    await new Promise((resolve) => setImmediate(resolve));

    assert.strictEqual(harness.warningMessages.length, 1);
    assert.strictEqual(harness.quickPickItems.length, 0, 'a dismissed warning must not show the picker');
    assert.strictEqual(harness.resolverCalls, 0, 'a dismissed warning must not start the server');
    assert.ok(
      harness.outputLines.some((line) => line.includes('multi-root workspace is ambiguous')),
      harness.outputLines.join('\n')
    );
  });

  test('restart reuses the picked root for the session', async () => {
    const harness = makeHarness(['/ws/alpha', '/ws/beta'], [alpha, beta]);
    harness.pickResponse = workspaceRootPickItems([alpha, beta])[1];
    const controller = makeController(harness);
    await controller.selectWorkspaceRoot();
    assert.deepStrictEqual(harness.serverCwds, ['/ws/beta']);

    await controller.restart();

    assert.strictEqual(harness.quickPickItems.length, 1, 'restart must not re-show the picker');
    assert.deepStrictEqual(
      harness.serverCwds,
      ['/ws/beta', '/ws/beta'],
      'an explicit restart after picking should reuse the picked root'
    );
  });

  test('a picked root that left the workspace falls back to ambiguous', async () => {
    const harness = makeHarness(['/ws/alpha', '/ws/beta'], [alpha, beta]);
    harness.pickResponse = workspaceRootPickItems([alpha, beta])[1];
    const controller = makeController(harness);
    await controller.selectWorkspaceRoot();
    assert.deepStrictEqual(harness.serverCwds, ['/ws/beta']);

    const gamma = fakeFolder('gamma', '/ws/gamma', 1);
    harness.setRoots(['/ws/alpha', '/ws/gamma']);
    harness.setFolders([alpha, gamma]);
    await controller.restart();

    assert.strictEqual(harness.resolverCalls, 1, 'a stale pick must not start the server again');
    assert.strictEqual(harness.warningMessages.length, 1, 'a stale pick should fall back to the ambiguous warning');
  });
});
