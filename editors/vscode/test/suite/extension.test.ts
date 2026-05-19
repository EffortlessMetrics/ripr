import * as assert from 'assert';
import { promises as fs } from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import {
  RiprClientController,
  RiprClientRuntime,
  RiprAgentLoopCommandTarget,
  RiprWorkspaceRootState,
  readFirstPrPacketStatus
} from '../../src/client';

suite('Extension Smoke', () => {
  suiteSetup(async () => {
    await cleanupEditorGapSmokeFiles();
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
    assert.ok(commands.includes('ripr.diagnoseSetup'));
    assert.ok(commands.includes('ripr.startCurrentRepair'));
    assert.ok(commands.includes('ripr.openFirstPrPacket'));
    assert.ok(commands.includes('ripr.copyFirstPrSummary'));
    assert.ok(commands.includes('ripr.copyFirstPrRepairPacket'));
    assert.ok(commands.includes('ripr.copyFirstPrVerifyCommand'));
    assert.ok(commands.includes('ripr.copyFirstPrReceiptCommand'));
    assert.ok(commands.includes('ripr.copyFirstPrRegenerationGuidance'));
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

  test('real extension first-pr bridge commands use safe packet artifacts', async function (this: Mocha.Context) {
    this.timeout(30000);
    await cleanupFirstPrBridgeSmokeFiles();
    await writeWorkspaceFile('target/ripr/first-pr/start-here.json', firstPrPacket({}));
    await writeWorkspaceFile('target/ripr/first-pr/start-here.md', '# RIPR first-pr packet\n');
    try {
      await vscode.commands.executeCommand('ripr.diagnoseSetup');
      await vscode.commands.executeCommand('ripr.showStatus');

      await vscode.commands.executeCommand('ripr.copyFirstPrSummary');
      const summary = await waitForClipboardText((text) =>
        text.includes('RIPR first-pr summary') &&
        text.includes('gap:rust:pricing:discount:threshold-boundary')
      );
      assert.ok(summary.includes('Does not prove runtime adequacy'), summary);

      await withCurrentFirstPrDiagnostic({
        canonical_gap_id: 'gap:rust:pricing:discount:threshold-boundary',
        gap_id: 'gap:pr:pricing:threshold-boundary'
      }, async () => {
        await vscode.commands.executeCommand('ripr.copyFirstPrRepairPacket');
        const repairPacket = await waitForClipboardText((text) =>
          text.includes('RIPR first-pr repair packet') &&
          text.includes('Repair route: AddBoundaryAssertion')
        );
        assert.ok(repairPacket.includes('Do not broaden scope.'), repairPacket);

        await vscode.commands.executeCommand('ripr.copyFirstPrVerifyCommand');
        assert.strictEqual(
          await waitForClipboardText((text) => text === 'cargo xtask fixtures boundary_gap'),
          'cargo xtask fixtures boundary_gap'
        );

        await vscode.commands.executeCommand('ripr.copyFirstPrReceiptCommand');
        assert.strictEqual(
          await waitForClipboardText((text) => text === 'ripr agent receipt --root . --json'),
          'ripr agent receipt --root . --json'
        );
      });

      await vscode.commands.executeCommand('ripr.openFirstPrPacket');
      const activeEditor = vscode.window.activeTextEditor;
      assert.ok(activeEditor, 'expected first-pr packet to open');
      assert.ok(
        activeEditor.document.uri.fsPath.replace(/\\/g, '/').endsWith('/target/ripr/first-pr/start-here.md'),
        activeEditor.document.uri.fsPath
      );
    } finally {
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await cleanupFirstPrBridgeSmokeFiles();
    }
  });

  test('real extension first-pr bridge commands fail closed for unsafe packet states', async function (this: Mocha.Context) {
    this.timeout(30000);
    await cleanupFirstPrBridgeSmokeFiles();
    try {
      await writeWorkspaceFile('target/ripr/reports/start-here.json', '{not-json');
      await writeClipboardText('first-pr-sentinel');
      await vscode.commands.executeCommand('ripr.copyFirstPrSummary');
      assert.strictEqual(await currentClipboardText(), 'first-pr-sentinel');

      await cleanupFirstPrBridgeSmokeFiles();
      await writeWorkspaceFile(
        'target/ripr/reports/start-here.json',
        firstPrPacket({ root: '../other-workspace' })
      );
      await writeClipboardText('first-pr-sentinel');
      await vscode.commands.executeCommand('ripr.copyFirstPrSummary');
      assert.strictEqual(await currentClipboardText(), 'first-pr-sentinel');

      await withCurrentFirstPrDiagnostic({
        canonical_gap_id: 'gap:rust:pricing:discount:threshold-boundary',
        gap_id: 'gap:pr:pricing:threshold-boundary'
      }, async () => {
        await vscode.commands.executeCommand('ripr.copyFirstPrVerifyCommand');
      });
      assert.strictEqual(await currentClipboardText(), 'first-pr-sentinel');
    } finally {
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await cleanupFirstPrBridgeSmokeFiles();
    }
  });

  test('real extension adoption assurance smoke keeps setup repair and first-pr actions bounded', async function (this: Mocha.Context) {
    this.timeout(45000);
    await cleanupAdoptionAssuranceSmokeFiles();
    await writeWorkspaceFile('ripr.toml', '[languages]\nenabled = ["rust"]\n');
    await writeWorkspaceFile(
      'target/ripr/reports/first-useful-action.json',
      firstActionReport({})
    );
    await writeWorkspaceFile(
      'target/ripr/agent/agent-receipt.json',
      agentReceipt({ movement: 'improved' })
    );
    await writeWorkspaceFile('target/ripr/first-pr/start-here.json', firstPrPacket({}));
    await writeWorkspaceFile('target/ripr/first-pr/start-here.md', '# RIPR first-pr packet\n');
    try {
      const ext = vscode.extensions.getExtension('EffortlessMetrics.ripr')!;
      await ext.activate();
      assert.strictEqual(ext.isActive, true);

      await vscode.commands.executeCommand('ripr.diagnoseSetup');
      await vscode.commands.executeCommand('ripr.showStatus');

      await vscode.commands.executeCommand('ripr.copyFirstPrSummary');
      const summary = await waitForClipboardText((text) =>
        text.includes('RIPR first-pr summary') &&
        text.includes('Gap identity: gap:rust:pricing:discount:threshold-boundary') &&
        text.includes('Does not prove runtime adequacy')
      );
      assert.ok(summary.includes('Verify command: cargo xtask fixtures boundary_gap'), summary);

      await withCurrentFirstPrDiagnostic({
        canonical_gap_id: 'gap:rust:pricing:discount:threshold-boundary',
        gap_id: 'gap:pr:pricing:threshold-boundary'
      }, async () => {
        await vscode.commands.executeCommand('ripr.copyFirstPrRepairPacket');
        const repairPacket = await waitForClipboardText((text) =>
          text.includes('RIPR first-pr repair packet') &&
          text.includes('Repair route: AddBoundaryAssertion')
        );
        assert.ok(repairPacket.includes('Do not broaden scope.'), repairPacket);

        await vscode.commands.executeCommand('ripr.copyFirstPrVerifyCommand');
        assert.strictEqual(
          await waitForClipboardText((text) => text === 'cargo xtask fixtures boundary_gap'),
          'cargo xtask fixtures boundary_gap'
        );

        await vscode.commands.executeCommand('ripr.copyFirstPrReceiptCommand');
        assert.strictEqual(
          await waitForClipboardText((text) => text === 'ripr agent receipt --root . --json'),
          'ripr agent receipt --root . --json'
        );
      });

      await cleanupFirstPrBridgeSmokeFiles();
      await writeWorkspaceFile(
        'target/ripr/reports/start-here.json',
        firstPrPacket({ root: '../other-workspace' })
      );
      await writeClipboardText('adoption-assurance-sentinel');
      await vscode.commands.executeCommand('ripr.copyFirstPrSummary');
      assert.strictEqual(await currentClipboardText(), 'adoption-assurance-sentinel');

      await cleanupFirstPrBridgeSmokeFiles();
      await writeWorkspaceFile('target/ripr/reports/start-here.json', '{not-json');
      await vscode.commands.executeCommand('ripr.copyFirstPrSummary');
      assert.strictEqual(await currentClipboardText(), 'adoption-assurance-sentinel');
    } finally {
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await cleanupAdoptionAssuranceSmokeFiles();
    }
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
    const contextCommand = assertCommandAction(actions, 'Inspect Test Gap - Copy Context', 'ripr.copyContext');
    const targetedBriefCommand = assertCommandAction(
      actions,
      'Write targeted test: copy brief',
      'ripr.copyTargetedTestBrief'
    );
    const packetCommand = assertCommandAction(
      actions,
      'Agent handoff: copy packet command',
      'ripr.copyAgentPacketCommand',
      'ripr agent packet'
    );
    const briefCommand = assertCommandAction(
      actions,
      'Agent handoff: copy brief command',
      'ripr.copyAgentBriefCommand',
      'ripr agent brief'
    );
    const afterSnapshotCommand = assertCommandAction(
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
    const receiptCommand = assertCommandAction(
      actions,
      'Review result: copy receipt command',
      'ripr.copyAgentReceiptCommand',
      'ripr agent receipt'
    );
    const assertionCommand = assertCommandAction(
      actions,
      'Write targeted test: copy suggested assertion',
      'ripr.copySuggestedAssertion'
    );
    const relatedTestCommand = assertCommandAction(
      actions,
      'Write targeted test: open best related test',
      'ripr.openRelatedTest'
    );

    await vscode.commands.executeCommand(contextCommand.command, ...(contextCommand.arguments ?? []));
    const contextPacket = await waitForClipboardText((text) =>
      text.includes('"schema_version": "0.3"') && text.includes('"seam_id": "67fc764ba37d77bd"')
    );
    const parsedContextPacket = JSON.parse(contextPacket) as {
      schema_version?: string;
      packets?: Array<{ seam_id?: string }>;
    };
    assert.strictEqual(parsedContextPacket.schema_version, '0.3');
    assert.strictEqual(parsedContextPacket.packets?.[0]?.seam_id, '67fc764ba37d77bd');

    await vscode.commands.executeCommand(targetedBriefCommand.command, ...(targetedBriefCommand.arguments ?? []));
    const targetedBriefText = await waitForClipboardText((text) => text.includes('Target seam:'));
    assert.ok(targetedBriefText.includes('Target seam:'), targetedBriefText);
    assert.ok(targetedBriefText.includes('src/lib.rs:2'), targetedBriefText);
    assert.ok(targetedBriefText.includes('predicate_boundary'), targetedBriefText);
    assert.ok(targetedBriefText.includes('Missing discriminator'), targetedBriefText);
    assert.ok(targetedBriefText.includes('tests/pricing.rs'), targetedBriefText);

    await vscode.commands.executeCommand(packetCommand.command, ...(packetCommand.arguments ?? []));
    const packetText = await waitForClipboardText((text) => text.includes('ripr agent packet'));
    assert.ok(packetText.includes('ripr agent packet --root . --seam-id 67fc764ba37d77bd'), packetText);
    assert.ok(packetText.includes('target/ripr/agent/agent-packet.json'), packetText);

    await vscode.commands.executeCommand(briefCommand.command, ...(briefCommand.arguments ?? []));
    const briefText = await waitForClipboardText((text) => text.includes('ripr agent brief'));
    assert.ok(briefText.includes('ripr agent brief --root . --seam-id 67fc764ba37d77bd'), briefText);
    assert.ok(briefText.includes('target/ripr/agent/agent-brief.json'), briefText);

    await vscode.commands.executeCommand(afterSnapshotCommand.command, ...(afterSnapshotCommand.arguments ?? []));
    const afterSnapshotText = await waitForClipboardText((text) =>
      text.includes('ripr check') && text.includes('target/ripr/pilot/after.repo-exposure.json')
    );
    assert.ok(afterSnapshotText.includes('ripr check --root . --base '), afterSnapshotText);
    assert.ok(afterSnapshotText.includes('--format repo-exposure-json'), afterSnapshotText);
    assert.ok(afterSnapshotText.includes('target/ripr/pilot/after.repo-exposure.json'), afterSnapshotText);

    await vscode.commands.executeCommand(verifyCommand.command, ...(verifyCommand.arguments ?? []));
    const verifyText = await waitForClipboardText((text) => text.includes('ripr agent verify'));
    assert.ok(verifyText.includes('ripr agent verify --root .'), verifyText);
    assert.ok(verifyText.includes('target/ripr/pilot/after.repo-exposure.json'), verifyText);

    await vscode.commands.executeCommand(receiptCommand.command, ...(receiptCommand.arguments ?? []));
    const receiptText = await waitForClipboardText((text) => text.includes('ripr agent receipt'));
    assert.ok(receiptText.includes('ripr agent receipt --root .'), receiptText);
    assert.ok(receiptText.includes('--seam-id 67fc764ba37d77bd'), receiptText);
    assert.ok(receiptText.includes('target/ripr/agent/agent-receipt.json'), receiptText);

    await vscode.commands.executeCommand(assertionCommand.command, ...(assertionCommand.arguments ?? []));
    const assertionText = await waitForClipboardText((text) => text.includes('assert_eq!(discounted_total('));
    assert.ok(assertionText.includes('assert_eq!(discounted_total('), assertionText);

    await vscode.commands.executeCommand(relatedTestCommand.command, ...(relatedTestCommand.arguments ?? []));
    const activeEditor = vscode.window.activeTextEditor;
    assert.ok(activeEditor, 'expected related test to open an editor');
    assert.ok(
      activeEditor.document.uri.fsPath.replace(/\\/g, '/').endsWith('/tests/pricing.rs'),
      activeEditor.document.uri.fsPath
    );
    assert.strictEqual(activeEditor.selection.active.line, 3);
  });

  test('real server surfaces preview gap diagnostic, hover, status, and bounded actions', async function (this: Mocha.Context) {
    this.timeout(75000);
    if (!process.env.RIPR_TEST_SERVER_PATH) {
      this.skip();
    }

    await cleanupEditorGapSmokeFiles();
    await writeEditorGapSmokeFiles();
    const uri = workspaceFileUri('src/pricing.ts');
    try {
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      const document = await vscode.workspace.openTextDocument(uri);
      assert.strictEqual(document.languageId, 'typescript');
      await vscode.window.showTextDocument(document);
      await vscode.commands.executeCommand('ripr.restartServer');

      const diagnostic = await waitForDiagnostic(
        uri,
        (entry) => entry.source === 'ripr' && diagnosticCode(entry) === 'ripr-gap-MissingBoundaryAssertion',
        60000
      );
      assert.ok(diagnostic.message.includes('preview advisory evidence'), diagnostic.message);

      const hoverPosition = new vscode.Position(
        diagnostic.range.start.line,
        diagnostic.range.start.character + 1
      );
      const hoverText = await waitForHoverText(uri, hoverPosition, (text) =>
        text.includes('**ripr** gap decision') &&
        text.includes('- Language: `typescript`') &&
        text.includes('- Status: `preview`') &&
        text.includes('Static limit: missing_import_graph') &&
        text.includes('Action: advisory only') &&
        text.includes('## Repair route')
      );
      assert.ok(
        hoverText.indexOf('Static limit: missing_import_graph') < hoverText.indexOf('Action: advisory only'),
        hoverText
      );
      assert.ok(
        hoverText.indexOf('Static limit: missing_import_graph') < hoverText.indexOf('## Repair route'),
        hoverText
      );

      const actions = await vscode.commands.executeCommand<Array<vscode.CodeAction | vscode.Command>>(
        'vscode.executeCodeActionProvider',
        uri,
        diagnostic.range
      );
      const repairPacketCommand = assertCommandAction(
        actions,
        'Inspect gap: copy repair packet',
        'ripr.copyContext'
      );
      const relatedTestCommand = assertCommandAction(
        actions,
        'Write targeted test: open best related test',
        'ripr.openRelatedTest'
      );
      const verifyCommand = assertCommandAction(
        actions,
        'Verify after test: copy verify command',
        'ripr.copyAgentVerifyCommand',
        'ripr agent verify'
      );
      const receiptCommand = assertCommandAction(
        actions,
        'Review result: copy receipt command',
        'ripr.copyAgentReceiptCommand',
        'ripr agent receipt'
      );
      const staticLimitCommand = assertCommandAction(
        actions,
        'Inspect gap: copy static-limit note',
        'ripr.copyContext'
      );

      await vscode.commands.executeCommand(repairPacketCommand.command, ...(repairPacketCommand.arguments ?? []));
      const repairPacketText = await waitForClipboardText((text) =>
        text.includes('"source": "gap_decision_ledger"') &&
        text.includes('"canonical_gap_id": "gap:typescript:pricing:threshold-boundary"')
      );
      const repairPacket = JSON.parse(repairPacketText) as {
        source?: string;
        packets?: Array<{ language?: string; language_status?: string; verify_command?: string }>;
      };
      assert.strictEqual(repairPacket.source, 'gap_decision_ledger');
      assert.strictEqual(repairPacket.packets?.[0]?.language, 'typescript');
      assert.strictEqual(repairPacket.packets?.[0]?.language_status, 'preview');
      assert.strictEqual(repairPacket.packets?.[0]?.verify_command, 'ripr agent verify --root . --json');

      await vscode.commands.executeCommand(staticLimitCommand.command, ...(staticLimitCommand.arguments ?? []));
      const staticLimitText = await waitForClipboardText((text) =>
        text.includes('Static limit: missing_import_graph')
      );
      assert.ok(staticLimitText.includes('TypeScript preview smoke uses syntax-first evidence.'), staticLimitText);

      await vscode.commands.executeCommand(verifyCommand.command, ...(verifyCommand.arguments ?? []));
      const verifyText = await waitForClipboardText((text) => text.includes('ripr agent verify --root . --json'));
      assert.strictEqual(verifyText, 'ripr agent verify --root . --json');

      await vscode.commands.executeCommand(receiptCommand.command, ...(receiptCommand.arguments ?? []));
      const receiptText = await waitForClipboardText((text) => text.includes('ripr agent receipt --root . --json'));
      assert.strictEqual(receiptText, 'ripr agent receipt --root . --json');

      await vscode.commands.executeCommand(relatedTestCommand.command, ...(relatedTestCommand.arguments ?? []));
      const activeEditor = vscode.window.activeTextEditor;
      assert.ok(activeEditor, 'expected related TypeScript test to open an editor');
      assert.ok(
        activeEditor.document.uri.fsPath.replace(/\\/g, '/').endsWith('/tests/pricing.test.ts'),
        activeEditor.document.uri.fsPath
      );
      assert.strictEqual(activeEditor.selection.active.line, 3);
    } finally {
      await cleanupEditorGapSmokeFiles();
      await vscode.commands.executeCommand('ripr.restartServer');
    }
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

  test('copyContext copies static-limit notes without LSP fallback for active workspace file', async () => {
    const relativePath = 'src/static-limit-note.rs';
    const uri = workspaceFileUri(relativePath);
    const context = createControllerTestContext({});
    try {
      await writeWorkspaceFile(relativePath, 'pub fn static_limit_note_target() {}\n');
      const document = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(document);
      await context.controller.start();
      await context.controller.copyContext({
        label: 'static_limit_note',
        note: 'Static limit: missing_import_graph\nBoundary: static evidence only; advisory action.'
      });

      assert.deepStrictEqual(context.client.requests, []);
      assert.strictEqual(
        context.clipboardWrites[0],
        'Static limit: missing_import_graph\nBoundary: static evidence only; advisory action.'
      );
    } finally {
      await context.dispose();
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await removeWorkspacePath(relativePath);
    }
  });

  test('copyContext copies first repair packets without LSP fallback for active workspace file', async () => {
    const relativePath = 'src/first-repair-packet.rs';
    const uri = workspaceFileUri(relativePath);
    const context = createControllerTestContext({});
    const packet = [
      'RIPR first repair packet',
      '',
      'Gap identity: gap:rust:pricing',
      'Language: rust',
      'Suggested action:',
      '- Add one focused assertion.',
      'Verify command:',
      'ripr agent verify --root . --json',
      'Receipt command:',
      'ripr agent receipt --root . --json',
      'Limits and non-claims:',
      '- Static editor evidence only.'
    ].join('\n');
    try {
      await writeWorkspaceFile(relativePath, 'pub fn first_repair_packet_target() {}\n');
      const document = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(document);
      await context.controller.start();
      await context.controller.copyContext({
        label: 'first_repair_packet',
        packet
      });

      assert.deepStrictEqual(context.client.requests, []);
      assert.strictEqual(context.clipboardWrites[0], packet);
      assert.ok(context.infoMessages.at(-1)?.includes('first repair packet'));
    } finally {
      await context.dispose();
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await removeWorkspacePath(relativePath);
    }
  });

  test('direct repair commands fail closed without active file or target URI', async () => {
    const context = createControllerTestContext({});
    try {
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await context.controller.start();

      await context.controller.copyContext({
        label: 'first_repair_packet',
        packet: 'RIPR first repair packet'
      });
      await context.controller.copyContext({
        label: 'static_limit_note',
        note: 'Static limit: missing_import_graph'
      });
      await context.controller.copyAgentLoopCommand(
        agentLoopCommandTarget(
          'gap_verify',
          'ripr agent verify --root . --json'
        )
      );

      assert.deepStrictEqual(context.clipboardWrites, []);
      assert.deepStrictEqual(context.client.requests, []);
      assert.strictEqual(context.runRiprCalls.length, 0);
      assert.strictEqual(context.infoMessages.length, 3);
      for (const message of context.infoMessages) {
        assert.ok(message.includes('active file or target URI'), message);
      }
    } finally {
      await context.dispose();
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    }
  });

  test('root-scoped repair commands reject active files outside selected root', async () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder, 'test workspace should be open');
    const selectedRoot = path.resolve('selected workspace root');
    const relativePath = 'src/root-mismatch.rs';
    const uri = workspaceFileUri(relativePath);
    const context = createControllerTestContext({
      workspaceRootState: {
        kind: 'selectedRoot',
        root: selectedRoot,
        roots: [selectedRoot, folder.uri.fsPath],
        detail: 'test selected root'
      }
    });

    try {
      await writeWorkspaceFile(relativePath, 'pub fn root_mismatch() {}\n');
      const document = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(document);
      await context.controller.start();

      await context.controller.copyContext({
        label: 'first_repair_packet',
        packet: 'RIPR first repair packet'
      });
      await context.controller.copyContext({
        label: 'static_limit_note',
        note: 'Static limit: missing_import_graph'
      });
      await context.controller.copyContext({
        uri: uri.toString(),
        line: 1,
        seam_id: 'abc123'
      });
      await context.controller.copyAgentLoopCommand(
        agentLoopCommandTarget(
          'gap_verify',
          'ripr agent verify --root . --json'
        )
      );
      await context.controller.openRelatedTest({
        uri: uri.toString(),
        line: 1,
        test_name: 'root_mismatch'
      });

      assert.deepStrictEqual(context.clipboardWrites, []);
      assert.strictEqual(context.client.requests.length, 0);
      assert.strictEqual(context.runRiprCalls.length, 0);
      assert.ok(context.infoMessages.length >= 5);
      for (const message of context.infoMessages.slice(-5)) {
        assert.ok(message.includes('different workspace root'), message);
      }
    } finally {
      await context.dispose();
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await removeWorkspacePath(relativePath);
    }
  });

  test('startCurrentRepair executes the nearest existing repair action', async () => {
    const relativePath = 'src/start-current-repair.rs';
    const uri = workspaceFileUri(relativePath);
    const collection = vscode.languages.createDiagnosticCollection('ripr-start-current-repair');
    let provider: vscode.Disposable | undefined;
    try {
      await writeWorkspaceFile(relativePath, [
        'pub fn far() {}',
        '',
        'pub fn filler_one() {}',
        'pub fn filler_two() {}',
        '',
        'pub fn near() {}',
        ''
      ].join('\n'));
      const document = await vscode.workspace.openTextDocument(uri);
      const editor = await vscode.window.showTextDocument(document);
      editor.selection = new vscode.Selection(new vscode.Position(5, 2), new vscode.Position(5, 2));

      const far = new vscode.Diagnostic(
        new vscode.Range(new vscode.Position(0, 0), new vscode.Position(0, 12)),
        'far ripr gap',
        vscode.DiagnosticSeverity.Warning
      );
      far.source = 'ripr';
      far.code = 'ripr-gap-MissingBoundaryAssertion';
      const near = new vscode.Diagnostic(
        new vscode.Range(new vscode.Position(5, 0), new vscode.Position(5, 13)),
        'near ripr gap',
        vscode.DiagnosticSeverity.Warning
      );
      near.source = 'ripr';
      near.code = 'ripr-gap-MissingBoundaryAssertion';
      collection.set(uri, [far, near]);

      provider = vscode.languages.registerCodeActionsProvider(
        { language: 'rust', scheme: 'file' },
        {
          provideCodeActions(_document, range) {
            const packet = range.start.line === 5
              ? 'nearest repair packet'
              : 'far repair packet';
            const action = new vscode.CodeAction('Copy first repair packet', vscode.CodeActionKind.QuickFix);
            action.command = {
              title: 'Copy first repair packet',
              command: 'ripr.copyContext',
              arguments: [{
                label: 'first_repair_packet',
                packet
              }]
            };
            return [action];
          }
        }
      );

      await vscode.commands.executeCommand('ripr.startCurrentRepair');

      const copied = await waitForClipboardText((text) => text === 'nearest repair packet');
      assert.strictEqual(copied, 'nearest repair packet');
    } finally {
      provider?.dispose();
      collection.dispose();
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await removeWorkspacePath(relativePath);
    }
  });

  test('status bar reports server readiness and refresh state', async () => {
    const context = createControllerTestContext({});
    try {
      await context.controller.start();

      assert.ok(context.status.text.includes('ripr: queued'));
      assert.ok(String(context.status.tooltip).includes('saved-workspace analysis is queued'));
      assert.ok(String(context.status.tooltip).includes('Workspace:'));
      assert.ok(String(context.status.tooltip).includes('Workspace root state: workspace_single_root'));
      assert.ok(String(context.status.tooltip).includes('Server command: ripr'));
      assert.ok(String(context.status.tooltip).includes('Extension state: extension_version_ok (0.6.0)'));
      assert.ok(String(context.status.tooltip).includes('ripr server state: ripr_version_ok (ripr 0.6.0-test)'));
      assert.ok(String(context.status.tooltip).includes('Workspace trust state: workspace_trusted'));
      assert.ok(String(context.status.tooltip).includes('Config state: config_missing'));
      assert.ok(String(context.status.tooltip).includes('Artifact directory state: artifact_dir_missing'));
      assert.ok(String(context.status.tooltip).includes('Server version: ripr 0.6.0-test'));
      assert.ok(String(context.status.tooltip).includes('Server started: yes'));
      assert.ok(String(context.status.tooltip).includes('Config: ripr.toml (missing'));
      assert.ok(String(context.status.tooltip).includes('Editor selectors: rust, typescript'));
      assert.ok(String(context.status.tooltip).includes('Enabled languages: not reported yet'));
      assert.ok(String(context.status.tooltip).includes('Available languages: not reported by server'));
      assert.ok(String(context.status.tooltip).includes('Evidence freshness: pending refresh'));
      assert.ok(String(context.status.tooltip).includes('Artifact first useful action report: target/ripr/reports/first-useful-action.json (missing'));
      assert.ok(String(context.status.tooltip).includes('Artifact gap decision ledger: target/ripr/reports/gap-decision-ledger.json (missing'));
      assert.ok(String(context.status.tooltip).includes('Artifact editor agent receipt: target/ripr/agent/agent-receipt.json (missing'));
      assert.ok(String(context.status.tooltip).includes('Next safe action:'));

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
        message: 'ripr analysis refresh completed in 42 ms: generation=1, diagnostics=0, files=0, findings=0, seam_diagnostics=0, enabled_languages=1, enabled_language_names=rust, published_files=0, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: no seams'));
      assert.ok(String(context.status.tooltip).includes('Enabled languages: rust'));
      assert.ok(String(context.status.tooltip).includes('last saved workspace state'));
      assert.ok(String(context.status.tooltip).includes('disabled or unavailable preview languages stay silent'));
      assert.ok(String(context.status.tooltip).includes('enabled and available in this ripr build'));
      assert.ok(String(context.status.tooltip).includes('Evidence freshness: current saved-workspace status reported by server refresh'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=1, diagnostics=0, files=0, findings=0, seam_diagnostics=0, enabled_languages=0, enabled_language_names=, published_files=0, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: languages off'));
      assert.ok(String(context.status.tooltip).includes('[languages] enabled = []'));
      assert.ok(String(context.status.tooltip).includes('Enabled languages: none'));
      assert.ok(String(context.status.tooltip).includes('ripr.toml [languages] enabled'));
      await context.controller.showStatus();
      assert.ok(context.infoMessages.at(-1)?.includes('no enabled languages'));
      assert.ok(context.outputLines.join('\n').includes('Enabled languages: none'));
      assert.ok(context.outputLines.join('\n').includes('Next safe action:'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=2, diagnostics=5, files=2, findings=4, seam_diagnostics=0, enabled_languages=1, enabled_language_names=rust, published_files=2, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: no seams'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=2, diagnostics=0, files=1, findings=0, seam_diagnostics=0, enabled_languages=3, enabled_language_names=rust|typescript|python, published_files=0, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: no seams'));
      assert.ok(String(context.status.tooltip).includes('Enabled languages: rust, typescript, python'));
      assert.ok(String(context.status.tooltip).includes('workspace root is correct'));
      assert.ok(String(context.status.tooltip).includes('available in this ripr build'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=3, diagnostics=2, files=1, findings=1, seam_diagnostics=1, enabled_languages=1, enabled_language_names=rust, published_files=1, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: diagnostics'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=4, diagnostics=2, files=1, findings=1, preview_findings=1, static_limits=1, seam_diagnostics=0, enabled_languages=3, enabled_language_names=rust|typescript|python, published_files=1, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: diagnostics'));
      assert.ok(String(context.status.tooltip).includes('1 preview'));
      assert.ok(String(context.status.tooltip).includes('syntax-first and advisory'));
      assert.ok(String(context.status.tooltip).includes('static limit'));
      assert.ok(String(context.status.tooltip).includes('Enabled languages: rust, typescript, python'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=5, diagnostics=2, files=1, findings=1, preview_findings=1, static_limits=1, seam_diagnostics=1, gap_artifacts=1, actionable_gap_artifacts=1, preview_gap_artifacts=1, no_action_gap_artifacts=0, gap_static_limits=1, gap_artifact_rejections=0, gap_artifact_rejection_kinds=, enabled_languages=3, enabled_language_names=rust|typescript|python, published_files=1, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: gap ready'));
      const actionableGapTooltip = String(context.status.tooltip);
      assert.ok(actionableGapTooltip.includes('preview-limited gap projection input'));
      assert.ok(actionableGapTooltip.includes('preview gap artifact input is syntax-first and advisory'));
      assert.ok(actionableGapTooltip.includes('gap static limit entry must be read before action language'));
      assert.ok(actionableGapTooltip.includes('1 actionable gap artifact validated for editor projection'));
      assert.ok(actionableGapTooltip.includes('Next safe action: Read static limits'));
      assert.ok(
        actionableGapTooltip.indexOf('gap static limit entry') <
          actionableGapTooltip.indexOf('1 actionable gap artifact'),
        actionableGapTooltip
      );
      const previewGapStatus = await showStatusReport(context);
      assert.ok(previewGapStatus.includes('ripr validated preview-limited gap projection input.'));
      assert.ok(previewGapStatus.includes('preview gap artifact input is syntax-first and advisory'));
      assert.ok(previewGapStatus.includes('gap static limit entry must be read before action language'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=6, diagnostics=0, files=0, findings=0, preview_findings=0, static_limits=0, seam_diagnostics=0, gap_artifacts=1, actionable_gap_artifacts=0, preview_gap_artifacts=0, no_action_gap_artifacts=1, gap_static_limits=0, gap_artifact_rejections=0, gap_artifact_rejection_kinds=, enabled_languages=1, enabled_language_names=rust, published_files=0, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: gap clear'));
      assert.ok(String(context.status.tooltip).includes('no local repair action'));
      const noActionStatus = await showStatusReport(context);
      assert.ok(noActionStatus.includes('ripr validated gap artifacts with no actionable gap.'));
      assert.ok(noActionStatus.includes('no local repair action'));
      assert.ok(noActionStatus.includes('Next safe action: No local repair action is projected'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=7, diagnostics=0, files=0, findings=0, preview_findings=0, static_limits=0, seam_diagnostics=0, gap_artifacts=0, actionable_gap_artifacts=0, preview_gap_artifacts=0, no_action_gap_artifacts=0, gap_static_limits=0, gap_artifact_rejections=1, gap_artifact_rejection_kinds=wrong_root, enabled_languages=1, enabled_language_names=rust, published_files=0, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: gap blocked'));
      assert.ok(String(context.status.tooltip).includes('wrong_root'));
      assert.ok(String(context.status.tooltip).includes('not projected'));
      assert.ok(String(context.status.tooltip).includes('never create diagnostics'));
      const wrongRootStatus = await showStatusReport(context);
      assert.ok(wrongRootStatus.includes('ripr ignored 1 unsafe gap artifact input.'));
      assert.ok(wrongRootStatus.includes('Rejected kind: wrong_root'));
      assert.ok(wrongRootStatus.includes('not projected'));
      assert.ok(wrongRootStatus.includes('never create diagnostics'));

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
        audience: 'developer',
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
      assert.ok(context.outputLines.join('\n').includes('First useful action: Add equality-boundary discriminator test'));
      assert.ok(context.outputLines.join('\n').includes('Report: target/ripr/reports/first-useful-action.json'));
    } finally {
      await context.dispose();
    }
  });

  test('status model reports setup files found and missing', async () => {
    const context = createControllerTestContext({
      files: {
        'ripr.toml': '[languages]\nenabled = ["rust"]\n',
        'target/ripr/reports/first-useful-action.json': firstActionReport({}),
        'target/ripr/reports/gap-decision-ledger.json': '{"schema_version":"0.1"}',
        'target/ripr/agent/agent-receipt.json': '{"schema_version":"0.1"}'
      }
    });
    try {
      await context.controller.start();
      await context.controller.showStatus();

      const statusOutput = context.outputLines.join('\n');
      assert.ok(statusOutput.includes('Config: ripr.toml (found; found in current workspace'));
      assert.ok(statusOutput.includes('Artifact first useful action report: target/ripr/reports/first-useful-action.json (found; found in current workspace'));
      assert.ok(statusOutput.includes('Artifact gap decision ledger: target/ripr/reports/gap-decision-ledger.json (found; found in current workspace'));
      assert.ok(statusOutput.includes('Artifact editor agent receipt: target/ripr/agent/agent-receipt.json (found; found in current workspace'));
      assert.ok(statusOutput.includes('First useful action: Add equality-boundary discriminator test'));
      assert.ok(statusOutput.includes('ripr server state: ripr_version_ok (ripr 0.6.0-test)'));
      assert.ok(statusOutput.includes('Config state: config_found'));
      assert.ok(statusOutput.includes('Artifact directory state: artifact_dir_present'));
      assert.ok(statusOutput.includes('Server version: ripr 0.6.0-test'));
    } finally {
      await context.dispose();
    }
  });

  test('status model projects existing receipt state without producing receipts', async () => {
    await withControllerTestContext({
      files: {
        'target/ripr/reports/first-useful-action.json': firstActionReport({}),
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.showStatus();
      const statusOutput = context.outputLines.join('\n');
      assert.ok(statusOutput.includes('Receipt status: missing; no matching receipt was found for seam 67fc764ba37d77bd'));
      assert.ok(statusOutput.includes('Receipt command: ripr agent receipt --root . --json'));
      assert.ok(statusOutput.includes('No receipt movement is claimed.'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/first-useful-action.json': firstActionReport({}),
        'target/ripr/agent/agent-receipt.json': agentReceipt({ movement: 'improved' })
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.showStatus();
      const statusOutput = context.outputLines.join('\n');
      assert.ok(statusOutput.includes('Receipt status: movement improved; matching receipt found for seam 67fc764ba37d77bd'));
      assert.ok(statusOutput.includes('does not prove runtime adequacy or gate eligibility'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/first-useful-action.json': firstActionReport({}),
        'target/ripr/agent/agent-receipt.json': agentReceipt({ movement: 'unchanged' })
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.showStatus();
      const statusOutput = context.outputLines.join('\n');
      assert.ok(statusOutput.includes('Receipt status: movement unchanged; matching receipt found for seam 67fc764ba37d77bd'));
      assert.ok(statusOutput.includes('inspect the focused test and missing discriminator'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/first-useful-action.json': firstActionReport({}),
        'target/ripr/agent/agent-receipt.json': agentReceipt({ seamId: 'different-seam' })
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.showStatus();
      const statusOutput = context.outputLines.join('\n');
      assert.ok(statusOutput.includes('Receipt status: gap mismatch; receipt seam different-seam does not match current seam 67fc764ba37d77bd'));
      assert.ok(statusOutput.includes('Receipt movement is not projected.'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/first-useful-action.json': firstActionReport({
          generated_at: '2026-05-15T12:00:00Z'
        }),
        'target/ripr/agent/agent-receipt.json': agentReceipt({
          generatedAt: '2026-05-15T11:00:00Z',
          movement: 'improved'
        })
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.showStatus();
      const statusOutput = context.outputLines.join('\n');
      assert.ok(statusOutput.includes('Receipt status: stale; receipt for seam 67fc764ba37d77bd is older than the current first useful action report.'));
      assert.ok(statusOutput.includes('rerun verify/receipt before trusting movement'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/first-useful-action.json': firstActionReport({}),
        'target/ripr/agent/agent-receipt.json': agentReceipt({ repoRoot: '/tmp/not-this-workspace' })
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.showStatus();
      const statusOutput = context.outputLines.join('\n');
      assert.ok(statusOutput.includes('Receipt status: wrong root; receipt root /tmp/not-this-workspace does not match this workspace.'));
      assert.ok(statusOutput.includes('Receipt movement is not projected.'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/first-useful-action.json': firstActionReport({}),
        'target/ripr/agent/agent-receipt.json': '{not-json'
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.showStatus();
      const statusOutput = context.outputLines.join('\n');
      assert.ok(statusOutput.includes('Receipt status: malformed; target/ripr/agent/agent-receipt.json could not be parsed as an agent receipt.'));
      assert.ok(statusOutput.includes('Receipt movement is not projected.'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });
  });

  test('first-pr packet validation is read-only and fail-closed', async () => {
    const workspaceRoot = path.resolve('first-pr-workspace');

    const missing = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {}));
    assert.strictEqual(missing.state, 'missing');
    assert.strictEqual(missing.relativePath, 'target/ripr/reports/start-here.json');

    const repairable = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/first-pr/start-here.json': firstPrPacket({})
    }));
    assert.strictEqual(repairable.state, 'topRepairableGap');
    assert.strictEqual(repairable.relativePath, 'target/ripr/first-pr/start-here.json');
    assert.strictEqual(repairable.gapId, 'gap:pr:pricing:threshold-boundary');
    assert.strictEqual(repairable.canonicalGapId, 'gap:rust:pricing:discount:threshold-boundary');
    assert.strictEqual(repairable.verifyCommand, 'cargo xtask fixtures boundary_gap');
    assert.strictEqual(repairable.relatedTest, 'tests/pricing.rs::premium_customer_gets_discount');

    const noAction = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/reports/start-here.json': firstPrPacket({
        status: 'no_action',
        selected: {
          state: 'empty_diff',
          reason: 'The PR diff is empty.'
        },
        commands: {}
      })
    }));
    assert.strictEqual(noAction.state, 'noAction');
    assert.strictEqual(noAction.selectedState, 'empty_diff');

    const blocked = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/reports/start-here.json': firstPrPacket({
        status: 'blocked',
        selected: {
          state: 'blocked_artifact',
          message: 'The gap decision ledger is blocked.',
          next_command: 'ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json'
        },
        commands: {
          next: 'ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json'
        }
      })
    }));
    assert.strictEqual(blocked.state, 'blocked');

    const malformed = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/reports/start-here.json': '{not-json'
    }));
    assert.strictEqual(malformed.state, 'malformed');

    const unsupported = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/reports/start-here.json': firstPrPacket({ kind: 'first_useful_action' })
    }));
    assert.strictEqual(unsupported.state, 'unsupportedSchema');

    const unsupportedSelectedState = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/reports/start-here.json': firstPrPacket({
        selected: {
          state: 'future_state'
        }
      })
    }));
    assert.strictEqual(unsupportedSelectedState.state, 'unsupportedSchema');

    const wrongRoot = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/reports/start-here.json': firstPrPacket({ root: '../other-workspace' })
    }));
    assert.strictEqual(wrongRoot.state, 'wrongRoot');

    const workspaceWithSpaces = path.resolve('first-pr workspace with spaces');
    const equivalentRoot = await readFirstPrPacketStatus(workspaceWithSpaces, firstPrReadFile(workspaceWithSpaces, {
      'target/ripr/reports/start-here.json': firstPrPacket({ root: path.join(workspaceWithSpaces, '.') })
    }));
    assert.strictEqual(equivalentRoot.state, 'topRepairableGap');

    if (process.platform === 'win32') {
      const windowsRoot = path.resolve('first-pr-windows-root');
      const windowsNormalizedRoot = await readFirstPrPacketStatus(windowsRoot, firstPrReadFile(windowsRoot, {
        'target/ripr/reports/start-here.json': firstPrPacket({ root: windowsRoot.toUpperCase() })
      }));
      assert.strictEqual(windowsNormalizedRoot.state, 'topRepairableGap');
    }

    const unsafeCommand = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/reports/start-here.json': firstPrPacket({
        commands: {
          verify: 'cargo xtask fixtures boundary_gap; rm -rf target'
        }
      })
    }));
    assert.strictEqual(unsafeCommand.state, 'unsafeCommand');

    const unsafePathPacket = JSON.parse(firstPrPacket({})) as Record<string, unknown>;
    unsafePathPacket.selected = {
      ...(unsafePathPacket.selected as Record<string, unknown>),
      repair: {
        related_test: '../outside.rs::test_escape',
        route: 'AddBoundaryAssertion',
        target_file: 'tests/pricing.rs'
      }
    };
    const unsafePath = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/reports/start-here.json': JSON.stringify(unsafePathPacket)
    }));
    assert.strictEqual(unsafePath.state, 'unsafePath');

    const unsafeInputPath = await readFirstPrPacketStatus(workspaceRoot, firstPrReadFile(workspaceRoot, {
      'target/ripr/reports/start-here.json': firstPrPacket({
        inputs: {
          gap_ledger: '../gap-decision-ledger.json'
        }
      })
    }));
    assert.strictEqual(unsafeInputPath.state, 'unsafePath');
  });

  test('status model projects first-pr packet state without producing packets', async () => {
    await withControllerTestContext({}, async (context) => {
      await context.controller.start();
      const statusOutput = await showStatusReport(context);
      assert.ok(statusOutput.includes('First PR packet: missing; target/ripr/reports/start-here.json was not found.'));
      assert.ok(statusOutput.includes('Next safe first-pr action: run cargo xtask first-pr'));
      const diagnosis = await diagnoseSetupReport(context);
      assert.ok(diagnosis.includes('First PR packet: missing; target/ripr/reports/start-here.json was not found.'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/first-pr/start-here.json': firstPrPacket({})
      }
    }, async (context) => {
      await context.controller.start();
      const statusOutput = await showStatusReport(context);
      assert.ok(statusOutput.includes('First PR packet: top repairable gap available; target/ripr/first-pr/start-here.json is advisory.'));
      assert.ok(statusOutput.includes('Packet: target/ripr/first-pr/start-here.md'));
      assert.ok(statusOutput.includes('Gap identity: gap:rust:pricing:discount:threshold-boundary'));
      assert.ok(statusOutput.includes('Verify: cargo xtask fixtures boundary_gap'));
      assert.ok(statusOutput.includes('Receipt: ripr agent receipt --root . --json'));
      assert.ok(statusOutput.includes('does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status'));
      const diagnosis = await diagnoseSetupReport(context);
      assert.ok(diagnosis.includes('First PR packet: top repairable gap available; target/ripr/first-pr/start-here.json is advisory.'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': firstPrPacket({
          status: 'no_action',
          selected: {
            state: 'empty_diff',
            reason: 'The PR diff is empty.'
          },
          commands: {}
        })
      }
    }, async (context) => {
      await context.controller.start();
      const statusOutput = await showStatusReport(context);
      assert.ok(statusOutput.includes('First PR packet: no actionable gap; target/ripr/reports/start-here.json reports empty_diff.'));
      assert.ok(statusOutput.includes('No local first-pr repair action is projected from this packet.'));
      assert.ok(statusOutput.includes('No-action first-pr state does not prove runtime adequacy'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': firstPrPacket({
          status: 'blocked',
          selected: {
            state: 'stale_artifact',
            message: 'The gap decision ledger is stale.',
            next_command: 'cargo xtask first-pr'
          },
          commands: {
            next: 'cargo xtask first-pr'
          }
        })
      }
    }, async (context) => {
      await context.controller.start();
      const statusOutput = await showStatusReport(context);
      assert.ok(statusOutput.includes('First PR packet: stale; target/ripr/reports/start-here.json reports stale upstream evidence.'));
      assert.ok(statusOutput.includes('First PR packet repair claims are suppressed.'));
      assert.ok(!statusOutput.includes('top repairable gap available'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': firstPrPacket({})
      }
    }, async (context) => {
      await context.controller.start();
      const document = await vscode.workspace.openTextDocument(workspaceFileUri('src/lib.rs'));
      context.controller.markWorkspaceStale(document);
      const statusOutput = await showStatusReport(context);
      assert.ok(statusOutput.includes('First PR packet: stale; target/ripr/reports/start-here.json exists, but editor evidence is stale.'));
      assert.ok(statusOutput.includes('Refresh saved-workspace evidence and rerun cargo xtask first-pr before inspecting or copying first-pr packet content.'));
      assert.ok(!statusOutput.includes('top repairable gap available'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': firstPrPacket({ root: '../other-workspace' })
      }
    }, async (context) => {
      await context.controller.start();
      const statusOutput = await showStatusReport(context);
      assert.ok(statusOutput.includes('First PR packet: wrong root; packet root ../other-workspace does not match this workspace.'));
      assert.ok(statusOutput.includes('Expected workspace root:'));
      assert.ok(statusOutput.includes('First PR packet repair claims are suppressed.'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': '{not-json'
      }
    }, async (context) => {
      await context.controller.start();
      const statusOutput = await showStatusReport(context);
      assert.ok(statusOutput.includes('First PR packet: malformed; target/ripr/reports/start-here.json could not be parsed as a first-pr packet.'));
      assert.ok(statusOutput.includes('First PR packet repair claims are suppressed.'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': firstPrPacket({
          commands: {
            verify: 'powershell -NoProfile -Command Get-ChildItem'
          }
        })
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.copyFirstPrVerifyCommand();
      assert.strictEqual(context.clipboardWrites.length, 0);
      assert.ok(context.infoMessages.at(-1)?.includes('unsafe command'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': firstPrPacket({
          commands: {
            verify: 'cargo xtask fixtures boundary_gap; rm -rf target'
          }
        })
      }
    }, async (context) => {
      await context.controller.start();
      const statusOutput = await showStatusReport(context);
      assert.ok(statusOutput.includes('First PR packet: unsafe command; target/ripr/reports/start-here.json contains a command payload outside the editor safety contract.'));
      assert.ok(statusOutput.includes('Copy-command first-pr packet actions are suppressed.'));
      assert.ok(!statusOutput.includes('top repairable gap available'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });
  });

  test('editor adoption baseline keeps setup receipt and first-pr projection read-only', async () => {
    await withControllerTestContext({
      files: {
        'ripr.toml': '[languages]\nenabled = ["rust"]\n',
        'target/ripr/reports/first-useful-action.json': firstActionReport({}),
        'target/ripr/agent/agent-receipt.json': agentReceipt({ movement: 'improved' }),
        'target/ripr/first-pr/start-here.json': firstPrPacket({}),
        'target/ripr/first-pr/start-here.md': '# RIPR first-pr packet\n'
      }
    }, async (context) => {
      await context.controller.start();
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=8, diagnostics=1, files=1, findings=1, preview_findings=0, static_limits=0, seam_diagnostics=1, gap_artifacts=1, actionable_gap_artifacts=1, preview_gap_artifacts=0, no_action_gap_artifacts=0, gap_static_limits=0, gap_artifact_rejections=0, gap_artifact_rejection_kinds=, enabled_languages=1, enabled_language_names=rust, published_files=1, cleared_files=0'
      });

      const diagnosis = await diagnoseSetupReport(context);
      assertReportIncludes(diagnosis, [
        'ripr setup diagnosis:',
        'Server command: ripr',
        'Server version: ripr 0.6.0-test',
        'Config: ripr.toml (found',
        'Enabled languages: rust',
        'ripr validated 1 actionable gap artifact.',
        'Next safe action: Open the related test or copy a bounded repair packet',
        'Receipt status: movement improved; matching receipt found for seam 67fc764ba37d77bd',
        'First PR packet: top repairable gap available; target/ripr/first-pr/start-here.json is advisory.'
      ]);

      const statusOutput = await showStatusReport(context);
      assertReportIncludes(statusOutput, [
        'ripr validated 1 actionable gap artifact.',
        'Artifact first useful action report: target/ripr/reports/first-useful-action.json (found',
        'Receipt status: movement improved; matching receipt found for seam 67fc764ba37d77bd',
        'First PR packet: top repairable gap available; target/ripr/first-pr/start-here.json is advisory.',
        'Verify: cargo xtask fixtures boundary_gap',
        'Receipt: ripr agent receipt --root . --json',
        'does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status'
      ]);
      assert.strictEqual(context.runRiprCalls.length, 0);
    });
  });

  test('first-pr packet actions copy bounded payloads for matching diagnostics', async () => {
    await withControllerTestContext({
      files: {
        'target/ripr/first-pr/start-here.json': firstPrPacket({}),
        'target/ripr/first-pr/start-here.md': '# RIPR first-pr packet\n'
      }
    }, async (context) => {
      await context.controller.start();

      await context.controller.copyFirstPrSummary();
      assert.ok(context.clipboardWrites.at(-1)?.includes('RIPR first-pr summary'));
      assert.ok(context.clipboardWrites.at(-1)?.includes('Gap identity: gap:rust:pricing:discount:threshold-boundary'));
      assert.ok(context.clipboardWrites.at(-1)?.includes('Does not prove runtime adequacy'));

      await withCurrentFirstPrDiagnostic({
        canonical_gap_id: 'gap:rust:pricing:discount:threshold-boundary',
        gap_id: 'gap:pr:pricing:threshold-boundary'
      }, async () => {
        await context.controller.copyFirstPrRepairPacket();
        const repairPacket = context.clipboardWrites.at(-1) ?? '';
        assert.ok(repairPacket.includes('RIPR first-pr repair packet'), repairPacket);
        assert.ok(repairPacket.includes('Repair route: AddBoundaryAssertion'), repairPacket);
        assert.ok(repairPacket.includes('Related test: tests/pricing.rs::premium_customer_gets_discount'), repairPacket);
        assert.ok(repairPacket.includes('Do not broaden scope.'), repairPacket);

        await context.controller.copyFirstPrVerifyCommand();
        assert.strictEqual(context.clipboardWrites.at(-1), 'cargo xtask fixtures boundary_gap');

        await context.controller.copyFirstPrReceiptCommand();
        assert.strictEqual(context.clipboardWrites.at(-1), 'ripr agent receipt --root . --json');
      });

      assert.strictEqual(context.runRiprCalls.length, 0);
    });
  });

  test('first-pr packet actions open only safe workspace-local markdown packets', async () => {
    await writeWorkspaceFile('target/ripr/first-pr/start-here.md', '# RIPR first-pr packet\n');
    try {
      await withControllerTestContext({
        files: {
          'target/ripr/first-pr/start-here.json': firstPrPacket({}),
          'target/ripr/first-pr/start-here.md': '# RIPR first-pr packet\n'
        }
      }, async (context) => {
        await context.controller.start();
        await context.controller.openFirstPrPacket();

        const activeEditor = vscode.window.activeTextEditor;
        assert.ok(activeEditor, 'expected first-pr Markdown packet to open');
        assert.ok(
          activeEditor.document.uri.fsPath.replace(/\\/g, '/').endsWith('/target/ripr/first-pr/start-here.md'),
          activeEditor.document.uri.fsPath
        );
        assert.ok(context.infoMessages.at(-1)?.includes('Opened ripr first-pr packet'));
        assert.strictEqual(context.runRiprCalls.length, 0);
      });
    } finally {
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await removeWorkspacePath('target/ripr/first-pr/start-here.md');
    }
  });

  test('first-pr packet diagnostic actions fail closed on mismatched or stale evidence', async () => {
    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': firstPrPacket({})
      }
    }, async (context) => {
      await context.controller.start();

      await withCurrentFirstPrDiagnostic({
        canonical_gap_id: 'gap:rust:other',
        gap_id: 'gap:pr:other'
      }, async () => {
        await context.controller.copyFirstPrRepairPacket();
      });
      assert.strictEqual(context.clipboardWrites.length, 0);
      assert.ok(context.infoMessages.at(-1)?.includes('does not match the first-pr packet gap identity'));

      const document = await vscode.workspace.openTextDocument(workspaceFileUri('src/lib.rs'));
      await vscode.window.showTextDocument(document);
      context.controller.markWorkspaceStale(document);
      await context.controller.copyFirstPrSummary();
      assert.strictEqual(context.clipboardWrites.length, 0);
      assert.ok(context.infoMessages.at(-1)?.includes('current saved-workspace evidence'));

      await context.controller.copyFirstPrRegenerationGuidance();
      assert.ok(context.clipboardWrites.at(-1)?.includes('cargo xtask first-pr'));
      assert.ok(context.clipboardWrites.at(-1)?.includes('editor does not run the command'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });
  });

  test('first-pr packet actions suppress unsafe, malformed, and missing packet states', async () => {
    await withControllerTestContext({}, async (context) => {
      await context.controller.start();
      await context.controller.copyFirstPrSummary();
      assert.strictEqual(context.clipboardWrites.length, 0);
      assert.ok(context.infoMessages.at(-1)?.includes('first-pr packet is missing'));

      await context.controller.copyFirstPrRegenerationGuidance();
      assert.ok(context.clipboardWrites.at(-1)?.includes('cargo xtask first-pr'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': '{not-json'
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.copyFirstPrSummary();
      assert.strictEqual(context.clipboardWrites.length, 0);
      assert.ok(context.infoMessages.at(-1)?.includes('malformed or unsupported'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': firstPrPacket({ root: '../other-workspace' })
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.openFirstPrPacket();
      assert.strictEqual(context.clipboardWrites.length, 0);
      assert.ok(context.infoMessages.at(-1)?.includes('belongs to another workspace'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      files: {
        'target/ripr/reports/start-here.json': firstPrPacket({
          commands: {
            verify: 'cargo xtask fixtures boundary_gap; rm -rf target'
          }
        })
      }
    }, async (context) => {
      await context.controller.start();
      await context.controller.copyFirstPrVerifyCommand();
      assert.strictEqual(context.clipboardWrites.length, 0);
      assert.ok(context.infoMessages.at(-1)?.includes('unsafe command'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });
  });

  test('diagnoseSetup writes read-only setup report', async () => {
    const context = createControllerTestContext({
      files: {
        'ripr.toml': '[languages]\nenabled = ["rust"]\n',
        'target/ripr/reports/first-useful-action.json': firstActionReport({}),
        'target/ripr/reports/gap-decision-ledger.json': '{"schema_version":"0.1"}',
        'target/ripr/agent/agent-receipt.json': '{"schema_version":"0.1"}'
      }
    });
    try {
      await context.controller.start();
      await context.controller.diagnoseSetup();

      const report = context.outputLines.join('\n');
      assert.ok(report.includes('ripr setup diagnosis:'));
      assert.ok(report.includes('Status: ripr saved-workspace analysis is queued.'));
      assert.ok(report.includes('ripr server state: ripr_version_ok (ripr 0.6.0-test)'));
      assert.ok(report.includes('Workspace trust state: workspace_trusted'));
      assert.ok(report.includes('Config state: config_found'));
      assert.ok(report.includes('Artifact directory state: artifact_dir_present'));
      assert.ok(report.includes('Server version: ripr 0.6.0-test'));
      assert.ok(report.includes('Config: ripr.toml (found; found in current workspace'));
      assert.ok(report.includes('Artifact gap decision ledger: target/ripr/reports/gap-decision-ledger.json (found'));
      assert.ok(report.includes('First useful action: Add equality-boundary discriminator test'));
      assert.ok(report.includes('Limits: read-only setup diagnosis only'));
      assert.ok(context.infoMessages.at(-1)?.includes('setup diagnosis'));
    } finally {
      await context.dispose();
    }
  });

  test('diagnoseSetup distinguishes first-run and no-output states', async () => {
    await withControllerTestContext({ workspaceRoot: null }, async (context) => {
      await context.controller.start();
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: Open a workspace for ripr diagnostics.',
        'Workspace: not open',
        'Server started: no; workspace unavailable',
        'Config: ripr.toml (no workspace',
        'Next safe action: Open a workspace folder'
      ]);
      assert.strictEqual(context.client.startCalls, 0);
    });

    await withControllerTestContext({
      resolveFailure: {
        message: 'Configured ripr.server.path does not exist.',
        detail: 'Missing configured ripr server path for this test.'
      }
    }, async (context) => {
      await context.controller.start();
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: ripr server is not available.',
        'Missing configured ripr server path for this test.',
        'Server: not resolved',
        'Server started: no; server unavailable',
        'Next safe action: Set ripr.server.path'
      ]);
      assert.strictEqual(context.client.startCalls, 0);
    });

    await withControllerTestContext({}, async (context) => {
      await context.controller.start();
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: ripr saved-workspace analysis is queued.',
        'Server command: ripr',
        'ripr server state: ripr_version_ok (ripr 0.6.0-test)',
        'Server version: ripr 0.6.0-test',
        'Config: ripr.toml (missing',
        'Artifact first useful action report: target/ripr/reports/first-useful-action.json (missing',
        'Evidence freshness: pending refresh'
      ]);
    });

    await withControllerTestContext({
      files: {
        'ripr.toml': '[languages]\nenabled = ["rust"]\n'
      }
    }, async (context) => {
      await context.controller.start();
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=1, diagnostics=0, files=0, findings=0, seam_diagnostics=0, enabled_languages=1, enabled_language_names=rust, published_files=0, cleared_files=0'
      });
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: ripr analysis completed with no actionable seam diagnostics.',
        'Config: ripr.toml (found',
        'Enabled languages: rust',
        'No ripr seam diagnostics were published',
        'disabled or unavailable preview languages stay silent'
      ]);
    });

    await withControllerTestContext({
      files: {
        'ripr.toml': '[languages]\nenabled = []\n'
      }
    }, async (context) => {
      await context.controller.start();
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=2, diagnostics=0, files=0, findings=0, seam_diagnostics=0, enabled_languages=0, enabled_language_names=, published_files=0, cleared_files=0'
      });
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: ripr analysis completed with no enabled languages.',
        'Enabled languages: none',
        '[languages] enabled = []',
        'Next safe action: Edit ripr.toml [languages] enabled'
      ]);
    });

    await withControllerTestContext({}, async (context) => {
      await context.controller.start();
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=3, diagnostics=0, files=0, findings=0, seam_diagnostics=0, gap_artifacts=0, actionable_gap_artifacts=0, preview_gap_artifacts=0, no_action_gap_artifacts=0, gap_static_limits=0, gap_artifact_rejections=1, gap_artifact_rejection_kinds=unavailable_language, enabled_languages=1, enabled_language_names=rust, published_files=0, cleared_files=0'
      });
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: ripr ignored 1 unsafe gap artifact input.',
        'Rejected kind: unavailable_language',
        'not projected',
        'never create diagnostics'
      ]);
    });

    await withControllerTestContext({
      firstActionJson: firstActionReport({})
    }, async (context) => {
      await context.controller.start();
      const document = await vscode.workspace.openTextDocument(workspaceFileUri('src/lib.rs'));
      context.controller.markWorkspaceStale(document);
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: ripr analysis is stale until the file is saved.',
        'Evidence freshness: stale; save or refresh before acting',
        'First useful action report: available, but editor evidence is stale.',
        'Save or refresh the workspace before acting on this report.'
      ]);
    });

    await withControllerTestContext({}, async (context) => {
      await context.controller.start();
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=4, diagnostics=0, files=0, findings=0, preview_findings=0, static_limits=0, seam_diagnostics=0, gap_artifacts=1, actionable_gap_artifacts=0, preview_gap_artifacts=0, no_action_gap_artifacts=1, gap_static_limits=0, gap_artifact_rejections=0, gap_artifact_rejection_kinds=, enabled_languages=1, enabled_language_names=rust, published_files=0, cleared_files=0'
      });
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: ripr validated gap artifacts with no actionable gap.',
        'no local repair action',
        'Next safe action: No local repair action is projected'
      ]);
    });

    await withControllerTestContext({}, async (context) => {
      await context.controller.start();
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=5, diagnostics=2, files=1, findings=1, preview_findings=0, static_limits=0, seam_diagnostics=1, gap_artifacts=1, actionable_gap_artifacts=1, preview_gap_artifacts=0, no_action_gap_artifacts=0, gap_static_limits=0, gap_artifact_rejections=0, gap_artifact_rejection_kinds=, enabled_languages=1, enabled_language_names=rust, published_files=1, cleared_files=0'
      });
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: ripr validated 1 actionable gap artifact.',
        '1 actionable gap artifact validated for editor projection.',
        'Next safe action: Open the related test or copy a bounded repair packet'
      ]);
    });
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
        audience: 'developer',
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
      await context.controller.showStatus();
      assert.ok(!context.outputLines.join('\n').includes('First useful action:'));
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
        audience: 'developer',
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

      await context.controller.showStatus();
      const output = context.outputLines.join('\n');
      assert.ok(output.includes('First useful action report: available, but editor evidence is stale.'));
      assert.ok(output.includes('Save or refresh the workspace before acting on this report.'));
      assert.ok(output.includes('Report: target/ripr/reports/first-useful-action.json'));
      assert.ok(!context.infoMessages.at(-1)?.includes('First useful action:'));
    } finally {
      await context.dispose();
    }
  });

  test('first useful action report fails closed for unsupported or incomplete JSON', async () => {
    const invalidReports: Array<{ name: string; firstActionJson?: string }> = [
      { name: 'missing report' },
      { name: 'invalid JSON', firstActionJson: '{' },
      { name: 'wrong kind', firstActionJson: firstActionReport({ kind: 'pr_review_front_panel' }) },
      { name: 'missing kind', firstActionJson: firstActionReport({ kind: undefined }) },
      { name: 'unsupported schema', firstActionJson: firstActionReport({ schema_version: '9.9' }) },
      { name: 'missing schema', firstActionJson: firstActionReport({ schema_version: undefined }) },
      { name: 'missing status', firstActionJson: firstActionReport({ status: undefined }) },
      { name: 'unknown status', firstActionJson: firstActionReport({ status: 'unknown_status' }) },
      { name: 'missing action kind', firstActionJson: firstActionReport({ action_kind: undefined }) },
      { name: 'unknown action kind', firstActionJson: firstActionReport({ action_kind: 'run_mutation' }) },
      { name: 'missing audience', firstActionJson: firstActionReport({ audience: undefined }) },
      { name: 'unknown audience', firstActionJson: firstActionReport({ audience: 'model' }) },
      { name: 'missing title', firstActionJson: firstActionReport({ title: undefined }) },
    ];

    for (const report of invalidReports) {
      const context = createControllerTestContext({
        firstActionJson: report.firstActionJson
      });
      try {
        await context.controller.start();

        assert.ok(
          context.status.text.includes('ripr: queued'),
          `${report.name} should keep the normal queued status`
        );
        assert.ok(
          !String(context.status.tooltip).includes('First useful action'),
          `${report.name} should not project first useful action details`
        );
        await context.controller.showStatus();
        assert.ok(
          !context.infoMessages.at(-1)?.includes('First useful action:'),
          `${report.name} should not include first useful action in Show Status`
        );
        assert.ok(
          !context.outputLines.join('\n').includes('First useful action:'),
          `${report.name} should not write first useful action detail to Show Status output`
        );
      } finally {
        await context.dispose();
      }
    }
  });

  test('first useful action status projection covers fallback statuses', async () => {
    const cases = [
      {
        status: 'stale',
        actionKind: 'refresh_evidence',
        icon: '$(warning)',
        title: 'Refresh stale evidence before acting'
      },
      {
        status: 'missing_required_artifact',
        actionKind: 'generate_missing_artifact',
        icon: '$(warning)',
        title: 'Generate the missing first-action input'
      },
      {
        status: 'unchanged_after_attempt',
        actionKind: 'revise_focused_test',
        icon: '$(warning)',
        title: 'Revise the focused test'
      },
      {
        status: 'baseline_only',
        actionKind: 'acknowledge_baseline',
        icon: '$(pass)',
        title: 'Acknowledge baseline debt'
      },
      {
        status: 'already_improved',
        actionKind: 'no_action',
        icon: '$(pass)',
        title: 'Static evidence already improved'
      },
      {
        status: 'no_actionable_seam',
        actionKind: 'no_action',
        icon: '$(pass)',
        title: 'No actionable seam'
      },
      {
        status: 'waived',
        actionKind: 'no_action',
        icon: '$(pass)',
        title: 'Waived by existing review state'
      },
      {
        status: 'suppressed',
        actionKind: 'no_action',
        icon: '$(pass)',
        title: 'Suppressed by repo policy'
      },
      {
        status: 'acknowledged',
        actionKind: 'acknowledge_baseline',
        icon: '$(pass)',
        title: 'Acknowledged for this review'
      },
    ];

    for (const entry of cases) {
      const context = createControllerTestContext({
        firstActionJson: firstActionReport({
          status: entry.status,
          action_kind: entry.actionKind,
          title: entry.title
        })
      });
      try {
        await context.controller.start();

        assert.ok(context.status.text.includes(entry.icon), `${entry.status} should use ${entry.icon}`);
        assert.ok(context.status.text.includes('ripr: first action'));
        assert.ok(String(context.status.tooltip).includes(`Status: ${entry.status}`));
        assert.ok(String(context.status.tooltip).includes(`Action: ${entry.actionKind}`));
        assert.ok(String(context.status.tooltip).includes(entry.title));
      } finally {
        await context.dispose();
      }
    }
  });

  test('status bar reports disabled configuration without starting server', async () => {
    const context = createControllerTestContext({ enabled: false });
    try {
      await context.controller.start();

      assert.ok(context.status.text.includes('ripr: disabled'));
      assert.ok(String(context.status.tooltip).includes('Set ripr.enabled to true'));
      assert.ok(String(context.status.tooltip).includes('Workspace:'));
      assert.ok(String(context.status.tooltip).includes('Server: not resolved'));
      assert.ok(String(context.status.tooltip).includes('Server started: no; extension disabled'));
      assert.ok(String(context.status.tooltip).includes('Config: ripr.toml'));
      assert.ok(String(context.status.tooltip).includes('Next safe action: Set ripr.enabled to true'));
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
      assert.ok(String(context.status.tooltip).includes('Workspace: not open'));
      assert.ok(String(context.status.tooltip).includes('Config: ripr.toml (no workspace'));
      assert.ok(String(context.status.tooltip).includes('Server started: no; workspace unavailable'));
      assert.ok(String(context.status.tooltip).includes('Next safe action: Open a workspace folder'));
      assert.strictEqual(context.client.startCalls, 0);
    } finally {
      await context.dispose();
    }
  });

  test('status bar reports ambiguous multi-root without starting server', async () => {
    const roots = [path.resolve('multi-root-a'), path.resolve('multi-root-b')];
    const context = createControllerTestContext({
      workspaceRootState: {
        kind: 'ambiguousMultiRoot',
        roots,
        detail: 'test multi-root ambiguity'
      }
    });
    try {
      await context.controller.start();

      assert.ok(context.status.text.includes('ripr: select root'));
      assert.ok(String(context.status.tooltip).includes('workspace_multi_root_ambiguous'));
      assert.ok(String(context.status.tooltip).includes('Root-scoped repair actions are suppressed'));
      assert.ok(String(context.status.tooltip).includes('Server started: no; workspace root is ambiguous'));
      assert.strictEqual(context.client.startCalls, 0);

      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Status: Select one workspace folder before using ripr repair actions.',
        'Workspace root state: workspace_multi_root_ambiguous',
        'Root-scoped repair actions are suppressed until one workspace folder is selected.',
        'Next safe action: Open a Rust or enabled preview-language file from one workspace folder'
      ]);
    } finally {
      await context.dispose();
    }
  });

  test('status model reports explicit multi-root selection', async () => {
    const selectedRoot = path.resolve('selected workspace root');
    const otherRoot = path.resolve('other workspace root');
    const context = createControllerTestContext({
      workspaceRootState: {
        kind: 'selectedRoot',
        root: selectedRoot,
        roots: [selectedRoot, otherRoot],
        detail: 'selected from active editor workspace folder'
      },
      files: {
        'target/ripr/first-pr/start-here.json': firstPrPacket({})
      }
    });
    try {
      await context.controller.start();
      const statusOutput = await showStatusReport(context);

      assert.ok(statusOutput.includes('Workspace root state: workspace_multi_root_selected'));
      assert.ok(statusOutput.includes(selectedRoot));
      assert.ok(statusOutput.includes(otherRoot));
      assert.ok(statusOutput.includes('First PR packet: top repairable gap available; target/ripr/first-pr/start-here.json is advisory.'));
      assert.strictEqual(context.client.startCalls, 1);
      assertRiprDocumentSelectorsScopedToWorkspace(context.clientOptions(), selectedRoot);
      assertCargoTomlWatcherScopedToWorkspace(context.watcherPatterns, selectedRoot);
      assert.strictEqual(context.runRiprCalls.length, 0);
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
      assert.ok(String(context.status.tooltip).includes('Workspace:'));
      assert.ok(String(context.status.tooltip).includes('Server: not resolved'));
      assert.ok(String(context.status.tooltip).includes('Server started: no; server unavailable'));
      assert.ok(String(context.status.tooltip).includes('Config: ripr.toml'));
      assert.ok(String(context.status.tooltip).includes('Next safe action: Set ripr.server.path'));
      assert.strictEqual(context.errorMessages.length, 1);
      assert.strictEqual(context.client.startCalls, 0);
    } finally {
      await context.dispose();
    }
  });

  test('setup diagnosis reports compatibility states and blocks unsafe repair start', async () => {
    await withControllerTestContext({
      serverVersion: 'ripr 0.5.0-test'
    }, async (context) => {
      await context.controller.start();
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Extension state: extension_version_ok (0.6.0)',
        'Expected ripr server version: 0.6.0',
        'ripr server state: ripr_version_too_old (reported ripr 0.5.0-test; expected 0.6.0)',
        'Workspace trust state: workspace_trusted',
        'Config state: config_missing',
        'Artifact directory state: artifact_dir_missing'
      ]);

      await withCurrentFirstPrDiagnostic({
        gap_id: 'gap:pr:pricing:threshold-boundary',
        canonical_gap_id: 'gap:rust:pricing:discount:threshold-boundary'
      }, async () => {
        await context.controller.startCurrentRepair();
      });
      assert.ok(context.infoMessages.at(-1)?.includes('ripr_version_too_old'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });

    await withControllerTestContext({
      workspaceTrusted: false
    }, async (context) => {
      await context.controller.start();
      const report = await diagnoseSetupReport(context);
      assertReportIncludes(report, [
        'Workspace trust state: workspace_untrusted',
        'ripr server state: ripr_version_ok (ripr 0.6.0-test)'
      ]);

      await withCurrentFirstPrDiagnostic({
        gap_id: 'gap:pr:pricing:threshold-boundary',
        canonical_gap_id: 'gap:rust:pricing:discount:threshold-boundary'
      }, async () => {
        await context.controller.startCurrentRepair();
      });
      assert.ok(context.infoMessages.at(-1)?.includes('workspace_untrusted'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });
  });

  test('language client registers workspace-scoped Rust default and preview document selectors', async () => {
    const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    assert.ok(workspaceRoot, 'test workspace should be open');
    const context = createControllerTestContext({});
    try {
      await context.controller.start();

      assertRiprDocumentSelectorsScopedToWorkspace(context.clientOptions(), workspaceRoot);
      assertCargoTomlWatcherScopedToWorkspace(context.watcherPatterns, workspaceRoot);
    } finally {
      await context.dispose();
    }
  });

  test('first-pr diagnostic commands fail closed when active file is outside selected root', async () => {
    const selectedRoot = path.resolve('selected workspace root');
    const otherRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    assert.ok(otherRoot, 'test workspace should be open');
    await withControllerTestContext({
      workspaceRootState: {
        kind: 'selectedRoot',
        root: selectedRoot,
        roots: [selectedRoot, otherRoot],
        detail: 'selected from active editor workspace folder'
      },
      files: {
        'target/ripr/first-pr/start-here.json': firstPrPacket({ root: selectedRoot })
      }
    }, async (context) => {
      await context.controller.start();

      await withCurrentFirstPrDiagnostic({
        canonical_gap_id: 'gap:rust:pricing:discount:threshold-boundary',
        gap_id: 'gap:pr:pricing:threshold-boundary'
      }, async () => {
        await context.controller.copyFirstPrRepairPacket();
        await context.controller.copyFirstPrVerifyCommand();
        await context.controller.copyFirstPrReceiptCommand();
      });

      assert.strictEqual(context.clipboardWrites.length, 0);
      assert.ok(context.infoMessages.every((message) =>
        message.includes('different workspace root')
      ), context.infoMessages.join('\n'));
      assert.strictEqual(context.runRiprCalls.length, 0);
    });
  });

  test('status bar reports stale saved-workspace analysis after routed file edits', async () => {
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

  test('preview-language edits mark stale status while unsupported files are ignored', async () => {
    const context = createControllerTestContext({});
    try {
      await context.controller.start();
      const pythonDocument = textDocument('python', workspaceFileUri('src/preview.py'));

      context.controller.markWorkspaceStale(pythonDocument);

      assert.ok(context.status.text.includes('ripr: stale'));
      assert.ok(String(context.status.tooltip).includes('src'));
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=6, diagnostics=2, files=1, findings=1, seam_diagnostics=1, published_files=1, cleared_files=0'
      });
      assert.ok(context.status.text.includes('ripr: stale'));
      assert.ok(String(context.status.tooltip).includes('Unsaved routed files'));

      context.controller.markWorkspaceSaved(pythonDocument);
      assert.ok(context.status.text.includes('ripr: queued'));

      const markdownDocument = textDocument('markdown', workspaceFileUri('README.md'));
      context.controller.markWorkspaceStale(markdownDocument);
      assert.ok(context.status.text.includes('ripr: queued'));
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
    const relativePath = 'src/agent-loop-command.rs';
    const uri = workspaceFileUri(relativePath);
    const context = createControllerTestContext({});
    try {
      await writeWorkspaceFile(relativePath, 'pub fn agent_loop_command_target() {}\n');
      const document = await vscode.workspace.openTextDocument(uri);
      await vscode.window.showTextDocument(document);
      const seamId = '67fc764ba37d77bd';
      const targets = [
        agentLoopCommandTarget(
          'agent_packet',
          `ripr agent packet --root . --seam-id ${seamId} --json > target/ripr/agent/agent-packet.json`,
          'target/ripr/agent/agent-packet.json',
          { seamId }
        ),
        agentLoopCommandTarget(
          'agent_brief',
          `ripr agent brief --root . --seam-id ${seamId} --json > target/ripr/agent/agent-brief.json`,
          'target/ripr/agent/agent-brief.json',
          { seamId }
        ),
        agentLoopCommandTarget(
          'after_snapshot',
          'ripr check --root . --base "origin/main with space" --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json',
          'target/ripr/pilot/after.repo-exposure.json',
          { base: 'origin/main with space', mode: 'ready' }
        ),
        agentLoopCommandTarget(
          'agent_verify',
          'ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json',
          'target/ripr/agent/agent-verify.json'
        ),
        agentLoopCommandTarget(
          'agent_receipt',
          `ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id ${seamId} --json --out target/ripr/agent/agent-receipt.json`,
          'target/ripr/agent/agent-receipt.json',
          { seamId }
        ),
        agentLoopCommandTarget(
          'gap_verify',
          'ripr agent verify --root . --json'
        ),
        agentLoopCommandTarget(
          'gap_receipt',
          'ripr agent receipt --root . --json'
        )
      ];

      for (const target of targets) {
        await context.controller.copyAgentLoopCommand(target);
      }

      assert.deepStrictEqual(
        context.clipboardWrites,
        targets.map((target) => target.command)
      );
    } finally {
      await context.dispose();
      await vscode.commands.executeCommand('workbench.action.closeAllEditors');
      await removeWorkspacePath(relativePath);
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

  test('agent loop command handler rejects unsupported or unsafe payloads', async () => {
    const context = createControllerTestContext({});
    try {
      const valid = agentLoopCommandTarget(
        'agent_verify',
        'ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json > target/ripr/agent/agent-verify.json',
        'target/ripr/agent/agent-verify.json'
      );

      await context.controller.copyAgentLoopCommand({
        ...valid,
        label: 'unknown'
      });
      await context.controller.copyAgentLoopCommand({
        ...valid,
        root: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath
      });
      await context.controller.copyAgentLoopCommand({
        ...valid,
        target_artifact: 'target/ripr/other.json'
      });
      await context.controller.copyAgentLoopCommand({
        ...valid,
        command: `${valid.command}; rm -rf target`
      });
      await context.controller.copyAgentLoopCommand(
        agentLoopCommandTarget(
          'agent_packet',
          'ripr agent packet --root . --seam-id other-seam --json > target/ripr/agent/agent-packet.json',
          'target/ripr/agent/agent-packet.json',
          { seamId: '67fc764ba37d77bd' }
        )
      );
      await context.controller.copyAgentLoopCommand(
        agentLoopCommandTarget(
          'agent_receipt',
          'ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id 67fc764ba37d77bd --json --out target/ripr/agent/agent-receipt.json',
          'target/ripr/agent/agent-receipt.json'
        )
      );
      await context.controller.copyAgentLoopCommand(
        agentLoopCommandTarget(
          'after_snapshot',
          'ripr check --root . --mode ready --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json',
          'target/ripr/pilot/after.repo-exposure.json',
          { base: 'origin/main with space', mode: 'ready' }
        )
      );

      assert.deepStrictEqual(context.clipboardWrites, []);
    } finally {
      await context.dispose();
    }
  });

  test('openRelatedTest opens the target uri and line', async () => {
    const context = createControllerTestContext({});
    const uri = workspaceFileUri('tests/pricing.rs');
    try {
      await context.controller.start();
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=7, diagnostics=1, files=1, findings=1, seam_diagnostics=1, enabled_languages=1, enabled_language_names=rust'
      });
      await context.controller.openRelatedTest({
        uri: uri.toString(),
        line: 4,
        test_name: 'below_threshold_has_no_discount'
      });

      assert.strictEqual(vscode.window.activeTextEditor?.document.uri.toString(), uri.toString());
      assert.strictEqual(vscode.window.activeTextEditor?.selection.active.line, 3);
    } finally {
      await context.dispose();
    }
  });

  test('openRelatedTest rejects stale, disabled, non-workspace, and unsupported targets', async () => {
    const context = createControllerTestContext({});
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder, 'test workspace should be open');
    try {
      await context.controller.start();
      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=8, diagnostics=0, files=1, findings=0, seam_diagnostics=0, enabled_languages=0, enabled_language_names='
      });
      await context.controller.openRelatedTest({
        uri: workspaceFileUri('tests/pricing.rs').toString(),
        line: 4,
        test_name: 'below_threshold_has_no_discount'
      });
      assert.ok(context.infoMessages.at(-1)?.includes('language is disabled'));

      context.client.emitNotification('window/logMessage', {
        message: 'ripr analysis refresh completed in 42 ms: generation=9, diagnostics=1, files=1, findings=1, seam_diagnostics=1, enabled_languages=1, enabled_language_names=rust'
      });
      await context.controller.openRelatedTest({
        uri: workspaceFileUri('Cargo.toml').toString(),
        line: 1,
        test_name: 'manifest'
      });
      assert.ok(context.infoMessages.at(-1)?.includes('Rust, TypeScript/JavaScript, or Python file'));

      const outsideUri = vscode.Uri.file(path.join(folder.uri.fsPath, '..', 'outside.rs'));
      await context.controller.openRelatedTest({
        uri: outsideUri.toString(),
        line: 1,
        test_name: 'outside'
      });
      assert.ok(context.infoMessages.at(-1)?.includes('inside the current workspace'));

      await context.controller.openRelatedTest({
        uri: 'untitled:preview.py',
        line: 1,
        test_name: 'scratch'
      });
      assert.ok(context.infoMessages.at(-1)?.includes('requires a file URI'));

      const routedDocument = textDocument('rust', workspaceFileUri('tests/pricing.rs'));
      context.controller.markWorkspaceStale(routedDocument);
      await context.controller.openRelatedTest({
        uri: routedDocument.uri.toString(),
        line: 4,
        test_name: 'below_threshold_has_no_discount'
      });
      assert.ok(context.infoMessages.at(-1)?.includes('current saved-workspace analysis'));
    } finally {
      await context.dispose();
    }
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
  files?: Record<string, string | null>;
  workspaceRoot?: string | null;
  workspaceRootState?: RiprWorkspaceRootState;
  resolveFailure?: { message: string; detail: string };
  serverVersion?: string;
  workspaceTrusted?: boolean;
}

function agentLoopCommandTarget(
  label: string,
  command: string,
  targetArtifact?: string,
  options: { seamId?: string; base?: string; mode?: string } = {}
): RiprAgentLoopCommandTarget {
  return {
    label,
    command,
    root: '.',
    base: options.base ?? 'origin/main',
    mode: options.mode ?? 'draft',
    seam_id: options.seamId,
    target_artifact: targetArtifact
  };
}

function firstActionReport(overrides: Record<string, unknown>): string {
  const report: Record<string, unknown> = {
    schema_version: '0.1',
    tool: 'ripr',
    kind: 'first_useful_action',
    root: '.',
    generated_at: '2026-05-15T12:00:00Z',
    status: 'actionable',
    audience: 'developer',
    action_kind: 'write_focused_test',
    title: 'Add equality-boundary discriminator test',
    selected: {
      seam_id: '67fc764ba37d77bd',
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
  };
  for (const [key, value] of Object.entries(overrides)) {
    if (value === undefined) {
      delete report[key];
    } else {
      report[key] = value;
    }
  }
  return JSON.stringify(report);
}

function firstPrPacket(overrides: Record<string, unknown>): string {
  const packet: Record<string, unknown> = {
    schema_version: '0.1',
    tool: 'ripr',
    kind: 'first_pr_start_here',
    root: '.',
    status: 'actionable',
    posture: 'advisory',
    selected: {
      state: 'top_gap',
      gap_id: 'gap:pr:pricing:threshold-boundary',
      canonical_gap_id: 'gap:rust:pricing:discount:threshold-boundary',
      kind: 'MissingBoundaryAssertion',
      changed_behavior: 'amount >= threshold',
      why: 'A related Rust test reaches this change, but no equality-boundary assertion was found.',
      verify_command: 'cargo xtask fixtures boundary_gap',
      receipt_command: 'ripr agent receipt --root . --json',
      repair: {
        related_test: 'tests/pricing.rs::premium_customer_gets_discount',
        route: 'AddBoundaryAssertion',
        target_file: 'tests/pricing.rs',
        suggested_assertion: 'assert_eq!(discount(100, 100), 90)'
      }
    },
    commands: {
      verify: 'cargo xtask fixtures boundary_gap',
      receipt: 'ripr agent receipt --root . --json'
    },
    limits: [
      'Composes explicit RIPR artifacts only.',
      'Does not edit source or generate tests.'
    ],
    warnings: []
  };
  for (const [key, value] of Object.entries(overrides)) {
    if (value === undefined) {
      delete packet[key];
    } else {
      packet[key] = value;
    }
  }
  return JSON.stringify(packet);
}

function firstPrReadFile(
  workspaceRoot: string,
  files: Record<string, string | null>
): RiprClientRuntime['readFile'] {
  return async (filePath: string) => {
    const relativePath = path.relative(workspaceRoot, filePath).replace(/\\/g, '/');
    const value = files[relativePath];
    if (value === null) {
      throw new Error(`cannot read ${relativePath}`);
    }
    return value;
  };
}

async function withCurrentFirstPrDiagnostic(
  data: Record<string, string>,
  run: () => Promise<void>
): Promise<void> {
  const relativePath = 'src/first-pr-actions.rs';
  const uri = workspaceFileUri(relativePath);
  const collection = vscode.languages.createDiagnosticCollection('ripr-first-pr-actions');
  try {
    await writeWorkspaceFile(relativePath, [
      'pub fn discounted_total(amount: u32, threshold: u32) -> u32 {',
      '    if amount >= threshold {',
      '        amount - 10',
      '    } else {',
      '        amount',
      '    }',
      '}',
      ''
    ].join('\n'));
    const document = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(document);
    editor.selection = new vscode.Selection(new vscode.Position(1, 7), new vscode.Position(1, 7));
    const diagnostic = new vscode.Diagnostic(
      new vscode.Range(new vscode.Position(1, 4), new vscode.Position(1, 24)),
      'ripr first-pr bridge test diagnostic',
      vscode.DiagnosticSeverity.Warning
    );
    diagnostic.source = 'ripr';
    diagnostic.code = 'ripr-gap-MissingBoundaryAssertion';
    (diagnostic as unknown as { data?: unknown }).data = data;
    collection.set(uri, [diagnostic]);
    await run();
  } finally {
    collection.dispose();
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    await removeWorkspacePath(relativePath);
  }
}

function agentReceipt(overrides: {
  seamId?: string;
  movement?: string;
  repoRoot?: string;
  generatedAt?: string;
}): string {
  const seamId = overrides.seamId ?? '67fc764ba37d77bd';
  const movement = overrides.movement ?? 'unchanged';
  return JSON.stringify({
    schema_version: '0.3',
    tool: 'ripr',
    status: 'advisory',
    inputs: {
      agent_verify_json: 'target/ripr/agent/agent-verify.json',
      before: 'target/ripr/pilot/repo-exposure.json',
      after: 'target/ripr/pilot/after.repo-exposure.json'
    },
    provenance: {
      repo_root: overrides.repoRoot ?? '.',
      generated_at: overrides.generatedAt ?? '2026-05-15T12:00:00Z',
      seam_id: seamId,
      movement,
      limits: {
        runtime_adequacy_claim: false,
        runtime_mutation_execution: false,
        static_artifact_relationship: true
      }
    },
    seam: {
      seam_id: seamId,
      seam_kind: 'predicate_boundary',
      file: 'src/lib.rs',
      line: 2,
      change: movement
    },
    summary: {
      next_action: {
        kind: movement,
        summary: 'Static receipt movement for test.',
        recommended_action: 'Inspect the focused test.',
        safe_to_merge: false
      }
    }
  });
}

async function withControllerTestContext(
  options: ControllerTestOptions,
  run: (context: ReturnType<typeof createControllerTestContext>) => Promise<void>
): Promise<void> {
  const context = createControllerTestContext(options);
  try {
    await run(context);
  } finally {
    await context.dispose();
  }
}

async function diagnoseSetupReport(context: ReturnType<typeof createControllerTestContext>): Promise<string> {
  context.outputLines.length = 0;
  await context.controller.diagnoseSetup();
  return context.outputLines.join('\n');
}

async function showStatusReport(context: ReturnType<typeof createControllerTestContext>): Promise<string> {
  context.outputLines.length = 0;
  await context.controller.showStatus();
  return context.outputLines.join('\n');
}

function assertReportIncludes(report: string, expectedLines: string[]): void {
  for (const expected of expectedLines) {
    assert.ok(report.includes(expected), `expected setup report to include ${expected}\n\n${report}`);
  }
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

function controllerWorkspaceRootState(options: ControllerTestOptions): RiprWorkspaceRootState {
  if (options.workspaceRootState) {
    return options.workspaceRootState;
  }
  const workspaceRoot = options.workspaceRoot === null
    ? undefined
    : options.workspaceRoot ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!workspaceRoot) {
    return {
      kind: 'noWorkspace',
      roots: [],
      detail: 'test workspace not open'
    };
  }
  return {
    kind: 'singleRoot',
    root: workspaceRoot,
    roots: [workspaceRoot],
    detail: 'single test workspace folder is active'
  };
}

function createControllerTestContext(options: ControllerTestOptions) {
  const client = new FakeLanguageClient(options);
  const outputLines: string[] = [];
  const output = fakeOutputChannel(outputLines);
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 99);
  const runRiprCalls: Array<{ command: string; args: string[]; cwd: string }> = [];
  const watcherPatterns: vscode.GlobPattern[] = [];
  const clipboardWrites: string[] = [];
  const infoMessages: string[] = [];
  const warningMessages: string[] = [];
  const errorMessages: string[] = [];
  let clientOptions: unknown;
  const configuredWorkspaceRootState = controllerWorkspaceRootState(options);
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
    workspaceRootState: () => configuredWorkspaceRootState,
    resolveServer: async () => options.resolveFailure ?? ({
      command: 'ripr',
      source: 'path',
      detail: 'test ripr on PATH',
      version: options.serverVersion ?? 'ripr 0.6.0-test'
    }),
    createLanguageClient: (_serverOptions, options) => {
      clientOptions = options;
      return client;
    },
    createFileSystemWatcher: (pattern) => {
      watcherPatterns.push(pattern);
      return fakeFileSystemWatcher();
    },
    readFile: async (filePath) => testFileContents(filePath, options),
    runRipr: async (command, args, cwd) => {
      runRiprCalls.push({ command, args, cwd });
      return options.cliResult ?? '{}';
    },
    writeClipboard: async (text) => {
      clipboardWrites.push(text);
    },
    isWorkspaceTrusted: () => options.workspaceTrusted ?? true,
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
    watcherPatterns,
    infoMessages,
    warningMessages,
    errorMessages,
    outputLines,
    clientOptions: () => clientOptions,
    dispose: async () => {
      await controller.stop();
      output.dispose();
      status.dispose();
    }
  };
}

function testFileContents(filePath: string, options: ControllerTestOptions): string | undefined {
  const normalizedPath = normalizeTestPath(filePath);
  for (const [candidate, contents] of Object.entries(options.files ?? {})) {
    const normalizedCandidate = normalizeTestPath(candidate);
    if (normalizedPath === normalizedCandidate || normalizedPath.endsWith(`/${normalizedCandidate}`)) {
      return contents ?? undefined;
    }
  }
  if (normalizedPath.endsWith('target/ripr/reports/first-useful-action.json')) {
    return options.firstActionJson ?? undefined;
  }
  return undefined;
}

function normalizeTestPath(filePath: string): string {
  return filePath.replace(/\\/g, '/');
}

function assertRiprDocumentSelectorsScopedToWorkspace(
  clientOptionsValue: unknown,
  workspaceRoot: string
): void {
  const clientOptions = clientOptionsValue as { documentSelector?: unknown };
  const selectors = clientOptions.documentSelector as Array<{
    language?: string;
    scheme?: string;
    pattern?: string;
  }> | undefined;
  assert.ok(Array.isArray(selectors), 'expected language client document selectors');
  assert.deepStrictEqual(selectors.map((selector) => ({
    language: selector.language,
    scheme: selector.scheme
  })), [
    { language: 'rust', scheme: 'file' },
    { language: 'typescript', scheme: 'file' },
    { language: 'typescriptreact', scheme: 'file' },
    { language: 'javascript', scheme: 'file' },
    { language: 'javascriptreact', scheme: 'file' },
    { language: 'python', scheme: 'file' }
  ]);

  for (const selector of selectors) {
    assert.ok(selector.pattern, `expected ${selector.language} selector to be workspace scoped`);
    assert.strictEqual(selector.pattern, `${normalizeTestPath(workspaceRoot)}/**/*`);
  }
}

function assertCargoTomlWatcherScopedToWorkspace(
  watcherPatterns: vscode.GlobPattern[],
  workspaceRoot: string
): void {
  assert.strictEqual(watcherPatterns.length, 1, 'expected one Cargo.toml watcher');
  const pattern = watcherPatterns[0] as {
    base?: string;
    baseUri?: vscode.Uri;
    pattern?: string;
  };
  assert.strictEqual(pattern.pattern, '**/Cargo.toml');
  const basePath = pattern.baseUri?.fsPath ?? pattern.base;
  assertWorkspacePathEqual(basePath ?? '', workspaceRoot);
}

function assertWorkspacePathEqual(actual: string, expected: string): void {
  const normalizedActual = normalizeTestPath(actual);
  const normalizedExpected = normalizeTestPath(expected);
  if (process.platform === 'win32') {
    assert.strictEqual(normalizedActual.toLowerCase(), normalizedExpected.toLowerCase());
    return;
  }
  assert.strictEqual(normalizedActual, normalizedExpected);
}

function fakeOutputChannel(lines: string[] = []): vscode.OutputChannel {
  return {
    name: 'ripr test',
    append: (value: string) => {
      lines.push(value);
    },
    appendLine: (value: string) => {
      lines.push(value);
    },
    clear: () => {
      lines.length = 0;
    },
    show: () => {},
    hide: () => {},
    dispose: () => {},
    replace: (value: string) => {
      lines.length = 0;
      lines.push(value);
    }
  } as vscode.OutputChannel;
}

function fakeFileSystemWatcher(): vscode.FileSystemWatcher {
  const event = (() => ({ dispose: () => {} })) as vscode.Event<vscode.Uri>;
  return {
    ignoreCreateEvents: false,
    ignoreChangeEvents: false,
    ignoreDeleteEvents: false,
    onDidCreate: event,
    onDidChange: event,
    onDidDelete: event,
    dispose: () => {}
  };
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

function workspaceFilePath(relativePath: string): string {
  return workspaceFileUri(relativePath).fsPath;
}

async function writeWorkspaceFile(relativePath: string, contents: string): Promise<void> {
  const filePath = workspaceFilePath(relativePath);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, contents, 'utf8');
}

async function removeWorkspacePath(relativePath: string): Promise<void> {
  await fs.rm(workspaceFilePath(relativePath), { force: true, recursive: true });
}

async function cleanupEditorGapSmokeFiles(): Promise<void> {
  await Promise.all([
    removeWorkspacePath('ripr.toml'),
    removeWorkspacePath('src/pricing.ts'),
    removeWorkspacePath('tests/pricing.test.ts'),
    removeWorkspacePath('target/ripr/reports/gap-decision-ledger.json')
  ]);
}

async function cleanupFirstPrBridgeSmokeFiles(): Promise<void> {
  await Promise.all([
    removeWorkspacePath('target/ripr/first-pr/start-here.json'),
    removeWorkspacePath('target/ripr/first-pr/start-here.md'),
    removeWorkspacePath('target/ripr/reports/start-here.json'),
    removeWorkspacePath('target/ripr/reports/start-here.md')
  ]);
}

async function cleanupAdoptionAssuranceSmokeFiles(): Promise<void> {
  await Promise.all([
    removeWorkspacePath('ripr.toml'),
    removeWorkspacePath('target/ripr/reports/first-useful-action.json'),
    removeWorkspacePath('target/ripr/agent/agent-receipt.json'),
    removeWorkspacePath('target/ripr/first-pr/start-here.json'),
    removeWorkspacePath('target/ripr/first-pr/start-here.md'),
    removeWorkspacePath('target/ripr/reports/start-here.json'),
    removeWorkspacePath('target/ripr/reports/start-here.md')
  ]);
}

async function writeEditorGapSmokeFiles(): Promise<void> {
  await writeWorkspaceFile(
    'ripr.toml',
    [
      '[languages]',
      'enabled = ["rust", "typescript"]',
      ''
    ].join('\n')
  );
  await writeWorkspaceFile(
    'src/pricing.ts',
    [
      'export function discountedTotal(amount: number, threshold: number): number {',
      '  if (amount >= threshold) {',
      '    return amount - 10;',
      '  }',
      '  return amount;',
      '}',
      ''
    ].join('\n')
  );
  await writeWorkspaceFile(
    'tests/pricing.test.ts',
    [
      "import { discountedTotal } from '../src/pricing';",
      '',
      "test('discount threshold boundary', () => {",
      '  expect(discountedTotal(50, 100)).toBe(50);',
      '});',
      ''
    ].join('\n')
  );
  await writeWorkspaceFile(
    'target/ripr/reports/gap-decision-ledger.json',
    JSON.stringify(editorGapSmokeLedger(), null, 2)
  );
}

function editorGapSmokeLedger(): unknown {
  return {
    schema_version: '0.1',
    tool: 'ripr',
    kind: 'gap_decision_ledger',
    status: 'advisory',
    root: '.',
    generated_at: '2026-05-14T00:00:00Z',
    inputs: { records: 'inline' },
    summary: { records_total: 1 },
    records: [
      {
        gap_id: 'gap:typescript:pricing:threshold-boundary',
        canonical_gap_id: 'gap:typescript:pricing:threshold-boundary',
        kind: 'MissingBoundaryAssertion',
        language: 'typescript',
        language_status: 'preview',
        scope: 'workspace',
        evidence_class: 'syntax_first_preview',
        gap_state: 'actionable',
        policy_state: 'advisory',
        repairability: 'repairable',
        static_limit_kind: 'missing_import_graph',
        static_limit_detail: 'TypeScript preview smoke uses syntax-first evidence.',
        static_limits: [
          {
            static_limit_kind: 'missing_import_graph',
            detail: 'TypeScript preview smoke uses syntax-first evidence without an import graph.'
          }
        ],
        repair_route: {
          route_kind: 'AddBoundaryAssertion',
          target_file: 'tests/pricing.test.ts',
          target_line: 4,
          related_test: 'tests/pricing.test.ts::discount threshold boundary',
          assertion_shape: 'expect(discountedTotal(100, 100)).toBe(90)',
          changed_behavior: 'amount >= threshold',
          stop_conditions: ['Stop if the related test no longer reaches discountedTotal.']
        },
        anchor: {
          file: 'src/pricing.ts',
          line: 2,
          owner: 'discountedTotal',
          dedupe_fingerprint: 'src/pricing.ts:discountedTotal:threshold'
        },
        evidence_ids: ['finding:typescript:pricing:threshold'],
        projection_eligibility: {
          lsp_diagnostic: { eligible: true, reason: 'editor gap cockpit smoke' },
          agent_packet: { eligible: true, reason: 'editor gap cockpit smoke' }
        },
        verification_commands: ['ripr agent verify --root . --json'],
        receipt_command: 'ripr agent receipt --root . --json',
        authority_boundary: 'advisory static preview evidence only'
      }
    ],
    warnings: [],
    limits: ['Preview static evidence only.']
  };
}

function textDocument(languageId: string, uri: vscode.Uri): vscode.TextDocument {
  return {
    languageId,
    uri
  } as vscode.TextDocument;
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

async function waitForClipboardText(
  predicate: (text: string) => boolean,
  timeoutMs = 5000
): Promise<string> {
  const started = Date.now();
  let lastText = '';
  while (Date.now() - started < timeoutMs) {
    lastText = await currentClipboardText();
    if (predicate(lastText)) {
      return lastText;
    }
    await sleep(50);
  }
  throw new Error(`timed out waiting for clipboard text. Last clipboard:\n${lastText}`);
}

async function currentClipboardText(): Promise<string> {
  const capturePath = process.env.RIPR_TEST_CLIPBOARD_CAPTURE_PATH;
  if (capturePath) {
    try {
      return await fs.readFile(capturePath, 'utf8');
    } catch (error) {
      if (isNodeError(error) && error.code === 'ENOENT') {
        return '';
      }
      throw error;
    }
  }
  return vscode.env.clipboard.readText();
}

async function writeClipboardText(text: string): Promise<void> {
  await vscode.env.clipboard.writeText(text);
  const capturePath = process.env.RIPR_TEST_CLIPBOARD_CAPTURE_PATH;
  if (capturePath) {
    await fs.writeFile(capturePath, text, 'utf8');
  }
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && 'code' in error;
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
