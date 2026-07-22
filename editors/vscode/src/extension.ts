import * as vscode from 'vscode';
import {
  RiprClientController,
  RiprAgentLoopCommandTarget,
  RiprClientRuntime,
  RiprContextTarget,
  RiprRelatedTestTarget,
  RiprSuggestedAssertionTarget,
  RiprTargetedTestBriefTarget,
} from './client';

let controller: RiprClientController | undefined;
let coalescedStart: Promise<void> | undefined;

export async function startServerOnce(
  currentController: Pick<RiprClientController, 'start'> | undefined
): Promise<void> {
  if (!currentController) {
    return;
  }
  if (coalescedStart) {
    await coalescedStart;
    return;
  }

  const start = currentController.start();
  coalescedStart = start;
  try {
    await start;
  } finally {
    if (coalescedStart === start) {
      coalescedStart = undefined;
    }
  }
}

export async function startAfterWorkspaceTrust(
  currentController: Pick<RiprClientController, 'start'> | undefined
): Promise<void> {
  await startServerOnce(currentController);
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const output = vscode.window.createOutputChannel('ripr');
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  controller = new RiprClientController(context, output, undefined, status);
  injectTrustedExtensionHostRuntime(controller, context);

  context.subscriptions.push(
    output,
    status,
    vscode.commands.registerCommand('ripr.restartServer', async () => controller?.restart()),
    vscode.commands.registerCommand('ripr.showOutput', () => controller?.showOutput()),
    vscode.commands.registerCommand('ripr.showStatus', () => controller?.showStatus()),
    vscode.commands.registerCommand('ripr.diagnoseSetup', () => controller?.diagnoseSetup()),
    vscode.commands.registerCommand('ripr.startCurrentRepair', async () =>
      controller?.startCurrentRepair()
    ),
    vscode.commands.registerCommand('ripr.copyCurrentRepairPacket', async () =>
      controller?.copyCurrentRepairPacket()
    ),
    vscode.commands.registerCommand('ripr.copyRepoGapMap', async () =>
      controller?.copyRepoGapMap()
    ),
    vscode.commands.registerCommand('ripr.openFirstPrPacket', async () =>
      controller?.openFirstPrPacket()
    ),
    vscode.commands.registerCommand('ripr.copyFirstPrSummary', async () =>
      controller?.copyFirstPrSummary()
    ),
    vscode.commands.registerCommand('ripr.copyFirstPrRepairPacket', async () =>
      controller?.copyFirstPrRepairPacket()
    ),
    vscode.commands.registerCommand('ripr.copyFirstPrVerifyCommand', async () =>
      controller?.copyFirstPrVerifyCommand()
    ),
    vscode.commands.registerCommand('ripr.copyFirstPrReceiptCommand', async () =>
      controller?.copyFirstPrReceiptCommand()
    ),
    vscode.commands.registerCommand('ripr.copyFirstPrRegenerationGuidance', async () =>
      controller?.copyFirstPrRegenerationGuidance()
    ),
    vscode.commands.registerCommand('ripr.copyContext', async (target?: RiprContextTarget) =>
      controller?.copyContext(target)
    ),
    vscode.commands.registerCommand(
      'ripr.copySuggestedAssertion',
      async (target?: RiprSuggestedAssertionTarget) => controller?.copySuggestedAssertion(target)
    ),
    vscode.commands.registerCommand(
      'ripr.copyTargetedTestBrief',
      async (target?: RiprTargetedTestBriefTarget) => controller?.copyTargetedTestBrief(target)
    ),
    vscode.commands.registerCommand(
      'ripr.copyAgentPacketCommand',
      async (target?: RiprAgentLoopCommandTarget) => controller?.copyAgentLoopCommand(target)
    ),
    vscode.commands.registerCommand(
      'ripr.copyAgentBriefCommand',
      async (target?: RiprAgentLoopCommandTarget) => controller?.copyAgentLoopCommand(target)
    ),
    vscode.commands.registerCommand(
      'ripr.copyAfterSnapshotCommand',
      async (target?: RiprAgentLoopCommandTarget) => controller?.copyAgentLoopCommand(target)
    ),
    vscode.commands.registerCommand(
      'ripr.copyAgentVerifyCommand',
      async (target?: RiprAgentLoopCommandTarget) => controller?.copyAgentLoopCommand(target)
    ),
    vscode.commands.registerCommand(
      'ripr.copyAgentReceiptCommand',
      async (target?: RiprAgentLoopCommandTarget) => controller?.copyAgentLoopCommand(target)
    ),
    vscode.commands.registerCommand('ripr.openRelatedTest', async (target?: RiprRelatedTestTarget) =>
      controller?.openRelatedTest(target)
    ),
    vscode.commands.registerCommand('ripr.openSettings', async () => {
      await vscode.commands.executeCommand('workbench.action.openSettings', 'ripr');
    }),
    // Cockpit commands
    vscode.commands.registerCommand('ripr.copyTopRepairPacket', async () =>
      controller?.copyTopRepairPacket()
    ),
    vscode.commands.registerCommand('ripr.copyTopVerifyCommand', async () =>
      controller?.copyTopVerifyCommand()
    ),
    vscode.commands.registerCommand('ripr.copyTopReceiptCommand', async () =>
      controller?.copyTopReceiptCommand()
    ),
    vscode.commands.registerCommand('ripr.openReport', async () =>
      controller?.openReport()
    ),
    vscode.commands.registerCommand('ripr.showTopLimitation', async () =>
      controller?.showTopLimitation()
    ),
    // Receipt status / route-quality inspection commands
    vscode.commands.registerCommand('ripr.showReceiptStatus', async () =>
      controller?.showReceiptStatus()
    ),
    vscode.commands.registerCommand('ripr.copyReceiptCommand', async () =>
      controller?.copyReceiptCommand()
    ),
    vscode.commands.registerCommand('ripr.openAttemptLedger', async () =>
      controller?.openAttemptLedger()
    ),
    vscode.commands.registerCommand('ripr.showRouteQuality', async () =>
      controller?.showRouteQuality()
    ),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (event.document.isDirty) {
        controller?.markWorkspaceStale(event.document);
      }
    }),
    vscode.workspace.onDidSaveTextDocument((document) => {
      controller?.markWorkspaceSaved(document);
    }),
    vscode.workspace.onDidCloseTextDocument((document) => {
      controller?.markWorkspaceClosed(document);
    }),
    vscode.workspace.onDidChangeWorkspaceFolders(async (event) => {
      if (event.added.length === 0) {
        return;
      }
      output.appendLine(
        `ripr workspace folder added (${event.added.length}); starting the server if none is running.`
      );
      try {
        await startServerOnce(controller);
      } catch (error) {
        output.appendLine(`ripr server start after workspace folder change failed: ${String(error)}`);
      }
    }),
    vscode.workspace.onDidGrantWorkspaceTrust(async () => {
      output.appendLine('ripr workspace trust granted; starting a fresh server session.');
      try {
        await startAfterWorkspaceTrust(controller);
      } catch (error) {
        output.appendLine(`ripr server start after workspace trust failed: ${String(error)}`);
      }
    }),
    // Auto-start when a workspace folder appears after a no-workspace start.
    // Without this listener, opening a single Rust file first (no folder)
    // hits the noWorkspace early return in client.ts and stops. Opening
    // a folder later does nothing — the user has to run `ripr: Restart
    // Server` manually. We only react when the PRE-event folder count was
    // zero and at least one folder was added, so we don't restart on
    // routine folder additions/removals in an already-running multi-root
    // workspace. The pre-event count is derived from the post-event count
    // by adding back removals and subtracting additions. (#2015)
    vscode.workspace.onDidChangeWorkspaceFolders(async (event) => {
      const folderCount = vscode.workspace.workspaceFolders?.length ?? 0;
      // Reconstruct the pre-event folder count. VS Code does not expose it
      // directly; derive it from the post-event count and the event delta.
      const preEventFolderCount = folderCount + event.removed.length - event.added.length;
      const zeroToFolderTransition = preEventFolderCount === 0 && event.added.length > 0;
      if (zeroToFolderTransition && !controller?.isRunning()) {
        output.appendLine('ripr workspace folder added; starting server session.');
        try {
          await controller?.start();
        } catch (error) {
          output.appendLine(`ripr server start after workspace folder added failed: ${String(error)}`);
        }
      }
    }),
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (
        event.affectsConfiguration('ripr.enabled') ||
        event.affectsConfiguration('ripr.server') ||
        event.affectsConfiguration('ripr.check') ||
        event.affectsConfiguration('ripr.baseRef') ||
        event.affectsConfiguration('ripr.trace')
      ) {
        await controller?.restart();
      }
    })
  );

  await startServerOnce(controller);
}

function injectTrustedExtensionHostRuntime(
  currentController: RiprClientController,
  context: vscode.ExtensionContext
): void {
  if (
    context.extensionMode !== vscode.ExtensionMode.Test ||
    process.env.RIPR_TEST_ASSUME_WORKSPACE_TRUSTED !== '1'
  ) {
    return;
  }

  // VS Code loads Workspace Trust before the extension-host test suite can
  // seed or change it. Override only the trust predicate in explicit test
  // mode; installed/development extensions continue to use
  // vscode.workspace.isTrusted and retain the production no-launch boundary.
  const runtime = (currentController as unknown as { runtime: RiprClientRuntime }).runtime;
  runtime.isWorkspaceTrusted = () => true;
}

export async function deactivate(): Promise<void> {
  await controller?.stop();
  controller = undefined;
}
