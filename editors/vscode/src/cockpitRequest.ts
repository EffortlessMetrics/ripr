/**
 * Cockpit LSP request outcomes extracted from client.ts (#5099).
 *
 * `fetchTopRepairPacket` and `fetchReceiptStatus` used to return `null` both
 * when no language client exists and when a live request fails. Callers then
 * reported "server not responding" in windows where no server was ever started.
 *
 * These helpers keep that split in one place: no client is not a hung request.
 */

export type CockpitRequestResult<T> =
  | { kind: 'no_client' }
  | { kind: 'unavailable' }
  | { kind: 'ok'; value: T };

export type CockpitRequestAbsence = Extract<
  CockpitRequestResult<unknown>,
  { kind: 'no_client' | 'unavailable' }
>;

export const NO_RUNNING_SERVER_MESSAGE =
  'No ripr server is running in this window — check the ripr status item or run ripr: Show Status.';

const HUNG_SERVER_WORDING = /did not respond|not responding/;

export function messageClaimsHungServer(message: string): boolean {
  return HUNG_SERVER_WORDING.test(message);
}

export function cockpitUnavailableMessage(
  result: CockpitRequestAbsence,
  unavailableMessage: string
): string {
  return result.kind === 'no_client' ? NO_RUNNING_SERVER_MESSAGE : unavailableMessage;
}

export function interpretCockpitObjectResponse(
  response: unknown
): Extract<CockpitRequestResult<Record<string, unknown>>, { kind: 'ok' | 'unavailable' }> {
  if (response === null || response === undefined) {
    return { kind: 'unavailable' };
  }
  if (typeof response === 'object') {
    return { kind: 'ok', value: response as Record<string, unknown> };
  }
  return { kind: 'unavailable' };
}
