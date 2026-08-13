import { Activity, Calendar, Coins, TrendingUp, type LucideIcon } from 'lucide-react';
import type {
  InferencePerformance,
  ModelUsageTrend,
  PricedTokenUsage,
  TokenBreakdown,
  UsageDayBucket,
  LocalUsage,
} from '../types/models';
import { TrendChart } from './TrendChart';
import { UsageHeatmap } from './UsageHeatmap';
import { TokenBarChart } from './TokenBarChart';
import { ThreadList } from './ThreadList';
import { useI18n } from '../i18n/I18nProvider';

interface UsagePanelProps {
  usage: LocalUsage | null | undefined;
}

export function UsagePanel({ usage }: UsagePanelProps) {
  const { t } = useI18n();
  const detailed = usage?.detailed_usage ?? null;
  const trend = usage?.usage_trend ?? null;

  return (
    <section className="space-y-4 usage-panel" aria-label={t('usage.localRecords')}>
      <div className="glass-panel p-4 sm:p-5 usage-panel-heading">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.12em] text-tertiary">{t('usage.title')}</p>
          <h2 className="mt-1 text-lg font-semibold text-primary">{t('usage.localTokenActivity')}</h2>
          <p className="mt-1 text-sm text-secondary">{t('usage.localDetail')}</p>
        </div>
        <div className="usage-panel-source">
          <span className="usage-source-chip">{sourceQualityLabel(trend?.source_quality, t)}</span>
          <span className="text-xs text-tertiary">{t('usage.notOfficial')}</span>
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <UsageMetricCard
          label={t('usage.today')}
          icon={Activity}
          usage={detailed?.today ?? null}
          fallbackTokens={usage?.today_tokens}
          accent="primary"
          t={t}
        />
        <UsageMetricCard
          label={t('usage.lastSevenDays')}
          icon={Calendar}
          usage={detailed?.seven_day ?? null}
          fallbackTokens={usage?.seven_day_tokens}
          accent="secondary"
          t={t}
        />
        <UsageMetricCard
          label={t('usage.lifetime')}
          icon={TrendingUp}
          usage={detailed?.lifetime ?? null}
          fallbackTokens={usage?.lifetime_tokens}
          accent="tertiary"
          t={t}
        />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-[minmax(0,1.05fr)_minmax(0,1fr)] gap-3">
        <UsageHeatmap trend={trend} />
        <TrendChart trend={trend} projectedMonthCostUsd={trend?.projected_month_cost_usd} />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
        <TokenBarChart data={usage?.daily_buckets ?? []} />
        <ThreadList threads={usage?.recent_threads ?? []} />
      </div>

      <InferencePerformanceCard performance={usage?.inference_performance ?? null} t={t} />

      <ModelTrendsCard trends={trend?.model_trends ?? null} t={t} />

      <div className="glass-panel px-4 py-3 sm:px-5 usage-panel-note" role="note">
        <Coins size={15} aria-hidden="true" />
        {detailed ? (
          <span>
            {t('usage.estimate', { value: formatUSD(detailed.lifetime.estimated_cost_usd) })}
          </span>
        ) : (
          <span>{t('usage.estimateUnavailable')}</span>
        )}
      </div>
    </section>
  );
}

interface UsageMetricCardProps {
  label: string;
  icon: LucideIcon;
  usage: PricedTokenUsage | null;
  fallbackTokens: number | null | undefined;
  accent: 'primary' | 'secondary' | 'tertiary';
  t: ReturnType<typeof useI18n>['t'];
}

function UsageMetricCard({ label, icon: Icon, usage, fallbackTokens, accent, t }: UsageMetricCardProps) {
  const value = usage ? visibleTotalTokens(usage.tokens) : fallbackTokens;
  const accentClass =
    accent === 'primary'
      ? 'bg-data-primary/20 text-data-primary'
      : accent === 'secondary'
        ? 'bg-data-secondary/20 text-data-secondary'
        : 'bg-data-tertiary/20 text-data-tertiary';

  return (
    <article className="glass-panel p-4 usage-metric-card">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-sm font-medium text-secondary">{label}</p>
          <p className="mt-1 text-2xl font-semibold text-primary tabular-nums">{formatTokens(value)}</p>
        </div>
        <span className={`p-2 rounded-lg border border-current/20 ${accentClass}`} aria-hidden="true">
          <Icon size={17} />
        </span>
      </div>
      <TokenBreakdownBar tokens={usage?.tokens ?? null} t={t} />
    </article>
  );
}

function TokenBreakdownBar({ tokens, t }: { tokens: TokenBreakdown | null; t: ReturnType<typeof useI18n>['t'] }) {
  const segments = splitTokenBreakdown(tokens, t);
  const total = segments.reduce((sum, segment) => sum + segment.value, 0);

  return (
    <div className="mt-4" aria-label={t('usage.tokenBreakdown')}>
      <div className="usage-token-track" aria-hidden="true">
        {total > 0 &&
          segments.map((segment) => (
            <span
              className={`usage-token-segment ${segment.className}`}
              key={segment.label}
              style={{ width: `${(segment.value / total) * 100}%` }}
            />
          ))}
      </div>
      {tokens ? (
        <div className="mt-2 grid grid-cols-3 gap-2 text-[11px] text-tertiary">
          {segments.map((segment) => (
            <span key={segment.label} className="min-w-0 truncate">
              {segment.label} {formatTokens(segment.value)}
            </span>
          ))}
        </div>
      ) : (
        <p className="mt-2 text-[11px] text-tertiary">{t('usage.detailedUnavailable')}</p>
      )}
    </div>
  );
}

function splitTokenBreakdown(
  tokens: TokenBreakdown | null,
  t: ReturnType<typeof useI18n>['t'],
): Array<{ label: string; value: number; className: string }> {
  const cached = Math.min(Math.max(tokens?.cached_input_tokens ?? 0, 0), Math.max(tokens?.input_tokens ?? 0, 0));
  const input = Math.max((tokens?.input_tokens ?? 0) - cached, 0);
  const output = Math.max(tokens?.output_tokens ?? 0, 0);
  return [
    { label: t('usage.input'), value: input, className: 'bg-data-primary' },
    { label: t('usage.cached'), value: cached, className: 'bg-data-secondary' },
    { label: t('usage.output'), value: output, className: 'bg-data-tertiary' },
  ];
}

function visibleTotalTokens(tokens: TokenBreakdown): number {
  return Math.max(tokens.total_tokens, tokens.input_tokens + tokens.output_tokens);
}

function formatTokens(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return '--';
  return Math.round(value).toLocaleString();
}

function formatUSD(value: number): string {
  if (!Number.isFinite(value)) return '--';
  return value.toFixed(2);
}

function sourceQualityLabel(value: 'detailed' | 'approximate' | null | undefined, t: ReturnType<typeof useI18n>['t']): string {
  if (value === 'detailed') return t('usage.detailedEvents');
  if (value === 'approximate') return t('usage.threadFallback');
  return t('usage.noSourceYet');
}

interface InferencePerformanceCardProps {
  performance: InferencePerformance | null;
  t: ReturnType<typeof useI18n>['t'];
}

function InferencePerformanceCard({ performance, t }: InferencePerformanceCardProps) {
  const models = performance?.models ?? [];
  return (
    <section className="glass-panel p-4 sm:p-5" aria-label={t('usage.inferenceTitle')}>
      <div className="flex items-start justify-between gap-3 mb-4">
        <div>
          <h3 className="text-sm font-semibold text-primary">{t('usage.inferenceTitle')}</h3>
          <p className="mt-0.5 text-xs text-tertiary">{t('usage.inferenceEmptyDetail')}</p>
        </div>
        {performance?.refreshed_at ? (
          <span className="shrink-0 text-xs text-tertiary tabular-nums">
            {new Date(performance.refreshed_at).toLocaleString()}
          </span>
        ) : null}
      </div>
      {models.length === 0 ? (
        <p className="text-sm text-secondary">{t('usage.inferenceEmpty')}</p>
      ) : (
        <div className="space-y-3">
          {models.map((model, index) => (
            <div key={index} className="rounded-xl border border-theme bg-surface-inset p-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <p className="text-sm font-medium text-primary truncate">
                  {model.model ?? t('common.unknown')}
                  {model.effort ? (
                    <span className="ml-1.5 text-xs text-tertiary capitalize">({model.effort})</span>
                  ) : null}
                </p>
                <span className="text-xs text-secondary tabular-nums">
                  {formatTokens(model.call_count)} {t('usage.inferenceCalls')}
                </span>
              </div>
              <div className="mt-2 grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-5 gap-2 text-xs">
                <Stat label={t('usage.inferenceAvgDuration')} value={formatDuration(model.average_duration_ms)} />
                <Stat label="P50" value={formatDuration(model.p50_duration_ms)} />
                <Stat label="P90" value={formatDuration(model.p90_duration_ms)} />
                <Stat label={t('usage.inferenceThroughput')} value={formatTokens(model.average_tokens_per_second)} />
                <Stat label={t('usage.inferenceReasoningRatio')} value={formatPercent(model.reasoning_output_ratio)} />
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-surface-elevated px-2.5 py-1.5">
      <p className="text-[10px] uppercase tracking-wide text-tertiary truncate">{label}</p>
      <p className="mt-0.5 text-sm font-medium text-primary tabular-nums">{value}</p>
    </div>
  );
}

interface ModelTrendsCardProps {
  trends: ModelUsageTrend[] | null;
  t: ReturnType<typeof useI18n>['t'];
}

function ModelTrendsCard({ trends, t }: ModelTrendsCardProps) {
  const models = [...(trends ?? [])]
    .filter((model) => model.model != null)
    .sort((a, b) => visibleTotalTokens(b.summary.seven_day.tokens) - visibleTotalTokens(a.summary.seven_day.tokens))
    .slice(0, 8);

  return (
    <section className="glass-panel p-4 sm:p-5" aria-label={t('usage.modelTrendsTitle')}>
      <h3 className="text-sm font-semibold text-primary mb-1">{t('usage.modelTrendsTitle')}</h3>
      <p className="text-xs text-tertiary mb-4">{t('usage.modelTrendsEmptyDetail')}</p>
      {models.length === 0 ? (
        <p className="text-sm text-secondary">{t('usage.modelTrendsEmpty')}</p>
      ) : (
        <div className="space-y-3">
          {models.map((model) => (
            <div key={model.id} className="flex items-center gap-3">
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium text-primary truncate">{model.model}</p>
                <p className="mt-0.5 text-xs text-tertiary tabular-nums">
                  {t('usage.activeDays', { value: model.active_day_count })} · {t('usage.dailyAverage')}{' '}
                  {formatTokens(model.summary.daily_average_tokens)}
                </p>
              </div>
              <ModelSparkline values={lastSevenDayTokens(model.day_buckets)} />
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function ModelSparkline({ values }: { values: number[] }) {
  const width = 120;
  const height = 36;
  if (values.length === 0) {
    return <span className="shrink-0 text-xs text-tertiary">--</span>;
  }
  const max = Math.max(...values, 1);
  const stepX = width / Math.max(values.length - 1, 1);
  const linePoints = values
    .map((value, index) => {
      const x = (index * stepX).toFixed(1);
      const y = (height - (value / max) * height).toFixed(1);
      return `${x},${y}`;
    })
    .join(' ');
  const areaPoints = `0,${height} ${linePoints} ${width},${height}`;
  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="shrink-0" aria-hidden="true">
      <polyline points={areaPoints} fill="var(--data-secondary)" opacity={0.18} />
      <polyline
        points={linePoints}
        fill="none"
        stroke="var(--data-secondary)"
        strokeWidth={1.5}
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
}

function lastSevenDayTokens(buckets: UsageDayBucket[]): number[] {
  const cutoff = Date.now() - 7 * 24 * 60 * 60 * 1000;
  return buckets
    .filter((bucket) => bucket.date >= cutoff)
    .map((bucket) => visibleTotalTokens(bucket.usage.tokens));
}

function formatDuration(ms: number): string {
  if (!Number.isFinite(ms)) return '--';
  if (ms >= 60000) return `${(ms / 60000).toFixed(1)} min`;
  return `${(ms / 1000).toFixed(1)} s`;
}

function formatPercent(ratio: number): string {
  if (!Number.isFinite(ratio)) return '--';
  return `${(ratio * 100).toFixed(0)}%`;
}
