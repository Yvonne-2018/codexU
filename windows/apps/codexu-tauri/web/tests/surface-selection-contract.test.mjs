import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const runnerPath = new URL('./e2e/all-surfaces.spec.mjs', import.meta.url);

test('visual runner exposes a single-surface selector without changing the default full run', async () => {
  const runner = await readFile(runnerPath, 'utf8');

  assert.match(runner, /CODEXU_VISUAL_SURFACE/);
  assert.match(runner, /selectSurfaces\(SURFACES, process\.env\.CODEXU_VISUAL_SURFACE\)/);
  assert.match(runner, /observations\.length, selectedSurfaces\.length/);
  assert.match(runner, /scope: selectedSurface \? 'single' : 'all'/);
});

test('visual runner rejects an unknown selected surface instead of silently running the wrong page', async () => {
  const runner = await readFile(runnerPath, 'utf8');

  assert.match(runner, /Unknown visual surface/);
});
