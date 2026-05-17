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

const RIPR_DOCUMENT_SELECTORS: Array<{ language: string; scheme: 'file' }> = [
  { language: 'rust', scheme: 'file' },
  { language: 'typescript', scheme: 'file' },
  { language: 'typescriptreact', scheme: 'file' },
  { language: 'javascript', scheme: 'file' },
  { language: 'javascriptreact', scheme: 'file' },
  { language: 'python', scheme: 'file' }
];

const RIPR_FILE_LANGUAGES = new Set(RIPR_DOCUMENT_SELECTORS.map((selector) => selector.language));
const RIPR_RELATED_TEST_LANGUAGE_BY_EXTENSION = new Map<string, 'rust' | 'typescript' | 'python'>([
  ['.rs', 'rust'],
  ['.ts', 'typescript'],
  ['.tsx', 'typescript'],
  ['.js', 'typescript'],
  ['.jsx', 'typescript'],
  ['.py', 'python']
]);
const RIPR_CONFIG_RELATIVE_PATH = 'ripr.toml';
const RIPR_SETUP_ARTIFACTS: RiprSetupArtifactDefinition[] = [
  {
    label: 'first useful action report',
    relativePath: 'target/ripr/reports/first-useful-action.json'
  },
  {
    label: 'gap decision ledger',
    relativePath: 'target/ripr/reports/gap-decision-ledger.json'
  },
  {
    label: 'editor agent receipt',
    relativePath: 'target/ripr/agent/agent-receipt.json'
  }
];
const RIPR_FIRST_PR_PACKET_ARTIFACTS = [
  {
    jsonRelativePath: 'target/ripr/reports/start-here.json',
    markdownRelativePath: 'target/ripr/reports/start-here.md'
  },
  {
    jsonRelativePath: 'target/ripr/first-pr/start-here.json',
    markdownRelativePath: 'target/ripr/first-pr/start-here.md'
  }
];

export interface RiprContextTarget {
  uri?: string;
  line?: number;
  label?: string;
  packet?: string;
  note?: string;
  finding_id?: string;
  probe_id?: string;
  seam_id?: string;
  seam_kind?: string;
  gap_id?: string;
  canonical_gap_id?: string;
  gap_kind?: string;
  gap_ledger?: string;
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

interface StartRepairAction {
  title: string;
  command: vscode.Command;
  priority: number;
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
  private readonly dirtyRiprDocuments = new Set<string>();
  private firstUsefulAction: FirstUsefulActionStatus | undefined;
  private setupStatus: RiprSetupStatus = setupStatusWithoutWorkspace();
  private status: RiprStatusState = {
    kind: 'stopped',
    summary: 'ripr server has not started.',
    detail: 'Open a workspace or run ripr: Restart Server.',
    nextStep: 'Open a workspace folder, then run ripr: Restart Server.'
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
    this.workspaceRoot = this.runtime.workspaceRoot();
    await this.refreshSetupStatusFiles();

    if (!config.enabled) {
      this.updateStatus({
        kind: 'disabled',
        summary: 'ripr editor analysis is disabled by configuration.',
        detail: 'Set ripr.enabled to true to start saved-workspace diagnostics.',
        nextStep: 'Set ripr.enabled to true, then run ripr: Restart Server.'
      });
      this.output.appendLine('ripr editor analysis is disabled by configuration.');
      return;
    }

    if (!this.workspaceRoot) {
      this.updateStatus({
        kind: 'noWorkspace',
        summary: 'Open a workspace for ripr diagnostics.',
        detail: 'The extension needs a workspace folder before it can start the language server.',
        nextStep: 'Open a workspace folder, then run ripr: Restart Server.'
      });
      this.output.appendLine('ripr workspace was not detected; open a workspace folder.');
      return;
    }

    this.updateStatus({
      kind: 'resolvingServer',
      summary: 'Resolving ripr server.',
      detail: `Workspace: ${this.workspaceRoot}`,
      nextStep: 'Wait for server resolution, or use ripr: Show Output if it stalls.'
    });
    const server = await this.runtime.resolveServer(this.context, config, this.output);
    if (!('command' in server)) {
      this.updateStatus({
        kind: 'serverUnavailable',
        summary: 'ripr server is not available.',
        detail: server.detail,
        nextStep: 'Set ripr.server.path, enable ripr.server.autoDownload, install with cargo install ripr, then retry.'
      });
      await this.showMissingServerMessage(server.message, server.detail);
      return;
    }
    this.server = server;
    this.updateStatus({
      kind: 'starting',
      summary: 'Starting ripr language server.',
      detail: `Server: ${server.source} (${server.detail})\nWorkspace: ${this.workspaceRoot}`,
      nextStep: 'Wait for server startup, or use ripr: Show Output if it stalls.'
    });

    const serverOptions: ServerOptions = {
      command: server.command,
      args: config.serverArgs,
      options: {
        cwd: this.workspaceRoot
      }
    };

    const clientOptions: LanguageClientOptions = {
      documentSelector: RIPR_DOCUMENT_SELECTORS,
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
    await this.refreshSetupStatusFiles();
    this.updateStatus({
      kind: 'analysisQueued',
      summary: 'ripr saved-workspace analysis is queued.',
      detail: `Server: ${server.source} (${server.detail})\nWorkspace: ${this.workspaceRoot}\nOpen or save a Rust or enabled preview-language file to refresh diagnostics.`,
      nextStep: 'Open or save a Rust or enabled preview-language file, then wait for diagnostics.'
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
    this.dirtyRiprDocuments.clear();
    while (this.notificationDisposables.length > 0) {
      this.notificationDisposables.pop()?.dispose();
    }
    if (client) {
      await client.stop();
    }
    this.updateStatus({
      kind: 'stopped',
      summary: 'ripr server has stopped.',
      detail: 'Run ripr: Restart Server to start analysis again.',
      nextStep: 'Run ripr: Restart Server.'
    });
  }

  markWorkspaceStale(document: vscode.TextDocument): void {
    if (!this.client || !isRiprFileDocument(document)) {
      return;
    }
    this.dirtyRiprDocuments.add(document.uri.toString());
    this.updateStatus({
      kind: 'stale',
      summary: 'ripr analysis is stale until the file is saved.',
      detail: `Unsaved changes: ${document.uri.fsPath}`,
      nextStep: 'Save the file, then wait for ripr to refresh saved-workspace diagnostics.'
    });
  }

  markWorkspaceSaved(document: vscode.TextDocument): void {
    if (!this.client || !isRiprFileDocument(document)) {
      return;
    }
    this.dirtyRiprDocuments.delete(document.uri.toString());
    if (this.dirtyRiprDocuments.size === 0 && this.status.kind === 'stale') {
      this.updateStatus({
        kind: 'analysisQueued',
        summary: 'ripr saved-workspace analysis is queued after save.',
        detail: `Saved changes: ${document.uri.fsPath}`,
        nextStep: 'Wait for ripr to refresh diagnostics.'
      });
    }
  }

  markWorkspaceClosed(document: vscode.TextDocument): void {
    if (!isRiprFileDocument(document)) {
      return;
    }
    this.dirtyRiprDocuments.delete(document.uri.toString());
    if (this.client && this.dirtyRiprDocuments.size === 0 && this.status.kind === 'stale') {
      this.updateStatus({
        kind: 'analysisQueued',
        summary: 'ripr saved-workspace analysis is queued after close.',
        detail: `Closed unsaved ${document.languageId} buffer: ${document.uri.fsPath}`,
        nextStep: 'Wait for ripr to refresh diagnostics.'
      });
    }
  }

  async copyContext(target?: RiprContextTarget): Promise<void> {
    if (target?.label === 'first_repair_packet' && typeof target.packet === 'string') {
      const packet = target.packet.trim();
      if (!packet) {
        this.runtime.showInformationMessage('No ripr first repair packet is available for this diagnostic.');
        return;
      }
      try {
        await this.runtime.writeClipboard(packet);
        this.runtime.showInformationMessage('Copied ripr first repair packet to clipboard.');
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        this.output.appendLine(`ripr copy first repair packet failed: ${message}`);
        this.runtime.showWarningMessage('ripr could not copy the first repair packet. See ripr output for details.');
      }
      return;
    }

    if (target?.label === 'static_limit_note' && typeof target.note === 'string') {
      const note = target.note.trim();
      if (!note) {
        this.runtime.showInformationMessage('No ripr static-limit note is available for this diagnostic.');
        return;
      }
      try {
        await this.runtime.writeClipboard(note);
        this.runtime.showInformationMessage('Copied ripr static-limit note to clipboard.');
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        this.output.appendLine(`ripr copy static-limit note failed: ${message}`);
        this.runtime.showWarningMessage('ripr could not copy the static-limit note. See ripr output for details.');
      }
      return;
    }

    const targetUri = uriFromTarget(target);
    const editor = vscode.window.activeTextEditor;
    const documentUri = targetUri ?? editor?.document.uri;
    if (!documentUri) {
      this.runtime.showInformationMessage('Open a Rust file before copying ripr context.');
      return;
    }

    const client = this.client;
    if (client && (target?.finding_id || target?.seam_id || target?.gap_id)) {
      try {
        const collectContextTarget: RiprContextTarget = {
          finding_id: target.finding_id,
          probe_id: target.probe_id,
          seam_id: target.seam_id,
          seam_kind: target.seam_kind,
          uri: target.uri,
          line: target.line,
        };
        if (target.gap_id) {
          collectContextTarget.gap_id = target.gap_id;
          collectContextTarget.canonical_gap_id = target.canonical_gap_id;
          collectContextTarget.gap_kind = target.gap_kind;
          collectContextTarget.gap_ledger = target.gap_ledger;
        }
        const packet = await client.sendRequest('workspace/executeCommand', {
          command: 'ripr.collectContext',
          arguments: [collectContextTarget],
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

  async startCurrentRepair(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isRiprFileDocument(editor.document)) {
      this.runtime.showInformationMessage('Open a Rust, TypeScript/JavaScript, or Python file before starting a ripr repair.');
      return;
    }
    const diagnostic = nearestGapDiagnostic(editor);
    if (!diagnostic) {
      this.runtime.showInformationMessage('No current ripr repair gap is available near the active selection.');
      return;
    }

    try {
      await vscode.commands.executeCommand('editor.action.showHover');
    } catch {
      // Hover is an ergonomic hint only; code actions remain the source of truth.
    }

    let actions: Array<vscode.CodeAction | vscode.Command> | undefined;
    try {
      actions = await vscode.commands.executeCommand<Array<vscode.CodeAction | vscode.Command>>(
        'vscode.executeCodeActionProvider',
        editor.document.uri,
        diagnostic.range
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr start current repair failed to collect code actions: ${message}`);
      this.runtime.showWarningMessage('ripr could not collect current repair actions. See ripr output for details.');
      return;
    }

    const candidates = startRepairActions(actions ?? []);
    if (candidates.length === 0) {
      this.runtime.showInformationMessage('No bounded ripr repair action is available for the current gap. Refresh saved-workspace analysis if this looks stale.');
      return;
    }

    const selected = candidates.length === 1
      ? candidates[0]
      : await pickStartRepairAction(candidates);
    if (!selected) {
      return;
    }
    await vscode.commands.executeCommand(
      selected.command.command,
      ...(selected.command.arguments ?? [])
    );
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
    if (uri.scheme !== 'file') {
      this.runtime.showInformationMessage('ripr related test navigation requires a file URI.');
      return;
    }
    if (!vscode.workspace.getWorkspaceFolder(uri)) {
      this.runtime.showInformationMessage('ripr related test must be inside the current workspace.');
      return;
    }
    const language = riprRelatedTestLanguage(uri.fsPath);
    if (!language) {
      this.runtime.showInformationMessage('ripr related test must be a Rust, TypeScript/JavaScript, or Python file.');
      return;
    }
    if (this.status.kind === 'stale') {
      this.runtime.showInformationMessage('ripr related test navigation requires current saved-workspace analysis; save or refresh first.');
      return;
    }
    if (this.status.enabledLanguages && !this.status.enabledLanguages.includes(language)) {
      this.runtime.showInformationMessage(`ripr related test language is disabled by current analysis status: ${language}.`);
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

  diagnoseSetup(): Promise<void> {
    return this.diagnoseSetupAsync();
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
        detail: message,
        nextStep: 'Wait for the current saved-workspace analysis refresh to finish.'
      });
      return;
    }
    if (message.startsWith('ripr analysis refresh started')) {
      this.updateStatus({
        kind: 'analysisRunning',
        summary: 'ripr saved-workspace analysis is running.',
        detail: message,
        nextStep: 'Wait for the current saved-workspace analysis refresh to finish.'
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
        detail: message,
        nextStep: 'Open ripr: Show Output, fix the reported issue, then run ripr: Restart Server.'
      });
    }
  }

  private statusAfterRefreshCompleted(message: string): RiprStatusState {
    if (this.dirtyRiprDocuments.size === 0) {
      return statusFromRefreshCompletedMessage(message);
    }
    return {
      kind: 'stale',
      summary: 'ripr analysis completed, but unsaved routed-file changes remain.',
      detail: [
        message,
        'Current diagnostics describe the last saved workspace state.',
        `Unsaved routed files: ${Array.from(this.dirtyRiprDocuments).join(', ')}`
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
    this.statusBar.tooltip = statusTooltip(this.status, this.firstUsefulAction, this.statusContext());
    this.statusBar.command = 'ripr.showStatus';
    this.statusBar.show();
  }

  private async showStatusAsync(): Promise<void> {
    await this.refreshSetupStatusFiles();
    await this.refreshFirstUsefulActionStatus();
    this.output.appendLine(`ripr status: ${statusSummary(this.status, this.firstUsefulAction)}`);
    const detail = statusTooltip(this.status, this.firstUsefulAction, this.statusContext());
    if (detail) {
      this.output.appendLine(detail);
    }
    this.output.show();
    this.runtime.showInformationMessage(statusSummary(this.status, this.firstUsefulAction));
  }

  private async diagnoseSetupAsync(): Promise<void> {
    await this.refreshSetupStatusFiles();
    await this.refreshFirstUsefulActionStatus();
    const report = setupDiagnosisReport(this.status, this.firstUsefulAction, this.statusContext());
    this.output.appendLine('ripr setup diagnosis:');
    this.output.appendLine(report);
    this.output.show();
    this.runtime.showInformationMessage('ripr setup diagnosis was written to the ripr output channel.');
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

  private async refreshSetupStatusFiles(): Promise<void> {
    this.setupStatus = await readSetupStatusFiles(this.workspaceRoot, this.runtime.readFile);
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

  private statusContext(): RiprStatusContext {
    return {
      workspaceRoot: this.workspaceRoot,
      server: this.server,
      documentLanguages: RIPR_DOCUMENT_SELECTORS.map((selector) => selector.language),
      setupStatus: this.setupStatus
    };
  }
}

interface RiprSetupArtifactDefinition {
  label: string;
  relativePath: string;
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
  | 'gapActionable'
  | 'gapNoAction'
  | 'gapArtifactWarning'
  | 'noActionableSeams'
  | 'noEnabledLanguages'
  | 'stale'
  | 'analysisFailed'
  | 'stopped';

interface RiprStatusState {
  kind: RiprStatusKind;
  summary: string;
  detail?: string;
  enabledLanguages?: string[];
  nextStep?: string;
}

interface RiprStatusContext {
  workspaceRoot?: string;
  server?: ResolvedServer;
  documentLanguages: string[];
  setupStatus: RiprSetupStatus;
}

type RiprSetupFileState = 'found' | 'missing' | 'unreadable' | 'noWorkspace';

interface RiprSetupFileStatus {
  label: string;
  relativePath: string;
  path?: string;
  state: RiprSetupFileState;
  detail?: string;
}

interface RiprSetupStatus {
  config: RiprSetupFileStatus;
  artifacts: RiprSetupFileStatus[];
  receipt: RiprReceiptArtifactStatus;
  firstPr: RiprFirstPrPacketStatus;
}

export type RiprFirstPrPacketState =
  | 'found'
  | 'topRepairableGap'
  | 'noAction'
  | 'blocked'
  | 'missing'
  | 'malformed'
  | 'unsupportedSchema'
  | 'wrongRoot'
  | 'unsafePath'
  | 'unsafeCommand'
  | 'unreadable'
  | 'noWorkspace';

export interface RiprFirstPrPacketStatus {
  relativePath: string;
  markdownRelativePath?: string;
  path?: string;
  markdownPath?: string;
  state: RiprFirstPrPacketState;
  detail?: string;
  status?: string;
  selectedState?: string;
  gapId?: string;
  canonicalGapId?: string;
  verifyCommand?: string;
  receiptCommand?: string;
  relatedTest?: string;
  repairTarget?: string;
  repoRoot?: string;
  warningCount?: number;
}

interface FirstUsefulActionStatus {
  status: string;
  actionKind: string;
  title: string;
  generatedAt?: string;
  seamId?: string;
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

type RiprReceiptArtifactState =
  | 'found'
  | 'missing'
  | 'unreadable'
  | 'malformed'
  | 'unsupportedSchema'
  | 'wrongRoot'
  | 'noWorkspace';

interface RiprReceiptArtifactStatus {
  relativePath: string;
  path?: string;
  state: RiprReceiptArtifactState;
  detail?: string;
  seamId?: string;
  movement?: string;
  repoRoot?: string;
  generatedAt?: string;
}

function statusText(kind: RiprStatusKind, firstAction?: FirstUsefulActionStatus): string {
  if (firstAction && shouldInlineFirstUsefulAction(kind)) {
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
    case 'gapActionable':
      return '$(lightbulb) ripr: gap ready';
    case 'gapNoAction':
      return '$(pass) ripr: gap clear';
    case 'gapArtifactWarning':
      return '$(warning) ripr: gap blocked';
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
  if (!firstAction || !shouldInlineFirstUsefulAction(status.kind)) {
    return status.summary;
  }
  return `${status.summary} First useful action: ${firstAction.title}`;
}

function statusTooltip(
  status: RiprStatusState,
  firstAction?: FirstUsefulActionStatus,
  context?: RiprStatusContext
): string {
  const lines = [status.summary];
  if (status.detail) {
    lines.push(status.detail);
  }
  if (context) {
    lines.push('', ...statusContextLines(status, context));
  }
  if (status.nextStep) {
    lines.push(`Next safe action: ${status.nextStep}`);
  }
  if (firstAction && canProjectFirstUsefulAction(status.kind)) {
    lines.push('', ...firstUsefulActionLines(firstAction));
  } else if (firstAction && status.kind === 'stale') {
    lines.push(
      '',
      'First useful action report: available, but editor evidence is stale.',
      'Save or refresh the workspace before acting on this report.',
      `Report: ${firstAction.reportPath}`
    );
  }
  if (context) {
    const receiptLines = receiptStatusLines(status, firstAction, context);
    if (receiptLines.length > 0) {
      lines.push('', ...receiptLines);
    }
  }
  return lines.join('\n');
}

function setupDiagnosisReport(
  status: RiprStatusState,
  firstAction: FirstUsefulActionStatus | undefined,
  context: RiprStatusContext
): string {
  const lines = [
    `Status: ${status.summary}`,
    ...statusContextLines(status, context)
  ];
  if (status.detail) {
    lines.push('', 'Detail:', status.detail);
  }
  if (status.nextStep) {
    lines.push('', `Next safe action: ${status.nextStep}`);
  }
  if (firstAction && canProjectFirstUsefulAction(status.kind)) {
    lines.push('', ...firstUsefulActionLines(firstAction));
  } else if (firstAction && status.kind === 'stale') {
    lines.push(
      '',
      'First useful action report: available, but editor evidence is stale.',
      'Save or refresh the workspace before acting on this report.',
      `Report: ${firstAction.reportPath}`
    );
  }
  const receiptLines = receiptStatusLines(status, firstAction, context);
  if (receiptLines.length > 0) {
    lines.push('', ...receiptLines);
  }
  lines.push(
    '',
    'Limits: read-only setup diagnosis only; no source edits, generated tests, provider calls, mutation execution, or gate decision.'
  );
  return lines.join('\n');
}

function statusContextLines(status: RiprStatusState, context: RiprStatusContext): string[] {
  const lines = [`Workspace: ${context.workspaceRoot ?? 'not open'}`];
  if (context.server) {
    lines.push(`Server: ${context.server.source} (${context.server.detail})`);
    lines.push(`Server command: ${context.server.command}`);
    lines.push(`Server version: ${context.server.version ?? 'not reported'}`);
  } else {
    lines.push('Server: not resolved');
    lines.push('Server version: not reported');
  }
  lines.push(`Server started: ${serverStartedSummary(status.kind)}`);
  lines.push(setupFileLine('Config', context.setupStatus.config));
  if (status.enabledLanguages) {
    lines.push(`Enabled languages: ${status.enabledLanguages.length > 0 ? status.enabledLanguages.join(', ') : 'none'}`);
  } else {
    lines.push('Enabled languages: not reported yet; read from ripr.toml by the server refresh.');
  }
  lines.push('Available languages: not reported by server; editor selectors can route enabled stable and preview languages.');
  lines.push(`Editor selectors: ${context.documentLanguages.join(', ')}`);
  lines.push(`Evidence freshness: ${evidenceFreshnessSummary(status.kind)}`);
  for (const artifact of context.setupStatus.artifacts) {
    lines.push(setupFileLine(`Artifact ${artifact.label}`, artifact));
  }
  return lines;
}

function setupFileLine(prefix: string, file: RiprSetupFileStatus): string {
  const detail = file.detail ? `; ${file.detail}` : '';
  return `${prefix}: ${file.relativePath} (${setupFileStateLabel(file.state)}${detail})`;
}

function setupFileStateLabel(state: RiprSetupFileState): string {
  switch (state) {
    case 'found':
      return 'found';
    case 'missing':
      return 'missing';
    case 'unreadable':
      return 'unreadable';
    case 'noWorkspace':
      return 'no workspace';
  }
}

function serverStartedSummary(kind: RiprStatusKind): string {
  switch (kind) {
    case 'analysisQueued':
    case 'analysisRunning':
    case 'analysisReady':
    case 'gapActionable':
    case 'gapNoAction':
    case 'gapArtifactWarning':
    case 'noActionableSeams':
    case 'noEnabledLanguages':
    case 'stale':
    case 'analysisFailed':
    case 'ready':
      return 'yes';
    case 'starting':
      return 'starting';
    case 'resolvingServer':
      return 'not yet; resolving server binary';
    case 'serverUnavailable':
      return 'no; server unavailable';
    case 'disabled':
      return 'no; extension disabled';
    case 'noWorkspace':
      return 'no; workspace unavailable';
    case 'stopped':
    default:
      return 'no; server stopped';
  }
}

function evidenceFreshnessSummary(kind: RiprStatusKind): string {
  switch (kind) {
    case 'stale':
      return 'stale; save or refresh before acting';
    case 'analysisQueued':
    case 'analysisRunning':
    case 'starting':
    case 'resolvingServer':
      return 'pending refresh';
    case 'analysisReady':
    case 'gapActionable':
    case 'gapNoAction':
    case 'gapArtifactWarning':
    case 'noActionableSeams':
      return 'current saved-workspace status reported by server refresh';
    case 'noEnabledLanguages':
      return 'not projected; languages are disabled';
    case 'serverUnavailable':
    case 'noWorkspace':
    case 'disabled':
    case 'stopped':
      return 'unknown; analysis is not running';
    case 'analysisFailed':
      return 'unknown; last refresh failed';
    case 'ready':
    default:
      return 'unknown until the next server refresh';
  }
}

function firstUsefulActionLines(firstAction: FirstUsefulActionStatus): string[] {
  const lines = [
    `First useful action: ${firstAction.title}`,
    `Status: ${firstAction.status}`,
    `Action: ${firstAction.actionKind}`,
  ];
  if (firstAction.seamId) {
    lines.push(`Gap identity: ${firstAction.seamId}`);
  }
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

function receiptStatusLines(
  status: RiprStatusState,
  firstAction: FirstUsefulActionStatus | undefined,
  context: RiprStatusContext
): string[] {
  const receipt = context.setupStatus.receipt;
  if (receipt.state === 'noWorkspace') {
    return [];
  }
  const currentSeam = firstAction?.seamId;
  if (status.kind === 'stale' && receipt.state === 'found') {
    return [
      'Receipt status: stale; refresh saved-workspace evidence before trusting receipt movement.',
      `Receipt: ${receipt.relativePath}`,
      'Receipt movement is not projected from stale editor evidence.'
    ];
  }
  if (!currentSeam) {
    if (receipt.state === 'missing') {
      return [];
    }
    return [
      `Receipt status: found; ${receipt.relativePath} exists, but no current gap identity is projected.`,
      'Receipt movement is not projected without a matching gap identity.'
    ];
  }
  switch (receipt.state) {
    case 'missing': {
      const lines = [
        `Receipt status: missing; no matching receipt was found for seam ${currentSeam}.`
      ];
      if (firstAction?.receiptCommand) {
        lines.push(`Receipt command: ${firstAction.receiptCommand}`);
      }
      lines.push('No receipt movement is claimed.');
      return lines;
    }
    case 'unreadable':
      return [
        `Receipt status: unreadable; ${receipt.relativePath} could not be read.`,
        receipt.detail ?? 'No reader detail was reported.',
        'Receipt movement is not projected.'
      ];
    case 'malformed':
      return [
        `Receipt status: malformed; ${receipt.relativePath} could not be parsed as an agent receipt.`,
        receipt.detail ?? 'No parser detail was reported.',
        'Receipt movement is not projected.'
      ];
    case 'unsupportedSchema':
      return [
        `Receipt status: malformed; ${receipt.relativePath} uses an unsupported receipt schema.`,
        receipt.detail ?? 'No schema detail was reported.',
        'Receipt movement is not projected.'
      ];
    case 'wrongRoot':
      return [
        `Receipt status: wrong root; receipt root ${receipt.repoRoot ?? 'unknown'} does not match this workspace.`,
        'Receipt movement is not projected.'
      ];
    case 'found':
      break;
  }
  if (!receipt.seamId) {
    return [
      `Receipt status: malformed; ${receipt.relativePath} is missing a seam identity.`,
      'Receipt movement is not projected.'
    ];
  }
  if (receipt.seamId !== currentSeam) {
    return [
      `Receipt status: gap mismatch; receipt seam ${receipt.seamId} does not match current seam ${currentSeam}.`,
      'Receipt movement is not projected.'
    ];
  }
  if (receiptIsOlderThanFirstAction(receipt, firstAction)) {
    return [
      `Receipt status: stale; receipt for seam ${currentSeam} is older than the current first useful action report.`,
      'Refresh saved-workspace evidence and rerun verify/receipt before trusting movement.'
    ];
  }
  if (receipt.movement === 'improved' || receipt.movement === 'resolved') {
    return [
      `Receipt status: movement improved; matching receipt found for seam ${currentSeam}.`,
      'Receipt records static movement only; it does not prove runtime adequacy or gate eligibility.'
    ];
  }
  if (receipt.movement === 'unchanged') {
    return [
      `Receipt status: movement unchanged; matching receipt found for seam ${currentSeam}.`,
      'Next safe action: inspect the focused test and missing discriminator before requesting another seam.'
    ];
  }
  return [
    `Receipt status: found; matching receipt exists for seam ${currentSeam}.`,
    `Receipt movement: ${receipt.movement ?? 'not reported'}`,
    'Receipt records static movement only; it does not prove runtime adequacy or gate eligibility.'
  ];
}

function receiptIsOlderThanFirstAction(
  receipt: RiprReceiptArtifactStatus,
  firstAction: FirstUsefulActionStatus | undefined
): boolean {
  const receiptTime = parseTimestamp(receipt.generatedAt);
  const firstActionTime = parseTimestamp(firstAction?.generatedAt);
  return receiptTime !== undefined && firstActionTime !== undefined && receiptTime < firstActionTime;
}

function parseTimestamp(value: string | undefined): number | undefined {
  if (!value) {
    return undefined;
  }
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function canProjectFirstUsefulAction(kind: RiprStatusKind): boolean {
  return kind === 'starting'
    || kind === 'analysisQueued'
    || kind === 'analysisRunning'
    || kind === 'analysisReady'
    || kind === 'gapActionable'
    || kind === 'gapNoAction'
    || kind === 'noActionableSeams'
    || kind === 'noEnabledLanguages'
    || kind === 'ready';
}

function shouldInlineFirstUsefulAction(kind: RiprStatusKind): boolean {
  return canProjectFirstUsefulAction(kind)
    && kind !== 'gapActionable'
    && kind !== 'gapNoAction';
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
  const previewFindings = numberField(message, 'preview_findings') ?? 0;
  const staticLimits = numberField(message, 'static_limits') ?? 0;
  const gapArtifacts = numberField(message, 'gap_artifacts') ?? 0;
  const actionableGapArtifacts = numberField(message, 'actionable_gap_artifacts') ?? 0;
  const previewGapArtifacts = numberField(message, 'preview_gap_artifacts') ?? 0;
  const noActionGapArtifacts = numberField(message, 'no_action_gap_artifacts') ?? 0;
  const gapStaticLimits = numberField(message, 'gap_static_limits') ?? 0;
  const gapArtifactRejections = numberField(message, 'gap_artifact_rejections') ?? 0;
  const enabledLanguageNames = stringListField(message, 'enabled_language_names');
  if (enabledLanguages === 0) {
    return {
      kind: 'noEnabledLanguages',
      summary: 'ripr analysis completed with no enabled languages.',
      enabledLanguages: [],
      nextStep: 'Edit ripr.toml [languages] enabled to include rust or an available preview language, then run ripr: Restart Server.',
      detail: [
        message,
        'No saved-workspace diagnostics are published because ripr.toml has [languages] enabled = [].',
        'Enable rust or an available preview language to restore editor diagnostics.'
      ].join('\n')
    };
  }
  if (gapArtifactRejections > 0) {
    const rejectionKinds = stringListField(message, 'gap_artifact_rejection_kinds') ?? [];
    const details = [
      message,
      `Rejected gap artifact ${plural(gapArtifactRejections, 'input')} ${gapArtifactRejections === 1 ? 'was' : 'were'} not projected.`
    ];
    if (rejectionKinds.length > 0) {
      details.push(`Rejected kind${rejectionKinds.length === 1 ? '' : 's'}: ${rejectionKinds.join(', ')}`);
    }
    details.push('Rejected gap artifacts never create diagnostics, hover repair routes, code actions, or receipts.');
    return {
      kind: 'gapArtifactWarning',
      summary: `ripr ignored ${gapArtifactRejections} unsafe gap artifact ${plural(gapArtifactRejections, 'input')}.`,
      enabledLanguages: enabledLanguageNames,
      nextStep: 'Regenerate ripr reports for the current workspace, then refresh saved-workspace diagnostics.',
      detail: details.join('\n')
    };
  }
  if (actionableGapArtifacts > 0) {
    const details = [message];
    if (previewGapArtifacts > 0) {
      details.push(
        `${previewGapArtifacts} preview gap artifact ${plural(previewGapArtifacts, 'input')} ${previewGapArtifacts === 1 ? 'is' : 'are'} syntax-first and advisory.`
      );
    }
    if (gapStaticLimits > 0) {
      details.push(
        `${gapStaticLimits} gap static limit ${plural(gapStaticLimits, 'entry', 'entries')} must be read before action language.`
      );
    }
    details.push(
      `${actionableGapArtifacts} actionable gap ${plural(actionableGapArtifacts, 'artifact')} validated for editor projection.`
    );
    return {
      kind: 'gapActionable',
      summary: gapStaticLimits > 0 || previewGapArtifacts > 0
        ? 'ripr validated preview-limited gap projection input.'
        : `ripr validated ${actionableGapArtifacts} actionable gap ${plural(actionableGapArtifacts, 'artifact')}.`,
      enabledLanguages: enabledLanguageNames,
      nextStep: gapStaticLimits > 0
        ? 'Read static limits before opening a related test or copying a repair, verify, or receipt command.'
        : 'Open the related test or copy a bounded repair packet, then verify and emit a receipt.',
      detail: details.join('\n')
    };
  }
  if (gapArtifacts > 0) {
    const details = [message];
    const noActionCount = noActionGapArtifacts > 0 ? noActionGapArtifacts : gapArtifacts;
    if (previewGapArtifacts > 0) {
      details.push(
        `${previewGapArtifacts} preview gap artifact ${plural(previewGapArtifacts, 'input')} ${previewGapArtifacts === 1 ? 'is' : 'are'} syntax-first and advisory.`
      );
    }
    if (gapStaticLimits > 0) {
      details.push(
        `${gapStaticLimits} gap static limit ${plural(gapStaticLimits, 'entry', 'entries')} must be read before any future action language.`
      );
    }
    details.push(
      `${noActionCount} gap ${plural(noActionCount, 'artifact')} reported no local repair action.`
    );
    return {
      kind: 'gapNoAction',
      summary: 'ripr validated gap artifacts with no actionable gap.',
      enabledLanguages: enabledLanguageNames,
      nextStep: 'No local repair action is projected; refresh after new saved changes or inspect ripr output if this is unexpected.',
      detail: details.join('\n')
    };
  }
  const seamDiagnostics = numberField(message, 'seam_diagnostics');
  if (previewFindings > 0) {
    const details = [
      message,
      `${previewFindings} preview finding${previewFindings === 1 ? '' : 's'} are syntax-first and advisory.`
    ];
    if (staticLimits > 0) {
      details.push(
        `${staticLimits} preview static limit${staticLimits === 1 ? '' : 's'} must be read before action language.`
      );
    }
    return {
      kind: 'analysisReady',
      summary: `ripr analysis completed with ${diagnostics ?? 0} diagnostics (${previewFindings} preview).`,
      enabledLanguages: enabledLanguageNames,
      nextStep: 'Read preview static limits before acting, then use only bounded ripr code actions.',
      detail: details.join('\n')
    };
  }
  if (seamDiagnostics !== undefined && seamDiagnostics === 0) {
    return {
      kind: 'noActionableSeams',
      summary: 'ripr analysis completed with no actionable seam diagnostics.',
      enabledLanguages: enabledLanguageNames,
      nextStep: 'If this is unexpected, save files, confirm the workspace root and enabled languages, then run ripr: Show Output.',
      detail: [
        message,
        'No ripr seam diagnostics were published for the last saved workspace state.',
        'Enabled languages determine which saved files can produce diagnostics; disabled or unavailable preview languages stay silent.',
        'If you expected diagnostics, confirm the file is saved, the workspace root is correct, and the language is enabled and available in this ripr build.'
      ].join('\n')
    };
  }
  return {
    kind: 'analysisReady',
    summary: `ripr analysis completed with ${diagnostics ?? 0} diagnostics.`,
    enabledLanguages: enabledLanguageNames,
    nextStep: 'Inspect diagnostics, then use bounded ripr hover and code actions for one focused test.',
    detail: message
  };
}

function numberField(message: string, field: string): number | undefined {
  const match = message.match(new RegExp(`${field}=(\\d+)`));
  return match ? Number.parseInt(match[1], 10) : undefined;
}

function stringListField(message: string, field: string): string[] | undefined {
  const match = message.match(new RegExp(`${field}=([^,\\s]*)`));
  if (!match) {
    return undefined;
  }
  if (match[1].trim().length === 0) {
    return [];
  }
  return match[1].split('|').filter((entry) => entry.length > 0);
}

function plural(count: number, singular: string, pluralForm?: string): string {
  return count === 1 ? singular : pluralForm ?? `${singular}s`;
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

async function readSetupStatusFiles(
  workspaceRoot: string | undefined,
  readFile: RiprClientRuntime['readFile']
): Promise<RiprSetupStatus> {
  if (!workspaceRoot) {
    return setupStatusWithoutWorkspace();
  }
  const config = await readSetupFileStatus(
    'ripr config',
    RIPR_CONFIG_RELATIVE_PATH,
    workspaceRoot,
    readFile,
    'built-in defaults are active until ripr.toml is added'
  );
  const artifacts = await Promise.all(RIPR_SETUP_ARTIFACTS.map((artifact) =>
    readSetupFileStatus(
      artifact.label,
      artifact.relativePath,
      workspaceRoot,
      readFile,
      'artifact missing; run or refresh saved-workspace evidence when needed'
    )
  ));
  const receipt = await readReceiptStatus(workspaceRoot, readFile);
  const firstPr = await readFirstPrPacketStatus(workspaceRoot, readFile);
  return { config, artifacts, receipt, firstPr };
}

async function readSetupFileStatus(
  label: string,
  relativePath: string,
  workspaceRoot: string,
  readFile: RiprClientRuntime['readFile'],
  missingDetail: string
): Promise<RiprSetupFileStatus> {
  const filePath = setupFilePath(workspaceRoot, relativePath);
  try {
    const contents = await readFile(filePath);
    return {
      label,
      relativePath,
      path: filePath,
      state: contents === undefined ? 'missing' : 'found',
      detail: contents === undefined ? missingDetail : 'found in current workspace'
    };
  } catch (error) {
    return {
      label,
      relativePath,
      path: filePath,
      state: 'unreadable',
      detail: error instanceof Error ? error.message : String(error)
    };
  }
}

function setupStatusWithoutWorkspace(): RiprSetupStatus {
  return {
    config: setupNoWorkspaceFile('ripr config', RIPR_CONFIG_RELATIVE_PATH),
    artifacts: RIPR_SETUP_ARTIFACTS.map((artifact) => setupNoWorkspaceFile(artifact.label, artifact.relativePath)),
    receipt: {
      relativePath: 'target/ripr/agent/agent-receipt.json',
      state: 'noWorkspace',
      detail: 'open a workspace before matching receipt artifacts'
    },
    firstPr: {
      relativePath: 'target/ripr/reports/start-here.json',
      markdownRelativePath: 'target/ripr/reports/start-here.md',
      state: 'noWorkspace',
      detail: 'open a workspace before matching first-pr packet artifacts'
    }
  };
}

function setupNoWorkspaceFile(label: string, relativePath: string): RiprSetupFileStatus {
  return {
    label,
    relativePath,
    state: 'noWorkspace',
    detail: 'open a workspace before matching saved-workspace files'
  };
}

function setupFilePath(workspaceRoot: string, relativePath: string): string {
  return path.join(workspaceRoot, ...relativePath.split('/'));
}

export async function readFirstPrPacketStatus(
  workspaceRoot: string,
  readFile: RiprClientRuntime['readFile']
): Promise<RiprFirstPrPacketStatus> {
  for (const artifact of RIPR_FIRST_PR_PACKET_ARTIFACTS) {
    const jsonPath = setupFilePath(workspaceRoot, artifact.jsonRelativePath);
    let raw: string | undefined;
    try {
      raw = await readFile(jsonPath);
    } catch (error) {
      return {
        relativePath: artifact.jsonRelativePath,
        markdownRelativePath: artifact.markdownRelativePath,
        path: jsonPath,
        markdownPath: setupFilePath(workspaceRoot, artifact.markdownRelativePath),
        state: 'unreadable',
        detail: error instanceof Error ? error.message : String(error)
      };
    }
    if (raw === undefined) {
      continue;
    }
    return validateFirstPrPacket(
      raw,
      workspaceRoot,
      artifact.jsonRelativePath,
      artifact.markdownRelativePath,
      jsonPath,
      setupFilePath(workspaceRoot, artifact.markdownRelativePath)
    );
  }
  return {
    relativePath: RIPR_FIRST_PR_PACKET_ARTIFACTS[0].jsonRelativePath,
    markdownRelativePath: RIPR_FIRST_PR_PACKET_ARTIFACTS[0].markdownRelativePath,
    path: setupFilePath(workspaceRoot, RIPR_FIRST_PR_PACKET_ARTIFACTS[0].jsonRelativePath),
    markdownPath: setupFilePath(workspaceRoot, RIPR_FIRST_PR_PACKET_ARTIFACTS[0].markdownRelativePath),
    state: 'missing',
    detail: 'first-pr start-here packet missing; run cargo xtask first-pr for the current workspace'
  };
}

function validateFirstPrPacket(
  raw: string,
  workspaceRoot: string,
  relativePath: string,
  markdownRelativePath: string,
  filePath: string,
  markdownPath: string
): RiprFirstPrPacketStatus {
  const base = {
    relativePath,
    markdownRelativePath,
    path: filePath,
    markdownPath
  };
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    return {
      ...base,
      state: 'malformed',
      detail: error instanceof Error ? error.message : String(error)
    };
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return {
      ...base,
      state: 'malformed',
      detail: 'first-pr packet JSON root is not an object'
    };
  }
  const packet = parsed as Record<string, unknown>;
  if (
    stringField(packet, 'schema_version') !== '0.1' ||
    stringField(packet, 'tool') !== 'ripr' ||
    stringField(packet, 'kind') !== 'first_pr_start_here'
  ) {
    return {
      ...base,
      state: 'unsupportedSchema',
      detail: 'expected ripr first_pr_start_here schema_version 0.1'
    };
  }
  const repoRoot = stringField(packet, 'root');
  if (!rootMatchesWorkspace(repoRoot, workspaceRoot)) {
    return {
      ...base,
      state: 'wrongRoot',
      repoRoot,
      detail: 'first-pr packet root does not match the active workspace'
    };
  }
  if (stringField(packet, 'posture') !== 'advisory') {
    return {
      ...base,
      state: 'unsupportedSchema',
      detail: 'first-pr packet must remain advisory'
    };
  }
  const status = boundedStringField(packet, 'status', FIRST_PR_PACKET_STATUSES);
  const selected = objectField(packet, 'selected');
  if (!status || !selected) {
    return {
      ...base,
      state: 'malformed',
      detail: 'first-pr packet is missing status or selected state'
    };
  }
  const selectedState = stringField(selected, 'state');
  if (!selectedState) {
    return {
      ...base,
      state: 'malformed',
      detail: 'first-pr packet selected state is missing'
    };
  }
  if (!FIRST_PR_PACKET_SELECTED_STATES.has(selectedState)) {
    return {
      ...base,
      state: 'unsupportedSchema',
      detail: 'first-pr packet selected state is not supported by this editor'
    };
  }
  const commands = objectField(packet, 'commands');
  for (const command of stringValues(commands)) {
    if (!firstPrCommandIsSafe(command)) {
      return {
        ...base,
        state: 'unsafeCommand',
        detail: 'first-pr packet command payload is not safe for editor projection'
      };
    }
  }
  const selectedCommands = [
    stringField(selected, 'agent_packet_command'),
    stringField(selected, 'verify_command'),
    stringField(selected, 'receipt_command'),
    stringField(selected, 'next_command'),
    stringField(selected, 'regeneration_command')
  ].filter((value): value is string => value !== undefined);
  if (selectedCommands.some((command) => !firstPrCommandIsSafe(command))) {
    return {
      ...base,
      state: 'unsafeCommand',
      detail: 'first-pr selected command payload is not safe for editor projection'
    };
  }
  const repair = objectField(selected, 'repair');
  const relatedTest = repair ? stringField(repair, 'related_test') : undefined;
  const repairTarget = repair ? stringField(repair, 'target_file') : undefined;
  const anchor = objectField(selected, 'anchor');
  const selectedArtifact = objectField(selected, 'artifact');
  const packetPaths = [
    ...stringValues(objectField(packet, 'inputs')),
    ...firstPrArtifactPaths(packet),
    relatedTest,
    repairTarget,
    anchor ? stringField(anchor, 'file') : undefined,
    selectedArtifact ? stringField(selectedArtifact, 'path') : undefined
  ].filter((value): value is string => value !== undefined);
  if (packetPaths.some((packetPath) => !firstPrPathIsWorkspaceLocal(packetPath))) {
    return {
      ...base,
      state: 'unsafePath',
      detail: 'first-pr packet repair path is outside the workspace'
    };
  }
  const common = {
    ...base,
    status,
    selectedState,
    gapId: stringField(selected, 'gap_id'),
    canonicalGapId: stringField(selected, 'canonical_gap_id'),
    verifyCommand: stringField(selected, 'verify_command'),
    receiptCommand: stringField(selected, 'receipt_command'),
    relatedTest,
    repairTarget,
    repoRoot,
    warningCount: arrayLength(packet, 'warnings')
  };
  if (status === 'actionable') {
    if (
      selectedState !== 'top_gap' ||
      (!common.gapId && !common.canonicalGapId) ||
      !common.verifyCommand
    ) {
      return {
        ...base,
        state: 'malformed',
        detail: 'actionable first-pr packet is missing top-gap identity or verify command'
      };
    }
    return { ...common, state: 'topRepairableGap' };
  }
  if (status === 'no_action') {
    if (!FIRST_PR_PACKET_NO_ACTION_STATES.has(selectedState)) {
      return {
        ...base,
        state: 'malformed',
        detail: 'first-pr no-action packet has a non-no-action selected state'
      };
    }
    return { ...common, state: 'noAction' };
  }
  if (status === 'blocked') {
    if (!FIRST_PR_PACKET_BLOCKED_STATES.has(selectedState)) {
      return {
        ...base,
        state: 'malformed',
        detail: 'first-pr blocked packet has a non-blocked selected state'
      };
    }
    return { ...common, state: 'blocked' };
  }
  return { ...common, state: 'found' };
}

const FIRST_PR_PACKET_STATUSES = new Set([
  'actionable',
  'no_action',
  'blocked'
]);
const FIRST_PR_PACKET_BLOCKED_STATES = new Set([
  'missing_artifact',
  'malformed_artifact',
  'stale_artifact',
  'wrong_root',
  'blocked_artifact',
  'timeout'
]);
const FIRST_PR_PACKET_NO_ACTION_STATES = new Set([
  'empty_diff',
  'no_action'
]);
const FIRST_PR_PACKET_SELECTED_STATES = new Set([
  'top_gap',
  ...FIRST_PR_PACKET_BLOCKED_STATES,
  ...FIRST_PR_PACKET_NO_ACTION_STATES
]);

function stringValues(value: Record<string, unknown> | undefined): string[] {
  if (!value) {
    return [];
  }
  return Object.values(value).filter((child): child is string =>
    typeof child === 'string' && child.trim() !== ''
  );
}

function firstPrCommandIsSafe(command: string): boolean {
  return command.trim() !== '' && !hasUnsafeShellMetacharacter(command);
}

function firstPrPathIsWorkspaceLocal(value: string): boolean {
  const pathPart = value.split('::')[0];
  if (!pathPart || path.isAbsolute(pathPart)) {
    return false;
  }
  const normalized = path.normalize(pathPart);
  return normalized !== '..' && !normalized.startsWith(`..${path.sep}`);
}

function firstPrArtifactPaths(packet: Record<string, unknown>): string[] {
  const artifacts = packet['artifacts'];
  if (!Array.isArray(artifacts)) {
    return [];
  }
  const paths: string[] = [];
  for (const artifact of artifacts) {
    if (artifact && typeof artifact === 'object' && !Array.isArray(artifact)) {
      const artifactPath = stringField(artifact as Record<string, unknown>, 'path');
      if (artifactPath) {
        paths.push(artifactPath);
      }
    }
  }
  return paths;
}

async function readReceiptStatus(
  workspaceRoot: string,
  readFile: RiprClientRuntime['readFile']
): Promise<RiprReceiptArtifactStatus> {
  const relativePath = 'target/ripr/agent/agent-receipt.json';
  const filePath = setupFilePath(workspaceRoot, relativePath);
  let raw: string | undefined;
  try {
    raw = await readFile(filePath);
  } catch (error) {
    return {
      relativePath,
      path: filePath,
      state: 'unreadable',
      detail: error instanceof Error ? error.message : String(error)
    };
  }
  if (raw === undefined) {
    return {
      relativePath,
      path: filePath,
      state: 'missing',
      detail: 'receipt artifact missing; run verify and receipt after a focused repair'
    };
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    return {
      relativePath,
      path: filePath,
      state: 'malformed',
      detail: error instanceof Error ? error.message : String(error)
    };
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return {
      relativePath,
      path: filePath,
      state: 'malformed',
      detail: 'receipt JSON root is not an object'
    };
  }
  const receipt = parsed as Record<string, unknown>;
  if (stringField(receipt, 'schema_version') !== '0.3' || stringField(receipt, 'tool') !== 'ripr') {
    return {
      relativePath,
      path: filePath,
      state: 'unsupportedSchema',
      detail: 'expected ripr agent receipt schema_version 0.3'
    };
  }
  const provenance = objectField(receipt, 'provenance');
  const repoRoot = provenance ? stringField(provenance, 'repo_root') : undefined;
  if (!rootMatchesWorkspace(repoRoot, workspaceRoot)) {
    return {
      relativePath,
      path: filePath,
      state: 'wrongRoot',
      repoRoot,
      detail: 'receipt repo_root does not match the active workspace'
    };
  }
  const seam = objectField(receipt, 'seam');
  return {
    relativePath,
    path: filePath,
    state: 'found',
    detail: 'receipt artifact found in current workspace',
    seamId: seam ? stringField(seam, 'seam_id') : provenance ? stringField(provenance, 'seam_id') : undefined,
    movement: provenance ? stringField(provenance, 'movement') : seam ? stringField(seam, 'change') : undefined,
    repoRoot,
    generatedAt: provenance ? stringField(provenance, 'generated_at') : undefined
  };
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
    generatedAt: stringField(report, 'generated_at'),
    seamId: selected ? stringField(selected, 'seam_id') : undefined,
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
  targetArtifact?: string;
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
  },
  gap_verify: {
    startsWith: 'ripr agent verify --root .',
    includes: ['--json'],
    requiresSeamId: false
  },
  gap_receipt: {
    startsWith: 'ripr agent receipt --root .',
    includes: ['--json'],
    requiresSeamId: false
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
    contract.targetArtifact !== undefined &&
    (typeof target.target_artifact !== 'string' ||
      target.target_artifact !== contract.targetArtifact)
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

function nearestGapDiagnostic(editor: vscode.TextEditor): vscode.Diagnostic | undefined {
  const position = editor.selection.active;
  return vscode.languages
    .getDiagnostics(editor.document.uri)
    .filter(isRiprGapDiagnostic)
    .map((diagnostic) => ({
      diagnostic,
      containsSelection: diagnostic.range.contains(position),
      lineDistance: Math.abs(diagnostic.range.start.line - position.line),
      characterDistance: Math.abs(diagnostic.range.start.character - position.character)
    }))
    .sort((left, right) =>
      Number(right.containsSelection) - Number(left.containsSelection) ||
      left.lineDistance - right.lineDistance ||
      left.characterDistance - right.characterDistance
    )[0]?.diagnostic;
}

function isRiprGapDiagnostic(diagnostic: vscode.Diagnostic): boolean {
  return diagnostic.source === 'ripr' && diagnosticCodeText(diagnostic).startsWith('ripr-gap-');
}

function diagnosticCodeText(diagnostic: vscode.Diagnostic): string {
  const code = diagnostic.code;
  if (typeof code === 'string' || typeof code === 'number') {
    return String(code);
  }
  if (code && typeof code === 'object' && 'value' in code) {
    return String(code.value);
  }
  return '';
}

function startRepairActions(actions: Array<vscode.CodeAction | vscode.Command>): StartRepairAction[] {
  const candidates: StartRepairAction[] = [];
  for (const action of actions) {
    const command = commandForAction(action);
    if (!command) {
      continue;
    }
    const title = action.title || command.title;
    const priority = startRepairActionPriority(title, command);
    if (priority === undefined) {
      continue;
    }
    candidates.push({ title, command, priority });
  }
  return candidates.sort((left, right) =>
    left.priority - right.priority || left.title.localeCompare(right.title)
  );
}

function commandForAction(action: vscode.CodeAction | vscode.Command): vscode.Command | undefined {
  const codeActionCommand = (action as vscode.CodeAction).command;
  if (codeActionCommand && typeof codeActionCommand !== 'string') {
    return codeActionCommand;
  }
  const command = (action as vscode.Command).command;
  return typeof command === 'string' ? action as vscode.Command : undefined;
}

function startRepairActionPriority(title: string, command: vscode.Command): number | undefined {
  if (
    command.command === 'ripr.copyContext' &&
    firstArgumentLabelIs(command, 'first_repair_packet')
  ) {
    return 0;
  }
  if (
    command.command === 'ripr.copyContext' &&
    (title === 'Inspect gap: copy repair packet' || firstArgumentLabelIs(command, 'gap_repair_packet'))
  ) {
    return 1;
  }
  if (command.command === 'ripr.openRelatedTest') {
    return 2;
  }
  if (command.command === 'ripr.copyAgentVerifyCommand' && firstArgumentLabelIs(command, 'gap_verify')) {
    return 3;
  }
  if (command.command === 'ripr.copyAgentReceiptCommand' && firstArgumentLabelIs(command, 'gap_receipt')) {
    return 4;
  }
  if (
    command.command === 'ripr.copyContext' &&
    (title === 'Inspect gap: copy static-limit note' || firstArgumentLabelIs(command, 'static_limit_note'))
  ) {
    return 5;
  }
  return undefined;
}

function firstArgumentLabelIs(command: vscode.Command, expected: string): boolean {
  const first = command.arguments?.[0];
  return Boolean(
    first &&
    typeof first === 'object' &&
    'label' in first &&
    (first as { label?: unknown }).label === expected
  );
}

async function pickStartRepairAction(actions: StartRepairAction[]): Promise<StartRepairAction | undefined> {
  const items = actions.map((action) => ({
    label: action.title,
    description: startRepairActionDescription(action.command),
    action
  }));
  const selected = await vscode.window.showQuickPick(items, {
    placeHolder: 'Start current ripr repair',
    matchOnDescription: true
  });
  return selected?.action;
}

function startRepairActionDescription(command: vscode.Command): string | undefined {
  switch (command.command) {
    case 'ripr.copyContext':
      return firstArgumentLabelIs(command, 'first_repair_packet')
        ? 'Copy the bounded packet'
        : 'Copy the gap context';
    case 'ripr.openRelatedTest':
      return 'Open the likely repair target';
    case 'ripr.copyAgentVerifyCommand':
      return 'Copy the verify command';
    case 'ripr.copyAgentReceiptCommand':
      return 'Copy the receipt command';
    default:
      return undefined;
  }
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

function isRiprFileDocument(document: vscode.TextDocument): boolean {
  return document.uri.scheme === 'file' && RIPR_FILE_LANGUAGES.has(document.languageId);
}

function riprRelatedTestLanguage(filePath: string): 'rust' | 'typescript' | 'python' | undefined {
  return RIPR_RELATED_TEST_LANGUAGE_BY_EXTENSION.get(path.extname(filePath).toLowerCase());
}

async function writeTestClipboardCapture(text: string): Promise<void> {
  const capturePath = process.env.RIPR_TEST_CLIPBOARD_CAPTURE_PATH;
  if (!capturePath) {
    return;
  }
  try {
    await fs.writeFile(capturePath, text, 'utf8');
  } catch {
    // Test capture must not make the user-facing clipboard command fail.
  }
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
