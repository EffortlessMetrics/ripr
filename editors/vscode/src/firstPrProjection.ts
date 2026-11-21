/**
 * First-PR packet projection helpers extracted from client.ts (#2543).
 *
 * Pure functions and the data model for the `ripr` first-PR start-here packet:
 * validation against the schema, workspace/path/command safety, status
 * predicates, and the summary / repair / regeneration / status-line text
 * builders. None of these depend on `RiprClientController` instance state.
 *
 * The controller instance methods (`openFirstPrPacket`, `copyFirstPr*`,
 * `copyFirstPrText`) stay in `client.ts`: they read `this.runtime`,
 * `this.output`, and the language client.
 */
import * as path from 'path';
import * as vscode from 'vscode';
import {
  stringField,
  boundedStringField,
  objectField,
  arrayLength,
  stringValues,
  rootMatchesWorkspace,
  hasUnsafeShellMetacharacter
} from './packetJson';

const FIRST_PR_STATIC_EVIDENCE_BOUNDARY = 'static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.';

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
  currentEvidenceStrength?: string;
  missingDiscriminator?: string;
  focusedProofIntent?: string;
  staticEvidenceBoundary?: string;
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

export function firstPrPacketStoredInTarget(packet: RiprFirstPrPacketStatus): boolean {
  return packet.state !== 'missing'
    && packet.state !== 'noWorkspace'
    && packet.relativePath.startsWith('target/ripr/');
}

export function firstPrPacketCanBecomeStale(state: RiprFirstPrPacketState): boolean {
  return state === 'found'
    || state === 'topRepairableGap'
    || state === 'noAction'
    || state === 'blocked';
}

export function firstPrPacketAllowsSummary(state: RiprFirstPrPacketState): boolean {
  return state === 'found'
    || state === 'topRepairableGap'
    || state === 'noAction';
}

export function firstPrPacketAllowsOpen(state: RiprFirstPrPacketState): boolean {
  return state === 'found'
    || state === 'topRepairableGap'
    || state === 'noAction';
}

export function firstPrHasRepairPacket(packet: RiprFirstPrPacketStatus): boolean {
  return Boolean(
    (packet.canonicalGapId ?? packet.gapId) &&
    packet.repairRoute &&
    (packet.relatedTest || packet.repairTarget) &&
    packet.verifyCommand &&
    packet.receiptCommand
  );
}

export function firstPrSuppressedMessage(packet: RiprFirstPrPacketStatus): string {
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

export function firstPrSummaryPacket(packet: RiprFirstPrPacketStatus): string {
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
  if (packet.currentEvidenceStrength) {
    lines.push(`Current evidence strength: ${packet.currentEvidenceStrength}`);
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
  lines.push(`- ${packet.staticEvidenceBoundary ?? FIRST_PR_STATIC_EVIDENCE_BOUNDARY}`);
  lines.push('- Does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.');
  lines.push('- Does not edit source, generate tests, publish PR comments, or run providers.');
  return lines.join('\n');
}

export function firstPrRepairPacket(packet: RiprFirstPrPacketStatus): string {
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
  if (packet.currentEvidenceStrength) {
    lines.push(`Current evidence strength: ${packet.currentEvidenceStrength}`);
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
  lines.push(`- ${packet.staticEvidenceBoundary ?? FIRST_PR_STATIC_EVIDENCE_BOUNDARY}`);
  lines.push('- Does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.');
  lines.push('- Does not edit source, generate tests, publish PR comments, or run providers.');
  return lines.join('\n');
}

export function firstPrRegenerationGuidance(packet: RiprFirstPrPacketStatus): string {
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

export function diagnosticMatchesFirstPrPacket(
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

export function firstPrTopRepairableGapLines(packet: RiprFirstPrPacketStatus): string[] {
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
  if (packet.currentEvidenceStrength) {
    lines.push(`Current evidence strength: ${packet.currentEvidenceStrength}`);
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
  lines.push(`Boundary: ${packet.staticEvidenceBoundary ?? FIRST_PR_STATIC_EVIDENCE_BOUNDARY}`);
  lines.push('First PR packet does not prove runtime adequacy, mutation coverage, policy eligibility, or gate status.');
  return lines;
}

export function firstPrBlockedPacketLines(packet: RiprFirstPrPacketStatus): string[] {
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

export function firstPrCommandIsSafe(command: string): boolean {
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

export function firstPrPathIsWorkspaceLocal(value: string): boolean {
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

export function validateFirstPrPacket(
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
    currentEvidenceStrength: stringField(selected, 'current_evidence_strength'),
    missingDiscriminator: stringField(selected, 'missing_discriminator'),
    focusedProofIntent: stringField(selected, 'focused_proof_intent'),
    staticEvidenceBoundary: stringField(selected, 'static_evidence_boundary'),
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
