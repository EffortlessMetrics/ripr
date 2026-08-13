import { promises as fs } from 'fs';
import * as path from 'path';
import * as assert from 'assert';
import * as vscode from 'vscode';

suite('Packaged Workspace Trust', () => {
  test('trusted host copies an actionable repair packet', async function () {
    if (process.env.RIPR_TEST_WORKSPACE_TRUST === 'untrusted') {
      this.skip();
      return;
    }

    assert.strictEqual(vscode.workspace.isTrusted, true);
    const extension = vscode.extensions.getExtension('EffortlessMetrics.ripr');
    assert.ok(extension, 'ripr extension should be present');
    await extension.activate();

    const workspace = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspace, 'packaged trust test requires a workspace');
    const reportPath = path.join(
      workspace.uri.fsPath,
      'target',
      'ripr',
      'reports',
      'actionable-gaps.json'
    );
    await fs.mkdir(path.dirname(reportPath), { recursive: true });
    await fs.writeFile(reportPath, JSON.stringify(actionableGapReport()), 'utf8');
    try {
      await vscode.env.clipboard.writeText('packaged-trust-sentinel');
      await vscode.commands.executeCommand('ripr.copyCurrentRepairPacket');
      const packet = await vscode.env.clipboard.readText();
      assert.notStrictEqual(packet, 'packaged-trust-sentinel');
      assert.ok(packet.includes('RIPR current repair packet'), packet);
      assert.ok(packet.includes('gap:rust:pricing:discount:threshold-boundary'), packet);
    } finally {
      await fs.rm(reportPath, { force: true });
    }
  });

  test('untrusted host keeps an actionable repair packet out of the clipboard', async function () {
    if (process.env.RIPR_TEST_WORKSPACE_TRUST !== 'untrusted') {
      this.skip();
      return;
    }

    assert.strictEqual(vscode.workspace.isTrusted, false);
    const extension = vscode.extensions.getExtension('EffortlessMetrics.ripr');
    assert.ok(extension, 'ripr extension should be present');
    await extension.activate();

    const workspace = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspace, 'packaged trust test requires a workspace');
    const reportPath = path.join(
      workspace.uri.fsPath,
      'target',
      'ripr',
      'reports',
      'actionable-gaps.json'
    );
    await fs.mkdir(path.dirname(reportPath), { recursive: true });
    await fs.writeFile(reportPath, JSON.stringify(actionableGapReport()), 'utf8');
    try {
      await vscode.env.clipboard.writeText('packaged-trust-sentinel');
      await vscode.commands.executeCommand('ripr.copyCurrentRepairPacket');
      assert.strictEqual(await vscode.env.clipboard.readText(), 'packaged-trust-sentinel');
    } finally {
      await fs.rm(reportPath, { force: true });
    }
  });
});

function actionableGapReport(): Record<string, unknown> {
  return {
    schema_version: '0.1',
    tool: 'ripr',
    report: 'actionable-gaps',
    root: '.',
    scope: 'repo',
    status: 'advisory',
    summary: {
      actionable_gaps: 1,
      packets_emitted: 1,
      public_projection_eligible_packets: 1,
      public_projection_excluded_packets: 0
    },
    run_limitations: [],
    packets: [
      {
        canonical_gap_id: 'gap:rust:pricing:discount:threshold-boundary',
        evidence_class: 'predicate_boundary',
        gap_state: 'actionable',
        actionability: 'extend_related_test',
        source_file: 'src/pricing.rs',
        changed_behavior: 'amount >= threshold',
        why: 'A related Rust test reaches this change, but no equality-boundary assertion was found.',
        primary_anchor: { file: 'src/pricing.rs', line: 42 },
        repair_kind: 'add_boundary_assertion',
        target_test_type: 'boundary_discriminator',
        target_test: 'tests/pricing.rs::premium_customer_gets_discount',
        assertion_shape: 'assert_eq!(discount(100, 100), 90)',
        missing_discriminators: [
          { value: 'amount == threshold', reason: 'equality boundary is not covered' }
        ],
        recommended_repair: 'Add exact assertion for amount == threshold.',
        related_test_or_observer: {
          file: 'tests/pricing.rs',
          name: 'premium_customer_gets_discount',
          line: 12
        },
        verify_command: 'ripr agent verify --root . --json',
        repair_route_source: 'canonical_item.repair_route',
        verify_command_source: 'canonical_item.verify_command',
        receipt_command_or_path: 'ripr agent receipt --root . --json',
        receipt_source: 'canonical_item.receipt_command',
        public_projection_eligible: true,
        projection_exclusion_reasons: [],
        raw_findings: [{ file: 'src/pricing.rs', line: 42, kind: 'weakly_exposed' }],
        confidence_basis: 'static_only'
      }
    ]
  };
}
