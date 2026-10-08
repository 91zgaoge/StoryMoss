import { useState } from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  CircleDollarSign,
  ClipboardCheck,
  Loader2,
  RefreshCw,
  ShieldAlert,
  Sparkles,
  XCircle,
} from 'lucide-react';
import { cn } from '@/utils/cn';
import { EmptyHint } from '@/components/ui/EmptyHint';
import { AiToolChips } from '@/components/ui/ai/AiToolChips';
import { AiInsightCards } from '@/components/ui/ai/AiInsightCards';
import { useAppStore } from '@/stores/appStore';
import {
  usePendingReviews,
  useQualityDebts,
  useRecomputeMaterial,
  useStaleMaterials,
  useResolvePendingReview,
  useResolveQualityDebt,
  useSetStylePreferenceStatus,
  useStoryCost,
  useStylePreferences,
} from '@/hooks/useMaintenance';
import toast from 'react-hot-toast';

/**
 * 运行维护（v0.64.0）。
 *
 * 把 P2/P3 阶段产出的四条「队列/账本」集中到一处呈现与决策：
 *  1. 质量债：质检降级/未解决的问题（含建议回收窗口）
 *  2. 待确认：分析自动新增的规则类资产（确认后才作为硬约束）
 *  3. 文风偏好：从作者手改中提炼的文风规则（可停用/启用）
 *  4. 成本：按故事聚合的调用与 token + 计费盲区告警
 * 全部为「只呈现与决策」——不会自动改写正文。
 */
type TabKey = 'debts' | 'pending' | 'style' | 'cost' | 'stale';

export function Maintenance() {
  const currentStory = useAppStore(s => s.currentStory);
  const storyId = currentStory?.id;
  const [tab, setTab] = useState<TabKey>('debts');

  const debts = useQualityDebts(storyId);
  const pending = usePendingReviews(storyId);
  const style = useStylePreferences(storyId);
  const cost = useStoryCost(storyId);
  const stale = useStaleMaterials(storyId);

  const resolveDebt = useResolveQualityDebt();
  const resolvePending = useResolvePendingReview();
  const setStyleStatus = useSetStylePreferenceStatus();

  if (!currentStory) {
    return (
      <div className="p-6">
        <EmptyHint>请先在「故事」中选择一个故事</EmptyHint>
      </div>
    );
  }

  const refreshing =
    debts.isFetching ||
    pending.isFetching ||
    style.isFetching ||
    cost.isFetching ||
    stale.isFetching;
  const refreshAll = () => {
    debts.refetch();
    pending.refetch();
    style.refetch();
    cost.refetch();
    stale.refetch();
  };

  const handleResolveDebt = async (id: string, status: 'resolved' | 'dismissed') => {
    try {
      await resolveDebt.mutateAsync({ debtId: id, status });
      toast.success(status === 'resolved' ? '已结清' : '已忽略');
    } catch (e) {
      toast.error(`操作失败: ${e}`);
    }
  };

  const handleResolvePending = async (id: string, status: 'confirmed' | 'rejected') => {
    try {
      await resolvePending.mutateAsync({ reviewId: id, status });
      toast.success(status === 'confirmed' ? '已确认' : '已拒绝');
    } catch (e) {
      toast.error(`操作失败: ${e}`);
    }
  };

  const handleToggleStyle = async (id: string, active: boolean) => {
    try {
      await setStyleStatus.mutateAsync({ preferenceId: id, active });
      toast.success(active ? '已启用该偏好' : '已停用该偏好');
    } catch (e) {
      toast.error(`操作失败: ${e}`);
    }
  };

  const tabs = [
    { key: 'debts', label: '质量债', count: debts.data?.length ?? 0 },
    { key: 'pending', label: '待确认', count: pending.data?.length ?? 0 },
    { key: 'style', label: '文风偏好', count: style.data?.length ?? 0 },
    { key: 'cost', label: '成本', count: undefined },
    { key: 'stale', label: '物料重算', count: stale.data?.length ?? 0 },
  ];

  return (
    <div className="p-6 max-w-6xl mx-auto">
      <div className="flex items-center justify-between mb-2">
        <h1 className="text-2xl font-display font-bold text-white flex items-center gap-2">
          <ShieldAlert className="w-6 h-6 text-cinema-gold" />
          运行维护
        </h1>
        <button
          onClick={refreshAll}
          disabled={refreshing}
          data-testid="maintenance-refresh"
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg bg-cinema-800/60 text-ai-ink-2 hover:text-ai-ink border border-cinema-700 transition-colors disabled:opacity-50"
        >
          <RefreshCw className={cn('w-3.5 h-3.5', refreshing && 'animate-spin')} />
          刷新
        </button>
      </div>
      <p className="text-sm text-ai-ink-3 mb-5">
        质量债、待确认、文风偏好与成本账本的集中视图。所有条目都由你决策处理，系统不会自动改写正文。
      </p>

      <div className="mb-5">
        <AiToolChips
          items={tabs.map(t => ({ key: t.key, label: t.label, count: t.count }))}
          activeKey={tab}
          onSelect={key => setTab(key as TabKey)}
          ariaLabel="运行维护分类"
        />
      </div>

      {tab === 'debts' && <DebtsSection storyId={storyId} onResolve={handleResolveDebt} />}
      {tab === 'pending' && <PendingSection storyId={storyId} onResolve={handleResolvePending} />}
      {tab === 'style' && <StyleSection storyId={storyId} onToggle={handleToggleStyle} />}
      {tab === 'cost' && <CostSection storyId={storyId} />}
      {tab === 'stale' && <StaleSection storyId={storyId} />}
    </div>
  );
}

/**
 * 物料重算（v0.64.11）：编辑旧章后，从旧正文推出来的跨章物料（后续章节摘要、
 * 分层摘要与全书纲要、连续性快照）会在这里显示「自第几章起失效」，一键重算。
 */
function StaleSection({ storyId }: SectionProps) {
  const { data: stale = [], isLoading } = useStaleMaterials(storyId);
  const recompute = useRecomputeMaterial();
  const [busy, setBusy] = useState(false);
  if (isLoading) return <Loading />;
  if (stale.length === 0) {
    return <EmptyHint>暂无失效物料。编辑较早章节后，受影响的跨章物料会出现在这里</EmptyHint>;
  }
  const fromChapter = Math.min(...stale.map(s => s.from_chapter));
  const handleRecompute = async () => {
    if (!storyId) return;
    setBusy(true);
    try {
      const report = await recompute.mutateAsync({ storyId, fromChapter });
      const parts = [
        `章节摘要 ${report.chapter_summaries} 条`,
        report.segment_summaries_deleted > 0
          ? `分层摘要重建 ${report.segment_summaries_deleted} 条`
          : null,
        report.checkpoints_rewritten > 0 ? `连续性快照 ${report.checkpoints_rewritten} 条` : null,
        report.segment_pending_llm ? '分层摘要待模型可用时后台补齐' : null,
      ].filter(Boolean);
      toast.success(`已重算第 ${report.from_chapter} 章及以后：${parts.join('、')}`);
    } catch (e) {
      toast.error(`重算失败: ${e}`);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="space-y-2" data-testid="maintenance-stale">
      <div className="flex items-center justify-between gap-3 rounded-lg border border-cinema-700 bg-cinema-800/40 p-3">
        <div className="text-xs text-ai-ink-2 leading-relaxed">
          下列物料是从旧正文推出来的，编辑后不会自动重算。重算会按当前正文重写第 {fromChapter}{' '}
          章及以后的章节摘要、分层摘要与全书纲要、连续性快照。
        </div>
        <ActionButton
          onClick={handleRecompute}
          disabled={busy || !storyId}
          testId="maintenance-recompute"
        >
          {busy ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <RefreshCw className="w-3.5 h-3.5" />
          )}
          重算第 {fromChapter} 章起
        </ActionButton>
      </div>
      {stale.map(item => (
        <div
          key={item.kind}
          className="rounded-lg border border-cinema-700 bg-cinema-800/40 p-3"
          data-testid={`stale-${item.kind}`}
        >
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-[11px] px-1.5 py-0.5 rounded bg-ai-accent-tint text-ai-accent-ink">
              {item.kind}
            </span>
            <span className="text-xs text-ai-ink-2">自第 {item.from_chapter} 章起失效</span>
            <span className="text-[11px] text-ai-ink-3">{item.updated_at.slice(0, 16)}</span>
          </div>
          {item.reason && (
            <p className="mt-1.5 text-xs text-ai-ink-2 leading-relaxed">{item.reason}</p>
          )}
        </div>
      ))}
    </div>
  );
}

function Loading() {
  return (
    <div className="flex items-center gap-2 text-ai-ink-3 py-10 justify-center">
      <Loader2 className="w-4 h-4 animate-spin" />
      加载中...
    </div>
  );
}

function SeverityChip({ severity }: { severity: string }) {
  const config =
    severity === 'critical'
      ? { label: '严重', className: 'text-ai-red border-ai-red/40' }
      : severity === 'warning'
        ? { label: '警告', className: 'text-ai-orange border-ai-orange/40' }
        : { label: '提示', className: 'text-ai-ink-2 border-ai-line' };
  return (
    <span className={cn('text-[11px] px-1.5 py-0.5 rounded border', config.className)}>
      {config.label}
    </span>
  );
}

function ActionButton({
  children,
  onClick,
  disabled,
  testId,
  tone,
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
  testId?: string;
  tone?: 'default' | 'danger';
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      data-testid={testId}
      className={cn(
        'flex items-center gap-1 px-2 py-1 text-xs rounded border transition-colors disabled:opacity-50',
        tone === 'danger'
          ? 'border-ai-line text-ai-ink-3 hover:text-ai-red hover:bg-[color-mix(in_srgb,var(--ai-red)_10%,transparent)]'
          : 'border-ai-line text-ai-ink-2 hover:text-ai-ink hover:bg-ai-hover'
      )}
    >
      {children}
    </button>
  );
}

interface SectionProps {
  storyId: string | undefined;
}

function DebtsSection({
  storyId,
  onResolve,
}: SectionProps & { onResolve: (id: string, status: 'resolved' | 'dismissed') => void }) {
  const { data: debts = [], isLoading } = useQualityDebts(storyId);
  if (isLoading) return <Loading />;
  if (debts.length === 0) {
    return (
      <EmptyHint>暂无未结清的质量债。质检降级放行或存在问题未处理时，条目会出现在这里</EmptyHint>
    );
  }
  return (
    <div className="space-y-2" data-testid="maintenance-debts">
      {debts.map(debt => (
        <div key={debt.id} className="rounded-lg border border-cinema-700 bg-cinema-800/40 p-3">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2 flex-wrap">
                <SeverityChip severity={debt.severity} />
                <span className="text-xs text-ai-ink-2">
                  {debt.chapter_number != null ? `第 ${debt.chapter_number} 章` : '全局'}
                </span>
                <span className="text-[11px] px-1.5 py-0.5 rounded bg-ai-accent-tint text-ai-accent-ink">
                  {debt.source}
                </span>
                {debt.suggested_window && (
                  <span className="text-[11px] text-ai-ink-3">建议 {debt.suggested_window}</span>
                )}
              </div>
              <p className="mt-1.5 text-xs text-ai-ink-2 leading-relaxed">{debt.detail}</p>
            </div>
            <div className="flex items-center gap-1 shrink-0">
              <ActionButton
                onClick={() => onResolve(debt.id, 'resolved')}
                testId="maintenance-debt-resolve"
              >
                <CheckCircle2 className="w-3.5 h-3.5" />
                结清
              </ActionButton>
              <ActionButton
                onClick={() => onResolve(debt.id, 'dismissed')}
                testId="maintenance-debt-dismiss"
                tone="danger"
              >
                <XCircle className="w-3.5 h-3.5" />
                忽略
              </ActionButton>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function PendingSection({
  storyId,
  onResolve,
}: SectionProps & { onResolve: (id: string, status: 'confirmed' | 'rejected') => void }) {
  const { data: reviews = [], isLoading } = usePendingReviews(storyId);
  if (isLoading) return <Loading />;
  if (reviews.length === 0) {
    return (
      <EmptyHint>
        暂无待确认项。分析自动新增的世界观硬规则等会进入这里，确认后才作为硬约束使用
      </EmptyHint>
    );
  }
  return (
    <div className="space-y-2" data-testid="maintenance-pending">
      {reviews.map(review => (
        <div key={review.id} className="rounded-lg border border-cinema-700 bg-cinema-800/40 p-3">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2 flex-wrap">
                <ClipboardCheck className="w-3.5 h-3.5 text-ai-ink-3" />
                <span className="text-sm font-medium text-white">{review.subject}</span>
                <span className="text-[11px] px-1.5 py-0.5 rounded bg-ai-accent-tint text-ai-accent-ink">
                  {review.kind === 'world_rule' ? '世界规则' : review.kind}
                </span>
              </div>
              {review.detail && (
                <p className="mt-1.5 text-xs text-ai-ink-2 leading-relaxed">{review.detail}</p>
              )}
            </div>
            <div className="flex items-center gap-1 shrink-0">
              <ActionButton
                onClick={() => onResolve(review.id, 'confirmed')}
                testId="maintenance-pending-confirm"
              >
                <CheckCircle2 className="w-3.5 h-3.5" />
                确认
              </ActionButton>
              <ActionButton
                onClick={() => onResolve(review.id, 'rejected')}
                testId="maintenance-pending-reject"
                tone="danger"
              >
                <XCircle className="w-3.5 h-3.5" />
                拒绝
              </ActionButton>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function StyleSection({
  storyId,
  onToggle,
}: SectionProps & { onToggle: (id: string, active: boolean) => void }) {
  const { data: preferences = [], isLoading } = useStylePreferences(storyId);
  if (isLoading) return <Loading />;
  if (preferences.length === 0) {
    return (
      <EmptyHint>还没有提炼出文风偏好。你手动修改 AI 稿件后，系统会对比改前改后自动提炼</EmptyHint>
    );
  }
  return (
    <div className="space-y-2" data-testid="maintenance-style">
      {preferences.map(pref => {
        const active = pref.status === 'active';
        return (
          <div
            key={pref.id}
            className={cn(
              'rounded-lg border p-3',
              active ? 'border-cinema-700 bg-cinema-800/40' : 'border-cinema-800 bg-cinema-900/30'
            )}
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <Sparkles
                    className={cn('w-3.5 h-3.5', active ? 'text-ai-accent' : 'text-ai-ink-3')}
                  />
                  <span
                    className={cn('text-sm', active ? 'text-white' : 'text-ai-ink-3 line-through')}
                  >
                    {pref.pattern}
                  </span>
                  {!active && <span className="text-[11px] text-ai-ink-3">已停用</span>}
                </div>
                {pref.evidence && (
                  <p className="mt-1 text-xs text-ai-ink-3 leading-relaxed">
                    依据：{pref.evidence}
                  </p>
                )}
              </div>
              <ActionButton
                onClick={() => onToggle(pref.id, !active)}
                testId="maintenance-style-toggle"
              >
                {active ? '停用' : '启用'}
              </ActionButton>
            </div>
          </div>
        );
      })}
    </div>
  );
}

function CostSection({ storyId }: SectionProps) {
  const { data, isLoading } = useStoryCost(storyId);
  if (isLoading) return <Loading />;
  if (!data) {
    return <EmptyHint>暂无调用记录</EmptyHint>;
  }
  const { summary, anomalies } = data;
  return (
    <div className="space-y-4" data-testid="maintenance-cost">
      <AiInsightCards
        columns={4}
        items={[
          { key: 'calls', label: '调用次数', value: String(summary.total_calls) },
          {
            key: 'tokens',
            label: '累计 Token',
            value: summary.total_tokens.toLocaleString(),
            tone: summary.budget_warning ? 'orange' : 'accent',
            sub: `提示 ${summary.warn_threshold.toLocaleString()} 起提醒`,
          },
          {
            key: 'failed',
            label: '失败调用',
            value: String(summary.failed_calls),
            tone: summary.failed_calls > 0 ? 'red' : 'green',
          },
          {
            key: 'zero',
            label: '零记账调用',
            value: String(summary.zero_token_calls),
            tone: summary.zero_token_calls > 0 ? 'orange' : 'green',
          },
        ]}
      />
      {summary.budget_warning && (
        <div className="flex items-start gap-2 rounded-lg border border-ai-orange/40 bg-[color-mix(in_srgb,var(--ai-orange)_8%,transparent)] p-3 text-xs text-ai-orange">
          <AlertTriangle className="w-4 h-4 shrink-0" />
          累计 token 已超过提示阈值——建议检查模型路由与后台任务频率（此处只提示，不熔断）。
        </div>
      )}
      {anomalies.map(anomaly => (
        <div
          key={anomaly.kind}
          data-testid="maintenance-cost-anomaly"
          className="flex items-start gap-2 rounded-lg border border-ai-red/40 bg-[color-mix(in_srgb,var(--ai-red)_8%,transparent)] p-3 text-xs text-ai-red"
        >
          <CircleDollarSign className="w-4 h-4 shrink-0" />
          {anomaly.detail}
        </div>
      ))}
      {summary.last_call_at && (
        <p className="text-xs text-ai-ink-3">
          最近一次调用：{summary.last_call_at}
          {summary.first_call_at ? ` ｜ 首次：${summary.first_call_at}` : ''}
        </p>
      )}
    </div>
  );
}
