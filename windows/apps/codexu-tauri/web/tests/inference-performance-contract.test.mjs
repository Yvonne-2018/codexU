import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const modelsPath = new URL('../src/types/models.ts', import.meta.url);
const homePath = new URL('../src/components/DashboardHome.tsx', import.meta.url);
const panelPath = new URL('../src/components/InferencePerformancePanel.tsx', import.meta.url);
const messagesPath = new URL('../src/i18n/messages.ts', import.meta.url);
const stylesheetPath = new URL('../src/index.css', import.meta.url);

test('exposes inference performance as an independent Dashboard branch after Usage', async () => {
  const [models, home, panel, messages, stylesheet] = await Promise.all([
    readFile(modelsPath, 'utf8'),
    readFile(homePath, 'utf8'),
    readFile(panelPath, 'utf8'),
    readFile(messagesPath, 'utf8'),
    readFile(stylesheetPath, 'utf8'),
  ]);

  assert.match(models, /interface InferencePerformanceHistory/);
  assert.match(models, /inference_performance: InferencePerformanceHistory \| null/);
  const detailedUsageBlock = models.match(/export interface DetailedUsage \{[\s\S]*?\n\}/)?.[0] ?? '';
  assert.doesNotMatch(detailedUsageBlock, /inference_performance/);

  assert.match(
    home,
    /'usage'\s*\|\s*'inference'\s*\|\s*'projects'/,
    'Inference tab should sit after Usage and before Projects',
  );
  assert.match(home, /<InferencePerformancePanel inference=\{usage\?\.inference_performance \?\? null\}/);

  assert.match(panel, /p50_duration_seconds/);
  assert.match(panel, /p90_duration_seconds/);
  assert.match(panel, /effective_output_tokens_per_second/);
  assert.match(panel, /average_daily_call_count/);
  assert.match(panel, /reasoning_output_tokens/);
  assert.match(panel, /inference\.tooltipBasis/);
  assert.match(panel, /role="tooltip"/);
  assert.match(panel, /tabIndex=\{0\}/);
  assert.match(panel, /onMouseEnter=/);
  assert.match(panel, /onFocus=/);
  assert.match(panel, /onClick=/);
  assert.match(panel, /onKeyDown=/);
  assert.match(panel, /average_duration_seconds/);
  assert.match(panel, /output_tokens/);
  assert.match(panel, /modelColor\(group\.model\)/);
  assert.match(panel, /stableModelColorSlot\(model\)/);
  assert.match(panel, /var\(--data-model-/);
  assert.doesNotMatch(panel, /index\s*%\s*3|var\(--data-series-/);
  assert.doesNotMatch(panel, /<title>/);
  assert.doesNotMatch(panel, /prompt|reply|raw|arguments/i);

  assert.match(messages, /inference:\s*\{/);
  assert.match(messages, /Inference performance/);
  assert.match(messages, /推理性能/);
  assert.match(messages, /Output tokens/);
  assert.match(messages, /完整调用耗时/);
  assert.match(messages, /Daily calls/);
  assert.match(messages, /Reasoning tokens/);
  assert.doesNotMatch(messages, /not TTFT|不是 TTFT|visible text decode TPS|可见文本解码/);

  assert.match(stylesheet, /\.inference-performance-panel/);
  assert.match(stylesheet, /\.inference-scatter-point-hit/);
  assert.match(stylesheet, /\.inference-point-tooltip/);
});
