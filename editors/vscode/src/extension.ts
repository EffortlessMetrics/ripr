import * as vscode from 'vscode';
import {
  RiprClientController,
  RiprAgentLoopCommandTarget,
  RiprContextTarget,
  RiprRelatedTestTarget,
  RiprSuggestedAssertionTarget,
  RiprTargetedTestBriefTarget,
} from './client';
import {
  DEFAULT_LIFECYCLE_SETTLE_BUDGET_MS,
  ExtensionLifecycleCoordinator,
} from './lifecycleCoordinator';

let controller: RiprClientController | undefined;
let lifecycleCoordinator = new ExtensionLifecycleCoordinator();

/** Test-only reset for suites that exercise activation-scoped helpers with
 * independent fake controllers in one extension-host process. Production
 * activation resets the coordinator before constructing its controller. */
export function resetLifecycleCoordinatorForTests(): void {
  lifecycleCoordinator = new ExtensionLifecycleCoordinator();
}

export async function startServerOnce(
  currentController: Pick<RiprClientController, 'start'> | undefined
): Promise<void> {
  await lifecycleCoordinator.start(currentController);
}

export async function restartServerOnce(
  currentController: Pick<RiprClientController, 'start' | 'stop'> | undefined,
  startSettleBudgetMs = DEFAULT_LIFECYCLE_SETTLE_BUDGET_MS
): Promise<void> {
  await lifecycleCoordinator.restart(currentController, startSettleBudgetMs);
}

export async function stopServerOnce(
  currentController: Pick<RiprClientController, 'start' | 'stop'> | undefined,
  startSettleBudgetMs = DEFAULT_LIFECYCLE_SETTLE_BUDGET_MS
): Promise<void> {
  await lifecycleCoordinator.stop(currentController, startSettleBudgetMs);
}

export async function startAfterWorkspaceTrust(
  currentController: Pick<RiprClientController, 'start'> | undefined
): Promise<void> {
  await startServerOnce(currentController);
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const output = vscode.window.createOutputChannel('ripr', { log: true });
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  lifecycleCoordinator = new ExtensionLifecycleCoordinator();
  controller = new RiprClientController(context, output, undefined, status);

  context.subscriptions.push(
    output,
    status,
    vscode.commands.registerCommand('ripr.restartServer', async () => {
      try {
        await restartServerOnce(controller);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        output.appendLine(`ripr server restart failed: ${message}`);
        void vscode.window.showErrorMessage(
          `ripr could not restart the server: ${message} Open ripr: Show Output, then retry.`
        );
      }
    }),
    vscode.commands.registerCommand('ripr.refreshDiagnostics', async () =>
      controller?.refreshDiagnostics()
    ),
    // No matching `onCommand:ripr.selectWorkspaceRoot` entry in package.json
    // `activationEvents`: since VS Code 1.74 a contributed command activates
    // its extension automatically, and engines.vscode is ^1.90.0. The
    // manifest's legacy onCommand list predates that guarantee and is not
    // extended for new commands. (#2077)
    vscode.commands.registerCommand('ripr.selectWorkspaceRoot', async () =>
      controller?.selectWorkspaceRoot()
    ),
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
    vscode.commands.registerCommand('ripr.copyRepairPacketAtCursor', async () =>
      controller?.copyRepairPacketAtCursor()
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
          await startServerOnce(controller);
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
        event.affectsConfiguration('ripr.baseRef')
      ) {
        try {
          await restartServerOnce(controller);
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          output.appendLine(`ripr server restart after configuration change failed: ${message}`);
        }
        return;
      }
      if (event.affectsConfiguration('ripr.trace')) {
        // Trace applies live (#2082): a full restart would drop every
        // published diagnostic and force a full re-analysis. When a
        // restart-triggering key changed in the same event, the restart
        // above already applied the new trace level at start().
        controller?.setTraceFromConfig();
      }
    })
  );

  await startServerOnce(controller);
}

export async function deactivate(): Promise<void> {
  const currentController = controller;
  controller = undefined;
  await stopServerOnce(currentController);
}
