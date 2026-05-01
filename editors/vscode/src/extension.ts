import * as vscode from 'vscode';
import { RiprClientController } from './client';

let controller: RiprClientController | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const output = vscode.window.createOutputChannel('ripr');
  controller = new RiprClientController(context, output);

  context.subscriptions.push(
    output,
    vscode.commands.registerCommand('ripr.restartServer', async () => controller?.restart()),
    vscode.commands.registerCommand('ripr.showOutput', () => controller?.showOutput()),
    vscode.commands.registerCommand('ripr.copyContext', async () => controller?.copyContext()),
    vscode.commands.registerCommand('ripr.openSettings', async () => {
      await vscode.commands.executeCommand('workbench.action.openSettings', 'ripr');
    }),
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (event.affectsConfiguration('ripr.server')) {
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
