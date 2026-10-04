import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';

const ALLOWED_COMPLETION_EVENT_PREFIXES = ['onCommand:', 'onSettingChanged:', 'onContext:', 'onView:', 'onLink:', 'extensionInstalled:'];

// VS Code built-in when-clause keys this walkthrough may name after `onContext:`.
// `workspaceTrusted` is not a VS Code context key; the built-in is `isWorkspaceTrusted`.
const DOCUMENTED_ON_CONTEXT_KEYS = new Set(['isWorkspaceTrusted', 'resourceLangId']);

interface WalkthroughStep {
  id: string;
  title: string;
  media?: { markdown?: string };
  completionEvents?: string[];
}

const EXPECTED_FLOW: Array<{ id: string; media: string; completionEvents: string[] }> = [
  {
    id: 'ripr.trustWorkspace',
    media: 'walkthrough/trust.md',
    completionEvents: ['onContext:isWorkspaceTrusted']
  },
  {
    id: 'ripr.openRustFile',
    media: 'walkthrough/open-file.md',
    completionEvents: ['onContext:resourceLangId == rust']
  },
  {
    id: 'ripr.readDiagnostics',
    media: 'walkthrough/diagnostics.md',
    completionEvents: []
  },
  {
    id: 'ripr.tryCodeAction',
    media: 'walkthrough/code-action.md',
    completionEvents: ['onCommand:ripr.showStatus']
  }
];

function onContextKeys(event: string): string[] | undefined {
  if (!event.startsWith('onContext:')) {
    return undefined;
  }
  const expression = event.slice('onContext:'.length).trim();
  if (expression.length === 0) {
    return [];
  }
  return expression.split(/\s*(?:&&|\|\|)\s*/).flatMap((clause) => {
    const clean = clause.trim().replace(/^!+/, '').trim();
    const match = /^[A-Za-z_][A-Za-z0-9_]*/.exec(clean);
    return match === null ? [] : [match[0]];
  });
}

function assertDocumentedOnContextKeys(event: string, label: string): void {
  const keys = onContextKeys(event);
  if (keys === undefined) {
    return;
  }
  assert.ok(keys.length > 0, `${label} has empty onContext expression: ${event}`);
  for (const key of keys) {
    assert.ok(
      DOCUMENTED_ON_CONTEXT_KEYS.has(key),
      `${label} names undocumented key ${key}: ${event}`
    );
  }
}

suite('Walkthrough Contribution Contract', () => {
  // Compiled tests live under out/test/suite; walk three levels back
  // to the extension root rather than resolving package/media under out/.
  const packageJsonPath = path.resolve(__dirname, '../../../package.json');
  const extensionRoot = path.resolve(__dirname, '../../..');

  function manifest(): { id: string; steps: WalkthroughStep[] } {
    const parsed = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
    const walkthroughs = parsed.contributes?.walkthroughs;
    assert.ok(Array.isArray(walkthroughs) && walkthroughs.length === 1, 'expected exactly one walkthrough');
    return walkthroughs[0];
  }

  test('the get-started walkthrough pins its ordered flow, media, and completion events', () => {
    const walkthrough = manifest();
    assert.strictEqual(walkthrough.id, 'ripr.getStarted');
    const actual = walkthrough.steps.map((step) => ({
      id: step.id,
      media: step.media?.markdown,
      completionEvents: step.completionEvents ?? []
    }));
    assert.deepStrictEqual(actual, EXPECTED_FLOW);
  });

  test('onContext events reject empty expressions and undocumented keys in every clause', () => {
    assert.ok(
      !DOCUMENTED_ON_CONTEXT_KEYS.has('workspaceTrusted'),
      'workspaceTrusted must not be treated as a documented VS Code context key'
    );
    assert.deepStrictEqual(onContextKeys('onCommand:ripr.showStatus'), undefined);
    assert.deepStrictEqual(onContextKeys('onContext:'), []);
    assert.deepStrictEqual(onContextKeys('onContext:workspaceTrusted'), ['workspaceTrusted']);
    assert.deepStrictEqual(onContextKeys('onContext:isWorkspaceTrusted'), ['isWorkspaceTrusted']);
    assert.deepStrictEqual(onContextKeys('onContext:!isWorkspaceTrusted'), ['isWorkspaceTrusted']);
    assert.deepStrictEqual(onContextKeys('onContext:resourceLangId == rust'), ['resourceLangId']);
    assert.deepStrictEqual(
      onContextKeys('onContext:isWorkspaceTrusted && workspaceTrusted'),
      ['isWorkspaceTrusted', 'workspaceTrusted']
    );
    assert.throws(() => assertDocumentedOnContextKeys('onContext:', 'empty'));
    assert.throws(() => assertDocumentedOnContextKeys('onContext:workspaceTrusted', 'invented'));
    assert.throws(() =>
      assertDocumentedOnContextKeys('onContext:isWorkspaceTrusted && workspaceTrusted', 'compound')
    );
    assertDocumentedOnContextKeys('onContext:isWorkspaceTrusted', 'trust');
    assertDocumentedOnContextKeys('onContext:!isWorkspaceTrusted', 'negated');
    assertDocumentedOnContextKeys('onContext:resourceLangId == rust', 'lang');
  });

  test('every media file exists and completion events use the supported vocabulary', () => {
    for (const step of EXPECTED_FLOW) {
      assert.ok(
        fs.existsSync(path.join(extensionRoot, step.media)),
        `walkthrough media does not exist: ${step.media}`
      );
      for (const event of step.completionEvents) {
        assert.ok(
          ALLOWED_COMPLETION_EVENT_PREFIXES.some((prefix) => event.startsWith(prefix)),
          `unsupported completion event on step ${step.id}: ${event}`
        );
        assertDocumentedOnContextKeys(event, `step ${step.id}`);
      }
    }
  });
});
