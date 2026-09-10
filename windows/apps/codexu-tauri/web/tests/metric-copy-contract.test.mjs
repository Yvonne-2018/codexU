import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const messagesPath = new URL('../src/i18n/messages.ts', import.meta.url);

test('inference and monthly copy should avoid implementation-level explanation in visible text', async () => {
  const messages = await readFile(messagesPath, 'utf8');

  // Inference: keep throughput copy concise and remove TTFT/TPS teaching.
  assert.match(messages, /Output tokens/);
  assert.match(messages, /完整调用耗时/i);
  assert.match(messages, /Reference ceiling|参考上限/);
  assert.match(messages, /Local estimate|本地估算/);
  assert.doesNotMatch(messages, /API-equivalent|API 等效/);
  assert.doesNotMatch(messages, /not official billing|不是官方账单/);
  assert.doesNotMatch(messages, /TTFT/i);
  assert.doesNotMatch(messages, /可见文本解码|visible text decode TPS/i);

  // Monthly progress: remove local estimate implementation details from visible copy.
  assert.doesNotMatch(messages, /log1p/i);
  assert.doesNotMatch(messages, /\$0-\$200/);
  assert.doesNotMatch(messages, /28%/);
});
