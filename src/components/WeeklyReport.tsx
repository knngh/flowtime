import React, { useState, useEffect, useMemo } from 'react';
import { getWeeklyReport, getDailySummary } from '../review';
import type { WeeklyReport, DailySummary, TimeDistributionItem, HighRiskTask } from '../types';

function secsToHms(totalSecs: number): string {
  const h = Math.floor(totalSecs / 3600);
  const m = Math.floor((totalSecs % 3600) / 60);
  if (h > 0) return `${h}h${m}min`;
  return `${m}min`;
}

// Pure SVG donut chart — no chart library needed
function DonutChart({
  items,
  size = 140,
}: {
  items: TimeDistributionItem[];
  size?: number;
}) {
  const total = items.reduce((s, it) => s + it.total_seconds, 0);
  if (total === 0) return <p className="text-xs text-gray-400">暂无数据</p>;

  const colors: Record<string, string> = {
    coding: '#3B82F6',
    meeting: '#F59E0B',
    communication: '#10B981',
    other: '#9CA3AF',
  };

  let cumAngle = 0;
  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2 - 12;
  const strokeW = 18;

  const slices = items.map((it) => {
    const pct = it.total_seconds / total;
    const angle = pct * 360;
    const startAngle = cumAngle;
    cumAngle += angle;
    const endAngle = cumAngle;

    const rad = (a: number) => ((a - 90) * Math.PI) / 180;
    const x1 = cx + r * Math.cos(rad(startAngle));
    const y1 = cy + r * Math.sin(rad(startAngle));
    const x2 = cx + r * Math.cos(rad(endAngle));
    const y2 = cy + r * Math.sin(rad(endAngle));
    const largeArc = angle > 180 ? 1 : 0;

    const d =
      pct >= 1
        ? `M ${cx + r},${cy} A ${r},${r} 0 1,1 ${cx + r - 0.001},${cy} Z`
        : `M ${x1},${y1} A ${r},${r} 0 ${largeArc},1 ${x2},${y2}`;

    return {
      ...it,
      d,
      color: colors[it.category] ?? '#9CA3AF',
      pct: (pct * 100).toFixed(1),
    };
  });

  return (
    <div className="flex items-center gap-4">
      <svg width={size} height={size} className="shrink-0">
        {slices.map((s, i) => (
          <path
            key={i}
            d={s.d}
            fill={s.color}
            stroke="#1f2937"
            strokeWidth={1}
            opacity={0.85}
          />
        ))}
        {/* center hole */}
        <circle cx={cx} cy={cy} r={r - strokeW} fill="#111827" />
        <text
          x={cx}
          y={cy + 4}
          textAnchor="middle"
          className="fill-gray-200 text-xs font-semibold"
        >
          {secsToHms(total)}
        </text>
      </svg>
      <ul className="space-y-1 text-xs">
        {slices.map((s) => (
          <li key={s.category} className="flex items-center gap-1.5">
            <span
              className="inline-block w-2.5 h-2.5 rounded-full shrink-0"
              style={{ backgroundColor: s.color }}
            />
            <span className="text-gray-300">{categoryLabel(s.category)}</span>
            <span className="text-gray-500 ml-auto">{s.pct}%</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function categoryLabel(cat: string): string {
  const m: Record<string, string> = {
    coding: '编码',
    meeting: '会议',
    communication: '沟通',
    other: '其他',
  };
  return m[cat] ?? cat;
}

function TrendBadge({
  current,
  previous,
  inverse = false,
}: {
  current: number;
  previous: number;
  inverse?: boolean;
}) {
  if (previous === 0) return <span className="text-gray-500 text-xs">—</span>;
  const diff = current - previous;
  const better = inverse ? diff < 0 : diff > 0;
  const color = diff === 0 ? 'text-gray-400' : better ? 'text-green-400' : 'text-red-400';
  const arrow = diff === 0 ? '→' : better ? '↑' : '↓';
  const pct = ((diff / Math.abs(previous)) * 100).toFixed(0);
  return (
    <span className={`${color} text-xs ml-1`}>
      {arrow} {Math.abs(Number(pct))}%
    </span>
  );
}

interface Props {
  onClose: () => void;
}

export default function WeeklyReport({ onClose }: Props) {
  const [weekOffset, setWeekOffset] = useState(0);
  const [report, setReport] = useState<WeeklyReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'week' | 'today'>('week');
  const [daily, setDaily] = useState<DailySummary | null>(null);

  const weekStart = useMemo(() => {
    const d = new Date();
    d.setDate(d.getDate() - ((d.getDay() + 6) % 7) - weekOffset * 7);
    return d.toISOString().split('T')[0];
  }, [weekOffset]);

  useEffect(() => {
    setLoading(true);
    getWeeklyReport(weekStart)
      .then(setReport)
      .finally(() => setLoading(false));
  }, [weekStart]);

  useEffect(() => {
    if (activeTab !== 'today') return;
    const today = new Date().toISOString().split('T')[0];
    getDailySummary(today).then(setDaily);
  }, [activeTab]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-gray-900 border border-gray-700 rounded-2xl shadow-2xl w-[820px] max-h-[90vh] overflow-y-auto p-6 text-gray-100">
        {/* Header */}
        <div className="flex items-center justify-between mb-5">
          <h2 className="text-lg font-bold tracking-tight">📊 复盘看板</h2>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-white text-xl leading-none"
          >
            ×
          </button>
        </div>

        {/* Tabs */}
        <div className="flex gap-1 mb-5 bg-gray-800 rounded-lg p-1 w-fit">
          {(['week', 'today'] as const).map((t) => (
            <button
              key={t}
              onClick={() => setActiveTab(t)}
              className={`px-3 py-1 rounded-md text-sm transition ${
                activeTab === t
                  ? 'bg-blue-600 text-white'
                  : 'text-gray-400 hover:text-white'
              }`}
            >
              {t === 'week' ? '本周报告' : '今日小结'}
            </button>
          ))}
        </div>

        {/* Week navigation */}
        {activeTab === 'week' && (
          <div className="flex items-center gap-3 mb-5">
            <button
              onClick={() => setWeekOffset((w) => w + 1)}
              className="px-2 py-1 rounded bg-gray-800 hover:bg-gray-700 text-sm"
            >
              ← 上周
            </button>
            <span className="text-sm text-gray-300">
              {report ? `${report.week_start} ~ ${report.week_end}` : '...'}
            </span>
            <button
              onClick={() => setWeekOffset((w) => Math.max(0, w - 1))}
              disabled={weekOffset === 0}
              className="px-2 py-1 rounded bg-gray-800 hover:bg-gray-700 text-sm disabled:opacity-40"
            >
              下周 →
            </button>
          </div>
        )}

        {loading && (
          <div className="text-center py-10 text-gray-500">加载中...</div>
        )}

        {/* ── Weekly Report ── */}
        {!loading && activeTab === 'week' && report && (
          <div className="space-y-6">
            {/* KPI cards */}
            <div className="grid grid-cols-3 gap-4">
              <KpiCard
                label="深度工作时间"
                value={secsToHms(report.total_focus_seconds)}
                sub={
                  <TrendBadge
                    current={report.total_focus_seconds}
                    previous={report.prev_week_focus_seconds}
                  />
                }
              />
              <KpiCard
                label="计划完成率"
                value={`${(report.completion_rate * 100).toFixed(0)}%`}
                sub={
                  <TrendBadge
                    current={report.completion_rate}
                    previous={report.prev_week_completion_rate}
                  />
                }
              />
              <KpiCard
                label="日均打断次数"
                value={String(report.avg_interruptions_per_day.toFixed(1))}
              />
            </div>

            {/* Time distribution + Focus sessions */}
            <div className="grid grid-cols-2 gap-4">
              <div className="bg-gray-800/60 rounded-xl p-4">
                <h3 className="text-sm font-semibold text-gray-300 mb-3">
                  时间分布
                </h3>
                <DonutChart items={report.time_distribution} />
              </div>
              <div className="bg-gray-800/60 rounded-xl p-4">
                <h3 className="text-sm font-semibold text-gray-300 mb-3">
                  专注会话
                </h3>
                <p className="text-3xl font-bold text-blue-400">
                  {report.focus_sessions_count}
                </p>
                <p className="text-xs text-gray-500 mt-1">次 / 周</p>
                <div className="mt-4 space-y-1 text-xs text-gray-400">
                  <p>
                    追踪总时长：
                    <span className="text-gray-200">
                      {secsToHms(report.total_tracked_seconds)}
                    </span>
                  </p>
                </div>
              </div>
            </div>

            {/* High-risk tasks */}
            {report.high_risk_tasks.length > 0 && (
              <div className="bg-red-900/20 border border-red-800/50 rounded-xl p-4">
                <h3 className="text-sm font-semibold text-red-400 mb-2">
                  ⚠️ 高风险任务（连续推迟 ≥3 天）
                </h3>
                <ul className="space-y-1">
                  {report.high_risk_tasks.map((t: HighRiskTask) => (
                    <li
                      key={t.id}
                      className="flex items-center gap-2 text-sm text-red-300"
                    >
                      <span className="w-1.5 h-1.5 rounded-full bg-red-500 shrink-0" />
                      {t.title}
                      <span className="text-xs text-red-500 ml-auto">
                        {t.status === 'deferred' ? '已推迟' : t.status}
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}

        {/* ── Daily Summary ── */}
        {activeTab === 'today' && (
          <div className="space-y-5">
            {!daily && (
              <div className="text-center py-10 text-gray-500">加载中...</div>
            )}
            {daily && (
              <>
                <div className="grid grid-cols-3 gap-4">
                  <KpiCard
                    label="今日深度工作"
                    value={secsToHms(daily.total_focus_seconds)}
                  />
                  <KpiCard
                    label="任务完成率"
                    value={`${(daily.completion_rate * 100).toFixed(0)}%`}
                  />
                  <KpiCard
                    label="打断次数"
                    value={String(daily.interruptions_blocked)}
                  />
                </div>
                <div className="bg-gray-800/60 rounded-xl p-4">
                  <h3 className="text-sm font-semibold text-gray-300 mb-3">
                    今日时间分布
                  </h3>
                  <DonutChart items={daily.time_distribution} size={120} />
                </div>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function KpiCard({
  label,
  value,
  sub,
}: {
  label: string;
  value: string;
  sub?: React.ReactNode;
}) {
  return (
    <div className="bg-gray-800/60 rounded-xl p-4 flex flex-col gap-1">
      <p className="text-xs text-gray-500 uppercase tracking-wide">{label}</p>
      <p className="text-2xl font-bold text-white">{value}</p>
      {sub && <div className="mt-0.5">{sub}</div>}
    </div>
  );
}
