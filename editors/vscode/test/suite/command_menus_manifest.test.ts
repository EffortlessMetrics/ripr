//
// Command menus / keybindings manifest contract (#2081).
//
// The repair-loop commands were palette-only: no `contributes.menus`, no
// `contributes.keybindings`, no grouping. This suite pins the contributed
// editor/context menu entries (the repair-loop group, gated on the ripr
// document languages), the two default keybindings, and the wiring
// invariant that every menu/keybinding command id is also a contributed
// command (fail-closed against a dangling menu entry).
//
// The targeted-test and agent-loop commands stay code-action-only: their
// handlers need the diagnostic payload a code action carries, which a
// context-menu invocation cannot supply (the agent-loop commands already
// follow that rule, #2083). This suite also pins that exclusion.
//
// Platform note: VS Code when-clauses have no diagnostic-source context
// key, so gating on `diagnosticSource == ripr` is not implementable; the
// menu entries gate on `editorTextFocus` + `resourceLangId` instead. The
// language set mirrors RIPR_DOCUMENT_SELECTORS in src/client.ts.
//

import * as assert from 'assert';
import { promises as fs } from 'fs';
import * as path from 'path';

interface MenuEntry {
  command?: string;
  when?: string;
  group?: string;
}

interface KeybindingEntry {
  command?: string;
  key?: string;
  mac?: string;
  when?: string;
}

interface CommandMenusManifest {
  contributes?: {
    commands?: Array<{ command?: string }>;
    menus?: {
      'editor/context'?: MenuEntry[];
    };
    keybindings?: KeybindingEntry[];
  };
}

const RIPR_MENU_WHEN =
  'editorTextFocus && resourceLangId =~ /^(rust|typescript|typescriptreact|javascript|javascriptreact|python)$/';

const EXPECTED_EDITOR_CONTEXT: Array<{ command: string; group: string }> = [
  { command: 'ripr.copyRepairPacketAtCursor', group: 'ripr@1' },
  { command: 'ripr.copyTopRepairPacket', group: 'ripr@2' },
  { command: 'ripr.copyTopVerifyCommand', group: 'ripr@3' },
  { command: 'ripr.copyTopReceiptCommand', group: 'ripr@4' },
  { command: 'ripr.showTopLimitation', group: 'ripr@5' },
  { command: 'ripr.showStatus', group: 'ripr@6' },
  { command: 'ripr.showReceiptStatus', group: 'ripr@7' },
  { command: 'ripr.showRouteQuality', group: 'ripr@8' }
];

// Payload-bound commands that must stay code-action-only: the targeted-test
// group and the agent-loop group (#2083) both need the diagnostic payload a
// code action carries, which an editor/context menu click cannot supply.
const CODE_ACTION_ONLY_COMMANDS = [
  'ripr.copySuggestedAssertion',
  'ripr.copyTargetedTestBrief',
  'ripr.openRelatedTest',
  'ripr.copyAgentPacketCommand',
  'ripr.copyAgentBriefCommand',
  'ripr.copyAfterSnapshotCommand',
  'ripr.copyAgentVerifyCommand',
  'ripr.copyAgentReceiptCommand'
];

const EXPECTED_KEYBINDINGS: Array<{ command: string; key: string; mac: string }> = [
  { command: 'ripr.copyRepairPacketAtCursor', key: 'ctrl+alt+shift+p', mac: 'cmd+alt+shift+p' },
  { command: 'ripr.showStatus', key: 'ctrl+alt+r', mac: 'cmd+alt+r' },
  { command: 'ripr.copyTopRepairPacket', key: 'ctrl+alt+p', mac: 'cmd+alt+p' }
];

async function readManifest(): Promise<CommandMenusManifest> {
  const manifestPath = path.resolve(__dirname, '../../../package.json');
  return JSON.parse(await fs.readFile(manifestPath, 'utf8')) as CommandMenusManifest;
}

suite('Command Menus Manifest', () => {
  test('editor/context surfaces the repair-loop commands gated on ripr languages', async () => {
    const manifest = await readManifest();
    const entries = manifest.contributes?.menus?.['editor/context'] ?? [];
    assert.deepStrictEqual(
      entries.map((entry) => ({ command: entry.command, group: entry.group })),
      EXPECTED_EDITOR_CONTEXT,
      'editor/context must surface the repair-loop commands in ripr groups'
    );
    for (const entry of entries) {
      assert.strictEqual(
        entry.when,
        RIPR_MENU_WHEN,
        `${entry.command} must gate on editorTextFocus and the ripr document languages`
      );
    }
  });

  test('payload-bound targeted-test and agent-loop commands stay out of editor/context', async () => {
    const manifest = await readManifest();
    const entries = new Set(
      (manifest.contributes?.menus?.['editor/context'] ?? []).map((entry) => entry.command)
    );
    for (const command of CODE_ACTION_ONLY_COMMANDS) {
      assert.ok(
        !entries.has(command),
        `${command} needs a diagnostic payload and must stay code-action-only, not in editor/context`
      );
    }
  });

  test('default keybindings cover showStatus and copyTopRepairPacket and stay user-overridable', async () => {
    const manifest = await readManifest();
    const keybindings = manifest.contributes?.keybindings ?? [];
    assert.deepStrictEqual(
      keybindings.map((entry) => ({ command: entry.command, key: entry.key, mac: entry.mac })),
      EXPECTED_KEYBINDINGS,
      'expected default keybindings for the two highest-signal commands'
    );
    for (const entry of keybindings) {
      assert.strictEqual(entry.when, 'editorTextFocus', `${entry.command} keybinding must scope to editorTextFocus`);
    }
  });

  test('every menu and keybinding command id is a contributed command', async () => {
    const manifest = await readManifest();
    const contributed = new Set(
      (manifest.contributes?.commands ?? []).map((entry) => entry.command)
    );
    const referenced = [
      ...(manifest.contributes?.menus?.['editor/context'] ?? []).map((entry) => entry.command),
      ...(manifest.contributes?.keybindings ?? []).map((entry) => entry.command)
    ];
    for (const command of referenced) {
      assert.ok(command && contributed.has(command), `menu/keybinding references unregistered command ${command}`);
    }
  });
});
