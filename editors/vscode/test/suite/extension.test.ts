import * as assert from 'assert';
import * as vscode from 'vscode';
import {
  RiprClientController,
  RiprClientRuntime
} from '../../src/client';

suite('Extension Smoke', () => {
  suiteSetup(async () => {
    await activateExtension();
  });

  test('extension is present', async () => {
    const ext = vscode.extensions.getExtension('EffortlessMetrics.ripr');
    assert.ok(ext, 'extension should be present');
  });

  test('extension activates in a Rust workspace', async () => {
    const ext = vscode.extensions.getExtension('EffortlessMetrics.ripr')!;
    await ext.activate();
    assert.strictEqual(ext.isActive, true);
  });

  test('commands are registered', async () => {
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes('ripr.restartServer'));
    assert.ok(commands.includes('ripr.showOutput'));
    assert.ok(commands.includes('ripr.copyContext'));
    assert.ok(commands.includes('ripr.copySuggestedAssertion'));
    assert.ok(commands.includes('ripr.copyTargetedTestBrief'));
    assert.ok(commands.includes('ripr.openRelatedTest'));
    assert.ok(commands.includes('ripr.openSettings'));
  });

  test('defaults-first check mode is draft', () => {
    const config = vscode.workspace.getConfiguration('ripr');
    assert.strictEqual(config.get('check.mode'), 'draft');
  });

  test('restartServer command is callable', async () => {
    // The command will fail because no ripr server is available in the
    // test environment, but it should not crash the extension.
    try {
      await vscode.commands.executeCommand('ripr.restartServer');
    } catch {
      // Expected: server resolution fails in test environment.
    }
  });

  test('copyContext with no active editor completes', async () => {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    // Should resolve without throwing even when no editor is open.
    await vscode.commands.executeCommand('ripr.copyContext');
  });

  test('copyContext accepts target with finding_id', async () => {
    const target = {
      uri: 'file:///workspace/src/lib.rs',
      line: 1,
      finding_id: 'probe:test:1:predicate',
      probe_id: 'probe:test:1:predicate',
    };
    // Should not throw when given a structured target.
    try {
      await vscode.commands.executeCommand('ripr.copyContext', target);
    } catch {
      // Expected: server resolution fails in test environment.
    }
  });

  test('copyContext with seam_id asks LSP before CLI fallback', async () => {
    const context = createControllerTestContext({ lspResult: { seam_packets: [{ seam_id: 'abc123' }] } });
    try {
      await context.controller.start();
      await context.controller.copyContext({
        uri: workspaceFileUri('src/lib.rs').toString(),
        line: 7,
        seam_id: 'abc123',
        seam_kind: 'predicate_boundary'
      });

      assert.strictEqual(context.client.requests.length, 1);
      assert.strictEqual(context.client.requests[0].method, 'workspace/executeCommand');
      assert.deepStrictEqual(context.client.requests[0].params, {
        command: 'ripr.collectContext',
        arguments: [{
          finding_id: undefined,
          probe_id: undefined,
          seam_id: 'abc123',
          seam_kind: 'predicate_boundary',
          uri: workspaceFileUri('src/lib.rs').toString(),
          line: 7,
        }]
      });
      assert.strictEqual(context.runRiprCalls.length, 0);
      assert.deepStrictEqual(JSON.parse(await vscode.env.clipboard.readText()), {
        seam_packets: [{ seam_id: 'abc123' }]
      });
    } finally {
      await context.dispose();
    }
  });

  test('copyContext falls back to CLI when seam LSP returns null', async () => {
    const context = createControllerTestContext({
      lspResult: null,
      cliResult: '{"fallback":true}\n'
    });
    try {
      await context.controller.start();
      await context.controller.copyContext({
        uri: workspaceFileUri('src/lib.rs').toString(),
        line: 9,
        seam_id: 'abc123'
      });

      assert.strictEqual(context.client.requests.length, 1);
      assert.strictEqual(context.runRiprCalls.length, 1);
      assert.deepStrictEqual(JSON.parse(await vscode.env.clipboard.readText()), {
        fallback: true
      });
    } finally {
      await context.dispose();
    }
  });

  test('copyContext falls back to CLI when seam LSP request fails', async () => {
    const context = createControllerTestContext({
      lspError: new Error('collectContext failed'),
      cliResult: '{"fallback":"after-error"}'
    });
    try {
      await context.controller.start();
      await context.controller.copyContext({
        uri: workspaceFileUri('src/lib.rs').toString(),
        line: 11,
        seam_id: 'abc123'
      });

      assert.strictEqual(context.client.requests.length, 1);
      assert.strictEqual(context.runRiprCalls.length, 1);
      assert.deepStrictEqual(JSON.parse(await vscode.env.clipboard.readText()), {
        fallback: 'after-error'
      });
    } finally {
      await context.dispose();
    }
  });

  test('copySuggestedAssertion copies assertion text', async () => {
    await vscode.commands.executeCommand('ripr.copySuggestedAssertion', {
      assertion: 'assert_eq!(quote.discount_applied, true);'
    });

    assert.strictEqual(
      await vscode.env.clipboard.readText(),
      'assert_eq!(quote.discount_applied, true);'
    );
  });

  test('copySuggestedAssertion ignores malformed args without throwing', async () => {
    await vscode.commands.executeCommand('ripr.copySuggestedAssertion', {
      assertion: ''
    });
    await vscode.commands.executeCommand('ripr.copySuggestedAssertion', {
      assertion: 42
    });
    await vscode.commands.executeCommand('ripr.copySuggestedAssertion');
  });

  test('copyTargetedTestBrief copies brief text', async () => {
    const brief = [
      'Target seam:',
      '- src/pricing.rs:88',
      '',
      'Add a targeted test:',
      '- Suggested name: discounted_total_boundary_discriminator'
    ].join('\n');

    await vscode.commands.executeCommand('ripr.copyTargetedTestBrief', { brief });

    assert.strictEqual(await vscode.env.clipboard.readText(), brief);
  });

  test('copyTargetedTestBrief ignores malformed args without throwing', async () => {
    await vscode.commands.executeCommand('ripr.copyTargetedTestBrief', {
      brief: ''
    });
    await vscode.commands.executeCommand('ripr.copyTargetedTestBrief', {
      brief: 42
    });
    await vscode.commands.executeCommand('ripr.copyTargetedTestBrief');
  });

  test('openRelatedTest opens the target uri and line', async () => {
    const uri = workspaceFileUri('Cargo.toml');
    await vscode.commands.executeCommand('ripr.openRelatedTest', {
      uri: uri.toString(),
      line: 1,
      test_name: 'manifest'
    });

    assert.strictEqual(vscode.window.activeTextEditor?.document.uri.toString(), uri.toString());
    assert.strictEqual(vscode.window.activeTextEditor?.selection.active.line, 0);
  });

  test('openRelatedTest ignores malformed args without throwing', async () => {
    await vscode.commands.executeCommand('ripr.openRelatedTest', {
      uri: 'not a uri',
      line: 1
    });
    await vscode.commands.executeCommand('ripr.openRelatedTest', {
      line: -4
    });
    await vscode.commands.executeCommand('ripr.openRelatedTest');
  });
});

interface ControllerTestOptions {
  lspResult?: unknown;
  lspError?: Error;
  cliResult?: string;
}

class FakeLanguageClient {
  readonly requests: Array<{ method: string; params: unknown }> = [];

  constructor(private readonly options: ControllerTestOptions) {}

  async sendRequest(method: string, params: unknown): Promise<unknown> {
    this.requests.push({ method, params });
    if (this.options.lspError) {
      throw this.options.lspError;
    }
    return this.options.lspResult;
  }

  setTrace(): void {}

  async start(): Promise<void> {}

  async stop(): Promise<void> {}
}

function createControllerTestContext(options: ControllerTestOptions) {
  const client = new FakeLanguageClient(options);
  const output = vscode.window.createOutputChannel('ripr test');
  const runRiprCalls: Array<{ command: string; args: string[]; cwd: string }> = [];
  const runtime: RiprClientRuntime = {
    getConfig: () => ({
      serverPath: '',
      serverArgs: ['lsp', '--stdio'],
      autoDownload: false,
      serverVersion: '',
      downloadBaseUrl: '',
      checkMode: 'draft',
      baseRef: 'origin/main',
      traceServer: 'off'
    }),
    resolveServer: async () => ({
      command: 'ripr',
      source: 'path',
      detail: 'test ripr on PATH'
    }),
    createLanguageClient: () => client,
    runRipr: async (command, args, cwd) => {
      runRiprCalls.push({ command, args, cwd });
      return options.cliResult ?? '{}';
    }
  };
  const controller = new RiprClientController({} as vscode.ExtensionContext, output, runtime);
  return {
    client,
    controller,
    runRiprCalls,
    dispose: async () => {
      await controller.stop();
      output.dispose();
    }
  };
}

async function activateExtension(): Promise<void> {
  const ext = vscode.extensions.getExtension('EffortlessMetrics.ripr');
  assert.ok(ext, 'extension should be present');
  await ext.activate();
}

function workspaceFileUri(relativePath: string): vscode.Uri {
  const folder = vscode.workspace.workspaceFolders?.[0];
  assert.ok(folder, 'test workspace should be open');
  return vscode.Uri.joinPath(folder.uri, ...relativePath.split('/'));
}
