import * as cp from 'child_process';
import { promises as fs } from 'fs';
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
  root?: string;
  base?: string;
  mode?: string;
  seam_id?: string;
  target_artifact?: string;
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
  readFile(filePath: string): Promise<string | undefined>;
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
  readFile: readOptionalFile,
  runRipr,
  writeClipboard: async (text) => {
    await vscode.env.clipboard.writeText(text);
    await writeTestClipboardCapture(text);
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
  private firstUsefulAction: FirstUsefulActionStatus | undefined;
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
    await this.refreshFirstUsefulActionStatus();
  }

  async restart(): Promise<void> {
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    const client = this.client;
    this.client = undefined;
    this.server = undefined;
    this.firstUsefulAction = undefined;
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
    const command = validatedAgentLoopCommand(target);
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

  showStatus(): Promise<void> {
    return this.showStatusAsync();
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
      void this.refreshFirstUsefulActionStatus();
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
    this.renderStatusBar();
  }

  private renderStatusBar(): void {
    if (!this.statusBar) {
      return;
    }
    this.statusBar.text = statusText(this.status.kind, this.firstUsefulAction);
    this.statusBar.tooltip = statusTooltip(this.status, this.firstUsefulAction);
    this.statusBar.command = 'ripr.showStatus';
    this.statusBar.show();
  }

  private async showStatusAsync(): Promise<void> {
    await this.refreshFirstUsefulActionStatus();
    this.output.appendLine(`ripr status: ${statusSummary(this.status, this.firstUsefulAction)}`);
    const detail = statusTooltip(this.status, this.firstUsefulAction);
    if (detail) {
      this.output.appendLine(detail);
    }
    this.output.show();
    this.runtime.showInformationMessage(statusSummary(this.status, this.firstUsefulAction));
  }

  private async refreshFirstUsefulActionStatus(): Promise<void> {
    const workspaceRoot = this.workspaceRoot;
    if (!workspaceRoot) {
      this.firstUsefulAction = undefined;
      this.renderStatusBar();
      return;
    }
    const reportPath = firstUsefulActionReportPath(workspaceRoot);
    try {
      const report = await this.runtime.readFile(reportPath);
      this.firstUsefulAction = report
        ? parseFirstUsefulAction(report, workspaceRoot, reportPath)
        : undefined;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.firstUsefulAction = undefined;
      this.output.appendLine(`ripr first useful action status unavailable: ${message}`);
    }
    this.renderStatusBar();
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
  | 'noEnabledLanguages'
  | 'stale'
  | 'analysisFailed'
  | 'stopped';

interface RiprStatusState {
  kind: RiprStatusKind;
  summary: string;
  detail?: string;
}

interface FirstUsefulActionStatus {
  status: string;
  actionKind: string;
  title: string;
  selectedLocation?: string;
  missingDiscriminator?: string;
  target?: string;
  relatedTest?: string;
  verifyCommand?: string;
  receiptCommand?: string;
  fallback?: string;
  reportPath: string;
  warningCount: number;
}

function statusText(kind: RiprStatusKind, firstAction?: FirstUsefulActionStatus): string {
  if (firstAction && canProjectFirstUsefulAction(kind)) {
    if (
      firstAction.status === 'stale' ||
      firstAction.status === 'missing_required_artifact' ||
      firstAction.status === 'unchanged_after_attempt'
    ) {
      return '$(warning) ripr: first action';
    }
    if (
      firstAction.status === 'already_improved' ||
      firstAction.status === 'baseline_only' ||
      firstAction.status === 'no_actionable_seam' ||
      firstAction.status === 'suppressed' ||
      firstAction.status === 'acknowledged' ||
      firstAction.status === 'waived'
    ) {
      return '$(pass) ripr: first action';
    }
    return '$(lightbulb) ripr: first action';
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
    case 'noEnabledLanguages':
      return '$(circle-slash) ripr: languages off';
    case 'stale':
      return '$(warning) ripr: stale';
    case 'analysisFailed':
      return '$(error) ripr: failed';
    case 'stopped':
    default:
      return 'ripr: stopped';
  }
}

function statusSummary(status: RiprStatusState, firstAction?: FirstUsefulActionStatus): string {
  if (!firstAction || !canProjectFirstUsefulAction(status.kind)) {
    return status.summary;
  }
  return `${status.summary} First useful action: ${firstAction.title}`;
}

function statusTooltip(status: RiprStatusState, firstAction?: FirstUsefulActionStatus): string {
  const lines = [status.summary];
  if (status.detail) {
    lines.push(status.detail);
  }
  if (firstAction && canProjectFirstUsefulAction(status.kind)) {
    lines.push('', ...firstUsefulActionLines(firstAction));
  } else if (firstAction && status.kind === 'stale') {
    lines.push(
      '',
      'First useful action report: available, but editor evidence is stale.',
      'Save or refresh the Rust workspace before acting on this report.',
      `Report: ${firstAction.reportPath}`
    );
  }
  return lines.join('\n');
}

function firstUsefulActionLines(firstAction: FirstUsefulActionStatus): string[] {
  const lines = [
    `First useful action: ${firstAction.title}`,
    `Status: ${firstAction.status}`,
    `Action: ${firstAction.actionKind}`,
  ];
  if (firstAction.selectedLocation) {
    lines.push(`Seam: ${firstAction.selectedLocation}`);
  }
  if (firstAction.missingDiscriminator) {
    lines.push(`Missing discriminator: ${firstAction.missingDiscriminator}`);
  }
  if (firstAction.target) {
    lines.push(`Target: ${firstAction.target}`);
  }
  if (firstAction.relatedTest) {
    lines.push(`Related test: ${firstAction.relatedTest}`);
  }
  if (firstAction.verifyCommand) {
    lines.push(`Verify: ${firstAction.verifyCommand}`);
  }
  if (firstAction.receiptCommand) {
    lines.push(`Receipt: ${firstAction.receiptCommand}`);
  }
  if (firstAction.fallback) {
    lines.push(`Fallback: ${firstAction.fallback}`);
  }
  lines.push(`Report: ${firstAction.reportPath}`);
  lines.push(`Warnings: ${firstAction.warningCount}`);
  lines.push('Advisory static evidence only; gate evaluation remains the pass/fail authority.');
  return lines;
}

function canProjectFirstUsefulAction(kind: RiprStatusKind): boolean {
  return kind === 'starting'
    || kind === 'analysisQueued'
    || kind === 'analysisRunning'
    || kind === 'analysisReady'
    || kind === 'noActionableSeams'
    || kind === 'noEnabledLanguages'
    || kind === 'ready';
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
  const enabledLanguages = numberField(message, 'enabled_languages');
  if (enabledLanguages === 0) {
    return {
      kind: 'noEnabledLanguages',
      summary: 'ripr analysis completed with no enabled languages.',
      detail: [
        message,
        'No saved-workspace diagnostics are published because ripr.toml has [languages] enabled = [].',
        'Enable rust, or an available preview language when that routing exists, to restore editor diagnostics.'
      ].join('\n')
    };
  }
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

function firstUsefulActionReportPath(workspaceRoot: string): string {
  return path.join(workspaceRoot, 'target', 'ripr', 'reports', 'first-useful-action.json');
}

async function readOptionalFile(filePath: string): Promise<string | undefined> {
  try {
    return await fs.readFile(filePath, 'utf8');
  } catch (error) {
    if (isFileNotFound(error)) {
      return undefined;
    }
    throw error;
  }
}

function isFileNotFound(error: unknown): boolean {
  return typeof error === 'object' && error !== null && 'code' in error
    && (error as { code?: unknown }).code === 'ENOENT';
}

function parseFirstUsefulAction(
  raw: string,
  workspaceRoot: string,
  reportPath: string
): FirstUsefulActionStatus | undefined {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return undefined;
  }
  if (!parsed || typeof parsed !== 'object') {
    return undefined;
  }
  const report = parsed as Record<string, unknown>;
  if (stringField(report, 'schema_version') !== '0.1') {
    return undefined;
  }
  if (stringField(report, 'kind') !== 'first_useful_action') {
    return undefined;
  }
  const status = boundedStringField(report, 'status', FIRST_USEFUL_ACTION_STATUSES);
  const actionKind = boundedStringField(report, 'action_kind', FIRST_USEFUL_ACTION_ACTIONS);
  const title = stringField(report, 'title');
  if (!status || !actionKind || !title) {
    return undefined;
  }
  if (!boundedStringField(report, 'audience', FIRST_USEFUL_ACTION_AUDIENCES)) {
    return undefined;
  }
  if (!rootMatchesWorkspace(stringField(report, 'root'), workspaceRoot)) {
    return undefined;
  }
  const selected = objectField(report, 'selected');
  const target = objectField(report, 'target');
  const commands = objectField(report, 'commands');
  const fallback = objectField(report, 'fallback');
  return {
    status,
    actionKind,
    title,
    selectedLocation: selectedLocation(selected),
    missingDiscriminator: selected ? stringField(selected, 'missing_discriminator') : undefined,
    target: target ? stringField(target, 'file') : undefined,
    relatedTest: target ? stringField(target, 'related_test') : undefined,
    verifyCommand: commands ? stringField(commands, 'verify') : undefined,
    receiptCommand: commands ? stringField(commands, 'receipt') : undefined,
    fallback: fallback
      ? stringField(fallback, 'summary') ?? stringField(fallback, 'kind')
      : undefined,
    reportPath: relativeWorkspacePath(workspaceRoot, reportPath),
    warningCount: arrayLength(report, 'warnings'),
  };
}

const FIRST_USEFUL_ACTION_STATUSES = new Set([
  'actionable',
  'stale',
  'missing_required_artifact',
  'baseline_only',
  'acknowledged',
  'waived',
  'suppressed',
  'no_actionable_seam',
  'already_improved',
  'unchanged_after_attempt'
]);

const FIRST_USEFUL_ACTION_ACTIONS = new Set([
  'write_focused_test',
  'refresh_evidence',
  'generate_missing_artifact',
  'acknowledge_baseline',
  'inspect_proof_report',
  'revise_focused_test',
  'no_action'
]);

const FIRST_USEFUL_ACTION_AUDIENCES = new Set([
  'developer',
  'reviewer',
  'agent'
]);

interface AgentLoopCommandContract {
  targetArtifact: string;
  startsWith: string;
  includes: string[];
  requiresSeamId: boolean;
}

const AGENT_LOOP_COMMAND_CONTRACTS: Record<string, AgentLoopCommandContract> = {
  agent_packet: {
    targetArtifact: 'target/ripr/agent/agent-packet.json',
    startsWith: 'ripr agent packet --root . --seam-id ',
    includes: [' --json > target/ripr/agent/agent-packet.json'],
    requiresSeamId: true
  },
  agent_brief: {
    targetArtifact: 'target/ripr/agent/agent-brief.json',
    startsWith: 'ripr agent brief --root . --seam-id ',
    includes: [' --json > target/ripr/agent/agent-brief.json'],
    requiresSeamId: true
  },
  after_snapshot: {
    targetArtifact: 'target/ripr/pilot/after.repo-exposure.json',
    startsWith: 'ripr check --root .',
    includes: [' --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json'],
    requiresSeamId: false
  },
  agent_verify: {
    targetArtifact: 'target/ripr/agent/agent-verify.json',
    startsWith: 'ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json',
    includes: [' > target/ripr/agent/agent-verify.json'],
    requiresSeamId: false
  },
  agent_receipt: {
    targetArtifact: 'target/ripr/agent/agent-receipt.json',
    startsWith: 'ripr agent receipt --root . --verify-json target/ripr/agent/agent-verify.json --seam-id ',
    includes: [' --json --out target/ripr/agent/agent-receipt.json'],
    requiresSeamId: true
  }
};

function validatedAgentLoopCommand(target?: RiprAgentLoopCommandTarget): string | undefined {
  if (!target) {
    return undefined;
  }
  const label = typeof target?.label === 'string' ? target.label : '';
  const contract = AGENT_LOOP_COMMAND_CONTRACTS[label];
  if (!contract) {
    return undefined;
  }
  const command = typeof target?.command === 'string' ? target.command.trim() : '';
  if (!command || hasUnsafeShellMetacharacter(command)) {
    return undefined;
  }
  if (target.root !== '.') {
    return undefined;
  }
  if (
    typeof target.target_artifact !== 'string' ||
    target.target_artifact !== contract.targetArtifact
  ) {
    return undefined;
  }
  if (contract.requiresSeamId && !boundedPayloadString(target.seam_id)) {
    return undefined;
  }
  if (
    contract.requiresSeamId &&
    !command.includes(` --seam-id ${shellArgToken(target.seam_id)} `)
  ) {
    return undefined;
  }
  if (!command.startsWith(contract.startsWith)) {
    return undefined;
  }
  if (!contract.includes.every((expected) => command.includes(expected))) {
    return undefined;
  }
  if (label === 'after_snapshot' && !afterSnapshotModeMatches(target.mode, command)) {
    return undefined;
  }
  if (
    label === 'after_snapshot' &&
    boundedPayloadString(target.base) &&
    !command.includes(` --base ${shellArgToken(target.base)} `)
  ) {
    return undefined;
  }
  return command;
}

function afterSnapshotModeMatches(mode: unknown, command: string): boolean {
  if (typeof mode !== 'string' || !['instant', 'draft', 'fast', 'deep', 'ready'].includes(mode)) {
    return false;
  }
  return command.includes(` --mode ${mode} `);
}

function boundedPayloadString(value: unknown): boolean {
  return typeof value === 'string' && value.length > 0 && value.length <= 256;
}

function hasUnsafeShellMetacharacter(command: string): boolean {
  return /[\r\n\0`;&|\\]/.test(command);
}

function shellArgToken(value: unknown): string {
  if (typeof value !== 'string') {
    return '';
  }
  return /^[A-Za-z0-9_./:-]+$/.test(value)
    ? value
    : `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
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
  const child = value[field];
  return child && typeof child === 'object' && !Array.isArray(child)
    ? child as Record<string, unknown>
    : undefined;
}

function stringField(value: Record<string, unknown>, field: string): string | undefined {
  const child = value[field];
  return typeof child === 'string' && child.trim() !== '' ? child : undefined;
}

function boundedStringField(
  value: Record<string, unknown>,
  field: string,
  allowed: Set<string>
): string | undefined {
  const child = stringField(value, field);
  return child && allowed.has(child) ? child : undefined;
}

function numberFieldValue(value: Record<string, unknown>, field: string): number | undefined {
  const child = value[field];
  return typeof child === 'number' && Number.isFinite(child) ? child : undefined;
}

function arrayLength(value: Record<string, unknown>, field: string): number {
  const child = value[field];
  return Array.isArray(child) ? child.length : 0;
}

function selectedLocation(selected: Record<string, unknown> | undefined): string | undefined {
  if (!selected) {
    return undefined;
  }
  const selectedPath = stringField(selected, 'path');
  if (!selectedPath) {
    return undefined;
  }
  const line = numberFieldValue(selected, 'line');
  return line === undefined ? selectedPath : `${selectedPath}:${Math.trunc(line)}`;
}

function firstWorkspaceFolder(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

function isRustFileDocument(document: vscode.TextDocument): boolean {
  return document.languageId === 'rust' && document.uri.scheme === 'file';
}

async function writeTestClipboardCapture(text: string): Promise<void> {
  const capturePath = process.env.RIPR_TEST_CLIPBOARD_CAPTURE_PATH;
  if (!capturePath) {
    return;
  }
  await fs.writeFile(capturePath, text, 'utf8');
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
