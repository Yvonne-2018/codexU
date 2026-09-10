import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppConfig, SettingsDto, SettingsResponse } from '../types/settings';
import {
  isTauriRuntimeAvailable,
  requireTauriRuntime,
} from '../utils/tauri';
import { getVisualTestData } from '../types/visualTest';

export function useSettings() {
  const [settings, setSettings] = useState<SettingsDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const normalizeSettings = (payload: SettingsResponse): SettingsDto => ({
    config: {
      codex_root: payload.codex_root,
      cache_dir: payload.cache_dir,
      theme: payload.theme,
      refresh_interval_secs: payload.refresh_interval_secs,
      tray_density: payload.tray_density,
      language: payload.language ?? 'auto',
      palette_id: payload.palette_id ?? 'codexu.default',
      query_codex_official_quota: payload.query_codex_official_quota ?? true,
      query_claude_official_quota: payload.query_claude_official_quota ?? true,
    },
    app_data_dir: payload.app_data_dir,
  });

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      requireTauriRuntime();
      const dto = await invoke<SettingsResponse>('get_settings');
      setSettings(normalizeSettings(dto));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const update = useCallback(async (patch: Partial<AppConfig>): Promise<AppConfig> => {
    setError(null);
    try {
      requireTauriRuntime();
      const updated = await invoke<AppConfig>('set_settings', { req: patch });
      setSettings((prev) => (prev ? { ...prev, config: updated } : null));
      return updated;
    } catch (e) {
      setError(String(e));
      throw e;
    }
  }, []);

  useEffect(() => {
    const visualSettings = getVisualTestData()?.settings;
    if (visualSettings) {
      setSettings(visualSettings);
      setLoading(false);
      return;
    }

    load();

    if (!isTauriRuntimeAvailable()) {
      return;
    }

    let unlisten: (() => void) | null = null;
    let cancelled = false;

    const subscribe = async () => {
      try {
        const unlistenFn = await listen('settings:changed', () => {
          load();
        });
        if (cancelled) {
          unlistenFn();
        } else {
          unlisten = unlistenFn;
        }
      } catch (e) {
        setError(String(e));
      }
    };
    subscribe();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [load]);

  return { settings, loading, update, reload: load, error };
}
