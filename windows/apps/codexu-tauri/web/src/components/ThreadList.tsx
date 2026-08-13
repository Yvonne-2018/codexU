import { Archive, Clock, Cpu } from 'lucide-react';
import type { LocalThread } from '../types/models';
import { useI18n } from '../i18n/I18nProvider';

interface ThreadListProps {
  threads: LocalThread[];
}

export function ThreadList({ threads }: ThreadListProps) {
  const { t } = useI18n();
  if (threads.length === 0) {
    return (
      <div className="glass-panel p-4 sm:p-5">
        <h3 className="text-sm font-semibold text-primary mb-4">{t('usage.recentThreads')}</h3>
        <p className="text-secondary text-sm">{t('usage.noThreads')}</p>
      </div>
    );
  }

  const sorted = [...threads].sort((a, b) => {
    const ta = a.updated_at ?? 0;
    const tb = b.updated_at ?? 0;
    return tb - ta;
  });

  return (
    <div className="glass-panel p-4 sm:p-5">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-semibold text-primary">{t('usage.recentThreads')}</h3>
        <span className="text-xs text-tertiary">{t('usage.threadsTotal', { count: threads.length })}</span>
      </div>
      <div className="space-y-2 max-h-80 overflow-auto">
        {sorted.slice(0, 20).map((thread) => (
          <div
            key={thread.id}
            className="flex items-center justify-between gap-3 p-3 rounded-xl glass-input hover:border-theme/80 transition-colors"
          >
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <p className="text-sm font-medium text-primary truncate">{thread.title}</p>
                {thread.archived && (
                  <span className="inline-flex items-center gap-1 chip-like bg-status-warn/12 text-status-warn border-status-warn/30">
                    <Archive size={10} /> {t('tasks.archived')}
                  </span>
                )}
              </div>
              <p className="text-xs text-tertiary truncate mt-0.5">{shortPath(thread.cwd, t)}</p>
            </div>
            <div className="flex items-center gap-3 text-xs text-secondary shrink-0">
              {thread.model && (
                <span className="inline-flex items-center gap-1">
                  <Cpu size={12} /> {thread.model}
                </span>
              )}
              {thread.updated_at && (
                <span className="inline-flex items-center gap-1">
                  <Clock size={12} /> {formatTime(thread.updated_at, t)}
                </span>
              )}
              <span className="font-medium text-primary">{formatNumber(thread.tokens)}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function shortPath(path: string, t: ReturnType<typeof useI18n>['t']): string {
  if (!path) return t('common.unknown');
  const normalized = path.replace(/\\/g, '/');
  const parts = normalized.split('/').filter(Boolean);
  return parts.slice(-2).join('/') || normalized;
}

function formatTime(ts: number, t: ReturnType<typeof useI18n>['t']): string {
  const date = new Date(ts);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffHrs = Math.floor(diffMs / (1000 * 60 * 60));
  if (diffHrs < 1) return t('usage.timeNow');
  if (diffHrs < 24) return t('usage.hoursAgo', { value: diffHrs });
  const diffDays = Math.floor(diffHrs / 24);
  if (diffDays < 7) return t('usage.daysAgo', { value: diffDays });
  return date.toLocaleDateString();
}

function formatNumber(n: number): string {
  return n.toLocaleString();
}
