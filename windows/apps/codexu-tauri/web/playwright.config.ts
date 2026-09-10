import { defineConfig, devices } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: false,
  forbidOnly: Boolean(process.env.CI),
  retries: 0,
  workers: 1,
  reporter: [['line']],
  outputDir: path.resolve(webRoot, '../../../../.local-artifacts/web-playwright/test-results'),
  use: {
    baseURL: 'http://127.0.0.1:1421',
    locale: 'en-US',
    timezoneId: 'Asia/Shanghai',
    colorScheme: 'light',
    ...devices['Desktop Chrome'],
    viewport: { width: 960, height: 760 },
    screenshot: 'off',
    trace: 'retain-on-failure',
    video: 'off',
  },
  webServer: {
    command: 'npm run dev -- --host 127.0.0.1 --port 1421',
    url: 'http://127.0.0.1:1421/visual-test.html',
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
