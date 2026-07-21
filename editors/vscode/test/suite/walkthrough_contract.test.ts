import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';

const ALLOWED_COMPLETION_EVENT_PREFIXES = ['onCommand:', 'onSettingChanged:', 'onContext:', 'onView:', 'onLink:', 'extensionInstalled:'];

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
    completionEvents: ['onContext:workspaceTrusted']
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

suite('Walkthrough Contribution Contract', () => {
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
      }
    }
  });
});
