import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const runnerPath = new URL('./e2e/all-surfaces.spec.mjs', import.meta.url);
const fixturePath = new URL('./e2e/support/fixture-data.mjs', import.meta.url);
const visualTypePath = new URL('../src/types/visualTest.ts', import.meta.url);

test('uses a repository fixture by default and keeps local Codex reads opt-in', async () => {
  const [runner, fixture, visualType] = await Promise.all([
    readFile(runnerPath, 'utf8'),
    readFile(fixturePath, 'utf8'),
    readFile(visualTypePath, 'utf8'),
  ]);

  assert.match(runner, /createVisualFixture/);
  assert.match(runner, /process\.env\.CODEXU_VISUAL_LIVE === '1'/);
  assert.match(runner, /\? await loadLiveDashboard\(\)\s*:\s*createVisualFixture\(\)/);
  assert.match(fixture, /mode: 'fixture'/);
  assert.match(fixture, /provider: 'fixture'/);
  assert.doesNotMatch(fixture, /C:\\Users|\.codex|[0-9a-f]{8}-[0-9a-f-]{27,}/i);
  assert.match(visualType, /mode: 'fixture' \| 'live-readonly'/);
  assert.match(visualType, /provider: 'fixture' \| 'codex-dashboard'/);
});
