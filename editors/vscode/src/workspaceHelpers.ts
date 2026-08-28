/**
 * Workspace, trace, and extension-version helpers extracted from client.ts
 * (#2553).
 *
 * These functions read VS Code / language-client APIs but have no dependence
 * on `RiprClientController` instance state, so they are extracted to shrink
 * the controller module and make the workspace-root state shape reviewable on
 * its own. Behavior is unchanged.
 */
import * as vscode from 'vscode';
import { Trace } from 'vscode-languageclient';
import type { RiprConfig } from './config';
import type { RiprWorkspaceRootState } from './client';
import { RIPR_DOCUMENT_SELECTORS } from './client';

export function riprDocumentSelectorsForWorkspace(
  workspaceRoot: string
): Array<{ language: string; scheme: 'file'; pattern: string }> {
  const workspacePattern = `${workspaceRoot.replace(/\\/g, '/')}/**/*`;
  return RIPR_DOCUMENT_SELECTORS.map((selector) => ({
    ...selector,
    pattern: workspacePattern
  }));
}

export function extensionVersion(context: vscode.ExtensionContext): string {
  const version = context.extension?.packageJSON?.version;
  return typeof version === 'string' && version.trim() !== '' ? version.replace(/^v/, '') : '0.8.0';
}

export function traceFromConfig(trace: RiprConfig['traceServer']): Trace {
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

/**
 * Quick-pick item for `ripr: Select Workspace Root` (#2077): the folder name
 * as the label and the folder path as the description, so the user can tell
 * same-named folders apart. `root` carries the picked folder back to the
 * controller.
 */
export interface WorkspaceRootPickItem extends vscode.QuickPickItem {
  root: string;
}

/**
 * Build the `ripr: Select Workspace Root` pick list from the workspace
 * folders. Kept pure and exported so the pick-list shape is reviewable and
 * testable without stubbing `vscode.window.showQuickPick` (#2077).
 */
export function workspaceRootPickItems(
  folders: readonly vscode.WorkspaceFolder[]
): WorkspaceRootPickItem[] {
  return folders.map((folder) => ({
    label: folder.name,
    description: folder.uri.fsPath,
    root: folder.uri.fsPath
  }));
}

export function currentWorkspaceRootState(): RiprWorkspaceRootState {
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

export function workspaceRootStateNoWorkspace(): RiprWorkspaceRootState {
  return {
    kind: 'noWorkspace',
    roots: [],
    detail: 'open a workspace folder before matching saved-workspace artifacts'
  };
}

export function workspaceRootStateLabel(state: RiprWorkspaceRootState): string {
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

export function workspaceRootStateDetail(state: RiprWorkspaceRootState): string {
  const lines = [
    state.detail ?? 'workspace root state is unavailable'
  ];
  if (state.roots.length > 0) {
    lines.push(`Workspace folders: ${state.roots.join(', ')}`);
  }
  lines.push('Root-scoped repair actions are suppressed until one workspace folder is selected.');
  return lines.join('\n');
}
