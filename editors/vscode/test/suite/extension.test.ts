import * as assert from 'assert';
import * as vscode from 'vscode';
import {
  RiprClientController,
  RiprClientRuntime
} from '../../src/client';

suite('Extension Smoke', () => {
  suiteSetup(async () => {
    await configureTestServer();
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
    assert.ok(commands.includes('ripr.showStatus'));
    assert.ok(commands.includes('ripr.copyContext'));
    assert.ok(commands.includes('ripr.copySuggestedAssertion'));
    assert.ok(commands.includes('ripr.copyTargetedTestBrief'));
    assert.ok(commands.includes('ripr.copyAgentPacketCommand'));
    assert.ok(commands.includes('ripr.copyAgentBriefCommand'));
    assert.ok(commands.includes('ripr.copyAfterSnapshotCommand'));
    assert.ok(commands.includes('ripr.copyAgentVerifyCommand'));
    assert.ok(commands.includes('ripr.copyAgentReceiptCommand'));
    assert.ok(commands.includes('ripr.openRelatedTest'));
    assert.ok(commands.includes('ripr.openSettings'));
  });

  test('defaults-first check mode is draft', () => {
    const config = vscode.workspace.getConfiguration('ripr');
    assert.strictEqual(config.inspect('check.mode')?.defaultValue, 'draft');
  });

  test('real server surfaces seam diagnostic, hover provider, and agent actions', async function (this: Mocha.Context) {
    this.timeout(75000);
    if (!process.env.RIPR_TEST_SERVER_PATH) {
      this.skip();
    }

    const uri = workspaceFileUri('src/lib.rs');
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    const document = await vscode.workspace.openTextDocument(uri);
    assert.strictEqual(document.languageId, 'rust');
    await vscode.window.showTextDocument(document);
    await vscode.commands.executeCommand('ripr.restartServer');

    const diagnostic = await waitForDiagnostic(
      uri,
      (entry) => entry.source === 'ripr' && diagnosticCode(entry) === 'ripr-seam-weakly-gripped',
      60000
    );
    assert.ok(diagnostic.message.includes('Weakly gripped behavioral seam'));

    const hoverPosition = new vscode.Position(
      diagnostic.range.start.line,
      diagnostic.range.start.character + 1
    );
    const hoverText = await waitForHoverText(uri, hoverPosition, (text) =>
      text.includes('**ripr** behavioral seam') &&
      text.includes('`weakly_gripped`') &&
      text.includes('## Missing discriminator')
    );
    assert.ok(hoverText.includes('**ripr** behavioral seam'), hoverText);
    assert.ok(hoverText.includes('`weakly_gripped`'), hoverText);
    assert.ok(hoverText.includes('## Missing discriminator'), hoverText);

    const actions = await vscode.commands.executeCommand<Array<vscode.CodeAction | vscode.Command>>(
      'vscode.executeCodeActionProvider',
      uri,
      diagnostic.range
    );
    const contextCommand = assertCommandAction(actions, 'Inspect seam: copy packet', 'ripr.copyContext');
    assertCommandAction(actions, 'Write targeted test: copy brief', 'ripr.copyTargetedTestBrief');
    assertCommandAction(
      actions,
      'Agent handoff: copy packet command',
      'ripr.copyAgentPacketCommand',
      'ripr agent packet'
    );
    assertCommandAction(
      actions,
      'Agent handoff: copy brief command',
      'ripr.copyAgentBriefCommand',
      'ripr agent brief'
    );
    assertCommandAction(
      actions,
      'Verify after test: copy after-snapshot command',
      'ripr.copyAfterSnapshotCommand',
      'ripr check'
    );
    const verifyCommand = assertCommandAction(
      actions,
      'Verify after test: copy verify command',
      'ripr.copyAgentVerifyCommand',
      'ripr agent verify'
    );
    assertCommandAction(
      actions,
      'Review result: copy receipt command',
      'ripr.copyAgentReceiptCommand',
      'ripr agent receipt'
    );
    const relatedTestCommand = assertCommandAction(
      actions,
      'Write targeted test: open best related test',
      'ripr.openRelatedTest'
    );

    await vscode.commands.executeCommand(contextCommand.command, ...(contextCommand.arguments ?? []));
    const contextPacket = await vscode.env.clipboard.readText();
    const parsedContextPacket = JSON.parse(contextPacket) as {
      schema_version?: string;
      packets?: Array<{ seam_id?: string }>;
    };
    assert.strictEqual(parsedContextPacket.schema_version, '0.3');
    assert.strictEqual(parsedContextPacket.packets?.[0]?.seam_id, '67fc764ba37d77bd');

    await vscode.commands.executeCommand(verifyCommand.command, ...(verifyCommand.arguments ?? []));
    const verifyText = await vscode.env.clipboard.readText();
    assert.ok(verifyText.includes('ripr agent verify --root .'), verifyText);
    assert.ok(verifyText.includes('target/ripr/pilot/after.repo-exposure.json'), verifyText);

    await vscode.commands.executeCommand(relatedTestCommand.command, ...(relatedTestCommand.arguments ?? []));
    const activeEditor = vscode.window.activeTextEditor;
    assert.ok(activeEditor, 'expected related test to open an editor');
    assert.ok(
      activeEditor.document.uri.fsPath.replace(/\\/g, '/').endsWith('/tests/pricing.rs'),
      activeEditor.document.uri.fsPath
    );
    assert.strictEqual(activeEditor.selection.active.line, 3);
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
      assert.deepStrictEqual(JSON.parse(context.clipboardWrites[0]), {
        seam_packets: [{ seam_id: 'abc123' }]
      });
    } finally {
      await context.dispose();
    }
  });

  test('status bar reports server readiness and refresh state', async () => {
    const context = createControllerTestContext({});
    try {
      await context.controller.start();

      assert.ok(context.status.text.includes('ripr: queued'));
      assert.ok(String(context.status.tooltip).includes('saved-workspace analysis is queued'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh queued: generation=1'
      });
      assert.ok(context.status.text.includes('ripr: queued'));
      assert.ok(String(context.status.tooltip).includes('generation=1'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh started: generation=1'
      });
      assert.ok(context.status.text.includes('ripr: analyzing'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=1, diagnostics=0, files=0, findings=0, seam_diagnostics=0, published_files=0, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: no seams'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=2, diagnostics=5, files=2, findings=4, seam_diagnostics=0, published_files=2, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: no seams'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=3, diagnostics=2, files=1, findings=1, seam_diagnostics=1, published_files=1, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: diagnostics'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh failed after 3 ms: workspace analysis failed'
      });
      assert.ok(context.status.text.includes('ripr: failed'));
      await context.controller.showStatus();
      assert.ok(context.infoMessages.at(-1)?.includes('analysis refresh failed'));
    } finally {
      await context.dispose();
    }
  });

  test('status bar projects existing first useful action report', async () => {
    const context = createControllerTestContext({
      firstActionJson: JSON.stringify({
        schema_version: '0.1',
        tool: 'ripr',
        kind: 'first_useful_action',
        status: 'actionable',
        action_kind: 'write_focused_test',
        title: 'Add equality-boundary discriminator test',
        selected: {
          path: 'src/lib.rs',
          line: 2,
          missing_discriminator: 'discount_threshold equality boundary'
        },
        target: {
          file: 'tests/pricing.rs',
          related_test: 'tests/pricing.rs::below_threshold_has_no_discount'
        },
        commands: {
          verify: 'ripr agent verify --root . --json',
          receipt: 'ripr agent receipt --root . --json'
        },
        warnings: []
      })
    });
    try {
      await context.controller.start();

      assert.ok(context.status.text.includes('ripr: first action'));
      assert.ok(String(context.status.tooltip).includes('Add equality-boundary discriminator test'));
      assert.ok(String(context.status.tooltip).includes('src/lib.rs:2'));
      assert.ok(String(context.status.tooltip).includes('discount_threshold equality boundary'));
      assert.ok(String(context.status.tooltip).includes('ripr agent verify --root . --json'));
      assert.strictEqual(context.runRiprCalls.length, 0);

      await context.controller.showStatus();
      assert.ok(context.infoMessages.at(-1)?.includes('First useful action: Add equality-boundary discriminator test'));
    } finally {
      await context.dispose();
    }
  });

  test('status bar ignores first useful action report for another workspace', async () => {
    const context = createControllerTestContext({
      workspaceRoot: '/tmp/ripr-workspace',
      firstActionJson: JSON.stringify({
        schema_version: '0.1',
        tool: 'ripr',
        kind: 'first_useful_action',
        root: '/tmp/other-workspace',
        status: 'actionable',
        action_kind: 'write_focused_test',
        title: 'Add equality-boundary discriminator test',
        selected: {
          path: 'src/lib.rs',
          line: 2
        },
        warnings: []
      })
    });
    try {
      await context.controller.start();

      assert.ok(context.status.text.includes('ripr: queued'));
      assert.ok(!String(context.status.tooltip).includes('First useful action'));
    } finally {
      await context.dispose();
    }
  });

  test('first useful action report does not hide stale editor status', async () => {
    const context = createControllerTestContext({
      firstActionJson: JSON.stringify({
        schema_version: '0.1',
        tool: 'ripr',
        kind: 'first_useful_action',
        status: 'actionable',
        action_kind: 'write_focused_test',
        title: 'Add equality-boundary discriminator test',
        selected: {
          path: 'src/lib.rs',
          line: 2
        },
        warnings: []
      })
    });
    try {
      await context.controller.start();
      assert.ok(context.status.text.includes('ripr: first action'));

      const document = await vscode.workspace.openTextDocument(workspaceFileUri('src/lib.rs'));
      context.controller.markWorkspaceStale(document);

      assert.ok(context.status.text.includes('ripr: stale'));
      assert.ok(String(context.status.tooltip).includes('editor evidence is stale'));
      assert.ok(!context.status.text.includes('first action'));
    } finally {
      await context.dispose();
    }
  });

  test('status bar reports disabled configuration without starting server', async () => {
    const context = createControllerTestContext({ enabled: false });
    try {
      await context.controller.start();

      assert.ok(context.status.text.includes('ripr: disabled'));
      assert.ok(String(context.status.tooltip).includes('Set ripr.enabled to true'));
      assert.strictEqual(context.client.startCalls, 0);
    } finally {
      await context.dispose();
    }
  });

  test('status bar reports missing workspace without starting server', async () => {
    const context = createControllerTestContext({ workspaceRoot: null });
    try {
      await context.controller.start();

      assert.ok(context.status.text.includes('ripr: open workspace'));
      assert.ok(String(context.status.tooltip).includes('needs a workspace folder'));
      assert.strictEqual(context.client.startCalls, 0);
    } finally {
      await context.dispose();
    }
  });

  test('status bar reports unavailable server without hanging on modal UI', async () => {
    const context = createControllerTestContext({
      resolveFailure: {
        message: 'Configured ripr.server.path does not exist.',
        detail: 'Missing configured ripr server path for this test.'
      }
    });
    try {
      await context.controller.start();

      assert.ok(context.status.text.includes('ripr: server missing'));
      assert.ok(String(context.status.tooltip).includes('Missing configured ripr server path'));
      assert.strictEqual(context.errorMessages.length, 1);
      assert.strictEqual(context.client.startCalls, 0);
    } finally {
      await context.dispose();
    }
  });

  test('status bar reports stale saved-workspace analysis after Rust edits', async () => {
    const context = createControllerTestContext({});
    try {
      await context.controller.start();
      const document = await vscode.workspace.openTextDocument(workspaceFileUri('src/lib.rs'));

      context.controller.markWorkspaceStale(document);

      assert.ok(context.status.text.includes('ripr: stale'));
      assert.ok(String(context.status.tooltip).includes(document.uri.fsPath));
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=4, diagnostics=2, files=1, findings=1, seam_diagnostics=1, published_files=1, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: stale'));
      assert.ok(String(context.status.tooltip).includes('last saved workspace state'));

      context.controller.markWorkspaceSaved(document);
      assert.ok(context.status.text.includes('ripr: queued'));
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=5, diagnostics=2, files=1, findings=1, seam_diagnostics=1, published_files=1, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: diagnostics'));

      context.controller.markWorkspaceStale(document);
      context.controller.markWorkspaceClosed(document);
      assert.ok(context.status.text.includes('ripr: queued'));
      await context.controller.showStatus();
      assert.ok(context.infoMessages.at(-1)?.includes('analysis is queued'));
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
      assert.deepStrictEqual(JSON.parse(context.clipboardWrites[0]), {
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
      assert.deepStrictEqual(JSON.parse(context.clipboardWrites[0]), {
        fallback: 'after-error'
      });
    } finally {
      await context.dispose();
    }
  });

  test('copySuggestedAssertion copies assertion text', async () => {
    const context = createControllerTestContext({});
    try {
      await context.controller.copySuggestedAssertion({
        assertion: 'assert_eq!(quote.discount_applied, true);'
      });

      assert.strictEqual(
        context.clipboardWrites[0],
        'assert_eq!(quote.discount_applied, true);'
      );
    } finally {
      await context.dispose();
    }
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

    const context = createControllerTestContext({});
    try {
      await context.controller.copyTargetedTestBrief({ brief });

      assert.strictEqual(context.clipboardWrites[0], brief);
    } finally {
      await context.dispose();
    }
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

  test('copyAgentLoopCommand copies command text', async () => {
    const command = 'ripr agent verify --root . --before before.json --after after.json --json';
    const context = createControllerTestContext({});
    try {
      await context.controller.copyAgentLoopCommand({ command });

      assert.strictEqual(context.clipboardWrites[0], command);
    } finally {
      await context.dispose();
    }
  });

  test('agent loop command handlers ignore malformed args without throwing', async () => {
    await vscode.commands.executeCommand('ripr.copyAgentPacketCommand', {
      command: ''
    });
    await vscode.commands.executeCommand('ripr.copyAgentBriefCommand', {
      command: 42
    });
    await vscode.commands.executeCommand('ripr.copyAfterSnapshotCommand');
    await vscode.commands.executeCommand('ripr.copyAgentVerifyCommand', {
      command: ''
    });
    await vscode.commands.executeCommand('ripr.copyAgentReceiptCommand');
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
  enabled?: boolean;
  lspResult?: unknown;
  lspError?: Error;
  cliResult?: string;
  firstActionJson?: string | null;
  workspaceRoot?: string | null;
  resolveFailure?: { message: string; detail: string };
}

class FakeLanguageClient {
  readonly requests: Array<{ method: string; params: unknown }> = [];
  startCalls = 0;
  private readonly notificationHandlers = new Map<string, Array<(params: unknown) => void>>();

  constructor(private readonly options: ControllerTestOptions) {}

  async sendRequest(method: string, params: unknown): Promise<unknown> {
    this.requests.push({ method, params });
    if (this.options.lspError) {
      throw this.options.lspError;
    }
    return this.options.lspResult;
  }

  onNotification(method: string, handler: (params: unknown) => void): vscode.Disposable {
    const handlers = this.notificationHandlers.get(method) ?? [];
    handlers.push(handler);
    this.notificationHandlers.set(method, handlers);
    return new vscode.Disposable(() => {
      const current = this.notificationHandlers.get(method) ?? [];
      this.notificationHandlers.set(method, current.filter((entry) => entry !== handler));
    });
  }

  emitNotification(method: string, params: unknown): void {
    for (const handler of this.notificationHandlers.get(method) ?? []) {
      handler(params);
    }
  }

  setTrace(): void {}

  async start(): Promise<void> {
    this.startCalls += 1;
  }

  async stop(): Promise<void> {}
}

function createControllerTestContext(options: ControllerTestOptions) {
  const client = new FakeLanguageClient(options);
  const output = fakeOutputChannel();
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 99);
  const runRiprCalls: Array<{ command: string; args: string[]; cwd: string }> = [];
  const clipboardWrites: string[] = [];
  const infoMessages: string[] = [];
  const warningMessages: string[] = [];
  const errorMessages: string[] = [];
  const configuredWorkspaceRoot = options.workspaceRoot === null
    ? undefined
    : options.workspaceRoot ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  const runtime: RiprClientRuntime = {
    getConfig: () => ({
      enabled: options.enabled ?? true,
      serverPath: '',
      serverArgs: ['lsp', '--stdio'],
      autoDownload: false,
      serverVersion: '',
      downloadBaseUrl: '',
      checkMode: 'draft',
      baseRef: 'origin/main',
      traceServer: 'off'
    }),
    workspaceRoot: () => configuredWorkspaceRoot,
    resolveServer: async () => options.resolveFailure ?? ({
      command: 'ripr',
      source: 'path',
      detail: 'test ripr on PATH'
    }),
    createLanguageClient: () => client,
    readFile: async () => options.firstActionJson ?? undefined,
    runRipr: async (command, args, cwd) => {
      runRiprCalls.push({ command, args, cwd });
      return options.cliResult ?? '{}';
    },
    writeClipboard: async (text) => {
      clipboardWrites.push(text);
    },
    showInformationMessage: async (message) => {
      infoMessages.push(message);
      return undefined;
    },
    showWarningMessage: async (message) => {
      warningMessages.push(message);
      return undefined;
    },
    showErrorMessage: async (message) => {
      errorMessages.push(message);
      return undefined;
    },
  };
  const controller = new RiprClientController({} as vscode.ExtensionContext, output, runtime, status);
  return {
    client,
    controller,
    status,
    runRiprCalls,
    clipboardWrites,
    infoMessages,
    warningMessages,
    errorMessages,
    dispose: async () => {
      await controller.stop();
      output.dispose();
      status.dispose();
    }
  };
}

function fakeOutputChannel(): vscode.OutputChannel {
  return {
    name: 'ripr test',
    append: () => {},
    appendLine: () => {},
    clear: () => {},
    show: () => {},
    hide: () => {},
    dispose: () => {},
    replace: () => {}
  } as vscode.OutputChannel;
}

async function activateExtension(): Promise<void> {
  const ext = vscode.extensions.getExtension('EffortlessMetrics.ripr');
  assert.ok(ext, 'extension should be present');
  await ext.activate();
}

async function configureTestServer(): Promise<void> {
  const testServerPath = process.env.RIPR_TEST_SERVER_PATH;
  if (!testServerPath) {
    return;
  }

  const config = vscode.workspace.getConfiguration('ripr');
  await config.update('server.path', testServerPath, vscode.ConfigurationTarget.Global);
  await config.update('server.autoDownload', false, vscode.ConfigurationTarget.Global);
  await config.update('baseRef', 'HEAD', vscode.ConfigurationTarget.Global);
  await config.update('check.mode', 'instant', vscode.ConfigurationTarget.Global);
}

function workspaceFileUri(relativePath: string): vscode.Uri {
  const folder = vscode.workspace.workspaceFolders?.[0];
  assert.ok(folder, 'test workspace should be open');
  return vscode.Uri.joinPath(folder.uri, ...relativePath.split('/'));
}

async function waitForDiagnostic(
  uri: vscode.Uri,
  predicate: (diagnostic: vscode.Diagnostic) => boolean,
  timeoutMs = 15000
): Promise<vscode.Diagnostic> {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const diagnostic = vscode.languages.getDiagnostics(uri).find(predicate);
    if (diagnostic) {
      return diagnostic;
    }
    await sleep(150);
  }
  const currentUriDiagnostics = vscode.languages
    .getDiagnostics(uri)
    .map((entry) => `${entry.source ?? '<no source>'}:${diagnosticCode(entry)}:${entry.message}`)
    .join('\n');
  const allDiagnostics = vscode.languages
    .getDiagnostics()
    .map(([diagnosticUri, entries]) =>
      [
        diagnosticUri.toString(),
        ...entries.map((entry) => `  ${entry.source ?? '<no source>'}:${diagnosticCode(entry)}:${entry.message}`),
      ].join('\n')
    )
    .join('\n');
  const workspaceFolders = vscode.workspace.workspaceFolders
    ?.map((folder) => folder.uri.fsPath)
    .join(', ') ?? '<none>';
  throw new Error([
    'timed out waiting for ripr seam diagnostic.',
    `Workspace folders: ${workspaceFolders}`,
    `Target URI: ${uri.toString()}`,
    `Current URI diagnostics:\n${currentUriDiagnostics}`,
    `All diagnostics:\n${allDiagnostics}`,
  ].join('\n'));
}

async function waitForHoverText(
  uri: vscode.Uri,
  position: vscode.Position,
  predicate: (text: string) => boolean,
  timeoutMs = 15000
): Promise<string> {
  const started = Date.now();
  let lastHoverText = '';
  while (Date.now() - started < timeoutMs) {
    const hovers = await vscode.commands.executeCommand<vscode.Hover[]>(
      'vscode.executeHoverProvider',
      uri,
      position
    );
    lastHoverText = hovers.map(hoverMarkdown).join('\n');
    if (predicate(lastHoverText)) {
      return lastHoverText;
    }
    await sleep(150);
  }
  throw new Error(`timed out waiting for ripr seam hover. Last hover:\n${lastHoverText}`);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function diagnosticCode(diagnostic: vscode.Diagnostic): string {
  const code = diagnostic.code;
  if (!code) {
    return '';
  }
  if (typeof code === 'string' || typeof code === 'number') {
    return String(code);
  }
  return String(code.value);
}

function hoverMarkdown(hover: vscode.Hover): string {
  return hover.contents
    .map((entry) => {
      if (typeof entry === 'string') {
        return entry;
      }
      if (entry instanceof vscode.MarkdownString) {
        return entry.value;
      }
      return entry.value;
    })
    .join('\n');
}

function assertCommandAction(
  actions: Array<vscode.CodeAction | vscode.Command>,
  title: string,
  command: string,
  commandText?: string
): vscode.Command {
  const action = actions.find((entry) => entry.title === title);
  assert.ok(action, `expected code action ${title}`);
  const actionCommand = commandForAction(action);
  assert.strictEqual(actionCommand?.command, command);
  if (commandText) {
    const firstArg = actionCommand?.arguments?.[0] as { command?: unknown } | undefined;
    if (typeof firstArg?.command !== 'string') {
      assert.fail(`expected ${title} to include a string command payload`);
    }
    const payload = firstArg.command;
    assert.ok(
      payload.includes(commandText),
      `expected ${title} command payload to include ${commandText}, got ${payload}`
    );
  }
  assert.ok(actionCommand, `expected ${title} to carry a command`);
  return actionCommand;
}

function commandForAction(action: vscode.CodeAction | vscode.Command): vscode.Command | undefined {
  const maybeCodeActionCommand = (action as vscode.CodeAction).command;
  if (maybeCodeActionCommand && typeof maybeCodeActionCommand === 'object') {
    return maybeCodeActionCommand;
  }
  const maybeCommand = action as vscode.Command;
  return typeof maybeCommand.command === 'string' ? maybeCommand : undefined;
}
