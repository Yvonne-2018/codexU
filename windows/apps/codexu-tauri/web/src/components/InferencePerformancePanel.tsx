import { Activity, Gauge, Info } from 'lucide-react';
import { useMemo, useState, type KeyboardEvent } from 'react';
import { useI18n } from '../i18n/I18nProvider';
import type {
  InferencePerformanceGroup,
  InferencePerformanceHistory,
  InferencePerformancePeriod,
} from '../types/models';

type PeriodKey = 'today' | 'seven_days' | 'twenty_eight_days';

const PERIODS: Array<{ id: PeriodKey; labelKey: 'inference.today' | 'inference.sevenDays' | 'inference.twentyEightDays' }> = [
  { id: 'today', labelKey: 'inference.today' },
  { id: 'seven_days', labelKey: 'inference.sevenDays' },
  { id: 'twenty_eight_days', labelKey: 'inference.twentyEightDays' },
];
const GRID_FRACTIONS = [0, 0.25, 0.5, 0.75, 1] as const;
const OUTLIER_COMPRESSION_RATIO = 1.6;
const OUTLIER_Y_OFFSET = 24;
const OUTLIER_LANE_HEIGHT = 44;
const OUTLIER_REGULAR_TOP_OFFSET = 64;
const MODEL_COLOR_SLOT_COUNT = 9;
const FNV_OFFSET_BASIS_64 = BigInt('14695981039346656037');
const FNV_PRIME_64 = BigInt('1099511628211');

interface InferencePerformancePanelProps {
  inference: InferencePerformanceHistory | null;
}

export function InferencePerformancePanel({ inference }: InferencePerformancePanelProps) {
  const { t } = useI18n();
  const [periodKey, setPeriodKey] = useState<PeriodKey>('today');
  const period = inference?.[periodKey] ?? null;
  const groups = period?.groups ?? [];

  return (
    <section className="glass-panel p-4 inference-performance-panel" aria-label={t('inference.ariaLabel')}>
      <div className="usage-panel-heading">
        <div className="dashboard-overview-heading">
          <span className="dashboard-overview-icon">
            <Gauge size={16} />
          </span>
          <div>
            <h3>{t('inference.title')}</h3>
            <p>{t('inference.subtitle')}</p>
          </div>
        </div>
        <div className="inference-period-switcher" role="tablist" aria-label={t('inference.periodSelector')}>
          {PERIODS.map((period) => (
            <button
              key={period.id}
              type="button"
              role="tab"
              aria-selected={periodKey === period.id}
              className={periodKey === period.id ? 'glass-button-solid' : 'glass-button'}
              onClick={() => setPeriodKey(period.id)}
            >
              {t(period.labelKey)}
            </button>
          ))}
        </div>
      </div>

      <div className="inference-help">
        <Info size={14} />
        <span>{t('inference.tooltipBasis')}</span>
      </div>

      {!period || groups.length === 0 ? (
        <div className="inference-empty" role="status">
          <Activity size={18} />
          <div>
            <strong>{t('inference.empty')}</strong>
            <p>{t('inference.emptyDetail')}</p>
          </div>
        </div>
      ) : (
        <InferenceScatterPlot period={period} />
      )}
    </section>
  );
}

function InferenceScatterPlot({ period }: { period: InferencePerformancePeriod }) {
  const { t } = useI18n();
  const [hoveredGroupId, setHoveredGroupId] = useState<string | null>(null);
  const [pinnedGroupId, setPinnedGroupId] = useState<string | null>(null);
  const groups = useMemo(
    () =>
      [...period.groups].sort((a, b) =>
        b.call_count !== a.call_count
          ? b.call_count - a.call_count
          : a.p50_duration_seconds - b.p50_duration_seconds,
      ),
    [period.groups],
  );
  const maxP90 = Math.max(1, ...groups.map((group) => group.p90_duration_seconds)) * 1.4;
  const throughputScale = resolveThroughputScale(groups);
  const maxBubble = Math.max(1, ...groups.map((group) => bubbleBasis(group, period)));
  const plotWidth = 720;
  const plotHeight = 320;
  const left = 54;
  const right = 24;
  const top = 18;
  const bottom = 42;
  const width = plotWidth - left - right;
  const height = plotHeight - top - bottom;
  const outlierY = top + OUTLIER_Y_OFFSET;
  const regularTop = throughputScale.outlierId === null ? top : top + OUTLIER_REGULAR_TOP_OFFSET;
  const regularHeight = top + height - regularTop;
  const outlierGroup =
    throughputScale.outlierId === null ? null : groups.find((group) => group.id === throughputScale.outlierId) ?? null;
  const activeGroupId = pinnedGroupId ?? hoveredGroupId;
  const activeGroup = groups.find((group) => group.id === activeGroupId) ?? null;
  const tooltipId = activeGroup ? `inference-tooltip-${safeDomId(activeGroup.id)}` : undefined;

  const x = (value: number) => left + clamp(value / maxP90) * width;
  const y = (group: InferencePerformanceGroup) =>
    group.id === throughputScale.outlierId
      ? outlierY
      : regularTop + (1 - clamp(group.effective_output_tokens_per_second / throughputScale.regularMax)) * regularHeight;

  const togglePinned = (groupId: string) => {
    setPinnedGroupId((current) => (current === groupId ? null : groupId));
  };
  const handlePointKeyDown = (event: KeyboardEvent<SVGGElement>, groupId: string) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      togglePinned(groupId);
    } else if (event.key === 'Escape') {
      setPinnedGroupId(null);
      setHoveredGroupId(null);
    }
  };

  return (
    <div className="inference-chart-wrap">
      <div className="inference-chart-stage">
        <svg
          className="inference-scatter"
          viewBox={`0 0 ${plotWidth} ${plotHeight}`}
          role="group"
          aria-label={t('inference.chartAria')}
          data-inference-scale-mode={throughputScale.outlierId === null ? 'continuous' : 'single-outlier'}
        >
          {outlierGroup && (
            <>
              <rect
                x={left}
                y={top}
                width={width}
                height={OUTLIER_LANE_HEIGHT}
                rx={10}
                className="inference-outlier-lane"
                aria-hidden="true"
              />
              <line
                x1={left}
                y1={outlierY}
                x2={left + width}
                y2={outlierY}
                className="inference-outlier-guide"
                aria-hidden="true"
              />
              <line
                x1={left - 4}
                y1={outlierY}
                x2={left + 4}
                y2={outlierY}
                className="inference-outlier-tick-mark"
                aria-hidden="true"
              />
              <text
                x={left - 8}
                y={outlierY + 3}
                textAnchor="end"
                className="inference-outlier-tick"
                data-inference-outlier-tick
              >
                {formatNumber(outlierGroup.effective_output_tokens_per_second)}
              </text>
              <g data-inference-axis-break aria-hidden="true">
                <line
                  x1={left - 5}
                  y1={regularTop - 8}
                  x2={left + 3}
                  y2={regularTop - 16}
                  className="inference-axis-break-line"
                />
                <line
                  x1={left + 2}
                  y1={regularTop - 8}
                  x2={left + 10}
                  y2={regularTop - 16}
                  className="inference-axis-break-line"
                />
              </g>
            </>
          )}
          {GRID_FRACTIONS.map((fraction) => {
            const gridX = left + fraction * width;
            return (
              <g key={`grid-x-${fraction}`}>
                <line
                  x1={gridX}
                  y1={top}
                  x2={gridX}
                  y2={top + height}
                  className="inference-grid-line"
                />
                <text
                  x={gridX}
                  y={top + height + 16}
                  textAnchor="middle"
                  className="inference-duration-tick"
                  data-inference-x-tick
                >
                  {formatDurationAxisTick(maxP90 * fraction)}
                </text>
              </g>
            );
          })}
          {GRID_FRACTIONS.map((fraction) => {
            const gridY = regularTop + fraction * regularHeight;
            return (
              <g key={`grid-y-${fraction}`}>
                <line
                  x1={left}
                  y1={gridY}
                  x2={left + width}
                  y2={gridY}
                  className="inference-grid-line"
                  data-inference-horizontal-grid
                />
                <line
                  x1={left - 4}
                  y1={gridY}
                  x2={left + 4}
                  y2={gridY}
                  className="inference-regular-tick-mark"
                  aria-hidden="true"
                />
                <text
                  x={left - 8}
                  y={gridY + 3}
                  textAnchor="end"
                  className="inference-regular-tick"
                  data-inference-regular-tick
                >
                  {formatThroughputTick(throughputScale.regularMax * (1 - fraction))}
                </text>
              </g>
            );
          })}

          <text x={left + width - 150} y={plotHeight - 12} className="inference-axis-label">
            {t('inference.xAxis')}
          </text>
          <text x={10} y={top + 8} className="inference-axis-label">
            tok/s
          </text>

          {groups.map((group) => {
            const medianX = x(group.p50_duration_seconds);
            const p90X = x(group.p90_duration_seconds);
            const pointY = y(group);
            const radius = 5 + Math.sqrt(bubbleBasis(group, period) / maxBubble) * 11;
            const color = modelColor(group.model);
            const label = `${group.model} · ${formatEffort(group.effort)}`;
            const isOutlier = group.id === throughputScale.outlierId;
            const labelReserve = isOutlier ? 156 : 130;
            return (
              <g
                key={group.id}
                className="inference-scatter-point"
                data-inference-group-id={group.id}
                data-inference-outlier={isOutlier ? 'true' : undefined}
                role="button"
                tabIndex={0}
                aria-label={`${label}. ${t('inference.tooltipCalls')}: ${group.call_count.toLocaleString()}. ${t(
                  'inference.tooltipAverageDuration',
                )}: ${formatDuration(group.average_duration_seconds)}.`}
                aria-describedby={activeGroupId === group.id ? tooltipId : undefined}
                aria-pressed={pinnedGroupId === group.id}
                onMouseEnter={() => setHoveredGroupId(group.id)}
                onMouseLeave={() => setHoveredGroupId((current) => (current === group.id ? null : current))}
                onFocus={() => setHoveredGroupId(group.id)}
                onBlur={() => setHoveredGroupId((current) => (current === group.id ? null : current))}
                onClick={() => togglePinned(group.id)}
                onKeyDown={(event) => handlePointKeyDown(event, group.id)}
              >
                <line
                  x1={medianX}
                  y1={pointY}
                  x2={p90X}
                  y2={pointY}
                  className="inference-whisker"
                  style={{ stroke: color }}
                />
                <line
                  x1={p90X}
                  y1={pointY - 5}
                  x2={p90X}
                  y2={pointY + 5}
                  className="inference-whisker"
                  style={{ stroke: color }}
                />
                <circle cx={medianX} cy={pointY} r={radius} className="inference-bubble" style={{ fill: color }} />
                <circle
                  cx={medianX}
                  cy={pointY}
                  r={Math.max(18, radius + 8)}
                  className="inference-scatter-point-hit"
                />
                <text
                  x={Math.min(p90X + 8, left + width - labelReserve)}
                  y={pointY + 3}
                  className={`inference-point-label${isOutlier ? ' inference-point-label-outlier' : ''}`}
                >
                  {shortModelName(group.model)} · {formatEffort(group.effort)}
                </text>
              </g>
            );
          })}
        </svg>

        {activeGroup && tooltipId ? (
          <div id={tooltipId} role="tooltip" className="inference-point-tooltip">
            <div className="inference-point-tooltip-heading">
              <strong>{activeGroup.model}</strong>
              <span>{formatEffort(activeGroup.effort)}</span>
            </div>
            <dl className="inference-point-tooltip-grid">
              <div>
                <dt>{t('inference.tooltipCalls')}</dt>
                <dd>{activeGroup.call_count.toLocaleString()}</dd>
              </div>
              <div>
                <dt>{t('inference.tooltipDailyCalls')}</dt>
                <dd>{formatNumber(activeGroup.average_daily_call_count)}</dd>
              </div>
              <div>
                <dt>{t('inference.tooltipAverageDuration')}</dt>
                <dd>{formatDuration(activeGroup.average_duration_seconds)}</dd>
              </div>
              <div>
                <dt>{t('inference.tooltipLatency')}</dt>
                <dd>
                  {formatDuration(activeGroup.p50_duration_seconds)} / {formatDuration(activeGroup.p90_duration_seconds)}
                </dd>
              </div>
              <div>
                <dt>{t('inference.tooltipThroughput')}</dt>
                <dd>{formatNumber(activeGroup.effective_output_tokens_per_second)} tok/s</dd>
              </div>
              <div>
                <dt>{t('inference.tooltipOutputTokens')}</dt>
                <dd>{activeGroup.output_tokens.toLocaleString()}</dd>
              </div>
              <div>
                <dt>{t('inference.tooltipReasoningTokens')}</dt>
                <dd>
                  {activeGroup.reasoning_output_tokens.toLocaleString()} · {formatPercent(reasoningShare(activeGroup))}
                </dd>
              </div>
            </dl>
            <p>{t('inference.tooltipScope')}</p>
          </div>
        ) : null}
      </div>

      <div className="inference-summary-grid">
        <div>
          <span>{t('inference.totalCalls')}</span>
          <strong>{period.total_call_count.toLocaleString()}</strong>
        </div>
        <div>
          <span>{t('inference.coverage')}</span>
          <strong>{period.coverage_day_count.toLocaleString()}</strong>
        </div>
        <div>
          <span>{t('inference.groups')}</span>
          <strong>{groups.length.toLocaleString()}</strong>
        </div>
      </div>
    </div>
  );
}

function bubbleBasis(group: InferencePerformanceGroup, period: InferencePerformancePeriod): number {
  return period.period === 'today' ? group.call_count : group.average_daily_call_count;
}

function modelColor(model: string): string {
  return `var(--data-model-${stableModelColorSlot(model) + 1})`;
}

function stableModelColorSlot(model: string): number {
  let hash = FNV_OFFSET_BASIS_64;
  for (const byte of new TextEncoder().encode(model)) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * FNV_PRIME_64);
  }
  return Number(hash % BigInt(MODEL_COLOR_SLOT_COUNT));
}

function resolveThroughputScale(groups: InferencePerformanceGroup[]): { outlierId: string | null; regularMax: number } {
  const byThroughput = [...groups].sort(
    (a, b) => b.effective_output_tokens_per_second - a.effective_output_tokens_per_second,
  );
  const highest = byThroughput[0];
  const secondHighest = byThroughput[1];
  const hasSingleOutlier =
    byThroughput.length >= 3 &&
    secondHighest.effective_output_tokens_per_second > 0 &&
    highest.effective_output_tokens_per_second >=
      secondHighest.effective_output_tokens_per_second * OUTLIER_COMPRESSION_RATIO;
  const regularGroups = hasSingleOutlier ? byThroughput.slice(1) : byThroughput;

  return {
    outlierId: hasSingleOutlier ? highest.id : null,
    regularMax: Math.max(1, ...regularGroups.map((group) => group.effective_output_tokens_per_second)) * 1.16,
  };
}

function safeDomId(value: string): string {
  return value.replace(/[^A-Za-z0-9_-]+/g, '-');
}

function clamp(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, value));
}

function shortModelName(model: string): string {
  return model.replace(/^gpt-/, '');
}

function formatEffort(effort: string): string {
  return effort.length > 0 ? `${effort[0].toUpperCase()}${effort.slice(1)}` : effort;
}

function formatDuration(seconds: number): string {
  return seconds < 10 ? `${seconds.toFixed(1)}s` : `${Math.round(seconds)}s`;
}

function formatNumber(value: number): string {
  return value < 10 ? value.toFixed(1) : Math.round(value).toLocaleString();
}

function formatThroughputTick(value: number): string {
  return Math.round(value).toLocaleString();
}

function formatDurationAxisTick(value: number): string {
  if (value === 0) return '0';
  return value < 10 ? `${value.toFixed(1)}s` : `${Math.round(value).toLocaleString()}s`;
}

function reasoningShare(group: InferencePerformanceGroup): number {
  return group.output_tokens > 0 ? group.reasoning_output_tokens / group.output_tokens : 0;
}

function formatPercent(value: number): string {
  return `${Math.round(clamp(value) * 100)}%`;
}
