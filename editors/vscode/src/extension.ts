import * as vscode from 'vscode';
import {
  RiprClientController,
  RiprAgentLoopCommandTarget,
  RiprContextTarget,
  RiprRelatedTestTarget,
  RiprSuggestedAssertionTarget,
  RiprTargetedTestBriefTarget
} from './client';

let controller: RiprClientController | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const output = vscode.window.createOutputChannel('ripr');
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  controller = new RiprClientController(context, output, undefined, status);

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

  await controller.start();
}

export async function deactivate(): Promise<void> {
  await controller?.stop();
  controller = undefined;
}
