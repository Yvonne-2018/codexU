import type { CodexDashboardSnapshot } from './models';
import type { SettingsDto } from './settings';

export interface VisualTestData {
  dashboard: CodexDashboardSnapshot;
  settings: SettingsDto;
  source: {
    mode: 'fixture' | 'live-readonly';
    provider: 'fixture' | 'codex-dashboard';
  };
}

declare global {
  interface Window {
    __CODEXU_VISUAL_DATA__?: VisualTestData;
  }
}

export function getVisualTestData(): VisualTestData | undefined {
  if (typeof window === 'undefined') return undefined;
  return window.__CODEXU_VISUAL_DATA__;
}
