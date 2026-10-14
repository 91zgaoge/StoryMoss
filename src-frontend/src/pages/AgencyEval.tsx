import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import { getEvalOverview, listCheckpoints, compareCheckpoints } from '@/services/api/agency';
import type { GateHistoryItem, PurposeUsage } from '@/services/api/agency';
import { AiDiffTable } from '@/components/ui/ai/AiDiffTable';
import { AiInsightCards } from '@/components/ui/ai/AiInsightCards';
import { AiRecordsTable, type AiRecordsSort } from '@/components/ui/ai/AiRecordsTable';

/** 解析 checkpoint metrics_json；key 与后端 agency/coordinator.rs compare_checkpoints 对齐
 *  （words_total / chapters_done / tokens_used / gate_scores 末条 weighted）。解析失败回退 null。 */
function parseCheckpointMetrics(json: string): Record<string, unknown> | null {
  try {
    return JSON.parse(json) as Record<string, unknown>;
  } catch {
    return null;
  }
}

/** 判定结果徽章：pass 绿 / revise 橙 / 其他红（color-mix tint，零扩令牌） */
function OutcomePill({ outcome }: { outcome: string }) {
  const color =
    outcome === 'pass'
      ? 'var(--ai-green)'
      : outcome === 'revise'
        ? 'var(--ai-orange)'
        : 'var(--ai-red)';
  return (
    <span
      className="inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium"
      style={{ color, background: `color-mix(in srgb, ${color} 12%, transparent)` }}
    >
      {outcome}
    </span>
  );
}

function metricNumber(m: Record<string, unknown> | null, key: string): number | null {
  const v = m?.[key];
  return typeof v === 'number' ? v : null;
}

function metricWeighted(m: Record<string, unknown> | null): number | null {
  const scores = m?.gate_scores;
  if (!Array.isArray(scores) || scores.length === 0) return null;
  const last = scores[scores.length - 1] as Record<string, unknown> | undefined;
  return typeof last?.weighted === 'number' ? last.weighted : null;
}

function GateTrendChart({ data }: { data: GateHistoryItem[] }) {
  const points = data.filter(d => d.weighted != null);
  if (points.length === 0) return <p className="text-sm text-ai-ink-3">暂无评分数据</p>;
  const w = 560;
  const h = 160;
  const pad = 28;
  const maxX = Math.max(points.length - 1, 1);
  const x = (i: number) => pad + (i / maxX) * (w - pad * 2);
  const y = (v: number) => h - pad - v * (h - pad * 2);
  const pathD = points
    .map((p, i) => `${i === 0 ? 'M' : 'L'}${x(i).toFixed(1)},${y(p.weighted!).toFixed(1)}`)
    .join(' ');
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="w-full max-w-2xl">
      <line
        x1={pad}
        y1={y(0.75)}
        x2={w - pad}
        y2={y(0.75)}
        stroke="var(--ai-orange)"
        strokeDasharray="4"
      />
      <text x={w - pad + 2} y={y(0.75)} fontSize="10" fill="var(--ai-orange)">
        0.75
      </text>
      <path d={pathD} fill="none" stroke="var(--ai-accent)" strokeWidth="2" />
      {points.map((p, i) => (
        <circle
          key={i}
          cx={x(i)}
          cy={y(p.weighted!)}
          r="3"
          fill={
            p.outcome === 'pass'
              ? 'var(--ai-green)'
              : p.outcome === 'revise'
                ? 'var(--ai-orange)'
                : 'var(--ai-red)'
          }
        />
      ))}
    </svg>
  );
}

function CheckpointCompare({ storyId }: { storyId: string }) {
  const { data: checkpoints } = useQuery({
    queryKey: ['agency-checkpoints', storyId],
    queryFn: () => listCheckpoints(storyId),
    enabled: !!storyId,
  });
  const [a, setA] = useState('');
  const [b, setB] = useState('');
  const { data: diff } = useQuery({
    queryKey: ['agency-checkpoint-diff', a, b],
    queryFn: () => compareCheckpoints(a, b),
    enabled: !!a && !!b && a !== b,
  });
  if (!checkpoints || checkpoints.length < 2) return null;
  return (
    <section>
      <h2 className="mb-2 font-medium">检查点对比</h2>
      <div className="flex gap-2">
        <select
          value={a}
          onChange={e => setA(e.target.value)}
          className="rounded border border-ai-line bg-ai-field px-2 py-1 text-sm text-ai-ink"
        >
          <option value="">基准…</option>
          {checkpoints.map(c => (
            <option key={c.id} value={c.id}>
              {c.milestone}
              {c.chapter_number != null ? ` · 第${c.chapter_number}章` : ''} ·{' '}
              {c.created_at.slice(0, 16)}
            </option>
          ))}
        </select>
        <select
          value={b}
          onChange={e => setB(e.target.value)}
          className="rounded border border-ai-line bg-ai-field px-2 py-1 text-sm text-ai-ink"
        >
          <option value="">对比…</option>
          {checkpoints.map(c => (
            <option key={c.id} value={c.id}>
              {c.milestone}
              {c.chapter_number != null ? ` · 第${c.chapter_number}章` : ''} ·{' '}
              {c.created_at.slice(0, 16)}
            </option>
          ))}
        </select>
      </div>
      {diff && (
        <AiDiffTable
          className="mt-2"
          title="指标对比"
          rows={(() => {
            const ma = parseCheckpointMetrics(
              checkpoints.find(c => c.id === a)?.metrics_json ?? ''
            );
            const mb = parseCheckpointMetrics(
              checkpoints.find(c => c.id === b)?.metrics_json ?? ''
            );
            const fmt = (v: number | null) => (v === null ? '—' : String(v));
            const fmtW = (v: number | null) => (v === null ? '—' : v.toFixed(2));
            return [
              {
                key: 'words',
                label: '字数',
                base: fmt(metricNumber(ma, 'words_total')),
                compare: fmt(metricNumber(mb, 'words_total')),
                delta: diff.words_delta,
              },
              {
                key: 'chapters',
                label: '章节',
                base: fmt(metricNumber(ma, 'chapters_done')),
                compare: fmt(metricNumber(mb, 'chapters_done')),
                delta: diff.chapters_delta,
              },
              {
                key: 'tokens',
                label: 'tokens',
                base: fmt(metricNumber(ma, 'tokens_used')),
                compare: fmt(metricNumber(mb, 'tokens_used')),
                delta: diff.tokens_delta,
                betterWhen: 'lower' as const,
              },
              {
                key: 'weighted',
                label: '加权分',
                base: fmtW(metricWeighted(ma)),
                compare: fmtW(metricWeighted(mb)),
                delta: diff.gate_weighted_delta,
                formatDelta: (d: number) => `${d >= 0 ? '+' : ''}${d.toFixed(2)}`,
              },
            ];
          })()}
        />
      )}
    </section>
  );
}

export default function AgencyEval() {
  const currentStory = useAppStore(s => s.currentStory);
  const [storyId] = useState(currentStory?.id ?? '');
  const { data, isLoading, error } = useQuery({
    queryKey: ['agency-eval-overview', storyId],
    queryFn: () => getEvalOverview(storyId),
    enabled: !!storyId,
    staleTime: 30_000,
  });

  // hooks 必须在早退 return 之前；data 可能为 undefined，用可选链
  const [usageSort, setUsageSort] = useState<AiRecordsSort>({ key: 'total_tokens', dir: -1 });
  const sortedUsage = useMemo(() => {
    const list = data?.token_usage ?? [];
    const key = usageSort.key as keyof PurposeUsage;
    return [...list].sort((a, b) => {
      const av = a[key];
      const bv = b[key];
      const cmp =
        typeof av === 'number' && typeof bv === 'number'
          ? av - bv
          : String(av).localeCompare(String(bv));
      return cmp * usageSort.dir;
    });
  }, [data?.token_usage, usageSort]);

  if (!currentStory) return <p className="p-6 text-ai-ink-3">请先选择一个故事</p>;
  if (isLoading) return <p className="p-6">加载评估数据…</p>;
  if (error) return <p className="p-6 text-red-500">加载失败：{String(error)}</p>;
  if (!data) return null;

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-xl font-semibold">创作评估 · {currentStory.title}</h1>
      <AiInsightCards
        columns={3}
        items={[
          {
            key: 'pass-rate',
            label: '质量门通过率',
            value: `${(data.pass_rate * 100).toFixed(0)}%`,
            tone: 'green',
            sub: `${data.gate_history.length} 次判定`,
          },
          {
            key: 'checkpoints',
            label: '检查点',
            value: String(data.checkpoints.length),
            tone: 'neutral',
            sub: '里程碑快照',
          },
          {
            key: 'human-signals',
            label: 'Human 信号',
            value:
              data.human_signals.length === 0
                ? '—'
                : `${((data.human_signals.reduce((a, s) => a + s.modification_ratio, 0) / data.human_signals.length) * 100).toFixed(0)}%`,
            tone: 'orange',
            sub: '平均修改率',
          },
        ]}
      />

      <section>
        <h2 className="mb-2 font-medium">Gate 加权分趋势（阈值 0.75）</h2>
        <GateTrendChart data={data.gate_history} />
      </section>

      <section>
        <h2 className="mb-2 font-medium">判定历史</h2>
        <AiRecordsTable
          ariaLabel="判定历史"
          rows={data.gate_history}
          rowKey={g => g.key + g.created_at}
          emptyText="暂无判定记录"
          columns={[
            {
              key: 'key',
              label: '条目',
              width: '30%',
              render: g => <span className="font-medium text-ai-ink">{g.key}</span>,
            },
            { key: 'outcome', label: '结果', render: g => <OutcomePill outcome={g.outcome} /> },
            {
              key: 'weighted',
              label: '加权',
              align: 'right',
              render: g => <span className="tabular-nums">{g.weighted?.toFixed(2) ?? '—'}</span>,
            },
            {
              key: 'code',
              label: 'code',
              align: 'right',
              render: g => <span className="tabular-nums">{g.code?.toFixed(2) ?? '—'}</span>,
            },
            {
              key: 'rule',
              label: 'rule',
              align: 'right',
              render: g => <span className="tabular-nums">{g.rule?.toFixed(2) ?? '—'}</span>,
            },
            {
              key: 'model',
              label: 'model',
              align: 'right',
              render: g => <span className="tabular-nums">{g.model?.toFixed(2) ?? '—'}</span>,
            },
            {
              key: 'time',
              label: '时间',
              render: g => <span className="text-ai-ink-3">{g.created_at.slice(0, 16)}</span>,
            },
          ]}
        />
      </section>

      <section>
        <h2 className="mb-2 font-medium">Agency token 用量（按角色，全局）</h2>
        <p className="mb-1 text-sm text-ai-ink-3">
          本故事累计（检查点）：{data.story_tokens.total_tokens} tokens /{' '}
          {data.story_tokens.run_count} runs
        </p>
        <AiRecordsTable
          ariaLabel="Agency token 用量（按角色，全局）"
          rows={sortedUsage}
          rowKey={u => u.purpose}
          sort={usageSort}
          onSortChange={setUsageSort}
          emptyText="暂无 token 用量记录"
          columns={[
            {
              key: 'purpose',
              label: '角色',
              width: '34%',
              render: u => (
                <span className="font-medium text-ai-ink">{u.purpose.replace('agency_', '')}</span>
              ),
            },
            {
              key: 'calls',
              label: '调用',
              align: 'right',
              sortable: true,
              render: u => <span className="tabular-nums">{u.calls}</span>,
            },
            {
              key: 'total_tokens',
              label: '总 tokens',
              align: 'right',
              sortable: true,
              render: u => <span className="tabular-nums">{u.total_tokens}</span>,
            },
            {
              key: 'total_duration_ms',
              label: '总耗时(ms)',
              align: 'right',
              sortable: true,
              render: u => <span className="tabular-nums">{u.total_duration_ms}</span>,
            },
          ]}
          footer={
            <span className="text-[12px] text-ai-ink-3">
              按角色合计：{data.token_usage.reduce((s, u) => s + u.calls, 0)} 次调用 ·{' '}
              {data.token_usage.reduce((s, u) => s + u.total_tokens, 0)} tokens ·{' '}
              {data.token_usage.reduce((s, u) => s + u.total_duration_ms, 0)}ms
            </span>
          }
        />
      </section>

      <section data-testid="editor-agreement">
        <h2 className="mb-2 font-medium">判官一致率</h2>
        {data.editor_agreement.total_findings === 0 ? (
          <p className="text-sm text-ai-ink-3">暂无编辑裁决记录（章节经质量门后会产生）</p>
        ) : (
          <>
            <p className="mb-2 text-xs text-ai-ink-2 leading-relaxed">
              编辑判 blocking 时会引用草稿原文；那段引用在成稿里被删除或改写 = 你采纳了它
              （「你的修改率」是同一章的整体改动幅度，可用来对照漏报）。只报不改：不参与 任何门限。
            </p>
            <AiInsightCards
              columns={3}
              items={[
                {
                  key: 'agreement-adoption',
                  label: '总采纳率',
                  value: `${(data.editor_agreement.adoption_rate * 100).toFixed(0)}%`,
                  tone: 'orange',
                  sub: `${data.editor_agreement.total_adopted}/${data.editor_agreement.total_judgeable} 条可判`,
                },
                {
                  key: 'agreement-findings',
                  label: '阻断命中',
                  value: String(data.editor_agreement.total_findings),
                  tone: 'neutral',
                  sub: '编辑判 blocking 条数',
                },
                {
                  key: 'agreement-density',
                  label: '命中密度',
                  value: data.editor_agreement.per_kilo_chars.toFixed(1),
                  tone: 'neutral',
                  sub: '每千字条数',
                },
              ]}
            />
            <AiRecordsTable
              ariaLabel="每章判官一致率"
              rows={data.editor_agreement.chapters}
              rowKey={c => String(c.chapter_number)}
              emptyText="暂无数据"
              columns={[
                {
                  key: 'chapter_number',
                  label: '章',
                  width: '14%',
                  render: c => (
                    <span className="font-medium text-ai-ink">第 {c.chapter_number} 章</span>
                  ),
                },
                {
                  key: 'findings',
                  label: '阻断',
                  align: 'right',
                  render: c => <span className="tabular-nums">{c.findings}</span>,
                },
                {
                  key: 'judgeable',
                  label: '可判',
                  align: 'right',
                  render: c => <span className="tabular-nums">{c.judgeable}</span>,
                },
                {
                  key: 'adoption_rate',
                  label: '采纳率',
                  align: 'right',
                  render: c =>
                    c.judgeable === 0 ? (
                      <span className="text-ai-ink-3">—</span>
                    ) : (
                      <span className="tabular-nums">{(c.adoption_rate * 100).toFixed(0)}%</span>
                    ),
                },
                {
                  key: 'modification_ratio',
                  label: '你的修改率',
                  align: 'right',
                  render: c =>
                    c.modification_ratio == null ? (
                      <span className="text-ai-ink-3">—</span>
                    ) : (
                      <span className="tabular-nums">
                        {(c.modification_ratio * 100).toFixed(0)}%
                      </span>
                    ),
                },
              ]}
            />
          </>
        )}
      </section>

      <CheckpointCompare storyId={storyId} />
    </div>
  );
}
