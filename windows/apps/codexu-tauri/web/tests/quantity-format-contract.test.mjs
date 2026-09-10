import assert from 'node:assert/strict';
import test from 'node:test';

const formatterModuleUrl = new URL('../src/utils/formatQuantity.js', import.meta.url);

test('formatQuantity keeps compact quantity values stable across boundaries', async () => {
  const { formatQuantity } = await import(formatterModuleUrl);

  assert.equal(formatQuantity(999), '999');
  assert.equal(formatQuantity(1000), '1.0K');
  assert.equal(formatQuantity(1200), '1.2K');
  assert.equal(formatQuantity(999500), '999.5K');
  assert.equal(formatQuantity(999950), '1.0M');
  assert.equal(formatQuantity(12_400_000), '12.4M');
  assert.equal(formatQuantity(1_200_000_000), '1.2B');
  assert.equal(formatQuantity(-1240), '-1.2K');
  assert.equal(formatQuantity(0), '0');
  assert.equal(formatQuantity(null), '--');
});
