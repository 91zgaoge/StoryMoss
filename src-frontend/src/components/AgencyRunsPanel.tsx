import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Bot, Loader2, Play, Sparkles } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { Textarea } from '@/components/ui/controls';
import { listRuns, startGenesis, type AgencyRun } from '@/services/api/agency';
import { useAppStore } from '@/stores/appStore';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const runsLogger = createLogger('ui:AgencyRunsPanel');

const STATUS_LABELS: Record<string, string> = {
  pending: '等待',
  running: '运行中',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消',
};

function statusTone(status: string): string {
  switch (status) {
    case 'running':
      return 'text-cinema-gold border-cinema-gold/30 bg-cinema-gold/10';
    case 'completed':
      return 'text-status-success border-status-success/30 bg-status-success/10';
    case 'failed':
      return 'text-status-danger border-status-danger/30 bg-status-danger/10';
    default:
      return 'text-ai-ink-3 border-borderSubtle bg-cinema-800/40';
  }
}

/** v0.86.0 A2：仪表盘「代理运行」——读活着的 agency_runs，并提供四角色创世入口。 */
export function AgencyRunsPanel() {
  const currentStory = useAppStore(s => s.currentStory);
  const setCurrentView = useAppStore(s => s.setCurrentView);
  const queryClient = useQueryClient();
  const [showGenesis, setShowGenesis] = useState(false);
  const [premise, setPremise] = useState('');
  const [starting, setStarting] = useState(false);

  const runsQuery = useQuery({
    queryKey: ['agency-dashboard-runs', currentStory?.id ?? null],
    queryFn: () => (currentStory ? listRuns(currentStory.id) : Promise.resolve([])),
    refetchInterval: query => (query.state.data?.some(r => r.status === 'running') ? 4000 : false),
  });

  const runs = [...(runsQuery.data ?? [])]
    .sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at))
    .slice(0, 6);

  const handleStartGenesis = async () => {
    const trimmed = premise.trim();
    if (!trimmed) {
      toast.error('请先填写故事构想');
      return;
    }
    setStarting(true);
    try {
      const runId = await startGenesis(trimmed);
      runsLogger.info('genesis started', { runId });
      toast.success('四角色创世已启动');
      setShowGenesis(false);
      setPremise('');
      queryClient.invalidateQueries({ queryKey: ['agency-dashboard-runs'] });
      queryClient.invalidateQueries({ queryKey: ['stories'] });
    } catch (error) {
      runsLogger.error('genesis start failed', { error });
      toast.error('发起创世失败，请重试');
    } finally {
      setStarting(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-xs text-ai-ink-3">
          {currentStory ? currentStory.title : '未选择故事'}
        </span>
        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" onClick={() => setCurrentView('agency-studio')}>
            <Bot className="mr-1 h-4 w-4" />
            代理工作室
          </Button>
          <Button variant="primary" size="sm" onClick={() => setShowGenesis(true)}>
            <Sparkles className="mr-1 h-4 w-4" />
            发起创世
          </Button>
        </div>
      </div>

      {runsQuery.isLoading ? (
        <div className="flex items-center justify-center py-10">
          <Loader2 className="h-5 w-5 animate-spin text-ai-ink-3" />
        </div>
      ) : runs.length === 0 ? (
        <div className="rounded-lg border border-dashed border-borderSubtle px-4 py-8 text-center">
          <p className="text-sm text-ai-ink-2">暂无代理运行</p>
          <p className="mt-1 text-xs text-ai-ink-3">
            发起一次创世，或到代理工作室派发续写 / 修订任务。
          </p>
        </div>
      ) : (
        <div className="space-y-2" data-testid="agency-runs-list">
          {runs.map(run => (
            <button
              key={run.id}
              type="button"
              onClick={() => setCurrentView('agency-studio')}
              className="flex w-full items-center gap-3 rounded-lg border border-borderSubtle bg-cinema-900/40 px-3 py-2.5 text-left transition-colors hover:border-cinema-gold/30 hover:bg-cinema-800/40"
            >
              <span
                className={`shrink-0 rounded border px-1.5 py-0.5 text-[11px] ${statusTone(run.status)}`}
              >
                {STATUS_LABELS[run.status] ?? run.status}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm text-ai-ink">
                  {run.premise || '（无构想）'}
                </span>
                <span className="mt-0.5 block text-xs text-ai-ink-3">
                  {run.lane ? `[${run.lane}] ` : ''}
                  {run.phase} · {formatTime(run.updated_at)}
                </span>
              </span>
            </button>
          ))}
        </div>
      )}

      {showGenesis && (
        <Modal
          open
          onClose={() => {
            if (!starting) setShowGenesis(false);
          }}
          title="发起四角色创世"
          size="sm"
        >
          <div className="space-y-4">
            <p className="text-xs leading-relaxed text-ai-ink-3">
              管理先补资产，主创写第一章，编审过质量门，导演统筹进度。
            </p>
            <label className="block text-sm text-ai-ink-2">
              故事构想
              <Textarea
                className="mt-1"
                rows={5}
                value={premise}
                onChange={e => setPremise(e.target.value)}
                placeholder="一句话或一段构想，例如：旧城拆迁中的修复师，在封闭的家族档案里发现母亲失踪的线索。"
              />
            </label>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={() => setShowGenesis(false)} disabled={starting}>
                取消
              </Button>
              <Button variant="primary" onClick={handleStartGenesis} isLoading={starting}>
                <Play className="mr-1 h-4 w-4" />
                开始创世
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </div>
  );
}

function formatTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}
