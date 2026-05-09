import * as cp from 'child_process';
import * as fs from 'fs';
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

interface FirstUsefulActionProjection {
  reportPath: string;
  status: string;
  actionKind: string;
  title: string;
  why?: string;
  seamId?: string;
  targetFile?: string;
  verifyCommandAvailable: boolean;
  receiptCommandAvailable: boolean;
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
  workspaceRoot(): string | undefined;
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
  showInformationMessage(message: string): Thenable<string | undefined>;
  showWarningMessage(message: string): Thenable<string | undefined>;
  showErrorMessage(message: string, ...items: string[]): Thenable<string | undefined>;
}

const defaultRuntime: RiprClientRuntime = {
  getConfig,
  workspaceRoot: firstWorkspaceFolder,
  resolveServer,
  createLanguageClient: (serverOptions, clientOptions) =>
    new LanguageClient('ripr', 'ripr', serverOptions, clientOptions),
  runRipr,
  writeClipboard: async (text) => {
    await vscode.env.clipboard.writeText(text);
  },
  showInformationMessage: (message) => vscode.window.showInformationMessage(message),
  showWarningMessage: (message) => vscode.window.showWarningMessage(message),
  showErrorMessage: (message, ...items) => vscode.window.showErrorMessage(message, ...items)
};

export class RiprClientController {
  private client: RiprLanguageClient | undefined;
  private server: ResolvedServer | undefined;
  private readonly notificationDisposables: vscode.Disposable[] = [];
  private readonly dirtyRustDocuments = new Set<string>();
  private firstUsefulAction: FirstUsefulActionProjection | undefined;
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

    const config = this.runtime.getConfig();
    if (!config.enabled) {
      this.updateStatus({
        kind: 'disabled',
        summary: 'ripr editor analysis is disabled by configuration.',
        detail: 'Set ripr.enabled to true to start saved-workspace diagnostics.'
      });
      this.output.appendLine('ripr editor analysis is disabled by configuration.');
      return;
    }

    this.workspaceRoot = this.runtime.workspaceRoot();
    if (!this.workspaceRoot) {
      this.updateStatus({
        kind: 'noWorkspace',
        summary: 'Open a Rust/Cargo workspace for ripr diagnostics.',
        detail: 'The extension needs a workspace folder before it can start the language server.'
      });
      this.output.appendLine('ripr workspace was not detected; open a Rust/Cargo workspace.');
      return;
    }

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
      kind: 'analysisQueued',
      summary: 'ripr saved-workspace analysis is queued.',
      detail: `Server: ${server.source} (${server.detail})\nWorkspace: ${this.workspaceRoot}\nOpen or save a Rust file to refresh diagnostics.`
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
    this.dirtyRustDocuments.clear();
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
    if (!this.client || !isRustFileDocument(document)) {
      return;
    }
    this.dirtyRustDocuments.add(document.uri.toString());
    this.updateStatus({
      kind: 'stale',
      summary: 'ripr analysis is stale until the Rust file is saved.',
      detail: `Unsaved changes: ${document.uri.fsPath}`
    });
  }

  markWorkspaceSaved(document: vscode.TextDocument): void {
    if (!this.client || !isRustFileDocument(document)) {
      return;
    }
    this.dirtyRustDocuments.delete(document.uri.toString());
    if (this.dirtyRustDocuments.size === 0 && this.status.kind === 'stale') {
      this.updateStatus({
        kind: 'analysisQueued',
        summary: 'ripr saved-workspace analysis is queued after save.',
        detail: `Saved changes: ${document.uri.fsPath}`
      });
    }
  }

  markWorkspaceClosed(document: vscode.TextDocument): void {
    if (!isRustFileDocument(document)) {
      return;
    }
    this.dirtyRustDocuments.delete(document.uri.toString());
    if (this.client && this.dirtyRustDocuments.size === 0 && this.status.kind === 'stale') {
      this.updateStatus({
        kind: 'analysisQueued',
        summary: 'ripr saved-workspace analysis is queued after close.',
        detail: `Closed unsaved Rust buffer: ${document.uri.fsPath}`
      });
    }
  }

  async copyContext(target?: RiprContextTarget): Promise<void> {
    const targetUri = uriFromTarget(target);
    const editor = vscode.window.activeTextEditor;
    const documentUri = targetUri ?? editor?.document.uri;
    if (!documentUri) {
      this.runtime.showInformationMessage('Open a Rust file before copying ripr context.');
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
          this.runtime.showInformationMessage('Copied ripr context to clipboard.');
          return;
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        this.output.appendLine(`ripr collectContext via LSP failed: ${message}`);
      }
    }

    const workspaceFolder = vscode.workspace.getWorkspaceFolder(documentUri);
    if (!workspaceFolder) {
      this.runtime.showInformationMessage('ripr context requires a workspace folder.');
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
      this.runtime.showInformationMessage('Copied ripr context to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr context failed: ${message}`);
      this.runtime.showWarningMessage(`ripr context failed for ${selector}. See ripr output for details.`);
    }
  }

  async copySuggestedAssertion(target?: RiprSuggestedAssertionTarget): Promise<void> {
    const assertion = typeof target?.assertion === 'string' ? target.assertion.trim() : '';
    if (!assertion) {
      this.runtime.showInformationMessage('No ripr suggested assertion is available for this diagnostic.');
      return;
    }
    try {
      await this.runtime.writeClipboard(assertion);
      this.runtime.showInformationMessage('Copied ripr suggested assertion to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy suggested assertion failed: ${message}`);
      this.runtime.showWarningMessage('ripr could not copy the suggested assertion. See ripr output for details.');
    }
  }

  async copyTargetedTestBrief(target?: RiprTargetedTestBriefTarget): Promise<void> {
    const brief = typeof target?.brief === 'string' ? target.brief.trim() : '';
    if (!brief) {
      this.runtime.showInformationMessage('No ripr targeted test brief is available for this diagnostic.');
      return;
    }
    try {
      await this.runtime.writeClipboard(brief);
      this.runtime.showInformationMessage('Copied ripr targeted test brief to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy targeted test brief failed: ${message}`);
      this.runtime.showWarningMessage('ripr could not copy the targeted test brief. See ripr output for details.');
    }
  }

  async copyAgentLoopCommand(target?: RiprAgentLoopCommandTarget): Promise<void> {
    const command = typeof target?.command === 'string' ? target.command.trim() : '';
    if (!command) {
      this.runtime.showInformationMessage('No ripr agent loop command is available for this diagnostic.');
      return;
    }
    try {
      await this.runtime.writeClipboard(command);
      this.runtime.showInformationMessage('Copied ripr agent loop command to clipboard.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy agent loop command failed: ${message}`);
      this.runtime.showWarningMessage('ripr could not copy the agent loop command. See ripr output for details.');
    }
  }

  async openRelatedTest(target?: RiprRelatedTestTarget): Promise<void> {
    const uri = uriFromTarget(target);
    if (!uri) {
      this.runtime.showInformationMessage('No ripr related test location is available for this diagnostic.');
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
      this.runtime.showWarningMessage('ripr could not open the related test. See ripr output for details.');
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
    this.firstUsefulAction = this.readFirstUsefulActionProjection();
    for (const line of firstUsefulActionStatusLines(this.status.kind, this.firstUsefulAction)) {
      this.output.appendLine(line);
    }
    this.output.show();
    this.runtime.showInformationMessage(this.status.summary);
  }

  private handleServerLog(params: unknown): void {
    const message = serverLogMessage(params);
    if (!message) {
      return;
    }
    if (message.startsWith('ripr analysis refresh queued')) {
      this.updateStatus({
        kind: 'analysisQueued',
        summary: 'ripr saved-workspace analysis is queued.',
        detail: message
      });
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
      this.updateStatus(this.statusAfterRefreshCompleted(message));
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

  private statusAfterRefreshCompleted(message: string): RiprStatusState {
    if (this.dirtyRustDocuments.size === 0) {
      return statusFromRefreshCompletedMessage(message);
    }
    return {
      kind: 'stale',
      summary: 'ripr analysis completed, but unsaved Rust changes remain.',
      detail: [
        message,
        'Current diagnostics describe the last saved workspace state.',
        `Unsaved Rust files: ${Array.from(this.dirtyRustDocuments).join(', ')}`
      ].join('\n')
    };
  }

  private updateStatus(status: RiprStatusState): void {
    this.status = status;
    this.firstUsefulAction = this.readFirstUsefulActionProjection();
    if (!this.statusBar) {
      return;
    }
    this.statusBar.text = statusText(status.kind, this.firstUsefulAction);
    const detailLines = [
      status.summary,
      status.detail,
      ...firstUsefulActionStatusLines(status.kind, this.firstUsefulAction)
    ].filter((line): line is string => Boolean(line));
    this.statusBar.tooltip = detailLines.join('\n');
    this.statusBar.command = 'ripr.showStatus';
    this.statusBar.show();
  }

  private readFirstUsefulActionProjection(): FirstUsefulActionProjection | undefined {
    if (!this.workspaceRoot) {
      return undefined;
    }
    const reportPath = path.join(
      this.workspaceRoot,
      'target',
      'ripr',
      'reports',
      'first-useful-action.json'
    );
    let reportText: string;
    try {
      reportText = fs.readFileSync(reportPath, 'utf8');
    } catch {
      return undefined;
    }

    let report: unknown;
    try {
      report = JSON.parse(reportText);
    } catch {
      return undefined;
    }
    return firstUsefulActionProjectionFromReport(report, this.workspaceRoot, reportPath);
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
    const selection = await this.runtime.showErrorMessage(
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
  | 'disabled'
  | 'noWorkspace'
  | 'resolvingServer'
  | 'serverUnavailable'
  | 'starting'
  | 'analysisQueued'
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

function statusText(kind: RiprStatusKind, firstAction?: FirstUsefulActionProjection): string {
  if (firstAction && canProjectFirstUsefulAction(kind)) {
    switch (firstAction.status) {
      case 'actionable':
        return '$(lightbulb) ripr: action available';
      case 'stale':
        return '$(warning) ripr: refresh action';
      case 'already_improved':
      case 'no_actionable_seam':
        return '$(pass) ripr: no action';
      default:
        return '$(info) ripr: first action';
    }
  }
  switch (kind) {
    case 'disabled':
      return '$(circle-slash) ripr: disabled';
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
    case 'analysisQueued':
      return '$(clock) ripr: queued';
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

function firstUsefulActionProjectionFromReport(
  report: unknown,
  workspaceRoot: string,
  reportPath: string
): FirstUsefulActionProjection | undefined {
  const object = objectValue(report);
  if (!object || stringField(object, 'kind') !== 'first_useful_action') {
    return undefined;
  }
  if (!rootMatchesWorkspace(stringField(object, 'root'), workspaceRoot)) {
    return undefined;
  }
  const status = stringField(object, 'status');
  const actionKind = stringField(object, 'action_kind');
  const title = stringField(object, 'title');
  if (!status || !actionKind || !title) {
    return undefined;
  }
  const selected = objectField(object, 'selected');
  const target = objectField(object, 'target');
  const commands = objectField(object, 'commands');
  return {
    reportPath: relativeWorkspacePath(workspaceRoot, reportPath),
    status,
    actionKind,
    title,
    why: stringField(object, 'why'),
    seamId: selected ? stringField(selected, 'seam_id') : undefined,
    targetFile: target ? stringField(target, 'file') : undefined,
    verifyCommandAvailable: Boolean(commands && stringField(commands, 'verify')),
    receiptCommandAvailable: Boolean(commands && stringField(commands, 'receipt')),
  };
}

function firstUsefulActionStatusLines(
  kind: RiprStatusKind,
  firstAction?: FirstUsefulActionProjection
): string[] {
  if (!firstAction) {
    return [];
  }
  if (!canProjectFirstUsefulAction(kind)) {
    if (kind === 'stale') {
      return [
        'First useful action report: available, but editor evidence is stale.',
        'Save or refresh the Rust workspace before acting on this report.',
        `Report: ${firstAction.reportPath}`,
      ];
    }
    return [
      'First useful action report: available, but editor analysis is not ready.',
      `Report: ${firstAction.reportPath}`,
    ];
  }

  const lines = [
    'First useful action:',
    `Status: ${firstAction.status}`,
    `Action: ${firstAction.actionKind}`,
    `Top action: ${firstAction.title}`,
  ];
  if (firstAction.why) {
    lines.push(`Why: ${firstAction.why}`);
  }
  if (firstAction.seamId) {
    lines.push(`Seam: ${firstAction.seamId}`);
  }
  if (firstAction.targetFile) {
    lines.push(`Target: ${firstAction.targetFile}`);
  }
  lines.push(`Verify command: ${firstAction.verifyCommandAvailable ? 'available' : 'not available'}`);
  lines.push(`Receipt command: ${firstAction.receiptCommandAvailable ? 'available' : 'not available'}`);
  lines.push(`Report: ${firstAction.reportPath}`);
  return lines;
}

function canProjectFirstUsefulAction(kind: RiprStatusKind): boolean {
  return kind === 'analysisReady' || kind === 'noActionableSeams' || kind === 'ready';
}

function rootMatchesWorkspace(root: string | undefined, workspaceRoot: string): boolean {
  if (!root || root === '.') {
    return true;
  }
  const resolvedRoot = path.isAbsolute(root)
    ? path.resolve(root)
    : path.resolve(workspaceRoot, root);
  return normalizePath(resolvedRoot) === normalizePath(path.resolve(workspaceRoot));
}

function relativeWorkspacePath(workspaceRoot: string, filePath: string): string {
  const relativePath = path.relative(workspaceRoot, filePath);
  return relativePath && !relativePath.startsWith('..') && !path.isAbsolute(relativePath)
    ? relativePath.replace(/\\/g, '/')
    : filePath;
}

function normalizePath(value: string): string {
  const normalized = path.normalize(value).replace(/\\/g, '/');
  return process.platform === 'win32' ? normalized.toLowerCase() : normalized;
}

function objectField(value: Record<string, unknown>, field: string): Record<string, unknown> | undefined {
  return objectValue(value[field]);
}

function stringField(value: Record<string, unknown>, field: string): string | undefined {
  const fieldValue = value[field];
  if (typeof fieldValue !== 'string') {
    return undefined;
  }
  const trimmed = fieldValue.trim();
  return trimmed ? trimmed : undefined;
}

function objectValue(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
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

function isRustFileDocument(document: vscode.TextDocument): boolean {
  return document.languageId === 'rust' && document.uri.scheme === 'file';
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
