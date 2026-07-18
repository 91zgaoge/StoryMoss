import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import {
  getLearningOverview,
  analyzeLearning,
  confirmPromotion,
  rejectPromotion,
  instinctFeedback,
} from '@/services/api/agency';
import type { Instinct } from '@/services/api/agency';

function ConfidenceBar({ value }: { value: number }) {
  const pct = Math.round(value * 100);
  const color = value >= 0.8 ? '#22c55e' : value >= 0.5 ? '#f59e0b' : '#9ca3af';
  return (
    <div className="h-2 w-24 rounded bg-gray-200">
      <div className="h-2 rounded" style={{ width: `${pct}%`, background: color }} />
    </div>
  );
}

export default function AgencyLearning() {
  const currentStory = useAppStore(s => s.currentStory);
  const storyId = currentStory?.id ?? '';
  const qc = useQueryClient();
  const [analyzing, setAnalyzing] = useState(false);
  const { data, isLoading, error } = useQuery({
    queryKey: ['agency-learning', storyId],
    queryFn: () => getLearningOverview(storyId),
    enabled: !!storyId,
    staleTime: 15_000,
  });
  const refresh = () => qc.invalidateQueries({ queryKey: ['agency-learning', storyId] });

  if (!currentStory) return <p className="p-6 text-gray-500">请先选择一个故事</p>;
  if (isLoading) return <p className="p-6">加载学习数据…</p>;
  if (error) return <p className="p-6 text-red-500">加载失败：{String(error)}</p>;
  if (!data) return null;

  const onAnalyze = async () => {
    setAnalyzing(true);
    try {
      await analyzeLearning(storyId);
      await refresh();
    } finally {
      setAnalyzing(false);
    }
  };

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">学习中心 · {currentStory.title}</h1>
        <button
          onClick={onAnalyze}
          disabled={analyzing || data.unanalyzed_count < 2}
          className="rounded bg-indigo-600 px-3 py-1 text-sm text-white disabled:opacity-40"
        >
          {analyzing ? '分析中…' : `立即分析（${data.unanalyzed_count} 条未分析观察）`}
        </button>
      </div>

      {data.candidates.length > 0 && (
        <section>
          <h2 className="mb-2 font-medium">晋升提案（{data.candidates.length}）</h2>
          <div className="space-y-2">
            {data.candidates.map(c => (
              <div
                key={c.id}
                className="flex items-center justify-between rounded border border-amber-300 bg-amber-50 p-3"
              >
                <div>
                  <div className="font-medium">{c.trigger}</div>
                  <div className="text-sm text-gray-600">{c.action}</div>
                  <div className="mt-1 flex items-center gap-2 text-xs text-gray-500">
                    <ConfidenceBar value={c.confidence} />
                    <span>{(c.confidence * 100).toFixed(0)}%</span>
                    <span>证据 {c.evidence_count}</span>
                  </div>
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={async () => {
                      await confirmPromotion(storyId, c.id);
                      await refresh();
                    }}
                    className="rounded bg-green-600 px-3 py-1 text-sm text-white"
                  >
                    确认为技能
                  </button>
                  <button
                    onClick={async () => {
                      await rejectPromotion(storyId, c.id);
                      await refresh();
                    }}
                    className="rounded border px-3 py-1 text-sm"
                  >
                    拒绝
                  </button>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      <section>
        <h2 className="mb-2 font-medium">已学模式（{data.instincts.length}）</h2>
        {data.instincts.length === 0 && (
          <p className="text-sm text-gray-500">尚无模式——创作几章后点击"立即分析"。</p>
        )}
        <div className="space-y-2">
          {data.instincts.map((i: Instinct) => (
            <div key={i.id} className="rounded border p-3">
              <div className="flex items-center justify-between">
                <div className="font-medium">{i.trigger}</div>
                <span className="text-xs text-gray-400">
                  {i.status}
                  {i.scope === 'global' ? ' · global' : ''}
                </span>
              </div>
              <div className="text-sm text-gray-600">{i.action}</div>
              <div className="mt-1 flex items-center gap-2 text-xs text-gray-500">
                <ConfidenceBar value={i.confidence} />
                <span>{(i.confidence * 100).toFixed(0)}%</span>
                <span>证据 {i.evidence_count}</span>
                <button
                  onClick={async () => {
                    await instinctFeedback(storyId, i.id, true);
                    await refresh();
                  }}
                  className="ml-2 underline"
                >
                  有用
                </button>
                <button
                  onClick={async () => {
                    await instinctFeedback(storyId, i.id, false);
                    await refresh();
                  }}
                  className="underline"
                >
                  不准
                </button>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section>
        <h2 className="mb-2 font-medium">最近观察</h2>
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-gray-500">
              <th>时间</th>
              <th>类型</th>
              <th>角色</th>
              <th>摘要</th>
            </tr>
          </thead>
          <tbody>
            {data.recent_observations
              .slice()
              .reverse()
              .map((o, idx) => (
                <tr key={idx} className="border-t">
                  <td className="text-gray-400">{o.ts.slice(5, 16)}</td>
                  <td>{o.kind}</td>
                  <td>{o.actor}</td>
                  <td className="max-w-md truncate">{JSON.stringify(o.payload)}</td>
                </tr>
              ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}
