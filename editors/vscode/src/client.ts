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

export interface RiprAgentLoopCommandTarget {
  command?: string;
  label?: string;
}

export interface RiprRelatedTestTarget {
  uri?: string;
  line?: number;
  test_name?: string;
}

interface RiprLanguageClient {
  onNotification(method: string, handler: (params: unknown) => void): vscode.Disposable;
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
  writeClipboard(text: string): Promise<void>;
}

const defaultRuntime: RiprClientRuntime = {
  getConfig,
  resolveServer,
  createLanguageClient: (serverOptions, clientOptions) =>
    new LanguageClient('ripr', 'ripr', serverOptions, clientOptions),
  runRipr,
  writeClipboard: async (text) => {
    await vscode.env.clipboard.writeText(text);
  }
};

export class RiprClientController {
  private client: RiprLanguageClient | undefined;
  private server: ResolvedServer | undefined;
  private readonly notificationDisposables: vscode.Disposable[] = [];
  private status: RiprStatusState = {
    kind: 'stopped',
    summary: 'ripr server has not started.',
    detail: 'Open a Rust/Cargo workspace or run ripr: Restart Server.'
  };
  private workspaceRoot: string | undefined;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly output: vscode.OutputChannel,
    private readonly runtime: RiprClientRuntime = defaultRuntime,
    private readonly statusBar?: vscode.StatusBarItem
  ) {
    this.updateStatus(this.status);
  }

  async start(): Promise<void> {
    if (this.client) {
      return;
    }

    this.workspaceRoot = firstWorkspaceFolder();
    if (!this.workspaceRoot) {
      this.updateStatus({
        kind: 'noWorkspace',
        summary: 'Open a Rust/Cargo workspace for ripr diagnostics.',
        detail: 'The extension needs a workspace folder before it can start the language server.'
      });
      this.output.appendLine('ripr workspace was not detected; open a Rust/Cargo workspace.');
      return;
    }

    const config = this.runtime.getConfig();
    this.updateStatus({
      kind: 'resolvingServer',
      summary: 'Resolving ripr server.',
      detail: `Workspace: ${this.workspaceRoot}`
    });
    const server = await this.runtime.resolveServer(this.context, config, this.output);
    if (!('command' in server)) {
      this.updateStatus({
        kind: 'serverUnavailable',
        summary: 'ripr server is not available.',
        detail: server.detail
      });
      await this.showMissingServerMessage(server.message, server.detail);
      return;
    }
    this.server = server;
    this.updateStatus({
      kind: 'starting',
      summary: 'Starting ripr language server.',
      detail: `Server: ${server.source} (${server.detail})\nWorkspace: ${this.workspaceRoot}`
    });

    const serverOptions: ServerOptions = {
      command: server.command,
      args: config.serverArgs,
      options: {
        cwd: this.workspaceRoot
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
    this.notificationDisposables.push(
      this.client.onNotification('window/logMessage', (params) => this.handleServerLog(params))
    );
    await this.client.start();
    this.updateStatus({
      kind: 'ready',
      summary: 'ripr server is ready; waiting for saved-workspace analysis.',
      detail: `Server: ${server.source} (${server.detail})\nWorkspace: ${this.workspaceRoot}`
    });
  }

  async restart(): Promise<void> {
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    const client = this.client;
    this.client = undefined;
    this.server = undefined;
    while (this.notificationDisposables.length > 0) {
      this.notificationDisposables.pop()?.dispose();
    }
    if (client) {
      await client.stop();
    }
    this.updateStatus({
      kind: 'stopped',
      summary: 'ripr server has stopped.',
      detail: 'Run ripr: Restart Server to start analysis again.'
    });
  }

  markWorkspaceStale(document: vscode.TextDocument): void {
    if (!this.client || document.languageId !== 'rust' || document.uri.scheme !== 'file') {
      return;
    }
    this.updateStatus({
      kind: 'stale',
      summary: 'ripr analysis is stale until the Rust file is saved.',
      detail: `Unsaved changes: ${document.uri.fsPath}`
    });
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
          await this.runtime.writeClipboard(JSON.stringify(packet, null, 2));
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
      await this.runtime.writeClipboard(context.trim());
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
      await this.runtime.writeClipboard(assertion);
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
      await this.runtime.writeClipboard(brief);
      vscode.window.showInformationMessage('Copied ripr targeted test brief to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy targeted test brief failed: ${message}`);
      vscode.window.showWarningMessage('ripr could not copy the targeted test brief. See ripr output for details.');
    }
  }

  async copyAgentLoopCommand(target?: RiprAgentLoopCommandTarget): Promise<void> {
    const command = typeof target?.command === 'string' ? target.command.trim() : '';
    if (!command) {
      vscode.window.showInformationMessage('No ripr agent loop command is available for this diagnostic.');
      return;
    }
    try {
      await this.runtime.writeClipboard(command);
      vscode.window.showInformationMessage('Copied ripr agent loop command to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy agent loop command failed: ${message}`);
      vscode.window.showWarningMessage('ripr could not copy the agent loop command. See ripr output for details.');
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

  showStatus(): void {
    this.output.appendLine(`ripr status: ${this.status.summary}`);
    if (this.status.detail) {
      this.output.appendLine(this.status.detail);
    }
    this.output.show();
    vscode.window.showInformationMessage(this.status.summary);
  }

  private handleServerLog(params: unknown): void {
    const message = serverLogMessage(params);
    if (!message) {
      return;
    }
    if (message.startsWith('ripr analysis refresh started')) {
      this.updateStatus({
        kind: 'analysisRunning',
        summary: 'ripr saved-workspace analysis is running.',
        detail: message
      });
      return;
    }
    if (message.startsWith('ripr analysis refresh completed')) {
      this.updateStatus(statusFromRefreshCompletedMessage(message));
      return;
    }
    if (message.startsWith('ripr analysis refresh failed')) {
      this.updateStatus({
        kind: 'analysisFailed',
        summary: 'ripr analysis refresh failed.',
        detail: message
      });
    }
  }

  private updateStatus(status: RiprStatusState): void {
    this.status = status;
    if (!this.statusBar) {
      return;
    }
    this.statusBar.text = statusText(status.kind);
    this.statusBar.tooltip = status.detail ? `${status.summary}\n${status.detail}` : status.summary;
    this.statusBar.command = 'ripr.showStatus';
    this.statusBar.show();
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
      await this.runtime.writeClipboard('cargo install ripr');
    } else if (selection === 'Retry') {
      await this.restart();
    }
  }
}

type RiprStatusKind =
  | 'noWorkspace'
  | 'resolvingServer'
  | 'serverUnavailable'
  | 'starting'
  | 'ready'
  | 'analysisRunning'
  | 'analysisReady'
  | 'noActionableSeams'
  | 'stale'
  | 'analysisFailed'
  | 'stopped';

interface RiprStatusState {
  kind: RiprStatusKind;
  summary: string;
  detail?: string;
}

function statusText(kind: RiprStatusKind): string {
  switch (kind) {
    case 'noWorkspace':
      return '$(folder) ripr: open workspace';
    case 'resolvingServer':
      return '$(sync~spin) ripr: resolving';
    case 'serverUnavailable':
      return '$(warning) ripr: server missing';
    case 'starting':
      return '$(sync~spin) ripr: starting';
    case 'ready':
      return '$(pass) ripr: ready';
    case 'analysisRunning':
      return '$(sync~spin) ripr: analyzing';
    case 'analysisReady':
      return '$(check) ripr: diagnostics';
    case 'noActionableSeams':
      return '$(circle-slash) ripr: no seams';
    case 'stale':
      return '$(warning) ripr: stale';
    case 'analysisFailed':
      return '$(error) ripr: failed';
    case 'stopped':
    default:
      return 'ripr: stopped';
  }
}

function serverLogMessage(params: unknown): string | undefined {
  if (!params || typeof params !== 'object' || !('message' in params)) {
    return undefined;
  }
  const message = (params as { message?: unknown }).message;
  return typeof message === 'string' ? message : undefined;
}

function statusFromRefreshCompletedMessage(message: string): RiprStatusState {
  const diagnostics = numberField(message, 'diagnostics');
  const seamDiagnostics = numberField(message, 'seam_diagnostics');
  if (seamDiagnostics !== undefined && seamDiagnostics === 0) {
    return {
      kind: 'noActionableSeams',
      summary: 'ripr analysis completed with no actionable seam diagnostics.',
      detail: message
    };
  }
  return {
    kind: 'analysisReady',
    summary: `ripr analysis completed with ${diagnostics ?? 0} diagnostics.`,
    detail: message
  };
}

function numberField(message: string, field: string): number | undefined {
  const match = message.match(new RegExp(`${field}=(\\d+)`));
  return match ? Number.parseInt(match[1], 10) : undefined;
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
