import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Bot, Loader2, RotateCcw, Square } from 'lucide-react';
import { crewStatus, crewRetryTask, crewCancelTask } from '@/services/api/agency';
import type { AgencyAgentTask } from '@/services/api/agency';
import { useAppStore } from '@/stores/appStore';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const tasksLogger = createLogger('ui:AgencyTaskList');

const STATUS_LABELS: Record<string, string> = {
  pending: '等待',
  running: '执行中',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消',
  stale: '疑似卡住',
};

/** v0.91.0 B3：代理工作室派发的子任务列表（导演/管理/编审/主创）。 */
export function AgencyTaskList() {
  const currentStory = useAppStore(s => s.currentStory);
  const queryClient = useQueryClient();
  const [busyId, setBusyId] = useState<string | null>(null);

  const query = useQuery({
    queryKey: ['agency-tasks-panel', currentStory?.id ?? null],
    queryFn: () => (currentStory ? crewStatus(currentStory.id) : Promise.resolve(null)),
    enabled: !!currentStory,
    refetchInterval: 5000,
  });

  const tasks = query.data?.tasks ?? [];

  const handleRetry = async (task: AgencyAgentTask) => {
    setBusyId(task.id);
    try {
      await crewRetryTask(task.id);
      toast.success('已重新派发该代理任务');
      queryClient.invalidateQueries({ queryKey: ['agency-tasks-panel'] });
    } catch (e) {
      tasksLogger.error('retry agency task failed', { error: e });
      toast.error('重试失败，请到代理工作室处理');
    } finally {
      setBusyId(null);
    }
  };

  const handleCancel = async (task: AgencyAgentTask) => {
    setBusyId(task.id);
    try {
      await crewCancelTask(task.id);
      toast.success('已请求取消该代理任务');
      queryClient.invalidateQueries({ queryKey: ['agency-tasks-panel'] });
    } catch (e) {
      tasksLogger.error('cancel agency task failed', { error: e });
      toast.error('取消失败，请到代理工作室处理');
    } finally {
      setBusyId(null);
    }
  };

  if (!currentStory) {
    return <p className="px-4 py-10 text-center text-sm text-ai-ink-3">请先选择一个故事</p>;
  }

  if (query.isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="h-5 w-5 animate-spin text-ai-ink-3" />
      </div>
    );
  }

  if (tasks.length === 0) {
    return (
      <div className="rounded-lg border border-dashed border-borderSubtle px-4 py-10 text-center">
        <Bot className="mx-auto mb-2 h-8 w-8 text-ai-ink-3" />
        <p className="text-sm text-ai-ink-2">暂无代理任务</p>
        <p className="mt-1 text-xs text-ai-ink-3">
          到代理工作室对导演派发任务后，这里会显示四个角色的执行进度。
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-2" data-testid="agency-task-list">
      {tasks.map(task => (
        <div
          key={task.id}
          className="flex flex-wrap items-center gap-3 rounded-lg border border-borderSubtle bg-cinema-900/40 px-3 py-2.5"
        >
          <span className="rounded border border-borderSubtle bg-cinema-800/60 px-1.5 py-0.5 text-[11px] text-ai-ink-2">
            {task.role}
          </span>
          <span
            className={`text-xs ${
              task.status === 'running'
                ? 'text-cinema-gold'
                : task.status === 'failed' || task.status === 'stale'
                  ? 'text-status-danger'
                  : task.status === 'completed'
                    ? 'text-status-success'
                    : 'text-ai-ink-3'
            }`}
          >
            {STATUS_LABELS[task.status] ?? task.status}
          </span>
          <span className="min-w-0 flex-1 truncate text-sm text-ai-ink" title={task.task}>
            {task.task}
          </span>
          {task.error && (
            <span className="w-full truncate text-xs text-status-danger">{task.error}</span>
          )}
          <div className="flex items-center gap-1">
            {(task.status === 'failed' || task.status === 'stale') && (
              <button
                type="button"
                disabled={busyId === task.id}
                onClick={() => handleRetry(task)}
                className="inline-flex items-center gap-1 rounded border border-borderSubtle px-2 py-1 text-xs text-ai-ink-2 transition-colors hover:bg-cinema-800 disabled:opacity-50"
              >
                <RotateCcw className="h-3 w-3" />
                重试
              </button>
            )}
            {(task.status === 'running' || task.status === 'pending') && (
              <button
                type="button"
                disabled={busyId === task.id}
                onClick={() => handleCancel(task)}
                className="inline-flex items-center gap-1 rounded border border-borderSubtle px-2 py-1 text-xs text-status-danger transition-colors hover:bg-cinema-800 disabled:opacity-50"
              >
                <Square className="h-3 w-3" />
                取消
              </button>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
