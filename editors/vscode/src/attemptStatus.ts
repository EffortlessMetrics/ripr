/**
 * Shared repair-attempt status adapter for the VS Code client (#4643,
 * RIPR-SPEC-0220).
 *
 * The semantic authority is the CLI DTO emitted by
 * `ripr agent status --attempt <id> --json` (RIPR-SPEC-0217, #4798); this
 * module is the *only* place the editor parses that document. The inventory
 * document (`ripr agent status --json`, same spec family) supplies the
 * attempt-id rows used for deterministic active-attempt resolution.
 *
 * Everything here is pure: no VS Code API, no `this`, no clocks, no
 * filesystem. Presentation can never strengthen the CLI's typed state: each
 * `status_class` maps to exactly one tone, `corrupt_or_unavailable` and
 * `failed` are the only error tones, and a document that fails validation is
 * `undefined` (unavailable) rather than something friendlier.
 */

/**
 * The resume vocabulary RIPR-SPEC-0217 pins. The editor layer must not add,
 * rename, split, or weaken members: a document whose `status_class` is not
 * one of these ten strings fails closed instead of being projected.
 */
export const ATTEMPT_STATUS_CLASSES = [
  'awaiting_edit',
  'prepared',
  'finished_current',
  'finished_historical',
  'stale',
  'incomparable',
  'failed',
  'limited',
  'corrupt_or_unavailable',
  'legacy_compatibility_only',
] as const;

export type AttemptStatusClass = (typeof ATTEMPT_STATUS_CLASSES)[number];

const ATTEMPT_STATUS_CLASS_SET: ReadonlySet<string> = new Set(ATTEMPT_STATUS_CLASSES);

export const ATTEMPT_CURRENTNESSES = ['current', 'historical', 'unknown'] as const;

export type AttemptCurrentness = (typeof ATTEMPT_CURRENTNESSES)[number];

const ATTEMPT_CURRENTNESS_SET: ReadonlySet<string> = new Set(ATTEMPT_CURRENTNESSES);

export interface AttemptNextAction {
  step: string;
  artifact?: string;
  reason?: string;
  command?: string;
}

/** The typed projection of one `agent_attempt_status` document. */
export interface AgentAttemptStatus {
  attemptId: string;
  seamId?: string;
  /** Manifest operational state; `null` when the manifest could not be validated. */
  state: string | null;
  statusClass: AttemptStatusClass;
  /** HEAD relation as reported by the attempt authority; `null` when unreadable. */
  headCurrent: boolean | null;
  currentness: AttemptCurrentness;
  unreadableReason: string | null;
  nextAction: AttemptNextAction | null;
  claimBoundary: string[];
  limitations: string[];
  nonClaims: string[];
}

/** The schema version of the `agent_attempt_status` envelope the adapter binds. */
export const AGENT_ATTEMPT_STATUS_SCHEMA_VERSION = '0.1';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function nonEmptyString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() !== '' ? value : undefined;
}

function nullableString(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

function stringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string => typeof item === 'string' && item.trim() !== '');
}

function parseNextAction(value: unknown): AttemptNextAction | null {
  if (value === null || value === undefined) {
    return null;
  }
  if (!isRecord(value)) {
    return null;
  }
  const step = nonEmptyString(value['step']);
  if (!step) {
    return null;
  }
  const action: AttemptNextAction = { step };
  const artifact = nonEmptyString(value['artifact']);
  const reason = nonEmptyString(value['reason']);
  const command = nonEmptyString(value['command']);
  if (artifact) {
    action.artifact = artifact;
  }
  if (reason) {
    action.reason = reason;
  }
  if (command) {
    action.command = command;
  }
  return action;
}

/**
 * Parse one `agent_attempt_status` document. Returns `undefined` when the
 * envelope is not the pinned shape — wrong kind/schema version, missing
 * attempt id, or a `status_class`/`currentness` outside the pinned
 * vocabulary. Fail-closed by construction: an unrecognised document is
 * "unavailable", never a guessed or downgraded-but-still-shown state.
 */
export function parseAgentAttemptStatus(raw: unknown): AgentAttemptStatus | undefined {
  if (!isRecord(raw)) {
    return undefined;
  }
  if (raw['schema_version'] !== AGENT_ATTEMPT_STATUS_SCHEMA_VERSION) {
    return undefined;
  }
  if (raw['kind'] !== 'agent_attempt_status') {
    return undefined;
  }
  const attempt = raw['attempt'];
  if (!isRecord(attempt)) {
    return undefined;
  }
  const attemptId = nonEmptyString(attempt['attempt_id']);
  if (!attemptId) {
    return undefined;
  }
  const statusClass = attempt['status_class'];
  if (typeof statusClass !== 'string' || !ATTEMPT_STATUS_CLASS_SET.has(statusClass)) {
    return undefined;
  }
  const currentness = attempt['currentness'];
  if (typeof currentness !== 'string' || !ATTEMPT_CURRENTNESS_SET.has(currentness)) {
    return undefined;
  }
  const headCurrent = attempt['head_current'];
  return {
    attemptId,
    seamId: nonEmptyString(attempt['seam_id']),
    state: nullableString(attempt['state']),
    statusClass: statusClass as AttemptStatusClass,
    headCurrent: typeof headCurrent === 'boolean' ? headCurrent : null,
    currentness: currentness as AttemptCurrentness,
    unreadableReason: nullableString(attempt['unreadable_reason']),
    nextAction: parseNextAction(raw['next_action']),
    claimBoundary: stringArray(raw['claim_boundary']),
    limitations: stringArray(raw['limitations']),
    nonClaims: stringArray(raw['non_claims']),
  };
}

/**
 * One row of the inventory document (`ripr agent status --json`), reduced to
 * the fields the editor needs for exact active-attempt resolution.
 */
export interface AttemptInventoryRow {
  attemptId: string;
  seamId?: string;
  state: string | null;
  disposition?: string;
  command?: string | null;
}

/**
 * Parse the inventory document's `repair_attempts` rows. Returns `undefined`
 * when the document is not the pinned inventory shape so the caller fails
 * closed instead of presenting an empty store as "no attempts".
 */
export function parseAttemptInventory(raw: unknown): AttemptInventoryRow[] | undefined {
  if (!isRecord(raw)) {
    return undefined;
  }
  if (raw['schema_version'] !== AGENT_ATTEMPT_STATUS_SCHEMA_VERSION) {
    return undefined;
  }
  if (nonEmptyString(raw['kind'])) {
    // A selected-attempt document (`kind: agent_attempt_status`) is not an
    // inventory; parsing it as one would manufacture rows that do not exist.
    return undefined;
  }
  const rows = raw['repair_attempts'];
  if (!Array.isArray(rows)) {
    return undefined;
  }
  const parsed: AttemptInventoryRow[] = [];
  for (const row of rows) {
    if (!isRecord(row)) {
      return undefined;
    }
    const attemptId = nonEmptyString(row['attempt_id']);
    if (!attemptId) {
      return undefined;
    }
    parsed.push({
      attemptId,
      seamId: nonEmptyString(row['seam_id']),
      state: nullableString(row['state']),
      disposition: nonEmptyString(row['disposition']),
      command: nonEmptyString(row['command']) ?? null,
    });
  }
  return parsed;
}

/**
 * The deterministic active-attempt selection law (#4643):
 *
 * - an explicit selection wins, even when the id is not in the inventory —
 *   the CLI then reports the typed `corrupt_or_unavailable` result for it;
 * - otherwise a remembered selection is honored only while it is still one
 *   of the inventory rows; a stale remembered id is discarded, never
 *   resurrected and never replaced by a guess;
 * - exactly one row selects that row;
 * - several rows with no valid selection require an explicit pick.
 *
 * Newest, first folder, most recently modified, and same-seam heuristics are
 * deliberately absent.
 */
export type ActiveAttemptResolution =
  | { kind: 'no_attempts' }
  | { kind: 'selected'; attemptId: string; via: 'explicit' | 'remembered' | 'single' }
  | { kind: 'selection_required'; attemptIds: string[] }
  | { kind: 'unknown_attempt'; attemptId: string };

export function resolveActiveAttempt(
  rows: readonly AttemptInventoryRow[],
  options?: { explicitAttemptId?: string; rememberedAttemptId?: string }
): ActiveAttemptResolution {
  const byId = new Map(rows.map((row) => [row.attemptId, row]));
  const explicit = options?.explicitAttemptId;
  if (explicit && explicit.trim() !== '') {
    return byId.has(explicit)
      ? { kind: 'selected', attemptId: explicit, via: 'explicit' }
      : { kind: 'unknown_attempt', attemptId: explicit };
  }
  const remembered = options?.rememberedAttemptId;
  if (remembered && byId.has(remembered)) {
    return { kind: 'selected', attemptId: remembered, via: 'remembered' };
  }
  if (rows.length === 0) {
    return { kind: 'no_attempts' };
  }
  if (rows.length === 1) {
    const only = rows[0];
    if (only) {
      return { kind: 'selected', attemptId: only.attemptId, via: 'single' };
    }
  }
  return { kind: 'selection_required', attemptIds: rows.map((row) => row.attemptId) };
}

export type AttemptStatusTone = 'pass' | 'info' | 'warning' | 'error';

export interface AttemptStatusPresentation {
  /** Human label for the class; `finished_historical` stays "historical". */
  label: string;
  tone: AttemptStatusTone;
  /** Status-bar text including the ripr prefix. */
  statusBarText: string;
  /** Tooltip / notification lines derived only from DTO fields. */
  summaryLines: string[];
}

/**
 * Per-class presentation. This mapping is the editor's whole claim about the
 * state: it may soften nothing and strengthen nothing. In particular
 * `finished_historical` is a warning (retained, readable, not current proof)
 * and `corrupt_or_unavailable` is an error (the authority refused the
 * artifact). Classes the CLI does not emit are not representable here.
 */
export const ATTEMPT_STATUS_PRESENTATIONS: Record<
  AttemptStatusClass,
  { label: string; tone: AttemptStatusTone }
> = {
  awaiting_edit: { label: 'awaiting edit', tone: 'info' },
  prepared: { label: 'prepared', tone: 'info' },
  finished_current: { label: 'finished', tone: 'pass' },
  finished_historical: { label: 'finished (historical)', tone: 'warning' },
  stale: { label: 'stale', tone: 'warning' },
  incomparable: { label: 'incomparable', tone: 'warning' },
  limited: { label: 'limited', tone: 'warning' },
  failed: { label: 'failed', tone: 'error' },
  corrupt_or_unavailable: { label: 'unavailable', tone: 'error' },
  legacy_compatibility_only: { label: 'legacy compatibility', tone: 'warning' },
};

const ATTEMPT_STATUS_CODICONS: Record<AttemptStatusClass, string> = {
  awaiting_edit: '$(edit)',
  prepared: '$(sync)',
  finished_current: '$(pass)',
  finished_historical: '$(history)',
  stale: '$(warning)',
  incomparable: '$(warning)',
  limited: '$(warning)',
  failed: '$(error)',
  corrupt_or_unavailable: '$(error)',
  legacy_compatibility_only: '$(warning)',
};

/**
 * Project one typed status into editor presentation. Every line comes from a
 * DTO field; when the DTO omits a fact the line is omitted, not invented.
 */
export function presentAttemptStatus(status: AgentAttemptStatus): AttemptStatusPresentation {
  const presentation = ATTEMPT_STATUS_PRESENTATIONS[status.statusClass];
  const lines: string[] = [
    `attempt: ${status.attemptId}`,
    `status: ${status.statusClass}`,
  ];
  if (status.seamId) {
    lines.push(`seam: ${status.seamId}`);
  }
  if (status.state !== null) {
    lines.push(`state: ${status.state}`);
  }
  lines.push(`currentness: ${status.currentness}`);
  if (status.headCurrent === false) {
    lines.push('head: moved since this attempt recorded it');
  }
  if (status.unreadableReason !== null) {
    lines.push(`unreadable: ${status.unreadableReason}`);
  }
  if (status.nextAction) {
    if (status.nextAction.command) {
      lines.push(`next: ${status.nextAction.command}`);
    } else if (status.nextAction.reason) {
      lines.push(`next: ${status.nextAction.reason}`);
    } else {
      lines.push(`next: ${status.nextAction.step}`);
    }
  }
  const firstLimitation = status.limitations[0];
  if (firstLimitation) {
    lines.push(`limitation: ${firstLimitation}`);
  }
  return {
    label: presentation.label,
    tone: presentation.tone,
    statusBarText: `${ATTEMPT_STATUS_CODICONS[status.statusClass]} ripr: attempt ${presentation.label}`,
    summaryLines: lines,
  };
}
