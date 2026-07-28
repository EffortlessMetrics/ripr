/**
 * Shared JSON/path helpers used by the first-PR packet reader, the
 * actionable-gap-queue reader, and the clipboard commands.
 *
 * Extracted from client.ts as part of the decomposition wave (#2438/#2552).
 * These are pure functions with no `this` dependence and no VS Code API calls.
 */

import * as path from 'path';

/**
 * Join a workspace root with a forward-slash relative path.
 */
export function setupFilePath(workspaceRoot: string, relativePath: string): string {
  return path.join(workspaceRoot, ...relativePath.split('/'));
}

/**
 * Extract non-empty string values from a record's values.
 */
export function stringValues(value: Record<string, unknown> | undefined): string[] {
  if (!value) {
    return [];
  }
  return Object.values(value).filter(
    (child): child is string => typeof child === 'string' && child.trim() !== ''
  );
}

/**
 * Check if a command contains shell metacharacters that could inject.
 */
export function hasUnsafeShellMetacharacter(command: string): boolean {
  return /[\r\n\0`;&|\\]/.test(command);
}

/**
 * Normalize a path for cross-platform comparison.
 */
export function normalizePath(value: string): string {
  const normalized = path.normalize(value).replace(/\\/g, '/');
  return process.platform === 'win32' ? normalized.toLowerCase() : normalized;
}

/**
 * Check if two paths resolve to the same workspace root.
 */
export function sameWorkspaceRoot(left: string, right: string): boolean {
  return normalizePath(path.resolve(left)) === normalizePath(path.resolve(right));
}

/**
 * Check if a root matches the workspace root (handles '.' and relative paths).
 */
export function rootMatchesWorkspace(root: string | undefined, workspaceRoot: string): boolean {
  if (!root || root === '.') {
    return true;
  }
  const resolvedRoot = path.isAbsolute(root)
    ? path.resolve(root)
    : path.resolve(workspaceRoot, root);
  return sameWorkspaceRoot(resolvedRoot, workspaceRoot);
}

/**
 * Get an object-typed field from a record.
 */
export function objectField(
  value: Record<string, unknown>,
  field: string
): Record<string, unknown> | undefined {
  const child = value[field];
  return child && typeof child === 'object' && !Array.isArray(child)
    ? (child as Record<string, unknown>)
    : undefined;
}

/**
 * Get a non-empty string field from a record.
 */
export function stringField(
  value: Record<string, unknown>,
  field: string
): string | undefined {
  const child = value[field];
  return typeof child === 'string' && child.trim() !== '' ? child : undefined;
}

/**
 * Get a string field that must be in an allowed set.
 */
export function boundedStringField(
  value: Record<string, unknown>,
  field: string,
  allowed: Set<string>
): string | undefined {
  const child = stringField(value, field);
  return child && allowed.has(child) ? child : undefined;
}

/**
 * Get the length of an array-typed field (0 if absent or not an array).
 */
export function arrayLength(value: Record<string, unknown>, field: string): number {
  const child = value[field];
  return Array.isArray(child) ? child.length : 0;
}

/**
 * Get a finite number field from a record.
 */
export function numberFieldValue(
  value: Record<string, unknown>,
  field: string
): number | undefined {
  const child = value[field];
  return typeof child === 'number' && Number.isFinite(child) ? child : undefined;
}
