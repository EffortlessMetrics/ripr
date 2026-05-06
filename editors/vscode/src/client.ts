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
import { resolveServer, ResolveFailure, ResolvedServer } from './serverResolver';

export interface RiprContextTarget {
  uri?: string;
  line?: number;
  finding_id?: string;
  probe_id?: string;
  seam_id?: string;
  seam_kind?: string;
}

export interface RiprSuggestedAssertionTarget {
  assertion?: string;
}

export interface RiprTargetedTestBriefTarget {
  brief?: string;
}

export interface RiprRelatedTestTarget {
  uri?: string;
  line?: number;
  test_name?: string;
}

interface RiprLanguageClient {
  sendRequest(method: string, params: unknown): Promise<unknown>;
  setTrace(trace: Trace): void;
  start(): Promise<void>;
  stop(): Promise<void>;
}

export interface RiprClientRuntime {
  getConfig(): RiprConfig;
  resolveServer(
    context: vscode.ExtensionContext,
    config: RiprConfig,
    output: vscode.OutputChannel
  ): Promise<ResolvedServer | ResolveFailure>;
  createLanguageClient(
    serverOptions: ServerOptions,
    clientOptions: LanguageClientOptions
  ): RiprLanguageClient;
  runRipr(command: string, args: string[], cwd: string): Promise<string>;
}

const defaultRuntime: RiprClientRuntime = {
  getConfig,
  resolveServer,
  createLanguageClient: (serverOptions, clientOptions) =>
    new LanguageClient('ripr', 'ripr', serverOptions, clientOptions),
  runRipr
};

export class RiprClientController {
  private client: RiprLanguageClient | undefined;
  private server: ResolvedServer | undefined;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly output: vscode.OutputChannel,
    private readonly runtime: RiprClientRuntime = defaultRuntime
  ) {}

  async start(): Promise<void> {
    if (this.client) {
      return;
    }

    const config = this.runtime.getConfig();
    const server = await this.runtime.resolveServer(this.context, config, this.output);
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
      initializationOptions: {
        baseRef: config.baseRef,
        checkMode: config.checkMode,
        includeUnchangedTests: true
      },
      outputChannel: this.output,
      revealOutputChannelOn: RevealOutputChannelOn.Never,
      traceOutputChannel: this.output,
      synchronize: {
        fileEvents: vscode.workspace.createFileSystemWatcher('**/Cargo.toml')
      }
    };

    this.output.appendLine(`Resolved ripr server from ${server.source}: ${server.detail}`);
    this.output.appendLine(`Starting ripr language server: ${server.command} ${config.serverArgs.join(' ')}`);
    this.client = this.runtime.createLanguageClient(serverOptions, clientOptions);
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

  async copyContext(target?: RiprContextTarget): Promise<void> {
    const targetUri = uriFromTarget(target);
    const editor = vscode.window.activeTextEditor;
    const documentUri = targetUri ?? editor?.document.uri;
    if (!documentUri) {
      vscode.window.showInformationMessage('Open a Rust file before copying ripr context.');
      return;
    }

    const client = this.client;
    if (client && (target?.finding_id || target?.seam_id)) {
      try {
        const packet = await client.sendRequest('workspace/executeCommand', {
          command: 'ripr.collectContext',
          arguments: [{
            finding_id: target.finding_id,
            probe_id: target.probe_id,
            seam_id: target.seam_id,
            seam_kind: target.seam_kind,
            uri: target.uri,
            line: target.line,
          }],
        });
        if (packet && typeof packet === 'object') {
          await vscode.env.clipboard.writeText(JSON.stringify(packet, null, 2));
          vscode.window.showInformationMessage('Copied ripr context to clipboard.');
          return;
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        this.output.appendLine(`ripr collectContext via LSP failed: ${message}`);
      }
    }

    const workspaceFolder = vscode.workspace.getWorkspaceFolder(documentUri);
    if (!workspaceFolder) {
      vscode.window.showInformationMessage('ripr context requires a workspace folder.');
      return;
    }

    const config = this.runtime.getConfig();
    const server = this.server ?? await this.resolveServerForCommand(config);
    if (!server) {
      return;
    }
    const relativePath = path.relative(workspaceFolder.uri.fsPath, documentUri.fsPath);
    const activeLine = editor ? editor.selection.active.line + 1 : undefined;
    const line = lineFromTarget(target) ?? activeLine ?? 1;
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
      const context = await this.runtime.runRipr(server.command, args, workspaceFolder.uri.fsPath);
      await vscode.env.clipboard.writeText(context.trim());
      vscode.window.showInformationMessage('Copied ripr context to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr context failed: ${message}`);
      vscode.window.showWarningMessage(`ripr context failed for ${selector}. See ripr output for details.`);
    }
  }

  async copySuggestedAssertion(target?: RiprSuggestedAssertionTarget): Promise<void> {
    const assertion = typeof target?.assertion === 'string' ? target.assertion.trim() : '';
    if (!assertion) {
      vscode.window.showInformationMessage('No ripr suggested assertion is available for this diagnostic.');
      return;
    }
    try {
      await vscode.env.clipboard.writeText(assertion);
      vscode.window.showInformationMessage('Copied ripr suggested assertion to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy suggested assertion failed: ${message}`);
      vscode.window.showWarningMessage('ripr could not copy the suggested assertion. See ripr output for details.');
    }
  }

  async copyTargetedTestBrief(target?: RiprTargetedTestBriefTarget): Promise<void> {
    const brief = typeof target?.brief === 'string' ? target.brief.trim() : '';
    if (!brief) {
      vscode.window.showInformationMessage('No ripr targeted test brief is available for this diagnostic.');
      return;
    }
    try {
      await vscode.env.clipboard.writeText(brief);
      vscode.window.showInformationMessage('Copied ripr targeted test brief to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy targeted test brief failed: ${message}`);
      vscode.window.showWarningMessage('ripr could not copy the targeted test brief. See ripr output for details.');
    }
  }

  async openRelatedTest(target?: RiprRelatedTestTarget): Promise<void> {
    const uri = uriFromTarget(target);
    if (!uri) {
      vscode.window.showInformationMessage('No ripr related test location is available for this diagnostic.');
      return;
    }
    try {
      const document = await vscode.workspace.openTextDocument(uri);
      const editor = await vscode.window.showTextDocument(document);
      const line = lineFromTarget(target) ?? 1;
      const position = new vscode.Position(Math.max(0, line - 1), 0);
      editor.selection = new vscode.Selection(position, position);
      editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr open related test failed: ${message}`);
      vscode.window.showWarningMessage('ripr could not open the related test. See ripr output for details.');
    }
  }

  showOutput(): void {
    this.output.show();
  }

  private async resolveServerForCommand(config: RiprConfig): Promise<ResolvedServer | undefined> {
    const server = await this.runtime.resolveServer(this.context, config, this.output);
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

function uriFromTarget(target: RiprContextTarget | undefined): vscode.Uri | undefined {
  if (!target?.uri) {
    return undefined;
  }
  try {
    return vscode.Uri.parse(target.uri);
  } catch {
    return undefined;
  }
}

function lineFromTarget(target: RiprContextTarget | undefined): number | undefined {
  if (typeof target?.line !== 'number' || !Number.isFinite(target.line) || target.line < 1) {
    return undefined;
  }
  return Math.floor(target.line);
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
