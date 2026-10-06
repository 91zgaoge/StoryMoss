import { useMemo } from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  Eye,
  GitBranch,
  Info,
  Loader2,
  RefreshCw,
  Wand2,
} from 'lucide-react';
import { cn } from '@/utils/cn';
import { EmptyHint } from '@/components/ui/EmptyHint';
import { useAppStore } from '@/stores/appStore';
import {
  useCascadeImpacts,
  useIgnoreCascadeImpact,
  useReanalyzeScene,
  useTriggerCascadeRewriteForImpact,
  type CascadeImpact,
} from '@/hooks/useCascadeImpacts';
import toast from 'react-hot-toast';

/**
 * 级联中心（v0.60.0 P0-T4）。
 *
 * 编辑旧章节后，后台自动分析「受影响的下游章节 + 疑似冲突」。本页只做呈现与
 * 决策——系统不会自动改写后文：
 *  - 去查看：跳到幕后场景编辑器定位目标章
 *  - 重跑分析：对该场景重新 ingest（完成后自动清除「分析可能已失效」标记）
 *  - 忽略：标记该条不再关注
 *  - 触发改写：调用既有 cascade_rewrite 引擎生成 Diff 预览（在任务中心审阅）
 */
export function CascadeCenter() {
  const currentStory = useAppStore(s => s.currentStory);
  const setPendingSceneId = useAppStore(s => s.setPendingSceneId);
  const setCurrentView = useAppStore(s => s.setCurrentView);
  const storyId = currentStory?.id;

  const { data: impacts = [], isLoading, refetch, isFetching } = useCascadeImpacts(storyId);
  const ignoreImpact = useIgnoreCascadeImpact();
  const reanalyzeScene = useReanalyzeScene();
  const triggerRewrite = useTriggerCascadeRewriteForImpact();

  const openImpacts = useMemo(
    () => impacts.filter(i => i.decision === 'open' || i.decision === 'rewrite_requested'),
    [impacts]
  );
  const batches = useMemo(() => groupByBatch(openImpacts), [openImpacts]);
  const ignoredCount = impacts.length - openImpacts.length;

  if (!currentStory) {
    return (
      <div className="p-6">
        <EmptyHint>请先在「故事」中选择一个故事</EmptyHint>
      </div>
    );
  }

  const handleLocate = (impact: CascadeImpact) => {
    setPendingSceneId(impact.target_scene_id);
    setCurrentView('scenes');
    toast('已定位到目标场景', { icon: '🧭' });
  };

  const handleIgnore = async (impact: CascadeImpact) => {
    try {
      await ignoreImpact.mutateAsync(impact.id);
      toast.success('已忽略该条目');
    } catch (e) {
      toast.error(`忽略失败: ${e}`);
    }
  };

  const handleReanalyze = async (impact: CascadeImpact) => {
    try {
      await reanalyzeScene.mutateAsync(impact.target_scene_id);
      toast.success('已开始重跑该章分析，完成后将自动清除失效标记');
    } catch (e) {
      toast.error(`重跑分析失败: ${e}`);
    }
  };

  const handleTriggerRewrite = async (impact: CascadeImpact) => {
    try {
      await triggerRewrite.mutateAsync(impact.id);
      toast.success('已创建级联改写任务，可在任务中心审阅 Diff');
    } catch (e) {
      toast.error(`触发改写失败: ${e}`);
    }
  };

  return (
    <div className="p-6 max-w-5xl mx-auto">
      <div className="flex items-center justify-between mb-2">
        <h1 className="text-2xl font-display font-bold text-white flex items-center gap-2">
          <GitBranch className="w-6 h-6 text-cinema-gold" />
          级联中心
        </h1>
        <button
          onClick={() => refetch()}
          disabled={isFetching}
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg bg-cinema-800/60 text-ai-ink-2 hover:text-ai-ink border border-cinema-700 transition-colors disabled:opacity-50"
          data-testid="cascade-refresh"
        >
          <RefreshCw className={cn('w-3.5 h-3.5', isFetching && 'animate-spin')} />
          刷新
        </button>
      </div>
      <p className="text-sm text-ai-ink-3 mb-6">
        编辑旧章节后系统自动列出受影响的下游章节与疑似冲突。系统不会自动改写后文，
        每条都由你决定处理方式。
      </p>

      {isLoading ? (
        <div className="flex items-center gap-2 text-ai-ink-3 py-10 justify-center">
          <Loader2 className="w-4 h-4 animate-spin" />
          加载中...
        </div>
      ) : batches.length === 0 ? (
        <EmptyHint>
          {ignoredCount > 0
            ? '没有待处理的改稿影响（历史条目均已忽略）'
            : '暂无改稿影响。修改旧章节正文后，这里会出现受影响的下游章节清单'}
        </EmptyHint>
      ) : (
        <div className="space-y-6" data-testid="cascade-batches">
          {batches.map(batch => (
            <div key={batch.batchId}>
              <div className="flex items-center gap-2 mb-2 px-1">
                <span className="text-sm font-medium text-ai-ink">
                  {batch.sourceChapter != null ? `第 ${batch.sourceChapter} 章改稿` : '改稿分析'}
                </span>
                <span className="text-xs text-ai-ink-3">
                  {formatTime(batch.createdAt)} · 影响 {batch.items.length} 章
                </span>
              </div>
              <div className="space-y-2">
                {batch.items.map(impact => (
                  <ImpactRow
                    key={impact.id}
                    impact={impact}
                    busy={
                      ignoreImpact.isPending || reanalyzeScene.isPending || triggerRewrite.isPending
                    }
                    onLocate={() => handleLocate(impact)}
                    onIgnore={() => handleIgnore(impact)}
                    onReanalyze={() => handleReanalyze(impact)}
                    onTriggerRewrite={() => handleTriggerRewrite(impact)}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {ignoredCount > 0 && batches.length > 0 && (
        <p className="mt-6 text-xs text-ai-ink-3">另有 {ignoredCount} 条已忽略的条目</p>
      )}
    </div>
  );
}

interface Batch {
  batchId: string;
  sourceChapter: number | null;
  createdAt: string;
  items: CascadeImpact[];
}

function groupByBatch(impacts: CascadeImpact[]): Batch[] {
  const map = new Map<string, Batch>();
  for (const impact of impacts) {
    const existing = map.get(impact.batch_id);
    if (existing) {
      existing.items.push(impact);
    } else {
      map.set(impact.batch_id, {
        batchId: impact.batch_id,
        sourceChapter: impact.source_chapter_number,
        createdAt: impact.created_at,
        items: [impact],
      });
    }
  }
  return Array.from(map.values());
}

function formatTime(iso: string): string {
  try {
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return iso;
    const pad = (n: number) => String(n).padStart(2, '0');
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(
      date.getHours()
    )}:${pad(date.getMinutes())}`;
  } catch {
    return iso;
  }
}

function SeverityBadge({ severity }: { severity: string }) {
  const config =
    severity === 'critical'
      ? { label: '冲突', icon: AlertTriangle, className: 'text-ai-red border-ai-red/40' }
      : severity === 'warning'
        ? { label: '可疑', icon: AlertTriangle, className: 'text-ai-orange border-ai-orange/40' }
        : { label: '联动', icon: Info, className: 'text-ai-ink-2 border-ai-line' };
  const Icon = config.icon;
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-1.5 py-0.5 text-[11px] rounded border',
        config.className
      )}
    >
      <Icon className="w-3 h-3" />
      {config.label}
    </span>
  );
}

interface ImpactRowProps {
  impact: CascadeImpact;
  busy: boolean;
  onLocate: () => void;
  onIgnore: () => void;
  onReanalyze: () => void;
  onTriggerRewrite: () => void;
}

function ImpactRow({
  impact,
  busy,
  onLocate,
  onIgnore,
  onReanalyze,
  onTriggerRewrite,
}: ImpactRowProps) {
  const chapterLabel =
    impact.target_chapter_number != null ? `第 ${impact.target_chapter_number} 章` : '目标场景';
  return (
    <div
      className="rounded-lg border border-cinema-700 bg-cinema-800/40 p-3"
      data-testid="cascade-impact-row"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-sm font-medium text-white">{chapterLabel}</span>
            <SeverityBadge severity={impact.severity} />
            {impact.impact_kind === 'conflict' && (
              <span className="text-[11px] px-1.5 py-0.5 rounded bg-[color-mix(in_srgb,var(--ai-red)_12%,transparent)] text-ai-red">
                冲突扫描
              </span>
            )}
            {impact.stale_flag && (
              <span
                className="text-[11px] px-1.5 py-0.5 rounded bg-[color-mix(in_srgb,var(--ai-orange)_12%,transparent)] text-ai-orange"
                data-testid="cascade-stale-badge"
              >
                分析可能已失效
              </span>
            )}
            {impact.decision === 'rewrite_requested' && (
              <span className="text-[11px] px-1.5 py-0.5 rounded bg-ai-accent-tint text-ai-accent-ink">
                已请求改写
              </span>
            )}
          </div>
          {impact.detail && (
            <p className="mt-1.5 text-xs text-ai-ink-2 leading-relaxed">{impact.detail}</p>
          )}
          {impact.evidence && (
            <p className="mt-1 text-xs text-ai-ink-3 leading-relaxed">证据：{impact.evidence}</p>
          )}
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <RowButton
            onClick={onLocate}
            disabled={busy}
            title="在场景页定位该章"
            testId="cascade-locate"
          >
            <Eye className="w-3.5 h-3.5" />
            去查看
          </RowButton>
          <RowButton
            onClick={onReanalyze}
            disabled={busy}
            title="重新分析该章（完成后清除失效标记）"
            testId="cascade-reanalyze"
          >
            <RefreshCw className={cn('w-3.5 h-3.5', busy && 'animate-spin')} />
            重跑分析
          </RowButton>
          <RowButton
            onClick={onTriggerRewrite}
            disabled={busy}
            title="生成改写的 Diff 预览（在任务中心审阅）"
            testId="cascade-rewrite"
          >
            <Wand2 className="w-3.5 h-3.5" />
            触发改写
          </RowButton>
          <RowButton
            onClick={onIgnore}
            disabled={busy}
            title="不再关注该条目"
            testId="cascade-ignore"
          >
            <CheckCircle2 className="w-3.5 h-3.5" />
            忽略
          </RowButton>
        </div>
      </div>
    </div>
  );
}

function RowButton({
  children,
  onClick,
  disabled,
  title,
  testId,
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
  title?: string;
  testId?: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      data-testid={testId}
      className="flex items-center gap-1 px-2 py-1 text-xs rounded border border-ai-line text-ai-ink-2 hover:text-ai-ink hover:bg-ai-hover transition-colors disabled:opacity-50"
    >
      {children}
    </button>
  );
}
