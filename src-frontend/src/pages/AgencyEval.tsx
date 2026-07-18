import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import { getEvalOverview, listCheckpoints, compareCheckpoints } from '@/services/api/agency';
import type { GateHistoryItem } from '@/services/api/agency';

function GateTrendChart({ data }: { data: GateHistoryItem[] }) {
  const points = data.filter(d => d.weighted != null);
  if (points.length === 0) return <p className="text-sm text-gray-500">暂无评分数据</p>;
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
      <line x1={pad} y1={y(0.75)} x2={w - pad} y2={y(0.75)} stroke="#f59e0b" strokeDasharray="4" />
      <text x={w - pad + 2} y={y(0.75)} fontSize="10" fill="#f59e0b">
        0.75
      </text>
      <path d={pathD} fill="none" stroke="#6366f1" strokeWidth="2" />
      {points.map((p, i) => (
        <circle
          key={i}
          cx={x(i)}
          cy={y(p.weighted!)}
          r="3"
          fill={p.outcome === 'pass' ? '#22c55e' : p.outcome === 'revise' ? '#f59e0b' : '#ef4444'}
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
          className="rounded border px-2 py-1 text-sm"
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
          className="rounded border px-2 py-1 text-sm"
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
        <div className="mt-2 grid grid-cols-4 gap-2 text-center text-sm">
          <div className="rounded border p-2">
            <div className="text-gray-500">字数</div>
            <div>
              {diff.words_delta >= 0 ? '+' : ''}
              {diff.words_delta}
            </div>
          </div>
          <div className="rounded border p-2">
            <div className="text-gray-500">章节</div>
            <div>
              {diff.chapters_delta >= 0 ? '+' : ''}
              {diff.chapters_delta}
            </div>
          </div>
          <div className="rounded border p-2">
            <div className="text-gray-500">tokens</div>
            <div>
              {diff.tokens_delta >= 0 ? '+' : ''}
              {diff.tokens_delta}
            </div>
          </div>
          <div className="rounded border p-2">
            <div className="text-gray-500">加权分</div>
            <div>
              {diff.gate_weighted_delta >= 0 ? '+' : ''}
              {diff.gate_weighted_delta.toFixed(2)}
            </div>
          </div>
        </div>
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

  if (!currentStory) return <p className="p-6 text-gray-500">请先选择一个故事</p>;
  if (isLoading) return <p className="p-6">加载评估数据…</p>;
  if (error) return <p className="p-6 text-red-500">加载失败：{String(error)}</p>;
  if (!data) return null;

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-xl font-semibold">创作评估 · {currentStory.title}</h1>
      <div className="grid grid-cols-3 gap-4">
        <div className="rounded border p-4">
          <div className="text-sm text-gray-500">质量门通过率</div>
          <div className="text-2xl font-bold">{(data.pass_rate * 100).toFixed(0)}%</div>
          <div className="text-xs text-gray-400">{data.gate_history.length} 次判定</div>
        </div>
        <div className="rounded border p-4">
          <div className="text-sm text-gray-500">检查点</div>
          <div className="text-2xl font-bold">{data.checkpoints.length}</div>
          <div className="text-xs text-gray-400">里程碑快照</div>
        </div>
        <div className="rounded border p-4">
          <div className="text-sm text-gray-500">Human 信号</div>
          <div className="text-2xl font-bold">
            {data.human_signals.length === 0
              ? '—'
              : `${((data.human_signals.reduce((a, s) => a + s.modification_ratio, 0) / data.human_signals.length) * 100).toFixed(0)}%`}
          </div>
          <div className="text-xs text-gray-400">平均修改率</div>
        </div>
      </div>

      <section>
        <h2 className="mb-2 font-medium">Gate 加权分趋势（阈值 0.75）</h2>
        <GateTrendChart data={data.gate_history} />
      </section>

      <section>
        <h2 className="mb-2 font-medium">判定历史</h2>
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-gray-500">
              <th>条目</th>
              <th>结果</th>
              <th>加权</th>
              <th>code</th>
              <th>rule</th>
              <th>model</th>
              <th>时间</th>
            </tr>
          </thead>
          <tbody>
            {data.gate_history.map(g => (
              <tr key={g.key + g.created_at} className="border-t">
                <td>{g.key}</td>
                <td>{g.outcome}</td>
                <td>{g.weighted?.toFixed(2) ?? '—'}</td>
                <td>{g.code?.toFixed(2) ?? '—'}</td>
                <td>{g.rule?.toFixed(2) ?? '—'}</td>
                <td>{g.model?.toFixed(2) ?? '—'}</td>
                <td className="text-gray-400">{g.created_at.slice(0, 16)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section>
        <h2 className="mb-2 font-medium">Agency token 用量（按角色，全局）</h2>
        <p className="mb-1 text-sm text-gray-500">
          本故事累计（检查点）：{data.story_tokens.total_tokens} tokens /{' '}
          {data.story_tokens.run_count} runs
        </p>
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-gray-500">
              <th>角色</th>
              <th>调用</th>
              <th>总 tokens</th>
              <th>总耗时(ms)</th>
            </tr>
          </thead>
          <tbody>
            {data.token_usage.map(u => (
              <tr key={u.purpose} className="border-t">
                <td>{u.purpose.replace('agency_', '')}</td>
                <td>{u.calls}</td>
                <td>{u.total_tokens}</td>
                <td>{u.total_duration_ms}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <CheckpointCompare storyId={storyId} />
    </div>
  );
}
