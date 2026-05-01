import * as cp from 'child_process';
import * as path from 'path';
import * as vscode from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  RevealOutputChannelOn,
  ServerOptions,
  Trace
} from 'vscode-languageclient/node';
import { getConfig, RiprConfig } from './config';
import { resolveServer, ResolvedServer } from './serverResolver';

export class RiprClientController {
  private client: LanguageClient | undefined;
  private server: ResolvedServer | undefined;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly output: vscode.OutputChannel
  ) {}

  async start(): Promise<void> {
    if (this.client) {
      return;
    }

    const config = getConfig();
    const server = await resolveServer(this.context, config, this.output);
    if (!('command' in server)) {
      await this.showMissingServerMessage(server.message, server.detail);
      return;
    }
    this.server = server;

    const serverOptions: ServerOptions = {
      command: server.command,
      args: config.serverArgs,
      options: {
        cwd: firstWorkspaceFolder()
      }
    };

    const clientOptions: LanguageClientOptions = {
      documentSelector: [{ language: 'rust', scheme: 'file' }],
      outputChannel: this.output,
      revealOutputChannelOn: RevealOutputChannelOn.Never,
      traceOutputChannel: this.output,
      synchronize: {
        fileEvents: vscode.workspace.createFileSystemWatcher('**/Cargo.toml')
      }
    };

    this.output.appendLine(`Resolved ripr server from ${server.source}: ${server.detail}`);
    this.output.appendLine(`Starting ripr language server: ${server.command} ${config.serverArgs.join(' ')}`);
    this.client = new LanguageClient('ripr', 'ripr', serverOptions, clientOptions);
    this.client.setTrace(traceFromConfig(config.traceServer));
    await this.client.start();
  }

  async restart(): Promise<void> {
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    const client = this.client;
    this.client = undefined;
    this.server = undefined;
    if (client) {
      await client.stop();
    }
  }

  async copyContext(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showInformationMessage('Open a Rust file before copying ripr context.');
      return;
    }

    const workspaceFolder = vscode.workspace.getWorkspaceFolder(editor.document.uri);
    if (!workspaceFolder) {
      vscode.window.showInformationMessage('ripr context requires a workspace folder.');
      return;
    }

    const config = getConfig();
    const server = this.server ?? await this.resolveServerForCommand(config);
    if (!server) {
      return;
    }
    const relativePath = path.relative(workspaceFolder.uri.fsPath, editor.document.uri.fsPath);
    const line = editor.selection.active.line + 1;
    const selector = `${relativePath}:${line}`;
    const args = [
      'context',
      '--root',
      workspaceFolder.uri.fsPath,
      '--base',
      config.baseRef,
      '--at',
      selector,
      '--json'
    ];

    try {
      const context = await runRipr(server.command, args, workspaceFolder.uri.fsPath);
      await vscode.env.clipboard.writeText(context.trim());
      vscode.window.showInformationMessage('Copied ripr context to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr context failed: ${message}`);
      vscode.window.showWarningMessage(`ripr context failed for ${selector}. See ripr output for details.`);
    }
  }

  showOutput(): void {
    this.output.show();
  }

  private async resolveServerForCommand(config: RiprConfig): Promise<ResolvedServer | undefined> {
    const server = await resolveServer(this.context, config, this.output);
    if ('command' in server) {
      this.server = server;
      return server;
    }
    await this.showMissingServerMessage(server.message, server.detail);
    return undefined;
  }

  private async showMissingServerMessage(summary: string, detail: string): Promise<void> {
    this.output.appendLine(summary);
    this.output.appendLine(detail);
    const selection = await vscode.window.showErrorMessage(
      'ripr server is not available. Enable automatic download, install with `cargo install ripr`, or set `ripr.server.path`.',
      'Open Settings',
      'Copy Install Command',
      'Retry'
    );
    if (selection === 'Open Settings') {
      await vscode.commands.executeCommand('workbench.action.openSettings', 'ripr.server');
    } else if (selection === 'Copy Install Command') {
      await vscode.env.clipboard.writeText('cargo install ripr');
    } else if (selection === 'Retry') {
      await this.restart();
    }
  }
}

function traceFromConfig(trace: RiprConfig['traceServer']): Trace {
  switch (trace) {
    case 'messages':
      return Trace.Messages;
    case 'verbose':
      return Trace.Verbose;
    case 'off':
    default:
      return Trace.Off;
  }
}

function firstWorkspaceFolder(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

function runRipr(command: string, args: string[], cwd: string): Promise<string> {
  return new Promise((resolve, reject) => {
    cp.execFile(command, args, { cwd, maxBuffer: 1024 * 1024 }, (error, stdout, stderr) => {
      if (error) {
        reject(new Error(stderr.trim() || error.message));
      } else {
        resolve(stdout);
      }
    });
  });
}
