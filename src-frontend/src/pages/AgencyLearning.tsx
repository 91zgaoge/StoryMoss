import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'react-hot-toast';
import { useAppStore } from '@/stores/appStore';
import {
  getLearningOverview,
  analyzeLearning,
  confirmPromotion,
  rejectPromotion,
  instinctFeedback,
  resetTechniqueStats,
} from '@/services/api/agency';
import type { Instinct } from '@/services/api/agency';
import { AiRecordsTable } from '@/components/ui/ai/AiRecordsTable';

function ConfidenceBar({ value }: { value: number }) {
  const pct = Math.round(value * 100);
  const color =
    value >= 0.8 ? 'var(--ai-green)' : value >= 0.5 ? 'var(--ai-orange)' : 'var(--ai-ink-3)';
  return (
    <div className="h-2 w-24 rounded bg-ai-inset">
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

  // 学习中心操作统一错误反馈：失败 toast.error，无论成败都刷新（finally）
  const runAction = async (action: () => Promise<unknown>, errorMsg: string) => {
    try {
      await action();
    } catch {
      toast.error(errorMsg);
    } finally {
      await refresh();
    }
  };

  if (!currentStory) return <p className="p-6 text-ai-ink-3">请先选择一个故事</p>;
  if (isLoading) return <p className="p-6">加载学习数据…</p>;
  if (error) return <p className="p-6 text-red-500">加载失败：{String(error)}</p>;
  if (!data) return null;

  const onAnalyze = async () => {
    setAnalyzing(true);
    try {
      await analyzeLearning(storyId);
    } catch {
      toast.error('分析失败，请稍后重试');
    } finally {
      setAnalyzing(false);
      await refresh();
    }
  };

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">学习中心 · {currentStory.title}</h1>
        <button
          onClick={onAnalyze}
          disabled={analyzing || data.unanalyzed_count < data.analyze_min_new}
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
                className="flex items-center justify-between rounded border border-ai-line p-3 text-ai-orange"
                style={{ background: 'color-mix(in srgb, var(--ai-orange) 12%, transparent)' }}
              >
                <div>
                  <div className="font-medium">{c.trigger}</div>
                  <div className="text-sm text-ai-ink-2">{c.action}</div>
                  <div className="mt-1 flex items-center gap-2 text-xs text-ai-ink-3">
                    <ConfidenceBar value={c.confidence} />
                    <span>{(c.confidence * 100).toFixed(0)}%</span>
                    <span>证据 {c.evidence_count}</span>
                  </div>
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={() =>
                      runAction(() => confirmPromotion(storyId, c.id), '确认晋升失败，请重试')
                    }
                    className="rounded bg-green-600 px-3 py-1 text-sm text-white"
                  >
                    确认为技能
                  </button>
                  <button
                    onClick={() =>
                      runAction(() => rejectPromotion(storyId, c.id), '拒绝晋升失败，请重试')
                    }
                    className="rounded border border-ai-line px-3 py-1 text-sm text-ai-ink"
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
          <p className="text-sm text-ai-ink-3">尚无模式——创作几章后点击"立即分析"。</p>
        )}
        <div className="space-y-2">
          {data.instincts.map((i: Instinct) => (
            <div key={i.id} className="rounded border border-ai-line bg-ai-surface p-3">
              <div className="flex items-center justify-between">
                <div className="font-medium">{i.trigger}</div>
                <span className="text-xs text-ai-ink-3">
                  {i.status}
                  {i.scope === 'global' ? ' · global' : ''}
                </span>
              </div>
              <div className="text-sm text-ai-ink-2">{i.action}</div>
              <div className="mt-1 flex items-center gap-2 text-xs text-ai-ink-3">
                <ConfidenceBar value={i.confidence} />
                <span>{(i.confidence * 100).toFixed(0)}%</span>
                <span>证据 {i.evidence_count}</span>
                <button
                  onClick={() =>
                    runAction(() => instinctFeedback(storyId, i.id, true), '反馈提交失败，请重试')
                  }
                  className="ml-2 underline"
                >
                  有用
                </button>
                <button
                  onClick={() =>
                    runAction(() => instinctFeedback(storyId, i.id, false), '反馈提交失败，请重试')
                  }
                  className="underline"
                >
                  不准
                </button>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section data-testid="technique-stats">
        <h2 className="mb-2 font-medium">
          章法偏好（每拍手法 · 作者裁决回灌）
          <button
            onClick={() =>
              runAction(() => resetTechniqueStats(storyId), '重置手法统计失败，请重试')
            }
            className="ml-3 text-xs underline"
            data-testid="reset-technique-stats"
          >
            重置统计
          </button>
        </h2>
        <p className="mb-2 text-xs text-ai-ink-3">
          每拍写作会从手法池轮换一条写作手段（如「迟一拍揭示」「平直命名」）。你对续写的 Tab 接受 /
          Esc 放弃会按时间归因到最近一拍的手法上：被接受的手法更常被选用，被反复
          放弃的会被轮换避开。样本不足 5 条时不影响选择。
        </p>
        <div className="space-y-2">
          {data.technique_stats.map(t => {
            const sample = t.accepts + t.rejects;
            const quality = sample === 0 ? 0 : (t.accepts - t.rejects) / sample;
            return (
              <div
                key={t.technique_id}
                className="flex items-center gap-3 rounded border border-ai-line bg-ai-surface p-2"
                data-testid={`technique-${t.technique_id}`}
              >
                <span className="w-28 font-medium">{t.label}</span>
                <ConfidenceBar value={(quality + 1) / 2} />
                <span className="text-xs tabular-nums text-ai-ink-3">
                  接受 {t.accepts} · 放弃 {t.rejects}
                  {sample < 5 ? ' · 样本不足' : ''}
                </span>
              </div>
            );
          })}
        </div>
      </section>

      <section>
        <h2 className="mb-2 font-medium">最近观察</h2>
        <AiRecordsTable
          ariaLabel="最近观察"
          rows={data.recent_observations
            .slice()
            .reverse()
            .map((o, idx) => ({ ...o, idx }))}
          rowKey={o => String(o.idx)}
          emptyText="暂无观察记录"
          columns={[
            {
              key: 'ts',
              label: '时间',
              render: o => <span className="text-ai-ink-3">{o.ts.slice(5, 16)}</span>,
            },
            { key: 'kind', label: '类型', render: o => o.kind },
            { key: 'actor', label: '角色', render: o => o.actor },
            {
              key: 'payload',
              label: '摘要',
              render: o => (
                <span className="block max-w-md truncate">{JSON.stringify(o.payload)}</span>
              ),
            },
          ]}
        />
      </section>
    </div>
  );
}
