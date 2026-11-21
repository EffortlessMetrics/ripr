/**
 * Pure status-bar render helpers extracted from client.ts (#2554).
 *
 * These functions map a `RiprStatusKind` / `RiprStatusState` to the
 * status-bar text, summary, and theme-color pair. They have no dependence on
 * `RiprClientController` instance state and no dependency on the line-renderer
 * cluster (`statusTooltip`, `repairFirstStatusLines`, …) that stays in
 * `client.ts`. Behavior is unchanged.
 */
import * as vscode from 'vscode';

export type RiprStatusKind =
  | 'disabled'
  | 'noWorkspace'
  | 'workspaceUntrusted'
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

export interface RiprStatusState {
  kind: RiprStatusKind;
  summary: string;
  detail?: string;
  enabledLanguages?: string[];
  nextStep?: string;
}

export interface FirstUsefulActionStatus {
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

/**
 * Background + foreground `ThemeColor` pair for the status bar item, or
 * `undefined` for the default idle/OK colour. The mapping follows VS Code's
 * documented convention (`statusBarItem.errorBackground`,
 * `statusBarItem.warningBackground` — note: `statusBar.*Background` does not
 * exist; the correct key is `statusBarItem.*`):
 *
 * - error (red): the analysis run failed or the server is unavailable.
 *   These need user attention.
 * - warning (yellow): the run is stale, the workspace is untrusted or
 *   ambiguous, or the gap-artifact validation flagged something. These are
 *   degraded-but-not-failed states.
 * - default: transient (analysisRunning/starting/analysisQueued), idle
 *   (ready/analysisReady/gapActionable/...), and the initial `stopped`
 *   state. Setting a colour here would cry wolf; the codicon + text already
 *   convey the state. `stopped` is the extension's initial state and must
 *   not turn the bar red on startup.
 */
export interface StatusBarColors {
  background: vscode.ThemeColor;
  foreground: vscode.ThemeColor;
}

export function canProjectFirstUsefulAction(kind: RiprStatusKind): boolean {
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

export function shouldInlineFirstUsefulAction(kind: RiprStatusKind): boolean {
  return canProjectFirstUsefulAction(kind)
    && kind !== 'gapActionable'
    && kind !== 'gapNoAction';
}

export function statusText(kind: RiprStatusKind, firstAction?: FirstUsefulActionStatus): string {
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
    case 'workspaceUntrusted':
      return '$(shield) ripr: untrusted workspace';
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

export function statusSummary(status: RiprStatusState, firstAction?: FirstUsefulActionStatus): string {
  if (!firstAction || !shouldInlineFirstUsefulAction(status.kind)) {
    return status.summary;
  }
  return `${status.summary} First useful action: ${firstAction.title}`;
}

export function statusBarColors(kind: RiprStatusKind): StatusBarColors | undefined {
  switch (kind) {
    case 'analysisFailed':
    case 'serverUnavailable':
      return {
        background: new vscode.ThemeColor('statusBarItem.errorBackground'),
        foreground: new vscode.ThemeColor('statusBarItem.errorForeground'),
      };
    case 'stale':
    case 'workspaceAmbiguous':
    case 'workspaceUntrusted':
    case 'gapArtifactWarning':
      return {
        background: new vscode.ThemeColor('statusBarItem.warningBackground'),
        foreground: new vscode.ThemeColor('statusBarItem.warningForeground'),
      };
    default:
      return undefined;
  }
}
