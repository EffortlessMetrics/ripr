import * as assert from 'assert';
import { promises as fs } from 'fs';
import * as path from 'path';

interface RiprExtensionManifest {
  capabilities?: {
    untrustedWorkspaces?: {
      supported?: string;
      description?: string;
      restrictedConfigurations?: string[];
    };
  };
  contributes?: {
    configuration?: {
      properties?: Record<string, Record<string, unknown>>;
    };
  };
}

suite('Workspace Trust Manifest', () => {
  test('limited support restricts executable server settings centrally', async () => {
    const manifestPath = path.resolve(__dirname, '../../../package.json');
    const manifest = JSON.parse(await fs.readFile(manifestPath, 'utf8')) as RiprExtensionManifest;
    const trust = manifest.capabilities?.untrustedWorkspaces;

    assert.strictEqual(trust?.supported, 'limited');
    assert.ok(
      typeof trust?.description === 'string' && trust.description.length > 0,
      'limited Workspace Trust support must explain which capabilities require trust'
    );

    const expectedRestrictedConfigurations = [
      'ripr.server.args',
      'ripr.server.autoDownload',
      'ripr.server.downloadBaseUrl',
      'ripr.server.path',
      'ripr.server.version'
    ];
    assert.deepStrictEqual(
      [...(trust?.restrictedConfigurations ?? [])].sort(),
      expectedRestrictedConfigurations
    );

    const properties = manifest.contributes?.configuration?.properties ?? {};
    for (const setting of expectedRestrictedConfigurations) {
      assert.ok(properties[setting], `expected contributed setting ${setting}`);
      assert.strictEqual(
        properties[setting].restrictedConfigurations,
        undefined,
        `${setting} must be governed by capabilities.untrustedWorkspaces.restrictedConfigurations`
      );
    }
  });
});
