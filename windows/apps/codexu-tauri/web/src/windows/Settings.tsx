import { useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  CheckCircle2,
  ExternalLink,
  FolderOpen,
  Loader2,
  Palette,
  Power,
  RefreshCw,
  Rocket,
  Stethoscope,
  Trash2,
  XCircle,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useSettings } from '../hooks/useSettings';
import type { InterfaceLanguage, ThemeMode, TrayDensity } from '../types/settings';
import { isTauriRuntimeAvailable, requireTauriRuntime } from '../utils/tauri';
import { applyAppTheme } from '../utils/appTheme';
import {
  DEFAULT_PALETTE_ID,
  PALETTE_CATALOG,
  type PaletteId,
} from '../utils/paletteCatalog';
import { useI18n } from '../i18n/I18nProvider';

interface DiagnosticsResult {
  codex_root: string;
  codex_root_exists: boolean;
  state_db_exists: boolean;
  claude_projects_exists: boolean;
  claude_tasks_exists: boolean;
  codex_executable: boolean;
  codex_quota_read_succeeded: boolean;
  messages: string[];
}

interface UpdateCheckResult {
  current_version: string | null;
  latest_version: string | null;
  release_url: string | null;
  checked_at: number | null;
  error: string | null;
}

export function Settings() {
  const canInvokeTauri = isTauriRuntimeAvailable();
  const { settings, update, error } = useSettings();
  const { t, language, preference, setPreference } = useI18n();
  const [saved, setSaved] = useState(false);
  const [diagnostics, setDiagnostics] = useState<DiagnosticsResult | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [diagnosticsRunning, setDiagnosticsRunning] = useState(false);
  const [updateCheck, setUpdateCheck] = useState<UpdateCheckResult | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [updateChecking, setUpdateChecking] = useState(false);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [autostartError, setAutostartError] = useState<string | null>(null);

  useEffect(() => {
    if (!canInvokeTauri) return;
    let cancelled = false;
    (async () => {
      try {
        requireTauriRuntime();
        const value = await invoke<boolean>('get_autostart');
        if (!cancelled) setAutostart(value);
      } catch (e) {
        if (!cancelled) setAutostartError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [canInvokeTauri]);

  useEffect(() => {
    applyAppTheme(
      settings?.config.theme ?? 'system',
      settings?.config.palette_id ?? DEFAULT_PALETTE_ID,
    );
  }, [settings?.config.theme, settings?.config.palette_id]);

  if (!settings) {
    if (error) {
      return (
        <div className="h-full flex flex-col">
          <header className="mx-4 mt-4 glass-toolbar px-5 py-3 rounded-2xl">
            <h1 className="text-lg font-semibold text-primary">{t('settings.title')}</h1>
          </header>
          <div className="flex-1 p-6">
            <div className="glass-panel p-4 text-sm text-status-error">
              {t('settings.failed', { error: error ?? t('common.unknownError') })}
            </div>
          </div>
        </div>
      );
    }

    return (
      <div className="h-full flex items-center justify-center bg-transparent">
        <div className="glass-panel px-6 py-8 text-sm text-secondary">{t('settings.loading')}</div>
      </div>
    );
  }

  const config = settings.config;

  const flashSaved = () => {
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  const runUpdate = async (action: () => Promise<unknown>) => {
    try {
      await action();
      flashSaved();
    } catch (e) {
      console.error(e);
    }
  };

  const pickDirectory = async (key: 'codex_root' | 'cache_dir') => {
    if (!canInvokeTauri) {
      return;
    }

    const selected = await open({ directory: true });
    if (selected) {
      await runUpdate(async () => {
        await update({ [key]: selected });
      });
    }
  };

  const handleTheme = async (theme: ThemeMode) => {
    if (!canInvokeTauri) {
      return;
    }

    await runUpdate(async () => {
      await update({ theme });
      applyAppTheme(theme, config.palette_id ?? DEFAULT_PALETTE_ID);
    });
  };

  const handlePalette = async (palette_id: PaletteId) => {
    if (!canInvokeTauri) {
      return;
    }

    await runUpdate(async () => {
      await update({ palette_id });
      applyAppTheme(config.theme, palette_id);
    });
  };

  const handleDensity = async (tray_density: TrayDensity) => {
    if (!canInvokeTauri) {
      return;
    }

    await runUpdate(async () => {
      await update({ tray_density });
    });
  };

  const handleLanguage = async (language: InterfaceLanguage) => {
    if (!canInvokeTauri) return;

    await runUpdate(async () => {
      await setPreference(language);
    });
  };

  const handleInterval = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (!canInvokeTauri) {
      return;
    }

    const secs = parseInt(e.target.value, 10);
    if (!isNaN(secs)) {
      await runUpdate(async () => {
        await update({ refresh_interval_secs: secs });
      });
    }
  };

  const clearCache = async () => {
    if (!canInvokeTauri) {
      return;
    }

    try {
      requireTauriRuntime();
      await invoke('clear_cache');
      flashSaved();
    } catch (e) {
      console.error(e);
    }
  };

  const refreshUsage = async () => {
    if (!canInvokeTauri) {
      return;
    }

    try {
      requireTauriRuntime();
      await invoke('refresh_usage');
      flashSaved();
    } catch (e) {
      console.error(e);
    }
  };

  const runDiagnostics = async () => {
    if (!canInvokeTauri) return;
    setDiagnosticsRunning(true);
    setDiagnosticsError(null);
    try {
      requireTauriRuntime();
      const result = await invoke<DiagnosticsResult>('run_diagnostics');
      setDiagnostics(result);
    } catch (e) {
      setDiagnosticsError(String(e));
    } finally {
      setDiagnosticsRunning(false);
    }
  };

  const checkUpdates = async () => {
    if (!canInvokeTauri) return;
    setUpdateChecking(true);
    setUpdateError(null);
    try {
      requireTauriRuntime();
      const result = await invoke<UpdateCheckResult>('check_for_updates');
      setUpdateCheck(result);
    } catch (e) {
      setUpdateError(String(e));
    } finally {
      setUpdateChecking(false);
    }
  };

  const toggleAutostart = async () => {
    if (!canInvokeTauri || autostart == null) return;
    const next = !autostart;
    setAutostartError(null);
    try {
      requireTauriRuntime();
      await invoke('set_autostart', { enabled: next });
      setAutostart(next);
    } catch (e) {
      setAutostartError(String(e));
    }
  };

  return (
    <div className="h-full flex flex-col">
      <header className="mx-4 mt-4 glass-toolbar px-5 py-3 rounded-2xl">
        <h1 className="text-lg font-semibold text-primary">{t('settings.title')}</h1>
      </header>

      <main className="flex-1 overflow-auto p-6 md:p-7">
        <div className="max-w-lg mx-auto w-full space-y-6">
          <Section title={t('settings.dataPaths')}>
            <PathField
              label={t('settings.codexDataRoot')}
              value={config.codex_root}
              onBrowse={() => pickDirectory('codex_root')}
            />
            <PathField
              label={t('settings.cacheDirectory')}
              value={config.cache_dir}
              onBrowse={() => pickDirectory('cache_dir')}
            />
          </Section>

          <Section title={t('settings.appearance')}>
            <div className="grid grid-cols-3 gap-2">
              {(['light', 'dark', 'system'] as ThemeMode[]).map((themeValue) => (
                <button
                  key={themeValue}
                  onClick={() => handleTheme(themeValue)}
                  disabled={!canInvokeTauri}
                  className={`px-3 py-2 rounded-full text-sm capitalize transition-all ${
                    config.theme === themeValue ? 'glass-button-solid' : 'text-secondary glass-button'
                  }`}
                >
                  {themeValue === 'light'
                    ? t('common.light')
                    : themeValue === 'dark'
                      ? t('common.dark')
                      : t('common.system')}
                </button>
              ))}
            </div>
            <div className="mt-5">
              <div className="flex items-center gap-2 text-sm text-secondary mb-1">
                <Palette size={14} />
                <span>{t('settings.palette')}</span>
              </div>
              <p className="text-xs text-tertiary mb-3">{t('settings.paletteDescription')}</p>
              <div className="grid grid-cols-2 gap-2">
                {PALETTE_CATALOG.map((palette) => {
                  const selected = config.palette_id === palette.id;
                  return (
                    <button
                      key={palette.id}
                      onClick={() => handlePalette(palette.id)}
                      disabled={!canInvokeTauri}
                      className={`text-left rounded-xl px-3 py-2 transition-all ${
                        selected ? 'glass-button-solid' : 'text-secondary glass-button'
                      }`}
                      aria-pressed={selected}
                    >
                      <span className="flex items-center gap-1.5 mb-1" aria-hidden="true">
                        {[palette.light.accent.primary, palette.light.accent.secondary, palette.light.accent.highlight].map(
                          (color) => (
                            <span
                              key={color}
                              className="w-3 h-3 rounded-full border border-white/50"
                              style={{ backgroundColor: color }}
                            />
                          ),
                        )}
                      </span>
                      <span className="block text-xs font-medium">
                        {palette.displayName[language]}
                      </span>
                      <span className="block text-[11px] text-tertiary mt-0.5">
                        {palette.shortDescription[language]}
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>
            <div className="mt-4">
              <p className="block text-sm text-secondary mb-2">{t('settings.interfaceLanguage')}</p>
              <div className="grid grid-cols-3 gap-2">
                {(['auto', 'zh-Hans', 'en'] as InterfaceLanguage[]).map((language) => (
                  <button
                    key={language}
                    onClick={() => handleLanguage(language)}
                    disabled={!canInvokeTauri}
                    className={`px-3 py-2 rounded-full text-sm transition-all ${
                      preference === language ? 'glass-button-solid' : 'text-secondary glass-button'
                    }`}
                  >
                    {language === 'auto'
                      ? t('common.auto')
                      : language === 'zh-Hans'
                        ? t('common.chinese')
                        : t('common.english')}
                  </button>
                ))}
              </div>
            </div>
          </Section>

          <Section title={t('settings.tray')}>
            <div className="grid grid-cols-3 gap-2">
              {(['minimal', 'classic', 'rich'] as TrayDensity[]).map((d) => (
                <button
                  key={d}
                  onClick={() => handleDensity(d)}
                  disabled={!canInvokeTauri}
                  className={`px-3 py-2 rounded-full text-sm capitalize transition-all ${
                    config.tray_density === d ? 'glass-button-solid' : 'text-secondary glass-button'
                  }`}
                >
                  {d}
                </button>
              ))}
            </div>
          </Section>

          <Section title={t('settings.refresh')}>
            <label className="block text-sm text-secondary mb-2">{t('settings.interval')}</label>
            <input
              type="number"
              min={10}
              max={3600}
              value={config.refresh_interval_secs}
              onChange={handleInterval}
              disabled={!canInvokeTauri}
              className="w-full px-3 py-2 glass-input text-primary text-sm"
            />
            <div className="flex gap-3 mt-4">
              <button
                onClick={refreshUsage}
                disabled={!canInvokeTauri}
                className="flex items-center gap-2 px-4 py-2 rounded-full glass-button-solid text-sm"
              >
                <RefreshCw size={14} /> {t('common.refreshNow')}
              </button>
              <button
                onClick={clearCache}
                disabled={!canInvokeTauri}
                className="flex items-center gap-2 px-4 py-2 rounded-full glass-button text-status-error border-status-error/30 text-status-error text-sm"
              >
                <Trash2 size={14} /> {t('common.clearCache')}
              </button>
            </div>
          </Section>

          <Section title={t('settings.diagnostics')}>
            <p className="text-xs text-tertiary mb-3">{t('settings.diagnosticsHint')}</p>
            <button
              onClick={runDiagnostics}
              disabled={!canInvokeTauri || diagnosticsRunning}
              className="flex items-center gap-2 px-4 py-2 rounded-full glass-button-solid text-sm"
            >
              {diagnosticsRunning ? (
                <Loader2 size={14} className="animate-spin" aria-hidden="true" />
              ) : (
                <Stethoscope size={14} aria-hidden="true" />
              )}
              {t('settings.runDiagnostics')}
            </button>
            {diagnosticsError ? (
              <p className="mt-3 text-xs text-status-error">
                {t('settings.diagFailed', { error: diagnosticsError })}
              </p>
            ) : null}
            {diagnostics ? <DiagnosticsResultView result={diagnostics} t={t} /> : null}
          </Section>

          <Section title={t('settings.updates')}>
            <button
              onClick={checkUpdates}
              disabled={!canInvokeTauri || updateChecking}
              className="flex items-center gap-2 px-4 py-2 rounded-full glass-button-solid text-sm"
            >
              {updateChecking ? (
                <Loader2 size={14} className="animate-spin" aria-hidden="true" />
              ) : (
                <Rocket size={14} aria-hidden="true" />
              )}
              {t('settings.checkUpdates')}
            </button>
            {updateError ? (
              <p className="mt-3 text-xs text-status-error">
                {t('settings.updateFailed', { error: updateError })}
              </p>
            ) : null}
            {updateCheck ? <UpdateCheckView result={updateCheck} t={t} /> : null}
          </Section>

          <Section title={t('settings.launchAtStartup')}>
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-start gap-2 min-w-0">
                <Power size={15} aria-hidden="true" className="mt-0.5 shrink-0 text-secondary" />
                <div className="min-w-0">
                  <p className="text-sm text-secondary">{t('settings.launchAtStartup')}</p>
                  <p className="text-xs text-tertiary mt-0.5">{t('settings.launchAtStartupDetail')}</p>
                </div>
              </div>
              <button
                role="switch"
                aria-checked={autostart === true}
                aria-label={t('settings.launchAtStartup')}
                disabled={!canInvokeTauri || autostart == null}
                onClick={toggleAutostart}
                className={`relative h-6 w-11 shrink-0 rounded-full transition-colors disabled:opacity-50 ${
                  autostart === true ? 'bg-data-primary' : 'bg-surface-inset'
                }`}
              >
                <span
                  className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${
                    autostart === true ? 'translate-x-5' : 'translate-x-0'
                  }`}
                />
              </button>
            </div>
            {autostartError ? (
              <p className="mt-3 text-xs text-status-error">
                {t('settings.autostartFailed', { error: autostartError })}
              </p>
            ) : null}
          </Section>

          <Section title={t('settings.about')}>
            <p className="text-sm text-secondary">{t('settings.version')}</p>
            <p className="text-xs text-tertiary mt-2">{t('settings.dataFolder', { path: settings.app_data_dir })}</p>
            <p className="text-xs text-tertiary mt-2">{t('settings.privacy')}</p>
            {!canInvokeTauri && (
              <p className="text-xs text-status-warn mt-3">
                {t('settings.browserWarning')}
              </p>
            )}
          </Section>

          {saved && <p className="text-center text-sm text-status-ok">{t('common.saved')}</p>}
          {error && <p className="text-center text-sm text-status-error">{t('settings.failed', { error })}</p>}
        </div>
      </main>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="glass-panel p-4 sm:p-5">
      <h2 className="text-sm font-semibold text-primary mb-3">{title}</h2>
      {children}
    </div>
  );
}

function PathField({
  label,
  value,
  onBrowse,
}: {
  label: string;
  value: string;
  onBrowse: () => void;
}) {
  return (
    <div className="mb-3 last:mb-0">
      <label className="block text-sm text-secondary mb-1">{label}</label>
      <div className="flex gap-2">
        <input
          readOnly
          value={value}
          className="flex-1 px-3 py-2 glass-input text-primary text-sm truncate"
        />
        <button
          onClick={onBrowse}
          className="px-3 py-2 rounded-full glass-button text-secondary"
        >
          <FolderOpen size={16} />
        </button>
      </div>
    </div>
  );
}

function DiagnosticsResultView({
  result,
  t,
}: {
  result: DiagnosticsResult;
  t: ReturnType<typeof useI18n>['t'];
}) {
  const checks = [
    { label: t('settings.diagCodexRoot'), ok: result.codex_root_exists, detail: result.codex_root },
    { label: t('settings.diagStateDbExists'), ok: result.state_db_exists },
    { label: t('settings.diagClaudeProjectsExists'), ok: result.claude_projects_exists },
    { label: t('settings.diagClaudeTasksExists'), ok: result.claude_tasks_exists },
    { label: t('settings.diagCodexExecutable'), ok: result.codex_executable },
    { label: t('settings.diagCodexQuotaRead'), ok: result.codex_quota_read_succeeded },
  ];

  return (
    <div className="mt-4">
      <div className="space-y-2">
        {checks.map((check) => (
          <div key={check.label} className="flex items-center justify-between gap-3 text-sm">
            <span className="flex min-w-0 items-center gap-2">
              {check.ok ? (
                <CheckCircle2 size={14} aria-hidden="true" className="shrink-0 text-status-ok" />
              ) : (
                <XCircle size={14} aria-hidden="true" className="shrink-0 text-status-error" />
              )}
              <span className="truncate text-secondary">{check.label}</span>
            </span>
            {check.detail ? (
              <span className="max-w-[55%] truncate text-xs text-tertiary" title={check.detail}>
                {check.detail}
              </span>
            ) : (
              <span className="shrink-0 text-xs text-tertiary">
                {check.ok ? t('settings.diagOk') : t('settings.diagMissing')}
              </span>
            )}
          </div>
        ))}
      </div>
      {result.messages.length > 0 ? (
        <ul className="mt-3 space-y-1 text-xs text-tertiary">
          {result.messages.map((message, index) => (
            <li key={index}>{message}</li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function UpdateCheckView({
  result,
  t,
}: {
  result: UpdateCheckResult;
  t: ReturnType<typeof useI18n>['t'];
}) {
  if (result.error) {
    return <p className="mt-3 text-xs text-status-error">{result.error}</p>;
  }

  const hasNewVersion =
    result.latest_version != null &&
    result.current_version != null &&
    result.latest_version !== result.current_version;

  return (
    <div className="mt-4 space-y-1.5">
      {result.checked_at ? (
        <p className="text-xs text-tertiary">
          {t('settings.checkedAt', { time: new Date(result.checked_at).toLocaleString() })}
        </p>
      ) : null}
      {hasNewVersion ? (
        <>
          <p className="text-xs text-secondary">
            {t('settings.currentVersion', { value: result.current_version ?? t('common.unknown') })} ·{' '}
            {t('settings.latestVersion', { value: result.latest_version ?? t('common.unknown') })}
          </p>
          <a
            href={result.release_url ?? '#'}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 text-sm text-accent"
          >
            {t('settings.openRelease')}
            <ExternalLink size={13} aria-hidden="true" />
          </a>
        </>
      ) : (
        <p className="text-sm text-secondary">{t('settings.upToDate')}</p>
      )}
    </div>
  );
}
