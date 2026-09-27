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
  return path.join(workspaceRoot, relativePath);
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
 *
 * Refused anywhere: line breaks, NUL, `` ` ``, `;`, `&`, `|`, `\`, `"` and
 * the typographic quotes PowerShell treats as quotes (U+2018..U+201F). The
 * server's `shell_arg` emits none of them outside a single-quoted span, and
 * with them gone a single-quoted span means the same thing in bash, zsh,
 * fish and PowerShell: nothing inside it expands.
 *
 * Refused outside single-quoted spans: `$`, `(` and `)`, which open command
 * and process substitution (`$(cmd)`, `<(cmd)`, `>(cmd)`; #4225). The server
 * quotes all three, so a quoted gap id such as `'len(x)>0'` still passes.
 * An unterminated span is refused.
 */
export function hasUnsafeShellMetacharacter(command: string): boolean {
  if (/[\r\n\0`;&|\\"\u2018-\u201f]/.test(command)) {
    return true;
  }
  const spans = command.split("'");
  return spans.length % 2 === 0 || spans.some((span, index) => index % 2 === 0 && /[$()]/.test(span));
}

/**
 * Quote one argument exactly as the server's `shell_arg`
 * (`crates/ripr/src/agent/loop_commands.rs`): bare when every character is in
 * `[A-Za-z0-9._/:-]`, otherwise single-quoted with `'` written as `'\''`.
 */
export function serverShellArg(value: string): string {
  return /^[A-Za-z0-9._/:-]+$/.test(value) ? value : `'${value.replace(/'/g, "'\\''")}'`;
}

/**
 * Whether the tail of a `> file` redirect writes exactly `artifact`.
 *
 * The server renders the tail with its `shell_arg` quoting: bare when every
 * character is in `[A-Za-z0-9._/:-]`, otherwise single-quoted. Since #3938 it
 * anchors the target at the resolved `--root` with forward slashes, so a
 * `--root .` command names `artifact` under the server's cwd. The legacy
 * relative form is still accepted. Any other root, file, `..` segment or
 * trailing token is rejected. Callers check the text before the redirect
 * with `hasUnsafeShellMetacharacter`; this function owns the tail.
 */
export function redirectTargetMatches(
  tail: string,
  artifact: string,
  roots: readonly string[]
): boolean {
  const quoted = /^'([^']*)'$/.exec(tail);
  const target = quoted ? quoted[1] : /^[A-Za-z0-9._/:-]+$/.test(tail) ? tail : undefined;
  // The quoted span may hold a workspace path's `&`, `;` or `$`: that text
  // must equal a local root below, so the server cannot choose it (#4225).
  // Line breaks, backslashes and typographic quotes are still refused, since
  // they end or escape the span in some shell (fish reads `\'` inside single
  // quotes as an escape; PowerShell closes a span on U+2018..U+201B).
  if (target === undefined || /[\r\n\0\\\u2018-\u201f]/.test(target)) {
    return false;
  }
  if (target === artifact) {
    return true;
  }
  if (!path.isAbsolute(target) || target.split('/').includes('..')) {
    return false;
  }
  const expected = normalizePath(target);
  return roots.some((root) => normalizePath(path.join(root, artifact)) === expected);
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
