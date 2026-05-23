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
import { requestedServerVersion, resolveServer, ResolveFailure, ResolvedServer } from './serverResolver';

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

function riprDocumentSelectorsForWorkspace(
  workspaceRoot: string
): Array<{ language: string; scheme: 'file'; pattern: string }> {
  const workspacePattern = `${workspaceRoot.replace(/\\/g, '/')}/**/*`;
  return RIPR_DOCUMENT_SELECTORS.map((selector) => ({
    ...selector,
    pattern: workspacePattern
  }));
}

const RIPR_SETUP_ARTIFACTS: RiprSetupArtifactDefinition[] = [
  {
    label: 'actionable gap queue',
    relativePath: 'target/ripr/reports/actionable-gaps.json'
  },
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
const ACTIONABLE_GAP_QUEUE_RELATIVE_PATH = 'target/ripr/reports/actionable-gaps.json';

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

type FirstPrPacketActionKind =
  | 'open'
  | 'summary'
  | 'repair'
  | 'verify'
  | 'receipt'
  | 'regenerate';

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
  workspaceRootState(): RiprWorkspaceRootState;
  resolveServer(
    context: vscode.ExtensionContext,
    config: RiprConfig,
    output: vscode.OutputChannel
  ): Promise<ResolvedServer | ResolveFailure>;
  createLanguageClient(
    serverOptions: ServerOptions,
    clientOptions: LanguageClientOptions
  ): RiprLanguageClient;
  createFileSystemWatcher(pattern: vscode.GlobPattern): vscode.FileSystemWatcher;
  readFile(filePath: string): Promise<string | undefined>;
  runRipr(command: string, args: string[], cwd: string): Promise<string>;
  writeClipboard(text: string): Promise<void>;
  isWorkspaceTrusted(): boolean;
  showInformationMessage(message: string): Thenable<string | undefined>;
  showWarningMessage(message: string): Thenable<string | undefined>;
  showErrorMessage(message: string, ...items: string[]): Thenable<string | undefined>;
}

export type RiprWorkspaceRootKind =
  | 'noWorkspace'
  | 'singleRoot'
  | 'selectedRoot'
  | 'ambiguousMultiRoot';

export interface RiprWorkspaceRootState {
  kind: RiprWorkspaceRootKind;
  root?: string;
  roots: string[];
  detail?: string;
}

const defaultRuntime: RiprClientRuntime = {
  getConfig,
  workspaceRootState: currentWorkspaceRootState,
  resolveServer,
  createLanguageClient: (serverOptions, clientOptions) =>
    new LanguageClient('ripr', 'ripr', serverOptions, clientOptions),
  createFileSystemWatcher: (pattern) => vscode.workspace.createFileSystemWatcher(pattern),
  readFile: readOptionalFile,
  runRipr,
  writeClipboard: async (text) => {
    await vscode.env.clipboard.writeText(text);
    await writeTestClipboardCapture(text);
  },
  isWorkspaceTrusted: () => vscode.workspace.isTrusted,
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
  private workspaceRootState: RiprWorkspaceRootState = workspaceRootStateNoWorkspace();
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
    this.workspaceRootState = this.runtime.workspaceRootState();
    this.workspaceRoot = this.workspaceRootState.root;
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

    if (this.workspaceRootState.kind === 'ambiguousMultiRoot') {
      this.updateStatus({
        kind: 'workspaceAmbiguous',
        summary: 'Select one workspace folder before using ripr repair actions.',
        detail: workspaceRootStateDetail(this.workspaceRootState),
        nextStep: 'Open a Rust or enabled preview-language file from one workspace folder, then run ripr: Restart Server.'
      });
      this.output.appendLine('ripr multi-root workspace is ambiguous; select a file before starting the server.');
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
      documentSelector: riprDocumentSelectorsForWorkspace(this.workspaceRoot),
      initializationOptions: {
        baseRef: config.baseRef,
        checkMode: config.checkMode,
        includeUnchangedTests: true
      },
      outputChannel: this.output,
      revealOutputChannelOn: RevealOutputChannelOn.Never,
      traceOutputChannel: this.output,
      synchronize: {
        fileEvents: this.runtime.createFileSystemWatcher(
          new vscode.RelativePattern(this.workspaceRoot, '**/Cargo.toml')
        )
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
    const targetUri = uriFromTarget(target);
    const rootBlocker = this.repairActionRootBlocker(targetUri);
    if (rootBlocker) {
      this.runtime.showInformationMessage(rootBlocker);
      return;
    }

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
    const rootBlocker = this.activeDocumentRootBlocker(editor.document);
    if (rootBlocker) {
      this.runtime.showInformationMessage(rootBlocker);
      return;
    }
    const setupBlocker = setupRepairBlocker(this.statusContext());
    if (setupBlocker) {
      this.runtime.showInformationMessage(setupBlocker);
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
    const rootBlocker = this.repairActionRootBlocker();
    if (rootBlocker) {
      this.runtime.showInformationMessage(rootBlocker);
      return;
    }

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

  async copyCurrentRepairPacket(): Promise<void> {
    await this.refreshSetupStatusFiles();
    const queue = this.setupStatus.actionableQueue;
    if (this.status.kind === 'stale' && actionableGapQueueCanBecomeStale(queue.state)) {
      this.runtime.showInformationMessage('ripr current repair packet requires current saved-workspace evidence; save or refresh first.');
      return;
    }
    if (!actionableGapQueueAllowsCurrentRepairPacket(queue)) {
      this.runtime.showInformationMessage(actionableGapQueueSuppressedMessage(queue));
      return;
    }
    try {
      await this.runtime.writeClipboard(currentRepairPacket(queue));
      this.runtime.showInformationMessage('ripr current repair packet copied.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy current repair packet failed: ${message}`);
      this.runtime.showWarningMessage('ripr could not copy the current repair packet. See ripr output for details.');
    }
  }

  async copyRepoGapMap(): Promise<void> {
    await this.refreshSetupStatusFiles();
    const queue = this.setupStatus.actionableQueue;
    if (!actionableGapQueueAllowsRepoGapMap(queue)) {
      this.runtime.showInformationMessage(actionableGapQueueRepoMapSuppressedMessage(queue));
      return;
    }
    try {
      await this.runtime.writeClipboard(repoGapMap(queue, this.setupStatus.receipt, this.setupStatus.firstPr));
      this.runtime.showInformationMessage('ripr repo gap map copied.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy repo gap map failed: ${message}`);
      this.runtime.showWarningMessage('ripr could not copy the repo gap map. See ripr output for details.');
    }
  }

  async openFirstPrPacket(): Promise<void> {
    const packet = await this.firstPrPacketForAction('open');
    if (!packet) {
      return;
    }
    if (!packet.markdownPath) {
      this.runtime.showInformationMessage('No ripr first-pr Markdown packet path is available.');
      return;
    }
    try {
      const markdown = await this.runtime.readFile(packet.markdownPath);
      if (markdown === undefined) {
        this.runtime.showInformationMessage(`ripr first-pr packet is missing: ${packet.markdownRelativePath ?? packet.relativePath}.`);
        return;
      }
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(packet.markdownPath));
      await vscode.window.showTextDocument(document);
      this.runtime.showInformationMessage('Opened ripr first-pr packet.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr open first-pr packet failed: ${message}`);
      this.runtime.showWarningMessage('ripr could not open the first-pr packet. See ripr output for details.');
    }
  }

  async copyFirstPrSummary(): Promise<void> {
    const packet = await this.firstPrPacketForAction('summary');
    if (!packet) {
      return;
    }
    await this.copyFirstPrText(firstPrSummaryPacket(packet), 'summary');
  }

  async copyFirstPrRepairPacket(): Promise<void> {
    const packet = await this.firstPrPacketForAction('repair');
    if (!packet || !this.currentDiagnosticMatchesFirstPrPacket(packet)) {
      return;
    }
    await this.copyFirstPrText(firstPrRepairPacket(packet), 'repair packet');
  }

  async copyFirstPrVerifyCommand(): Promise<void> {
    const packet = await this.firstPrPacketForAction('verify');
    if (!packet || !this.currentDiagnosticMatchesFirstPrPacket(packet)) {
      return;
    }
    await this.copyFirstPrText(packet.verifyCommand ?? '', 'verify command');
  }

  async copyFirstPrReceiptCommand(): Promise<void> {
    const packet = await this.firstPrPacketForAction('receipt');
    if (!packet || !this.currentDiagnosticMatchesFirstPrPacket(packet)) {
      return;
    }
    await this.copyFirstPrText(packet.receiptCommand ?? '', 'receipt command');
  }

  async copyFirstPrRegenerationGuidance(): Promise<void> {
    const packet = await this.firstPrPacketForAction('regenerate');
    if (!packet) {
      return;
    }
    await this.copyFirstPrText(firstPrRegenerationGuidance(packet), 'regeneration guidance');
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
    const workspaceFolder = vscode.workspace.getWorkspaceFolder(uri);
    if (!workspaceFolder) {
      this.runtime.showInformationMessage('ripr related test must be inside the current workspace.');
      return;
    }
    if (this.workspaceRoot && !sameWorkspaceRoot(workspaceFolder.uri.fsPath, this.workspaceRoot)) {
      this.runtime.showInformationMessage('ripr related test belongs to a different workspace root than the active ripr session.');
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

  private async firstPrPacketForAction(kind: FirstPrPacketActionKind): Promise<RiprFirstPrPacketStatus | undefined> {
    await this.refreshSetupStatusFiles();
    const packet = this.setupStatus.firstPr;
    if (this.status.kind === 'stale' && firstPrPacketCanBecomeStale(packet.state)) {
      this.runtime.showInformationMessage('ripr first-pr actions require current saved-workspace evidence; save or refresh first.');
      return kind === 'regenerate' ? packet : undefined;
    }
    if (kind === 'regenerate') {
      return packet.state === 'noWorkspace' ? undefined : packet;
    }
    if (kind === 'summary') {
      if (firstPrPacketAllowsSummary(packet.state)) {
        return packet;
      }
      this.runtime.showInformationMessage(firstPrSuppressedMessage(packet));
      return undefined;
    }
    if (kind === 'open') {
      if (firstPrPacketAllowsOpen(packet.state)) {
        return packet;
      }
      this.runtime.showInformationMessage(firstPrSuppressedMessage(packet));
      return undefined;
    }
    if (packet.state !== 'topRepairableGap') {
      this.runtime.showInformationMessage(firstPrSuppressedMessage(packet));
      return undefined;
    }
    if (kind === 'repair' && !firstPrHasRepairPacket(packet)) {
      this.runtime.showInformationMessage('ripr first-pr repair packet is missing required typed repair fields.');
      return undefined;
    }
    if (kind === 'verify' && !packet.verifyCommand) {
      this.runtime.showInformationMessage('ripr first-pr verify command is not available.');
      return undefined;
    }
    if (kind === 'receipt' && !packet.receiptCommand) {
      this.runtime.showInformationMessage('ripr first-pr receipt command is not available.');
      return undefined;
    }
    return packet;
  }

  private currentDiagnosticMatchesFirstPrPacket(packet: RiprFirstPrPacketStatus): boolean {
    const editor = vscode.window.activeTextEditor;
    if (!editor || !isRiprFileDocument(editor.document)) {
      this.runtime.showInformationMessage('Open the diagnostic file before copying diagnostic-scoped first-pr packet actions.');
      return false;
    }
    const rootBlocker = this.activeDocumentRootBlocker(editor.document);
    if (rootBlocker) {
      this.runtime.showInformationMessage(rootBlocker);
      return false;
    }
    const diagnostic = nearestGapDiagnostic(editor);
    if (!diagnostic) {
      this.runtime.showInformationMessage('No current ripr gap diagnostic is available for this first-pr packet.');
      return false;
    }
    if (!diagnosticMatchesFirstPrPacket(diagnostic, packet)) {
      this.runtime.showInformationMessage('The current diagnostic does not match the first-pr packet gap identity.');
      return false;
    }
    return true;
  }

  private async copyFirstPrText(text: string, label: string): Promise<void> {
    const trimmed = text.trim();
    if (!trimmed) {
      this.runtime.showInformationMessage(`No ripr first-pr ${label} is available.`);
      return;
    }
    try {
      await this.runtime.writeClipboard(trimmed);
      this.runtime.showInformationMessage(`Copied ripr first-pr ${label} to clipboard.`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`ripr copy first-pr ${label} failed: ${message}`);
      this.runtime.showWarningMessage(`ripr could not copy the first-pr ${label}. See ripr output for details.`);
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
    const config = this.runtime.getConfig();
    return {
      extensionVersion: extensionVersion(this.context),
      expectedServerVersion: requestedServerVersion(this.context, config),
      workspaceTrusted: this.runtime.isWorkspaceTrusted(),
      workspaceRootState: this.workspaceRootState,
      workspaceRoot: this.workspaceRoot,
      activeDocumentRelativePath: activeDocumentRelativePath(this.workspaceRoot),
      server: this.server,
      documentLanguages: RIPR_DOCUMENT_SELECTORS.map((selector) => selector.language),
      setupStatus: this.setupStatus
    };
  }

  private activeDocumentRootBlocker(document: vscode.TextDocument): string | undefined {
    return this.repairActionRootBlocker(document.uri);
  }

  private repairActionRootBlocker(targetUri?: vscode.Uri): string | undefined {
    const uri = targetUri ?? vscode.window.activeTextEditor?.document.uri;
    if (!uri) {
      return 'ripr repair actions require an active file or target URI in the current workspace.';
    }
    if (uri.scheme !== 'file') {
      return 'ripr repair actions require a file URI in the current workspace.';
    }
    const workspaceFolder = vscode.workspace.getWorkspaceFolder(uri);
    if (!workspaceFolder) {
      return 'ripr repair actions require the active or target file to belong to a workspace folder.';
    }
    if (this.workspaceRoot && !sameWorkspaceRoot(workspaceFolder.uri.fsPath, this.workspaceRoot)) {
      return 'ripr repair actions are suppressed because the active or target file belongs to a different workspace root than the active ripr session.';
    }
    return undefined;
  }

}

interface RiprSetupArtifactDefinition {
  label: string;
  relativePath: string;
}

type RiprStatusKind =
  | 'disabled'
  | 'noWorkspace'
  | 'workspaceAmbiguous'
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
  extensionVersion: string;
  expectedServerVersion: string;
  workspaceTrusted: boolean;
  workspaceRootState: RiprWorkspaceRootState;
  workspaceRoot?: string;
  activeDocumentRelativePath?: string;
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
  actionableQueue: RiprActionableGapQueueStatus;
  receipt: RiprReceiptArtifactStatus;
  firstPr: RiprFirstPrPacketStatus;
}

export type RiprActionableGapQueueState =
  | 'topActionableGap'
  | 'noAction'
  | 'reportOnly'
  | 'staticLimitOnly'
  | 'blocked'
  | 'missing'
  | 'malformed'
  | 'unsupportedSchema'
  | 'wrongRoot'
  | 'stale'
  | 'unsafePath'
  | 'unsafeCommand'
  | 'noWorkspace';

export interface RiprActionableGapQueueStatus {
  relativePath: string;
  path?: string;
  state: RiprActionableGapQueueState;
  detail?: string;
  repoRoot?: string;
  actionableGaps?: number;
  packetsEmitted?: number;
  reportOnlyGaps?: number;
  staticLimitOnlyGaps?: number;
  topRepair?: string;
  sourceFile?: string;
  evidenceClass?: string;
  actionability?: string;
  canonicalGapId?: string;
  seamId?: string;
  findingId?: string;
  language?: string;
  languageStatus?: string;
  relatedTest?: string;
  targetTestType?: string;
  assertionShape?: string;
  verifyCommand?: string;
  receiptCommandOrPath?: string;
  confidenceBasis?: string;
  projectionExclusionReasons?: string[];
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
  selectedKind?: string;
  changedBehavior?: string;
  missingDiscriminator?: string;
  focusedProofIntent?: string;
  why?: string;
  gapId?: string;
  canonicalGapId?: string;
  repairRoute?: string;
  suggestedAssertion?: string;
  verifyCommand?: string;
  receiptCommand?: string;
  receiptPath?: string;
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
    case 'workspaceAmbiguous':
      return '$(warning) ripr: select root';
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
  if (context) {
    lines.push('', ...repairFirstStatusLines(status, firstAction, context));
  }
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
    const queueLines = actionableGapQueueStatusLines(status, context);
    if (queueLines.length > 0) {
      lines.push('', ...queueLines);
    }
    const receiptLines = receiptStatusLines(status, firstAction, context);
    if (receiptLines.length > 0) {
      lines.push('', ...receiptLines);
    }
    const firstPrLines = firstPrPacketStatusLines(status, context);
    if (firstPrLines.length > 0) {
      lines.push('', ...firstPrLines);
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
  const queueLines = actionableGapQueueStatusLines(status, context);
  if (queueLines.length > 0) {
    lines.push('', ...queueLines);
  }
  if (receiptLines.length > 0) {
    lines.push('', ...receiptLines);
  }
  const firstPrLines = firstPrPacketStatusLines(status, context);
  if (firstPrLines.length > 0) {
    lines.push('', ...firstPrLines);
  }
  lines.push(
    '',
    'Limits: read-only setup diagnosis only; no source edits, generated tests, provider calls, mutation execution, or gate decision.'
  );
  return lines.join('\n');
}

function statusContextLines(status: RiprStatusState, context: RiprStatusContext): string[] {
  const lines = [`Workspace: ${context.workspaceRoot ?? 'not open'}`];
  lines.push(`Workspace root state: ${workspaceRootStateLabel(context.workspaceRootState)}`);
  lines.push(...setupCompatibilityLines(context));
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

type RiprSetupState =
  | 'extension_version_ok'
  | 'ripr_missing'
  | 'ripr_version_ok'
  | 'ripr_version_too_old'
  | 'ripr_version_unknown'
  | 'workspace_trusted'
  | 'workspace_untrusted'
  | 'workspace_not_open'
  | 'config_found'
  | 'config_missing'
  | 'config_unreadable'
  | 'config_not_applicable'
  | 'artifact_dir_present'
  | 'artifact_dir_missing'
  | 'artifact_dir_not_applicable';

function setupCompatibilityLines(context: RiprStatusContext): string[] {
  const serverState = riprServerVersionState(context);
  return [
    `Extension state: extension_version_ok (${context.extensionVersion})`,
    `Expected ripr server version: ${context.expectedServerVersion}`,
    `ripr server state: ${serverState.state}${serverState.detail ? ` (${serverState.detail})` : ''}`,
    `Workspace trust state: ${workspaceTrustState(context)}`,
    `Config state: ${configSetupState(context.setupStatus.config)}`,
    `Artifact directory state: ${artifactDirectoryState(context.setupStatus)}`
  ];
}

function setupRepairBlocker(context: RiprStatusContext): string | undefined {
  if (context.workspaceRootState.kind === 'ambiguousMultiRoot') {
    return 'ripr setup is not ready for repair actions: workspace_multi_root_ambiguous. Select one workspace folder, then rerun ripr: Diagnose Setup.';
  }
  if (workspaceTrustState(context) === 'workspace_untrusted') {
    return 'ripr setup is not trusted for repair actions: workspace_untrusted. Trust the workspace, then rerun ripr: Diagnose Setup.';
  }
  const serverState = riprServerVersionState(context);
  if (serverState.state === 'ripr_missing') {
    return 'ripr setup is not ready for repair actions: ripr_missing. Run ripr: Diagnose Setup for install and server path guidance.';
  }
  if (serverState.state === 'ripr_version_too_old') {
    return 'ripr setup is not ready for repair actions: ripr_version_too_old. Update the ripr server or pin a compatible ripr.server.version.';
  }
  if (serverState.state === 'ripr_version_unknown') {
    return 'ripr setup is not ready for repair actions: ripr_version_unknown. Run ripr: Diagnose Setup before acting on editor repair packets.';
  }
  return undefined;
}

function extensionVersion(context: vscode.ExtensionContext): string {
  const version = context.extension?.packageJSON?.version;
  return typeof version === 'string' && version.trim() !== '' ? version.replace(/^v/, '') : '0.7.0';
}

function workspaceTrustState(context: RiprStatusContext): RiprSetupState {
  if (!context.workspaceRoot) {
    return 'workspace_not_open';
  }
  return context.workspaceTrusted ? 'workspace_trusted' : 'workspace_untrusted';
}

function configSetupState(config: RiprSetupFileStatus): RiprSetupState {
  switch (config.state) {
    case 'found':
      return 'config_found';
    case 'missing':
      return 'config_missing';
    case 'unreadable':
      return 'config_unreadable';
    case 'noWorkspace':
      return 'config_not_applicable';
  }
}

function artifactDirectoryState(setup: RiprSetupStatus): RiprSetupState {
  if (setup.artifacts.some((artifact) => artifact.state === 'noWorkspace') || setup.firstPr.state === 'noWorkspace') {
    return 'artifact_dir_not_applicable';
  }
  const trackedArtifactFound = setup.artifacts.some((artifact) => artifact.state === 'found')
    || actionableGapQueueStoredInTarget(setup.actionableQueue)
    || setup.receipt.state === 'found'
    || firstPrPacketStoredInTarget(setup.firstPr);
  return trackedArtifactFound ? 'artifact_dir_present' : 'artifact_dir_missing';
}

function actionableGapQueueStoredInTarget(queue: RiprActionableGapQueueStatus): boolean {
  return queue.state !== 'missing'
    && queue.state !== 'noWorkspace'
    && queue.relativePath.startsWith('target/ripr/');
}

function firstPrPacketStoredInTarget(packet: RiprFirstPrPacketStatus): boolean {
  return packet.state !== 'missing'
    && packet.state !== 'noWorkspace'
    && packet.relativePath.startsWith('target/ripr/');
}

function riprServerVersionState(context: RiprStatusContext): { state: RiprSetupState; detail?: string } {
  const version = context.server?.version;
  if (!context.server) {
    return { state: 'ripr_missing' };
  }
  if (!version || version.trim() === '') {
    return { state: 'ripr_version_unknown', detail: 'server did not report --version output' };
  }
  const actual = parseVersion(version);
  const expected = parseVersion(context.expectedServerVersion);
  if (!actual || !expected) {
    return { state: 'ripr_version_unknown', detail: `reported ${version}` };
  }
  if (compareVersions(actual, expected) < 0) {
    return { state: 'ripr_version_too_old', detail: `reported ${version}; expected ${context.expectedServerVersion}` };
  }
  return { state: 'ripr_version_ok', detail: version };
}

function parseVersion(value: string): [number, number, number] | undefined {
  for (let index = 0; index < value.length; index += 1) {
    if (!isAsciiDigit(value.charCodeAt(index))) {
      continue;
    }
    const major = readVersionNumber(value, index);
    if (!major || value.charAt(major.nextIndex) !== '.') {
      continue;
    }
    const minor = readVersionNumber(value, major.nextIndex + 1);
    if (!minor || value.charAt(minor.nextIndex) !== '.') {
      continue;
    }
    const patch = readVersionNumber(value, minor.nextIndex + 1);
    if (!patch) {
      continue;
    }
    return [major.value, minor.value, patch.value];
  }
  return undefined;
}

function readVersionNumber(value: string, startIndex: number): { value: number; nextIndex: number } | undefined {
  let nextIndex = startIndex;
  while (nextIndex < value.length && isAsciiDigit(value.charCodeAt(nextIndex))) {
    nextIndex += 1;
  }
  if (nextIndex === startIndex) {
    return undefined;
  }
  return {
    value: Number(value.slice(startIndex, nextIndex)),
    nextIndex
  };
}

function isAsciiDigit(code: number): boolean {
  return code >= 48 && code <= 57;
}

function compareVersions(left: [number, number, number], right: [number, number, number]): number {
  for (let index = 0; index < left.length; index += 1) {
    const diff = left[index] - right[index];
    if (diff !== 0) {
      return diff;
    }
  }
  return 0;
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
    case 'workspaceAmbiguous':
      return 'no; workspace root is ambiguous';
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
    case 'workspaceAmbiguous':
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

function repairFirstStatusLines(
  status: RiprStatusState,
  firstAction: FirstUsefulActionStatus | undefined,
  context: RiprStatusContext
): string[] {
  const queue = context.setupStatus.actionableQueue;
  const lines = ['Editor repair cockpit:'];
  lines.push(`Workspace actionable state: ${workspaceActionableState(queue)}`);
  lines.push(`Current-file actionable state: ${currentFileActionableState(queue, context.activeDocumentRelativePath)}`);
  if (queue.state === 'topActionableGap') {
    lines.push(`Top repair item: ${queue.topRepair ?? 'not_available'}`);
    const identity = queue.canonicalGapId ?? queue.seamId ?? queue.findingId;
    if (identity) {
      lines.push(`Top repair gap: ${identity}`);
    }
    lines.push(`Related test or target: ${queue.relatedTest ?? queue.sourceFile ?? 'not_available'}`);
    lines.push(`Verify command: ${queue.verifyCommand ?? 'not_available'}`);
    lines.push(`Receipt state: ${receiptStateForRepairFirst(context.setupStatus.receipt, firstAction, queue)}`);
    lines.push('Next safe command: run ripr: Start Current Repair, or ripr: Copy Current Repair Packet.');
  } else {
    lines.push('Top repair item: not_available');
    lines.push('Related test or target: not_available');
    lines.push('Verify command: not_available');
    lines.push(`Receipt state: ${receiptStateForRepairFirst(context.setupStatus.receipt, firstAction, queue)}`);
    lines.push(`Next safe command: ${repairFirstFailClosedCommand(status, queue, context)}`);
  }
  lines.push('Boundary: advisory static projection only; no source edits, generated tests, provider calls, mutation execution, gate decision, or merge approval.');
  return lines;
}

function workspaceActionableState(queue: RiprActionableGapQueueStatus): string {
  switch (queue.state) {
    case 'topActionableGap':
      return `repairable (${queue.actionableGaps ?? 1} actionable; ${queue.relativePath})`;
    case 'noAction':
      return `no_action (${queue.relativePath})`;
    case 'reportOnly':
      return `report_only (${queue.reportOnlyGaps ?? 0} report-only; ${queue.relativePath})`;
    case 'staticLimitOnly':
      return `static_limit_only (${queue.staticLimitOnlyGaps ?? 0} static-limited; ${queue.relativePath})`;
    case 'blocked':
    case 'missing':
    case 'malformed':
    case 'unsupportedSchema':
    case 'wrongRoot':
    case 'stale':
    case 'unsafePath':
    case 'unsafeCommand':
      return `fail_closed (${queue.state}; ${queue.relativePath})`;
    case 'noWorkspace':
      return 'fail_closed (no workspace)';
  }
}

function currentFileActionableState(
  queue: RiprActionableGapQueueStatus,
  activeRelativePath: string | undefined
): string {
  if (!activeRelativePath) {
    return 'fail_closed (no active workspace file)';
  }
  if (queue.state !== 'topActionableGap') {
    return `fail_closed (${queue.state}; ${activeRelativePath})`;
  }
  if (queue.sourceFile && sameWorkspaceRelativePath(activeRelativePath, queue.sourceFile)) {
    return `repairable (${activeRelativePath})`;
  }
  return `no_matching_gap (${activeRelativePath}; top repair source ${queue.sourceFile ?? 'not_available'})`;
}

function receiptStateForRepairFirst(
  receipt: RiprReceiptArtifactStatus,
  firstAction: FirstUsefulActionStatus | undefined,
  queue: RiprActionableGapQueueStatus
): string {
  if (receipt.state === 'found' && receipt.movement) {
    return `found (${receipt.movement})`;
  }
  if (receipt.state !== 'missing' && receipt.state !== 'noWorkspace') {
    return receipt.state;
  }
  if (queue.receiptCommandOrPath) {
    return `missing; command available (${queue.receiptCommandOrPath})`;
  }
  if (firstAction?.receiptCommand) {
    return `missing; command available (${firstAction.receiptCommand})`;
  }
  return receipt.state;
}

function repairFirstFailClosedCommand(
  status: RiprStatusState,
  queue: RiprActionableGapQueueStatus,
  context: RiprStatusContext
): string {
  const setupBlocker = setupRepairBlocker(context);
  if (setupBlocker) {
    return setupBlocker;
  }
  if (status.kind === 'stale' && actionableGapQueueCanBecomeStale(queue.state)) {
    return 'Save the file or refresh saved-workspace evidence before acting on repair packets.';
  }
  return actionableGapQueueSuppressedMessage(queue);
}

function sameWorkspaceRelativePath(left: string, right: string): boolean {
  return normalizePath(left) === normalizePath(right);
}

function actionableGapQueueStatusLines(
  status: RiprStatusState,
  context: RiprStatusContext
): string[] {
  const queue = context.setupStatus.actionableQueue;
  if (queue.state === 'noWorkspace') {
    return [];
  }
  if (status.kind === 'stale' && actionableGapQueueCanBecomeStale(queue.state)) {
    return [
      `Actionable gap queue: stale; ${queue.relativePath} exists, but editor evidence is stale.`,
      'Refresh saved-workspace evidence before acting on the queue.',
      'Actionable gap queue is advisory static evidence only; it does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.'
    ];
  }
  switch (queue.state) {
    case 'missing':
      return [
        `Actionable gap queue: missing; ${queue.relativePath} was not found.`,
        'Next safe queue action: run cargo xtask evidence-quality-audit or refresh saved-workspace evidence.'
      ];
    case 'malformed':
      return [
        `Actionable gap queue: malformed; ${queue.relativePath} could not be parsed as an actionable-gaps packet.`,
        queue.detail ?? 'No parser detail was reported.',
        'Queue repair actions are suppressed.'
      ];
    case 'unsupportedSchema':
      return [
        `Actionable gap queue: malformed; ${queue.relativePath} uses an unsupported actionable-gaps schema.`,
        queue.detail ?? 'No schema detail was reported.',
        'Queue repair actions are suppressed.'
      ];
    case 'wrongRoot':
      return [
        `Actionable gap queue: wrong root; queue root ${queue.repoRoot ?? 'unknown'} does not match this workspace.`,
        `Expected workspace root: ${context.workspaceRoot ?? 'unknown'}.`,
        'Queue repair actions are suppressed.'
      ];
    case 'stale':
      return [
        `Actionable gap queue: stale; ${queue.relativePath} reports stale upstream evidence.`,
        queue.detail ?? 'Refresh saved-workspace evidence before acting on the queue.',
        'Queue repair actions are suppressed.'
      ];
    case 'unsafePath':
      return [
        `Actionable gap queue: unsafe path; ${queue.relativePath} references a path outside this workspace.`,
        queue.detail ?? 'No path detail was reported.',
        'Queue repair actions are suppressed.'
      ];
    case 'unsafeCommand':
      return [
        `Actionable gap queue: unsafe command; ${queue.relativePath} contains a command payload outside the editor safety contract.`,
        queue.detail ?? 'No command detail was reported.',
        'Queue repair actions are suppressed.'
      ];
    case 'blocked':
      return [
        `Actionable gap queue: blocked; ${queue.relativePath} has bounded run limitations or producer exclusion reasons.`,
        queue.detail ?? 'No limitation detail was reported.',
        'Queue repair actions are suppressed.'
      ];
    case 'topActionableGap':
      return actionableGapQueueTopLines(queue);
    case 'reportOnly':
      return [
        `Actionable gap queue: report-only; ${queue.relativePath} has no repairable queue item.`,
        `Report-only gaps: ${queue.reportOnlyGaps ?? 0}. Static-limit-only gaps: ${queue.staticLimitOnlyGaps ?? 0}.`,
        'No local queue repair action is projected from report-only evidence.'
      ];
    case 'staticLimitOnly':
      return [
        `Actionable gap queue: static-limit-only; ${queue.relativePath} has no repairable queue item.`,
        `Static-limit-only gaps: ${queue.staticLimitOnlyGaps ?? 0}.`,
        'Static-limit-only evidence is advisory and must not become a repair packet without a typed repair route.'
      ];
    case 'noAction':
      return [
        `Actionable gap queue: no actionable gap; ${queue.relativePath} reports zero safe repair packets.`,
        'No local queue repair action is projected from this packet.',
        'No-action queue state does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.'
      ];
  }
}

function actionableGapQueueTopLines(queue: RiprActionableGapQueueStatus): string[] {
  const lines = [
    `Actionable gap queue: top repair ready; ${queue.relativePath} is advisory.`,
    `Actionable gaps: ${queue.actionableGaps ?? 1}. Report-only gaps: ${queue.reportOnlyGaps ?? 0}. Static-limit-only gaps: ${queue.staticLimitOnlyGaps ?? 0}.`
  ];
  if (queue.topRepair) {
    lines.push(`Top repair: ${queue.topRepair}`);
  }
  const identity = queue.canonicalGapId ?? queue.seamId ?? queue.findingId;
  if (identity) {
    lines.push(`Gap identity: ${identity}`);
  }
  if (queue.language) {
    lines.push(`Language: ${queue.language}${queue.languageStatus ? ` (${queue.languageStatus})` : ''}`);
  }
  if (queue.relatedTest) {
    lines.push(`Related test: ${queue.relatedTest}`);
  }
  if (queue.targetTestType) {
    lines.push(`Target test type: ${queue.targetTestType}`);
  }
  if (queue.assertionShape) {
    lines.push(`Target assertion: ${queue.assertionShape}`);
  }
  if (queue.verifyCommand) {
    lines.push(`Verify: ${queue.verifyCommand}`);
  }
  if (queue.receiptCommandOrPath) {
    lines.push(`Receipt: ${queue.receiptCommandOrPath}`);
  }
  if (queue.confidenceBasis) {
    lines.push(`Confidence basis: ${queue.confidenceBasis}`);
  }
  lines.push('Next safe queue action: inspect the related test or matching diagnostic; Copy Current Repair Packet is a later queue action.');
  lines.push('Actionable gap queue is advisory static evidence only; it does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.');
  return lines;
}

function actionableGapQueueAllowsCurrentRepairPacket(queue: RiprActionableGapQueueStatus): boolean {
  return queue.state === 'topActionableGap'
    && Boolean(queue.canonicalGapId)
    && Boolean(queue.topRepair)
    && Boolean(queue.relatedTest)
    && Boolean(queue.verifyCommand)
    && Boolean(queue.receiptCommandOrPath)
    && queue.languageStatus !== 'disabled'
    && queue.languageStatus !== 'unavailable'
    && queue.languageStatus !== 'unsupported';
}

function actionableGapQueueSuppressedMessage(queue: RiprActionableGapQueueStatus): string {
  switch (queue.state) {
    case 'missing':
      return 'ripr actionable gap queue is missing; run cargo xtask evidence-quality-audit or refresh saved-workspace evidence.';
    case 'malformed':
      return 'ripr actionable gap queue is malformed; repair packet actions are suppressed.';
    case 'unsupportedSchema':
      return 'ripr actionable gap queue schema is unsupported; repair packet actions are suppressed.';
    case 'wrongRoot':
      return 'ripr actionable gap queue belongs to another workspace; repair packet actions are suppressed.';
    case 'stale':
      return 'ripr actionable gap queue is stale; refresh saved-workspace evidence before copying repair packets.';
    case 'unsafePath':
      return 'ripr actionable gap queue contains an unsafe path; repair packet actions are suppressed.';
    case 'unsafeCommand':
      return 'ripr actionable gap queue contains an unsafe command; repair packet actions are suppressed.';
    case 'blocked':
      return 'ripr actionable gap queue is blocked by missing typed fields or producer limitations; repair packet actions are suppressed.';
    case 'reportOnly':
      return 'ripr actionable gap queue is report-only; no current repair packet is available.';
    case 'staticLimitOnly':
      return 'ripr actionable gap queue is static-limit-only; no current repair packet is available.';
    case 'noAction':
      return 'ripr actionable gap queue has no actionable gap; no current repair packet is available.';
    case 'noWorkspace':
      return 'Open a workspace before copying a ripr current repair packet.';
    case 'topActionableGap':
      return 'ripr actionable gap queue is missing required repair packet fields; repair packet actions are suppressed.';
  }
}

function currentRepairPacket(queue: RiprActionableGapQueueStatus): string {
  const lines = [
    'RIPR current repair packet',
    '',
    'Task',
    `Repair this one canonical gap: ${queue.canonicalGapId ?? 'unknown'}.`,
    '',
    'Context'
  ];
  pushOptionalLine(lines, 'Source', queue.sourceFile);
  pushOptionalLine(lines, 'Language', queue.languageStatus ? `${queue.language ?? 'unknown'} (${queue.languageStatus})` : queue.language);
  pushOptionalLine(lines, 'Evidence class', queue.evidenceClass);
  pushOptionalLine(lines, 'Actionability', queue.actionability);
  pushOptionalLine(lines, 'Confidence basis', queue.confidenceBasis);
  lines.push(`Related test: ${queue.relatedTest ?? 'not provided by typed queue artifact'}`);
  lines.push('');
  lines.push('Repair');
  pushOptionalLine(lines, 'Repair kind', queue.topRepair);
  pushOptionalLine(lines, 'Target test type', queue.targetTestType);
  pushOptionalLine(lines, 'Target assertion or output proof', queue.assertionShape);
  lines.push('');
  lines.push('Verification');
  lines.push(`Run: ${queue.verifyCommand}`);
  lines.push('');
  lines.push('Receipt');
  lines.push(`Record: ${queue.receiptCommandOrPath}`);
  lines.push('');
  lines.push('Stop conditions');
  lines.push('- Stop if the queue artifact is stale, wrong-root, malformed, or missing this canonical gap id.');
  lines.push('- Stop if the related test or target assertion no longer matches the changed behavior.');
  lines.push('- Stop if the verify command cannot be run safely in the current workspace.');
  lines.push('');
  lines.push('Do not do');
  lines.push('- Do not broaden scope beyond this one gap.');
  lines.push('- Do not edit production code unless the focused test exposes a real issue.');
  lines.push('- Do not generate tests automatically, call providers, run mutation execution, or claim runtime adequacy.');
  lines.push('- Do not treat this advisory static packet as a gate decision, merge approval, or policy eligibility claim.');
  return lines.join('\n');
}

function actionableGapQueueAllowsRepoGapMap(queue: RiprActionableGapQueueStatus): boolean {
  return queue.state === 'topActionableGap'
    || queue.state === 'noAction'
    || queue.state === 'reportOnly'
    || queue.state === 'staticLimitOnly'
    || queue.state === 'blocked';
}

function actionableGapQueueRepoMapSuppressedMessage(queue: RiprActionableGapQueueStatus): string {
  switch (queue.state) {
    case 'missing':
      return 'ripr actionable gap queue is missing; run cargo xtask evidence-quality-audit before copying a repo gap map.';
    case 'malformed':
      return 'ripr actionable gap queue is malformed; repo gap map is suppressed.';
    case 'unsupportedSchema':
      return 'ripr actionable gap queue schema is unsupported; repo gap map is suppressed.';
    case 'wrongRoot':
      return 'ripr actionable gap queue belongs to another workspace; repo gap map is suppressed.';
    case 'unsafePath':
      return 'ripr actionable gap queue contains an unsafe path; repo gap map is suppressed.';
    case 'unsafeCommand':
      return 'ripr actionable gap queue contains an unsafe command; repo gap map is suppressed.';
    case 'noWorkspace':
      return 'Open a workspace before copying a ripr repo gap map.';
    case 'stale':
      return 'ripr actionable gap queue is stale; refresh saved-workspace evidence before copying a repo gap map.';
    case 'topActionableGap':
    case 'noAction':
    case 'reportOnly':
    case 'staticLimitOnly':
    case 'blocked':
      return 'ripr repo gap map is unavailable for the current queue state.';
  }
}

function repoGapMap(
  queue: RiprActionableGapQueueStatus,
  receipt: RiprReceiptArtifactStatus,
  firstPr: RiprFirstPrPacketStatus
): string {
  const lines = [
    'RIPR repo gap map',
    '',
    'Scope',
    `Artifact: ${queue.relativePath}`,
    `Queue state: ${queue.state}`,
    `Actionable gaps: ${queue.actionableGaps ?? 0}`,
    `Report-only gaps: ${queue.reportOnlyGaps ?? 0}`,
    `Static-limit-only gaps: ${queue.staticLimitOnlyGaps ?? 0}`,
    '',
    'Top queue item'
  ];
  if (queue.state === 'topActionableGap') {
    pushOptionalLine(lines, 'Canonical gap id', queue.canonicalGapId);
    pushOptionalLine(lines, 'Repair kind', queue.topRepair);
    pushOptionalLine(lines, 'Language', queue.languageStatus ? `${queue.language ?? 'unknown'} (${queue.languageStatus})` : queue.language);
    pushOptionalLine(lines, 'Related test', queue.relatedTest);
    pushOptionalLine(lines, 'Verify', queue.verifyCommand);
    pushOptionalLine(lines, 'Receipt command or path', queue.receiptCommandOrPath);
  } else {
    lines.push(`No top repair packet is available in state ${queue.state}.`);
    if (queue.detail) {
      lines.push(`Detail: ${queue.detail}`);
    }
  }
  lines.push('');
  lines.push('Receipt state');
  lines.push(`State: ${receipt.state}`);
  pushOptionalLine(lines, 'Movement', receipt.movement);
  pushOptionalLine(lines, 'Receipt seam', receipt.seamId);
  lines.push('');
  lines.push('First PR packet state');
  lines.push(`State: ${firstPr.state}`);
  pushOptionalLine(lines, 'First PR gap id', firstPr.canonicalGapId ?? firstPr.gapId);
  lines.push('');
  lines.push('Safe next commands');
  lines.push('- Refresh saved-workspace evidence before acting on stale queue state.');
  lines.push('- Run cargo xtask evidence-quality-audit to regenerate actionable-gaps artifacts.');
  lines.push('- Use ripr: Copy Current Repair Packet only for a validated top actionable gap.');
  lines.push('');
  lines.push('Non-claims');
  lines.push('- This map is read-only orientation.');
  lines.push('- It is not a gate decision, merge approval, runtime proof, mutation proof, coverage claim, or policy eligibility claim.');
  return lines.join('\n');
}

function pushOptionalLine(lines: string[], label: string, value: string | undefined): void {
  if (value) {
    lines.push(`${label}: ${value}`);
  }
}

function firstPrPacketStatusLines(
  status: RiprStatusState,
  context: RiprStatusContext
): string[] {
  const packet = context.setupStatus.firstPr;
  if (packet.state === 'noWorkspace') {
    return [];
  }
  if (status.kind === 'stale' && firstPrPacketCanBecomeStale(packet.state)) {
    return [
      `First PR packet: stale; ${packet.relativePath} exists, but editor evidence is stale.`,
      'Refresh saved-workspace evidence and rerun cargo xtask first-pr before inspecting or copying first-pr packet content.',
      'First PR packet is advisory only; it does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.'
    ];
  }
  switch (packet.state) {
    case 'missing':
      return [
        `First PR packet: missing; ${packet.relativePath} was not found.`,
        'Next safe first-pr action: run cargo xtask first-pr for the current workspace after verify/receipt artifacts exist.'
      ];
    case 'unreadable':
      return [
        `First PR packet: unreadable; ${packet.relativePath} could not be read.`,
        packet.detail ?? 'No reader detail was reported.',
        'First PR packet repair claims are suppressed.'
      ];
    case 'malformed':
      return [
        `First PR packet: malformed; ${packet.relativePath} could not be parsed as a first-pr packet.`,
        packet.detail ?? 'No parser detail was reported.',
        'First PR packet repair claims are suppressed.'
      ];
    case 'unsupportedSchema':
      return [
        `First PR packet: malformed; ${packet.relativePath} uses an unsupported first-pr packet schema.`,
        packet.detail ?? 'No schema detail was reported.',
        'First PR packet repair claims are suppressed.'
      ];
    case 'wrongRoot':
      return [
        `First PR packet: wrong root; packet root ${packet.repoRoot ?? 'unknown'} does not match this workspace.`,
        `Expected workspace root: ${context.workspaceRoot ?? 'unknown'}.`,
        'First PR packet repair claims are suppressed.'
      ];
    case 'unsafePath':
      return [
        `First PR packet: unsafe path; ${packet.relativePath} references a path outside this workspace.`,
        packet.detail ?? 'No path detail was reported.',
        'Open/copy first-pr packet actions are suppressed.'
      ];
    case 'unsafeCommand':
      return [
        `First PR packet: unsafe command; ${packet.relativePath} contains a command payload outside the editor safety contract.`,
        packet.detail ?? 'No command detail was reported.',
        'Copy-command first-pr packet actions are suppressed.'
      ];
    case 'topRepairableGap':
      return firstPrTopRepairableGapLines(packet);
    case 'noAction':
      return [
        `First PR packet: no actionable gap; ${packet.relativePath} reports ${packet.selectedState ?? 'no_action'}.`,
        'No local first-pr repair action is projected from this packet.',
        'No-action first-pr state does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.'
      ];
    case 'blocked':
      return firstPrBlockedPacketLines(packet);
    case 'found':
      return [
        `First PR packet: found; ${packet.relativePath} is advisory.`,
        'Inspect the packet before carrying evidence into PR review.',
        'First PR packet does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.'
      ];
  }
}

function firstPrPacketCanBecomeStale(state: RiprFirstPrPacketState): boolean {
  return state === 'found'
    || state === 'topRepairableGap'
    || state === 'noAction'
    || state === 'blocked';
}

function actionableGapQueueCanBecomeStale(state: RiprActionableGapQueueState): boolean {
  return state === 'topActionableGap'
    || state === 'noAction'
    || state === 'reportOnly'
    || state === 'staticLimitOnly'
    || state === 'blocked';
}

function firstPrPacketAllowsSummary(state: RiprFirstPrPacketState): boolean {
  return state === 'found'
    || state === 'topRepairableGap'
    || state === 'noAction';
}

function firstPrPacketAllowsOpen(state: RiprFirstPrPacketState): boolean {
  return state === 'found'
    || state === 'topRepairableGap'
    || state === 'noAction';
}

function firstPrHasRepairPacket(packet: RiprFirstPrPacketStatus): boolean {
  return Boolean(
    (packet.canonicalGapId ?? packet.gapId) &&
    packet.repairRoute &&
    (packet.relatedTest || packet.repairTarget) &&
    packet.verifyCommand &&
    packet.receiptCommand
  );
}

function firstPrSuppressedMessage(packet: RiprFirstPrPacketStatus): string {
  switch (packet.state) {
    case 'missing':
      return 'ripr first-pr packet is missing; run cargo xtask first-pr after verify/receipt artifacts exist.';
    case 'unreadable':
      return 'ripr first-pr packet is unreadable; bounded first-pr actions are suppressed.';
    case 'malformed':
    case 'unsupportedSchema':
      return 'ripr first-pr packet is malformed or unsupported; bounded first-pr actions are suppressed.';
    case 'wrongRoot':
      return 'ripr first-pr packet belongs to another workspace; bounded first-pr actions are suppressed.';
    case 'unsafePath':
      return 'ripr first-pr packet references an unsafe path; bounded first-pr actions are suppressed.';
    case 'unsafeCommand':
      return 'ripr first-pr packet contains an unsafe command; copy-command actions are suppressed.';
    case 'noAction':
      return 'ripr first-pr packet has no actionable gap; no repair packet is projected.';
    case 'blocked':
      return 'ripr first-pr packet is blocked; copy regeneration guidance before acting.';
    case 'noWorkspace':
      return 'Open a workspace before using ripr first-pr packet actions.';
    case 'found':
    case 'topRepairableGap':
      return 'ripr first-pr packet does not contain a bounded action for this command.';
  }
}

function firstPrSummaryPacket(packet: RiprFirstPrPacketStatus): string {
  const lines = [
    'RIPR first-pr summary',
    '',
    `State: ${packet.state}`,
    `Packet: ${packet.markdownRelativePath ?? packet.relativePath}`
  ];
  if (packet.selectedState) {
    lines.push(`Selected state: ${packet.selectedState}`);
  }
  if (packet.canonicalGapId ?? packet.gapId) {
    lines.push(`Gap identity: ${packet.canonicalGapId ?? packet.gapId}`);
  }
  if (packet.selectedKind) {
    lines.push(`Gap kind: ${packet.selectedKind}`);
  }
  if (packet.changedBehavior) {
    lines.push(`Changed behavior: ${packet.changedBehavior}`);
  }
  if (packet.missingDiscriminator) {
    lines.push(`Missing discriminator: ${packet.missingDiscriminator}`);
  }
  if (packet.focusedProofIntent) {
    lines.push(`Focused proof intent: ${packet.focusedProofIntent}`);
  }
  if (packet.why) {
    lines.push(`Why this matters: ${packet.why}`);
  }
  if (packet.relatedTest) {
    lines.push(`Related test: ${packet.relatedTest}`);
  }
  if (packet.repairTarget) {
    lines.push(`Repair target: ${packet.repairTarget}`);
  }
  if (packet.verifyCommand) {
    lines.push(`Verify command: ${packet.verifyCommand}`);
  }
  if (packet.receiptCommand) {
    lines.push(`Receipt command: ${packet.receiptCommand}`);
  }
  if (packet.receiptPath) {
    lines.push(`Receipt path: ${packet.receiptPath}`);
  }
  lines.push(`Warnings: ${packet.warningCount ?? 0}`);
  lines.push('');
  lines.push('Limits and non-claims:');
  lines.push('- Advisory static evidence only.');
  lines.push('- Does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.');
  lines.push('- Does not edit source, generate tests, publish PR comments, or run providers.');
  return lines.join('\n');
}

function firstPrRepairPacket(packet: RiprFirstPrPacketStatus): string {
  const lines = [
    'RIPR first-pr repair packet',
    '',
    `First PR packet: ${packet.markdownRelativePath ?? packet.relativePath}`,
    `Gap identity: ${packet.canonicalGapId ?? packet.gapId ?? 'unknown'}`
  ];
  if (packet.selectedKind) {
    lines.push(`Gap kind: ${packet.selectedKind}`);
  }
  if (packet.changedBehavior) {
    lines.push(`Changed behavior: ${packet.changedBehavior}`);
  }
  if (packet.missingDiscriminator) {
    lines.push(`Missing discriminator: ${packet.missingDiscriminator}`);
  }
  if (packet.focusedProofIntent) {
    lines.push(`Focused proof intent: ${packet.focusedProofIntent}`);
  }
  if (packet.why) {
    lines.push(`Why this matters: ${packet.why}`);
  }
  if (packet.repairRoute) {
    lines.push(`Repair route: ${packet.repairRoute}`);
  }
  if (packet.repairTarget) {
    lines.push(`Repair target: ${packet.repairTarget}`);
  }
  if (packet.relatedTest) {
    lines.push(`Related test: ${packet.relatedTest}`);
  }
  if (packet.suggestedAssertion) {
    lines.push(`Suggested assertion: ${packet.suggestedAssertion}`);
  }
  lines.push('');
  lines.push('Verify command:');
  lines.push(packet.verifyCommand ?? 'not available');
  lines.push('');
  lines.push('Receipt command:');
  lines.push(packet.receiptCommand ?? 'not available');
  if (packet.receiptPath) {
    lines.push('');
    lines.push('Receipt path:');
    lines.push(packet.receiptPath);
  }
  lines.push('');
  lines.push('Instructions:');
  lines.push('- Add one focused test for this gap.');
  lines.push('- Do not broaden scope.');
  lines.push('- Run the verify command, then emit the receipt.');
  lines.push('- Return the receipt path and result.');
  lines.push('');
  lines.push('Limits and non-claims:');
  lines.push('- Static editor evidence only.');
  lines.push('- Does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.');
  lines.push('- Does not edit source, generate tests, publish PR comments, or run providers.');
  return lines.join('\n');
}

function firstPrRegenerationGuidance(packet: RiprFirstPrPacketStatus): string {
  const lines = [
    'RIPR first-pr regeneration guidance',
    '',
    `Current state: ${packet.state}`,
    `Packet: ${packet.relativePath}`
  ];
  if (packet.detail) {
    lines.push(`Detail: ${packet.detail}`);
  }
  if (packet.selectedState) {
    lines.push(`Selected state: ${packet.selectedState}`);
  }
  lines.push('');
  lines.push('Next safe action:');
  lines.push('cargo xtask first-pr');
  lines.push('');
  lines.push('Limits and non-claims:');
  lines.push('- This is copied guidance only; the editor does not run the command.');
  lines.push('- Regenerate first-pr artifacts for the current workspace before carrying evidence into PR review.');
  return lines.join('\n');
}

function diagnosticMatchesFirstPrPacket(
  diagnostic: vscode.Diagnostic,
  packet: RiprFirstPrPacketStatus
): boolean {
  const packetIds = [
    packet.canonicalGapId,
    packet.gapId
  ].filter((value): value is string => value !== undefined);
  if (packetIds.length === 0) {
    return false;
  }
  const diagnosticIds = [
    diagnosticDataString(diagnostic, 'canonical_gap_id'),
    diagnosticDataString(diagnostic, 'gap_id'),
    diagnosticDataString(diagnostic, 'seam_id'),
    diagnosticDataString(diagnostic, 'finding_id')
  ].filter((value): value is string => value !== undefined);
  return packetIds.some((packetId) => diagnosticIds.includes(packetId));
}

function diagnosticDataString(diagnostic: vscode.Diagnostic, field: string): string | undefined {
  const data = (diagnostic as unknown as { data?: unknown }).data;
  if (!data || typeof data !== 'object' || Array.isArray(data)) {
    return undefined;
  }
  const value = (data as Record<string, unknown>)[field];
  return typeof value === 'string' && value.trim() !== '' ? value : undefined;
}

function firstPrTopRepairableGapLines(packet: RiprFirstPrPacketStatus): string[] {
  const lines = [
    `First PR packet: top repairable gap available; ${packet.relativePath} is advisory.`,
    `Packet: ${packet.markdownRelativePath ?? packet.relativePath}`
  ];
  if (packet.canonicalGapId ?? packet.gapId) {
    lines.push(`Gap identity: ${packet.canonicalGapId ?? packet.gapId}`);
  }
  if (packet.changedBehavior) {
    lines.push(`Changed behavior: ${packet.changedBehavior}`);
  }
  if (packet.missingDiscriminator) {
    lines.push(`Missing discriminator: ${packet.missingDiscriminator}`);
  }
  if (packet.focusedProofIntent) {
    lines.push(`Focused proof intent: ${packet.focusedProofIntent}`);
  }
  if (packet.relatedTest) {
    lines.push(`Related test: ${packet.relatedTest}`);
  }
  if (packet.repairTarget) {
    lines.push(`Repair target: ${packet.repairTarget}`);
  }
  if (packet.verifyCommand) {
    lines.push(`Verify: ${packet.verifyCommand}`);
  }
  if (packet.receiptCommand) {
    lines.push(`Receipt: ${packet.receiptCommand}`);
  }
  if (packet.receiptPath) {
    lines.push(`Receipt path: ${packet.receiptPath}`);
  }
  lines.push(`Warnings: ${packet.warningCount ?? 0}`);
  lines.push('First PR packet does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.');
  return lines;
}

function firstPrBlockedPacketLines(packet: RiprFirstPrPacketStatus): string[] {
  switch (packet.selectedState) {
    case 'missing_artifact':
      return [
        `First PR packet: missing; ${packet.relativePath} reports a missing upstream artifact.`,
        'Regenerate the named artifact, then rerun cargo xtask first-pr.',
        'First PR packet repair claims are suppressed.'
      ];
    case 'stale_artifact':
      return [
        `First PR packet: stale; ${packet.relativePath} reports stale upstream evidence.`,
        'Refresh saved-workspace evidence and rerun cargo xtask first-pr before acting.',
        'First PR packet repair claims are suppressed.'
      ];
    case 'wrong_root':
      return [
        `First PR packet: wrong root; ${packet.relativePath} reports an upstream artifact for another workspace.`,
        'Regenerate first-pr inputs for the current workspace.',
        'First PR packet repair claims are suppressed.'
      ];
    case 'malformed_artifact':
      return [
        `First PR packet: malformed; ${packet.relativePath} reports a malformed upstream artifact.`,
        'Regenerate the malformed artifact, then rerun cargo xtask first-pr.',
        'First PR packet repair claims are suppressed.'
      ];
    case 'timeout':
      return [
        `First PR packet: blocked; ${packet.relativePath} reports a timeout while composing first-pr evidence.`,
        'Rerun cargo xtask first-pr or inspect the blocked artifact before acting.',
        'First PR packet repair claims are suppressed.'
      ];
    case 'blocked_artifact':
    default:
      return [
        `First PR packet: blocked; ${packet.relativePath} reports ${packet.selectedState ?? 'blocked_artifact'}.`,
        'Inspect or regenerate first-pr inputs before carrying evidence into PR review.',
        'First PR packet repair claims are suppressed.'
      ];
  }
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
        `Expected workspace root: ${context.workspaceRoot ?? 'unknown'}.`,
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
  const actionableQueue = await readActionableGapQueueStatus(workspaceRoot, readFile);
  const receipt = await readReceiptStatus(workspaceRoot, readFile);
  const firstPr = await readFirstPrPacketStatus(workspaceRoot, readFile);
  return { config, artifacts, actionableQueue, receipt, firstPr };
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
    actionableQueue: {
      relativePath: ACTIONABLE_GAP_QUEUE_RELATIVE_PATH,
      state: 'noWorkspace',
      detail: 'open a workspace before matching actionable gap queue artifacts'
    },
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

export async function readActionableGapQueueStatus(
  workspaceRoot: string,
  readFile: RiprClientRuntime['readFile']
): Promise<RiprActionableGapQueueStatus> {
  const filePath = setupFilePath(workspaceRoot, ACTIONABLE_GAP_QUEUE_RELATIVE_PATH);
  let raw: string | undefined;
  try {
    raw = await readFile(filePath);
  } catch (error) {
    return {
      relativePath: ACTIONABLE_GAP_QUEUE_RELATIVE_PATH,
      path: filePath,
      state: 'malformed',
      detail: error instanceof Error ? error.message : String(error)
    };
  }
  if (raw === undefined) {
    return {
      relativePath: ACTIONABLE_GAP_QUEUE_RELATIVE_PATH,
      path: filePath,
      state: 'missing',
      detail: 'actionable-gaps queue missing; run cargo xtask evidence-quality-audit'
    };
  }
  return validateActionableGapQueue(raw, workspaceRoot, filePath);
}

function validateActionableGapQueue(
  raw: string,
  workspaceRoot: string,
  filePath: string
): RiprActionableGapQueueStatus {
  const base = {
    relativePath: ACTIONABLE_GAP_QUEUE_RELATIVE_PATH,
    path: filePath
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
      detail: 'actionable-gaps JSON root is not an object'
    };
  }
  const queue = parsed as Record<string, unknown>;
  if (
    stringField(queue, 'schema_version') !== '0.1' ||
    stringField(queue, 'tool') !== 'ripr' ||
    stringField(queue, 'report') !== 'actionable-gaps'
  ) {
    return {
      ...base,
      state: 'unsupportedSchema',
      detail: 'expected ripr actionable-gaps report schema_version 0.1'
    };
  }
  const repoRoot = stringField(queue, 'root');
  if (!rootMatchesWorkspace(repoRoot, workspaceRoot)) {
    return {
      ...base,
      state: 'wrongRoot',
      repoRoot,
      detail: 'actionable-gaps root does not match the active workspace'
    };
  }
  if (stringField(queue, 'status') === 'stale' || staleFreshness(queue)) {
    return {
      ...base,
      state: 'stale',
      detail: 'actionable-gaps report freshness is stale'
    };
  }
  const runLimitations = queue['run_limitations'];
  if (Array.isArray(runLimitations) && runLimitations.length > 0) {
    return {
      ...base,
      state: 'blocked',
      detail: 'actionable-gaps report carries run_limitations, so counts are not complete repo truth'
    };
  }
  const packets = queue['packets'];
  if (!Array.isArray(packets)) {
    return {
      ...base,
      state: 'malformed',
      detail: 'actionable-gaps report must contain packets[]'
    };
  }
  const summary = objectField(queue, 'summary');
  const actionableGaps = summary ? numberFieldValue(summary, 'actionable_gaps') : undefined;
  const packetsEmitted = summary ? numberFieldValue(summary, 'packets_emitted') : undefined;
  if (packets.length === 0) {
    if (actionableGaps === 0 && packetsEmitted === 0) {
      return {
        ...base,
        state: 'noAction',
        actionableGaps,
        packetsEmitted
      };
    }
    return {
      ...base,
      state: 'malformed',
      detail: 'empty actionable-gaps report must carry completed zero-count summary'
    };
  }

  let reportOnlyGaps = 0;
  let staticLimitOnlyGaps = 0;
  let noActionGaps = 0;
  let topActionable: Record<string, unknown> | undefined;
  for (const packet of packets) {
    if (!packet || typeof packet !== 'object' || Array.isArray(packet)) {
      return {
        ...base,
        state: 'malformed',
        detail: 'actionable-gaps packets must be objects'
      };
    }
    const packetObject = packet as Record<string, unknown>;
    const packetPaths = actionableGapQueuePacketPaths(packetObject);
    if (packetPaths.some((packetPath) => !firstPrPathIsWorkspaceLocal(packetPath))) {
      return {
        ...base,
        state: 'unsafePath',
        detail: 'actionable-gaps packet path is outside the workspace'
      };
    }
    for (const command of actionableGapQueuePacketCommands(packetObject)) {
      if (!actionableGapQueueCommandIsSafe(command)) {
        return {
          ...base,
          state: 'unsafeCommand',
          detail: 'actionable-gaps packet command payload is not safe for editor projection'
        };
      }
    }
    if (stringField(packetObject, 'gap_state') === 'actionable') {
      topActionable ??= packetObject;
    } else if (packetIsStaticLimitOnly(packetObject)) {
      staticLimitOnlyGaps += 1;
    } else if (packetIsReportOnly(packetObject)) {
      reportOnlyGaps += 1;
    } else if (packetIsNoAction(packetObject)) {
      noActionGaps += 1;
    }
  }

  if (!topActionable) {
    if (staticLimitOnlyGaps > 0) {
      return {
        ...base,
        state: 'staticLimitOnly',
        actionableGaps,
        packetsEmitted,
        reportOnlyGaps,
        staticLimitOnlyGaps
      };
    }
    if (reportOnlyGaps > 0) {
      return {
        ...base,
        state: 'reportOnly',
        actionableGaps,
        packetsEmitted,
        reportOnlyGaps,
        staticLimitOnlyGaps
      };
    }
    if (noActionGaps === packets.length) {
      return {
        ...base,
        state: 'noAction',
        actionableGaps,
        packetsEmitted,
        reportOnlyGaps,
        staticLimitOnlyGaps
      };
    }
    return {
      ...base,
      state: 'blocked',
      detail: 'actionable-gaps report has no validated actionable packet'
    };
  }

  const projectionReasons = stringArrayField(topActionable, 'projection_exclusion_reasons');
  if (
    stringField(topActionable, 'repair_route_source') !== 'canonical_item.repair_route' ||
    stringField(topActionable, 'verify_command_source') !== 'canonical_item.verify_command' ||
    topActionable['public_projection_eligible'] !== true ||
    projectionReasons.length > 0
  ) {
    return {
      ...base,
      state: 'blocked',
      detail: projectionReasons.length > 0
        ? `producer excluded packet from projection: ${projectionReasons.join(', ')}`
        : 'producer did not mark the actionable packet safe for public projection'
    };
  }
  const verifyCommand = stringField(topActionable, 'verify_command');
  const receiptCommandOrPath = stringField(topActionable, 'receipt_command_or_path');
  const canonicalGapId = stringField(topActionable, 'canonical_gap_id');
  if (!canonicalGapId) {
    return {
      ...base,
      state: 'blocked',
      detail: 'actionable packet is missing canonical_gap_id'
    };
  }
  if (!verifyCommand || !receiptCommandOrPath) {
    return {
      ...base,
      state: 'malformed',
      detail: 'actionable packet is missing verify or receipt payload'
    };
  }
  return {
    ...base,
    state: 'topActionableGap',
    actionableGaps,
    packetsEmitted,
    reportOnlyGaps,
    staticLimitOnlyGaps,
    topRepair: stringField(topActionable, 'repair_kind'),
    sourceFile: stringField(topActionable, 'source_file'),
    evidenceClass: stringField(topActionable, 'evidence_class'),
    actionability: stringField(topActionable, 'actionability'),
    canonicalGapId,
    seamId: stringField(topActionable, 'seam_id'),
    findingId: stringField(topActionable, 'finding_id'),
    language: actionableGapPacketLanguage(topActionable),
    languageStatus: actionableGapPacketLanguageStatus(topActionable),
    relatedTest: actionableGapPacketRelatedTest(topActionable),
    targetTestType: stringField(topActionable, 'target_test_type'),
    assertionShape: stringField(topActionable, 'assertion_shape'),
    verifyCommand,
    receiptCommandOrPath,
    confidenceBasis: stringField(topActionable, 'confidence_basis')
  };
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
    selectedArtifact ? stringField(selectedArtifact, 'path') : undefined,
    stringField(selected, 'receipt_path')
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
    selectedKind: stringField(selected, 'kind'),
    changedBehavior: stringField(selected, 'changed_behavior'),
    missingDiscriminator: stringField(selected, 'missing_discriminator'),
    focusedProofIntent: stringField(selected, 'focused_proof_intent'),
    why: stringField(selected, 'why'),
    gapId: stringField(selected, 'gap_id'),
    canonicalGapId: stringField(selected, 'canonical_gap_id'),
    repairRoute: repair ? stringField(repair, 'route') : undefined,
    suggestedAssertion: repair ? stringField(repair, 'suggested_assertion') : undefined,
    verifyCommand: stringField(selected, 'verify_command'),
    receiptCommand: stringField(selected, 'receipt_command'),
    receiptPath: stringField(selected, 'receipt_path'),
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
  const normalized = command.trim().replace(/\s+/g, ' ');
  return normalized !== ''
    && !hasUnsafeShellMetacharacter(normalized)
    && FIRST_PR_SAFE_COMMAND_PREFIXES.some((prefix) =>
      normalized === prefix || normalized.startsWith(`${prefix} `)
    );
}

const FIRST_PR_SAFE_COMMAND_PREFIXES = [
  'cargo xtask first-pr',
  'cargo xtask fixtures',
  'cargo xtask goldens check',
  'ripr first-pr',
  'ripr start-here',
  'ripr reports gap-ledger',
  'ripr first-action',
  'ripr review-comments',
  'ripr agent packet',
  'ripr agent verify',
  'ripr agent receipt',
  'ripr gate evaluate',
  'ripr outcome'
];

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

function staleFreshness(value: Record<string, unknown>): boolean {
  const freshness = stringField(value, 'freshness');
  if (freshness === 'stale') {
    return true;
  }
  const editorContext = objectField(value, 'editor_context');
  return editorContext ? stringField(editorContext, 'freshness') === 'stale' : false;
}

function actionableGapQueuePacketPaths(packet: Record<string, unknown>): string[] {
  const observer = objectField(packet, 'related_test_or_observer');
  const anchor = objectField(packet, 'primary_anchor');
  const receiptPath = actionableGapQueueReceiptPath(packet);
  return [
    stringField(packet, 'source_file'),
    stringField(packet, 'target_test'),
    stringField(packet, 'target_file'),
    anchor ? stringField(anchor, 'file') : undefined,
    observer ? stringField(observer, 'file') : undefined,
    observer ? stringField(observer, 'path') : undefined,
    observer ? stringField(observer, 'related_test') : undefined,
    observer ? stringField(observer, 'test') : undefined,
    observer ? stringField(observer, 'target_file') : undefined,
    receiptPath
  ].filter((value): value is string => value !== undefined);
}

function actionableGapQueuePacketCommands(packet: Record<string, unknown>): string[] {
  const commands = [stringField(packet, 'verify_command')];
  const receipt = stringField(packet, 'receipt_command_or_path');
  if (receipt && looksLikeActionableQueueCommand(receipt)) {
    commands.push(receipt);
  }
  return commands.filter((value): value is string => value !== undefined);
}

function actionableGapQueueReceiptPath(packet: Record<string, unknown>): string | undefined {
  const receipt = stringField(packet, 'receipt_command_or_path');
  return receipt && !looksLikeActionableQueueCommand(receipt) ? receipt : undefined;
}

function looksLikeActionableQueueCommand(value: string): boolean {
  const normalized = value.trim();
  return normalized.startsWith('cargo ') || normalized.startsWith('ripr ');
}

function packetIsStaticLimitOnly(packet: Record<string, unknown>): boolean {
  return stringField(packet, 'gap_state') === 'static_limit_only'
    || stringField(packet, 'actionability') === 'static_limit_only';
}

function packetIsReportOnly(packet: Record<string, unknown>): boolean {
  return stringField(packet, 'gap_state') === 'report_only'
    || stringField(packet, 'actionability') === 'report_only';
}

function packetIsNoAction(packet: Record<string, unknown>): boolean {
  const state = stringField(packet, 'gap_state');
  const actionability = stringField(packet, 'actionability');
  return actionability === 'no_action'
    || state === 'already_improved'
    || state === 'already_observed'
    || state === 'baseline_only'
    || state === 'internal'
    || state === 'internal_only'
    || state === 'no_actionable_seam'
    || state === 'not_policy_targeted'
    || state === 'resolved'
    || state === 'suppressed'
    || state === 'acknowledged'
    || state === 'waived';
}

function actionableGapPacketLanguage(packet: Record<string, unknown>): string | undefined {
  const direct = stringField(packet, 'language');
  if (direct) {
    return direct;
  }
  return firstRawFindingString(packet, 'language');
}

function actionableGapPacketLanguageStatus(packet: Record<string, unknown>): string | undefined {
  const direct = stringField(packet, 'language_status');
  if (direct) {
    return direct;
  }
  return firstRawFindingString(packet, 'language_status');
}

function actionableGapPacketRelatedTest(packet: Record<string, unknown>): string | undefined {
  const observer = objectField(packet, 'related_test_or_observer');
  return stringField(packet, 'target_test')
    ?? (observer ? stringField(observer, 'related_test') : undefined)
    ?? (observer ? stringField(observer, 'test') : undefined)
    ?? (observer ? stringField(observer, 'file') : undefined);
}

function firstRawFindingString(packet: Record<string, unknown>, field: string): string | undefined {
  const findings = packet['raw_findings'];
  if (!Array.isArray(findings)) {
    return undefined;
  }
  for (const finding of findings) {
    if (finding && typeof finding === 'object' && !Array.isArray(finding)) {
      const value = stringField(finding as Record<string, unknown>, field);
      if (value) {
        return value;
      }
    }
  }
  return undefined;
}

function actionableGapQueueCommandIsSafe(command: string): boolean {
  const normalized = command.trim().replace(/\s+/g, ' ');
  return normalized !== ''
    && !hasUnsafeShellMetacharacter(normalized)
    && ACTIONABLE_QUEUE_SAFE_COMMAND_PREFIXES.some((prefix) =>
      normalized === prefix || normalized.startsWith(`${prefix} `)
    );
}

const ACTIONABLE_QUEUE_SAFE_COMMAND_PREFIXES = [
  'cargo xtask evidence-quality-audit',
  'cargo xtask evidence-quality-scorecard',
  'cargo xtask fixtures',
  'cargo xtask goldens check',
  'ripr agent verify',
  'ripr agent receipt',
  'ripr outcome',
  'ripr first-pr',
  'ripr start-here'
];

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
  return sameWorkspaceRoot(resolvedRoot, workspaceRoot);
}

function sameWorkspaceRoot(left: string, right: string): boolean {
  return normalizePath(path.resolve(left)) === normalizePath(path.resolve(right));
}

function activeDocumentRelativePath(workspaceRoot: string | undefined): string | undefined {
  const document = vscode.window.activeTextEditor?.document;
  if (!workspaceRoot || !document || !isRiprFileDocument(document) || document.uri.scheme !== 'file') {
    return undefined;
  }
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
  if (!workspaceFolder || !sameWorkspaceRoot(workspaceFolder.uri.fsPath, workspaceRoot)) {
    return undefined;
  }
  return relativeWorkspacePath(workspaceRoot, document.uri.fsPath);
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

function stringArrayField(value: Record<string, unknown>, field: string): string[] {
  const child = value[field];
  return Array.isArray(child)
    ? child.filter((item): item is string => typeof item === 'string' && item.trim() !== '')
    : [];
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

function currentWorkspaceRootState(): RiprWorkspaceRootState {
  const folders = vscode.workspace.workspaceFolders ?? [];
  if (folders.length === 0) {
    return workspaceRootStateNoWorkspace();
  }
  if (folders.length === 1) {
    return {
      kind: 'singleRoot',
      root: folders[0].uri.fsPath,
      roots: [folders[0].uri.fsPath],
      detail: 'single workspace folder is active'
    };
  }
  const activeEditor = vscode.window.activeTextEditor;
  const activeFolder = activeEditor && activeEditor.document.uri.scheme === 'file'
    ? vscode.workspace.getWorkspaceFolder(activeEditor.document.uri)
    : undefined;
  if (activeFolder) {
    return {
      kind: 'selectedRoot',
      root: activeFolder.uri.fsPath,
      roots: folders.map((folder) => folder.uri.fsPath),
      detail: 'selected from active editor workspace folder'
    };
  }
  return {
    kind: 'ambiguousMultiRoot',
    roots: folders.map((folder) => folder.uri.fsPath),
    detail: 'multiple workspace folders are open and no active editor selected a safe root'
  };
}

function workspaceRootStateNoWorkspace(): RiprWorkspaceRootState {
  return {
    kind: 'noWorkspace',
    roots: [],
    detail: 'open a workspace folder before matching saved-workspace artifacts'
  };
}

function workspaceRootStateLabel(state: RiprWorkspaceRootState): string {
  switch (state.kind) {
    case 'singleRoot':
      return `workspace_single_root (${state.root ?? 'unknown'})`;
    case 'selectedRoot':
      return `workspace_multi_root_selected (${state.root ?? 'unknown'}; roots: ${state.roots.join(', ')})`;
    case 'ambiguousMultiRoot':
      return `workspace_multi_root_ambiguous (roots: ${state.roots.join(', ') || 'unknown'})`;
    case 'noWorkspace':
    default:
      return 'workspace_not_open';
  }
}

function workspaceRootStateDetail(state: RiprWorkspaceRootState): string {
  const lines = [
    state.detail ?? 'workspace root state is unavailable'
  ];
  if (state.roots.length > 0) {
    lines.push(`Workspace folders: ${state.roots.join(', ')}`);
  }
  lines.push('Root-scoped repair actions are suppressed until one workspace folder is selected.');
  return lines.join('\n');
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
