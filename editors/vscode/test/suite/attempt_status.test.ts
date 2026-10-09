//
// Shared repair-attempt status adapter parity suite (#4643, RIPR-SPEC-0220).
//
// The fixtures under test-fixtures/attempt-status/ hold documents in the exact
// shape `ripr agent status --json` / `--attempt <id> --json` emit (RIPR-SPEC-
// 0217, #4798; contract: docs/OUTPUT_SCHEMA.md). These tests are the parity
// enforcement named by #4643's acceptance criteria: every fixture flows through
// src/attemptStatus.ts, so removing or bypassing the adapter turns this suite
// red (compile failure or parse rejection) instead of letting the editor
// drift onto its own state vocabulary.
//

import * as assert from 'assert';
import { promises as fs } from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import {
  ATTEMPT_STATUS_CLASSES,
  parseAgentAttemptStatus,
  parseAttemptInventory,
  presentAttemptStatus,
  resolveActiveAttempt,
  type AttemptStatusTone
} from '../../src/attemptStatus';
import { RiprClientController, RiprClientRuntime } from '../../src/client';
import { RiprConfig } from '../../src/config';
import { compatibleLspEvidence } from './testCompatibility';

const FIXTURE_DIR = path.resolve(__dirname, '../../../test-fixtures/attempt-status');

async function loadFixture(name: string): Promise<unknown> {
  const raw = await fs.readFile(path.join(FIXTURE_DIR, name), 'utf8');
  return JSON.parse(raw) as unknown;
}

/**
 * The tone each class must carry. This is the acceptance-criteria enforcement
 * that the editor never strengthens the CLI's typed state: historical,
 * limited, legacy, stale and incomparable stay warnings; only failed and
 * corrupt_or_unavailable are errors; only finished_current passes.
 */
const EXPECTED_TONES: Record<string, AttemptStatusTone> = {
  awaiting_edit: 'info',
  prepared: 'info',
  finished_current: 'pass',
  finished_historical: 'warning',
  stale: 'warning',
  incomparable: 'warning',
  failed: 'error',
  limited: 'warning',
  corrupt_or_unavailable: 'error',
  legacy_compatibility_only: 'warning'
};

suite('Agent attempt status adapter parity (#4643)', () => {
  test('every pinned status class has a fixture that parses to that exact class', async () => {
    assert.strictEqual(ATTEMPT_STATUS_CLASSES.length, 10, 'RIPR-SPEC-0217 pins ten classes');
    for (const statusClass of ATTEMPT_STATUS_CLASSES) {
      const parsed = parseAgentAttemptStatus(await loadFixture(`status-${statusClass}.json`));
      assert.ok(parsed, `fixture for ${statusClass} must parse`);
      assert.strictEqual(parsed.statusClass, statusClass);
      assert.strictEqual(typeof parsed.attemptId, 'string');
      assert.ok(
        parsed.currentness === 'current' || parsed.currentness === 'historical' || parsed.currentness === 'unknown',
        `currentness must stay in the pinned vocabulary for ${statusClass}`
      );
      const presentation = presentAttemptStatus(parsed);
      assert.strictEqual(
        presentation.tone,
        EXPECTED_TONES[statusClass],
        `${statusClass} must present as tone ${EXPECTED_TONES[statusClass]}, never stronger`
      );
      assert.ok(
        presentation.statusBarText.includes(presentation.label),
        `status bar text must name the class label for ${statusClass}`
      );
    }
  });

  test('presentation derives only from DTO fields and names moved HEAD and next action', async () => {
    const historical = parseAgentAttemptStatus(await loadFixture('status-finished_historical.json'));
    assert.ok(historical);
    const lines = presentAttemptStatus(historical).summaryLines;
    assert.ok(lines.some((line) => line.includes('currentness: historical')));
    assert.ok(lines.some((line) => line.includes('head: moved')));

    const awaiting = parseAgentAttemptStatus(await loadFixture('status-awaiting_edit.json'));
    assert.ok(awaiting);
    const awaitingLines = presentAttemptStatus(awaiting).summaryLines;
    assert.ok(awaiting.nextAction?.command, 'awaiting_edit fixture must carry the recorded after command');
    assert.ok(awaitingLines.some((line) => line.startsWith('next: ') && line.includes('--phase after')));
  });

  test('an unrecognized document fails closed instead of projecting a state', async () => {
    const valid = await loadFixture('status-awaiting_edit.json') as Record<string, unknown>;

    // A class the CLI does not emit — the classic strengthening attempt.
    const strengthened = {
      ...valid,
      attempt: { ...(valid['attempt'] as Record<string, unknown>), status_class: 'passed' }
    };
    assert.strictEqual(parseAgentAttemptStatus(strengthened), undefined);

    assert.strictEqual(parseAgentAttemptStatus({ ...valid, kind: 'agent_status' }), undefined);
    assert.strictEqual(parseAgentAttemptStatus({ ...valid, schema_version: '0.2' }), undefined);
    assert.strictEqual(parseAgentAttemptStatus('not json at all'), undefined);
    assert.strictEqual(parseAgentAttemptStatus(null), undefined);
    assert.strictEqual(parseAgentAttemptStatus([]), undefined);

    // A selected-attempt document must never parse as an inventory: treating
    // it as rows would manufacture attempts that do not exist.
    assert.strictEqual(parseAttemptInventory(valid), undefined);
  });

  test('inventory fixture rows keep store order and reject foreign shapes', async () => {
    const rows = parseAttemptInventory(await loadFixture('inventory.json'));
    assert.ok(rows);
    assert.strictEqual(rows.length, 3);
    const ids = rows.map((row) => row.attemptId);
    // Adversarial fixture order (a finished attempt listed before two awaiting
    // ones): the adapter preserves document order so resolution can never read
    // "first row" or "newest-looking" into it.
    assert.ok(ids[0]?.includes('0ccc'), `first row must stay the document's first row: ${ids.join(', ')}`);
    assert.strictEqual(parseAttemptInventory({ schema_version: '0.1', repair_attempts: [{ seam_id: 'x' }] }), undefined);
    assert.strictEqual(parseAttemptInventory({ repair_attempts: [] }), undefined);
  });
});

suite('Active attempt resolution law (#4643)', () => {
  const row = (attemptId: string): { attemptId: string; state: string | null } => ({
    attemptId,
    state: 'awaiting_edit'
  });
  const ROWS = [row('repair-attempt-0ccc3333cccc3333cccc3333cccc3333cccc3333'), row('repair-attempt-0aaa1111aaaa1111aaaa1111aaaa1111aaaa1111')];

  test('zero attempts, one attempt, and several attempts are distinct results', () => {
    assert.deepStrictEqual(resolveActiveAttempt([]), { kind: 'no_attempts' });
    assert.deepStrictEqual(resolveActiveAttempt([row('a1')]), { kind: 'selected', attemptId: 'a1', via: 'single' });
    assert.deepStrictEqual(resolveActiveAttempt(ROWS), {
      kind: 'selection_required',
      attemptIds: ROWS.map((r) => r.attemptId)
    });
  });

  test('a remembered selection is honored only while it is still a real row', () => {
    assert.deepStrictEqual(
      resolveActiveAttempt(ROWS, { rememberedAttemptId: 'repair-attempt-0aaa1111aaaa1111aaaa1111aaaa1111aaaa1111' }),
      { kind: 'selected', attemptId: 'repair-attempt-0aaa1111aaaa1111aaaa1111aaaa1111aaaa1111', via: 'remembered' }
    );
    // Stale remembered id: dropped, and with several rows the result is an
    // explicit pick — never a silent fallback to another attempt.
    assert.deepStrictEqual(
      resolveActiveAttempt(ROWS, { rememberedAttemptId: 'repair-attempt-deadbeefdeadbeefdeadbeefdeadbeefdeadbeef' }),
      { kind: 'selection_required', attemptIds: ROWS.map((r) => r.attemptId) }
    );
    // Stale remembered id with exactly one row selects that single row.
    assert.deepStrictEqual(
      resolveActiveAttempt([row('only-one')], { rememberedAttemptId: 'repair-attempt-deadbeefdeadbeefdeadbeefdeadbeefdeadbeef' }),
      { kind: 'selected', attemptId: 'only-one', via: 'single' }
    );
  });

  test('an explicit selection wins, and an unknown explicit id stays an exact query', () => {
    assert.deepStrictEqual(
      resolveActiveAttempt(ROWS, { explicitAttemptId: 'repair-attempt-0ccc3333cccc3333cccc3333cccc3333cccc3333' }),
      { kind: 'selected', attemptId: 'repair-attempt-0ccc3333cccc3333cccc3333cccc3333cccc3333', via: 'explicit' }
    );
    // The CLI reports the typed corrupt_or_unavailable document for this id;
    // the editor must not substitute another attempt.
    assert.deepStrictEqual(
      resolveActiveAttempt(ROWS, { explicitAttemptId: 'repair-attempt-deadbeefdeadbeefdeadbeefdeadbeefdeadbeef' }),
      { kind: 'unknown_attempt', attemptId: 'repair-attempt-deadbeefdeadbeefdeadbeefdeadbeefdeadbeef' }
    );
  });
});

suite('Show Repair Attempt Status command (#4643)', () => {
  const enabledConfig: RiprConfig = {
    enabled: true,
    serverPath: '/sentinel/trusted/ripr',
    serverArgs: ['lsp', '--stdio'],
    autoDownload: false,
    serverVersion: 'sentinel-trusted-version',
    downloadBaseUrl: 'https://sentinel.invalid/ripr',
    checkMode: 'draft',
    baseRef: 'origin/main',
    includeUnchangedTests: true,
    seamDiagnostics: true,
    diagnosticProfile: 'actionable',
    traceServer: 'off'
  };

  interface AttemptBarRecording {
    text: string;
    tooltip: string;
    shown: number;
    hidden: number;
  }

  interface RuntimeHarness {
    runtime: RiprClientRuntime;
    bar: AttemptBarRecording;
    runRiprCalls: string[][];
    quickPickCalls: number;
    infoMessages: string[];
    warningMessages: string[];
    memento: Map<string, unknown>;
  }

  function harness(options: {
    trusted?: boolean;
    inventory: unknown;
    selected: unknown;
    /** Per-attempt status payloads so a `--attempt <id>` read returns the document for that exact id. */
    selectedByAttempt?: Record<string, unknown>;
    remembered?: Record<string, unknown>;
    pickIndex?: number;
    /** The workspace root this harness reports; defaults to /workspace. */
    root?: string;
    onRunRipr?: () => Promise<void>;
  }): RuntimeHarness {
    const runRiprCalls: string[][] = [];
    const infoMessages: string[] = [];
    const warningMessages: string[] = [];
    const memento = new Map<string, unknown>(Object.entries(options.remembered ?? {}));
    const bar: AttemptBarRecording = { text: '', tooltip: '', shown: 0, hidden: 0 };
    const counters = { quickPickCalls: 0 };

    const runtime: RiprClientRuntime = {
      getConfig: () => enabledConfig,
      workspaceRootState: () => {
        const root = options.root ?? '/workspace';
        return { kind: 'singleRoot', root, roots: [root] };
      },
      workspaceFolders: () => [],
      showQuickPick: <T extends vscode.QuickPickItem>(items: T[]): Thenable<T | undefined> => {
        counters.quickPickCalls += 1;
        const index = options.pickIndex;
        return Promise.resolve(index === undefined ? undefined : items[index]);
      },
      resolveServer: async () => ({
        command: '/sentinel/trusted/ripr',
        source: 'configured',
        detail: 'sentinel server',
        installationState: 'unmanaged',
        compatibilityResult: compatibleLspEvidence
      }),
      createLanguageClient: () => ({
        onNotification: () => ({ dispose: () => undefined }),
        sendRequest: async () => ({}),
        setTrace: () => undefined,
        start: async () => undefined,
        stop: async () => undefined
      }),
      createFileSystemWatcher: () => ({} as unknown as vscode.FileSystemWatcher),
      readFile: async () => undefined,
      runRipr: async (_command: string, args: string[]) => {
        runRiprCalls.push(args);
        if (options.onRunRipr) {
          await options.onRunRipr();
        }
        const attemptFlag = args.indexOf('--attempt');
        const payload = attemptFlag >= 0
          ? options.selectedByAttempt?.[args[attemptFlag + 1] ?? ''] ?? options.selected
          : options.inventory;
        if (typeof payload === 'string') {
          return payload;
        }
        return JSON.stringify(payload);
      },
      writeClipboard: async () => undefined,
      isWorkspaceTrusted: () => options.trusted ?? true,
      showInformationMessage: (message: string) => {
        infoMessages.push(message);
        return Promise.resolve(undefined);
      },
      showWarningMessage: (message: string) => {
        warningMessages.push(message);
        return Promise.resolve(undefined);
      },
      showErrorMessage: async () => undefined
    };

    return {
      runtime,
      bar,
      runRiprCalls,
      infoMessages,
      warningMessages,
      memento,
      get quickPickCalls() {
        return counters.quickPickCalls;
      }
    };
  }

  function controllerFor(harness: RuntimeHarness): RiprClientController {
    const output = { appendLine: () => undefined, show: () => undefined } as unknown as vscode.LogOutputChannel;
    const context = {
      workspaceState: {
        get: (key: string) => harness.memento.get(key),
        update: (key: string, value: unknown) => {
          harness.memento.set(key, value);
          return Promise.resolve();
        }
      }
    } as unknown as vscode.ExtensionContext;
    const recording = harness.bar;
    const barItem = {
      get text() {
        return recording.text;
      },
      set text(value: string) {
        recording.text = value;
      },
      get tooltip() {
        return recording.tooltip;
      },
      set tooltip(value: string) {
        recording.tooltip = value;
      },
      command: undefined,
      backgroundColor: undefined,
      color: undefined,
      show: () => {
        recording.shown += 1;
      },
      hide: () => {
        recording.hidden += 1;
      }
    };
    return new RiprClientController(
      context,
      output,
      harness.runtime,
      undefined,
      barItem as unknown as vscode.StatusBarItem
    );
  }

  test('an untrusted workspace never reads repair authority', async () => {
    const inventory = await loadFixture('inventory.json');
    const selected = await loadFixture('status-awaiting_edit.json');
    const h = harness({ trusted: false, inventory, selected });
    const controller = controllerFor(h);

    await controller.showAttemptStatus();

    assert.deepStrictEqual(h.runRiprCalls, [], 'no CLI read may happen in an untrusted workspace');
    assert.ok(
      h.infoMessages.some((message) => message.includes('untrusted')),
      `expected an untrusted-workspace message, got: ${h.infoMessages.join(' | ')}`
    );
  });

  test('one current attempt resolves directly and renders the typed class without strengthening', async () => {
    const selected = await loadFixture('status-awaiting_edit.json');
    const singleId = 'repair-attempt-0123456789abcdef01234567';
    const inventory = {
      schema_version: '0.1',
      repair_attempts: [
        { attempt_id: singleId, seam_id: '67fc764ba37d77bd', state: 'awaiting_edit', disposition: 'resumable', command: null, receipt: null, last_after_refusal: null }
      ]
    };
    const h = harness({ inventory, selected });
    const controller = controllerFor(h);

    await controller.showAttemptStatus();

    assert.strictEqual(h.runRiprCalls.length, 2, 'inventory read then selected read');
    assert.deepStrictEqual(h.runRiprCalls[0], ['agent', 'status', '--root', '/workspace', '--json']);
    assert.deepStrictEqual(h.runRiprCalls[1], ['agent', 'status', '--root', '/workspace', '--attempt', singleId, '--json']);
    assert.strictEqual(h.bar.text, '$(edit) ripr: attempt awaiting edit');
    assert.ok(h.bar.tooltip.includes('awaiting_edit'));
    assert.ok(h.infoMessages.some((message) => message.includes('awaiting edit')));
  });

  test('several current attempts require an explicit pick and the pick is remembered per root', async () => {
    const inventory = await loadFixture('inventory.json');
    const selected = await loadFixture('status-stale.json');
    const pickedId = 'repair-attempt-0bbb2222bbbb2222bbbb2222bbbb2222bbbb2222';
    // The document for the picked attempt must carry the picked identity, or
    // the identity-binding refusal (correctly) rejects the render. The id
    // lives under `attempt`, matching the RIPR-SPEC-0217 envelope.
    const staleDoc = selected as { attempt: Record<string, unknown> } & Record<string, unknown>;
    const pickedDoc = { ...staleDoc, attempt: { ...staleDoc.attempt, attempt_id: pickedId } };
    const h = harness({ inventory, selected, selectedByAttempt: { [pickedId]: pickedDoc }, pickIndex: 2 });
    const controller = controllerFor(h);

    await controller.showAttemptStatus();

    assert.deepStrictEqual(h.runRiprCalls[1], ['agent', 'status', '--root', '/workspace', '--attempt', pickedId, '--json']);
    const stored = h.memento.get('ripr.activeAttemptSelection.v1') as Record<string, string>;
    assert.strictEqual(stored['/workspace'], pickedId, 'the explicit pick must be remembered for the root');

    // Second run: the remembered selection resolves without a pick.
    const h2 = harness({
      inventory,
      selected,
      selectedByAttempt: { [pickedId]: pickedDoc },
      remembered: { 'ripr.activeAttemptSelection.v1': { '/workspace': pickedId } }
    });
    const controller2 = controllerFor(h2);
    await controller2.showAttemptStatus();
    assert.strictEqual(h2.quickPickCalls, 0, 'no quick pick expected when the remembered selection is valid');
    assert.deepStrictEqual(h2.runRiprCalls[1], ['agent', 'status', '--root', '/workspace', '--attempt', pickedId, '--json']);
    assert.strictEqual(h2.bar.text, '$(warning) ripr: attempt stale');
  });

  test('a remembered selection for another root is never honored', async () => {
    const inventory = await loadFixture('inventory.json');
    const selected = await loadFixture('status-awaiting_edit.json');
    const otherRootPick = 'repair-attempt-0bbb2222bbbb2222bbbb2222bbbb2222bbbb2222';
    const h = harness({
      inventory,
      selected,
      root: '/ws2',
      pickIndex: undefined,
      remembered: { 'ripr.activeAttemptSelection.v1': { '/workspace': otherRootPick } }
    });
    const controller = controllerFor(h);

    await controller.showAttemptStatus();

    assert.strictEqual(
      h.quickPickCalls,
      1,
      'a selection remembered for a different root must not suppress the picker'
    );
    assert.strictEqual(h.runRiprCalls.length, 1, 'a dismissed cross-root pick issues no selected read');
    assert.strictEqual(h.bar.shown, 0, 'no attempt state may be presented');
  });

  test('dismissing the pick never guesses between several attempts', async () => {
    const inventory = await loadFixture('inventory.json');
    const selected = await loadFixture('status-awaiting_edit.json');
    const h = harness({ inventory, selected, pickIndex: undefined });
    const controller = controllerFor(h);

    await controller.showAttemptStatus();

    assert.strictEqual(h.runRiprCalls.length, 1, 'only the inventory read; no selected read without a pick');
    assert.strictEqual(h.bar.shown, 0, 'no attempt state may be presented after a dismissed pick');
    assert.ok(h.bar.hidden >= 1, 'the previous attempt presentation must be hidden on dismissal');
  });

  test('an explicit attempt id argument wins over the inventory and is remembered', async () => {
    const inventory = await loadFixture('inventory.json');
    const selected = await loadFixture('status-awaiting_edit.json');
    const explicitId = 'repair-attempt-0123456789abcdef01234567';
    const h = harness({ inventory, selected, selectedByAttempt: { [explicitId]: selected } });
    const controller = controllerFor(h);

    await controller.showAttemptStatus(explicitId);

    assert.strictEqual(h.quickPickCalls, 0, 'an explicit id never opens the picker');
    assert.deepStrictEqual(h.runRiprCalls[1], ['agent', 'status', '--root', '/workspace', '--attempt', explicitId, '--json']);
    const stored = h.memento.get('ripr.activeAttemptSelection.v1') as Record<string, string>;
    assert.strictEqual(stored['/workspace'], explicitId, 'the explicit id must be remembered for the root');
  });

  test('a status document for a different attempt id is refused, never re-labelled', async () => {
    const selected = await loadFixture('status-awaiting_edit.json');
    const requestedId = 'repair-attempt-0aaa1111aaaa1111aaaa1111aaaa1111aaaa1111';
    const inventory = {
      schema_version: '0.1',
      repair_attempts: [
        { attempt_id: requestedId, seam_id: '67fc764ba37d77bd', state: 'awaiting_edit', disposition: 'resumable', command: null, receipt: null, last_after_refusal: null }
      ]
    };
    const h = harness({ inventory, selected });
    const controller = controllerFor(h);

    await controller.showAttemptStatus();

    assert.strictEqual(h.bar.text, '$(warning) ripr: attempt status unavailable');
    assert.ok(h.bar.tooltip.includes('mismatched'), 'the refusal must name the id mismatch');
  });

  test('a stop during the read abandons the render instead of repainting a dead session', async () => {
    const selected = await loadFixture('status-awaiting_edit.json');
    const inventory = {
      schema_version: '0.1',
      repair_attempts: [
        { attempt_id: 'repair-attempt-0123456789abcdef01234567', seam_id: '67fc764ba37d77bd', state: 'awaiting_edit', disposition: 'resumable', command: null, receipt: null, last_after_refusal: null }
      ]
    };
    let controllerRef: RiprClientController | undefined;
    const h = harness({
      inventory,
      selected,
      onRunRipr: async () => {
        await controllerRef?.stop();
      }
    });
    const controller = controllerFor(h);
    controllerRef = controller;

    await controller.showAttemptStatus();

    assert.strictEqual(h.bar.shown, 0, 'no presentation may be rendered after a mid-read stop');
  });

  test('garbage from the CLI renders unavailable, never a friendly state', async () => {
    const h = harness({ inventory: '{not json', selected: '' });
    const controller = controllerFor(h);

    await controller.showAttemptStatus();

    assert.ok(h.warningMessages.length > 0, 'a failed read must warn');
    assert.strictEqual(h.bar.text, '$(warning) ripr: attempt status unavailable');
  });

  test('a corrupt_or_unavailable attempt is presented as an error tone', async () => {
    const selected = await loadFixture('status-corrupt_or_unavailable.json');
    const inventory = {
      schema_version: '0.1',
      repair_attempts: [
        { attempt_id: 'repair-attempt-0123456789abcdef01234567', seam_id: null, state: null, disposition: 'ended', command: null, receipt: null, last_after_refusal: null }
      ]
    };
    const h = harness({ inventory, selected });
    const controller = controllerFor(h);

    await controller.showAttemptStatus();

    assert.strictEqual(h.bar.text, '$(error) ripr: attempt unavailable');
    assert.ok(h.bar.tooltip.includes('unreadable'));
  });
});
