import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { loggedInvoke } from '@/services/tauri';
import {
  ListChecks,
  Play,
  Square,
  Trash2,
  Clock,
  Heart,
  AlertCircle,
  CheckCircle2,
  XCircle,
  Loader2,
  Plus,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import { cn } from '@/utils/cn';
import {
  useTasks,
  useCreateTask,
  useDeleteTask,
  useTriggerTask,
  useCancelTask,
  useTaskLogs,
  useCascadeRewriteResult,
  useApplyCascadeRewrite,
  useRejectCascadeRewrite,
  type Task,
  type TaskLog,
  type RewriteSegment,
} from '@/hooks/useTasks';
import toast from 'react-hot-toast';

type StatusFilter = 'all' | 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

const statusConfig: Record<string, { label: string; color: string; icon: React.ElementType }> = {
  pending: { label: '等待中', color: 'text-gray-400', icon: Clock },
  running: { label: '执行中', color: 'text-blue-400', icon: Loader2 },
  completed: { label: '已完成', color: 'text-green-400', icon: CheckCircle2 },
  failed: { label: '失败', color: 'text-red-400', icon: XCircle },
  cancelled: { label: '已取消', color: 'text-orange-400', icon: AlertCircle },
};

const scheduleTypeLabels: Record<string, string> = {
  once: '一次性',
  daily: '每天',
  weekly: '每周',
  cron: '定时',
};

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}秒`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}分钟`;
  return `${Math.floor(seconds / 3600)}小时`;
}

function getHeartbeatStatus(task: Task): { status: 'ok' | 'warning' | 'dead'; text: string } {
  if (task.status !== 'running') return { status: 'ok', text: '-' };
  if (!task.last_heartbeat_at) {
    if (!task.last_run_at) return { status: 'warning', text: '未开始' };
    const elapsed = (Date.now() - new Date(task.last_run_at).getTime()) / 1000;
    if (elapsed > task.heartbeat_timeout_seconds) return { status: 'dead', text: '已超时' };
    return { status: 'warning', text: '等待心跳' };
  }
  const elapsed = (Date.now() - new Date(task.last_heartbeat_at).getTime()) / 1000;
  if (elapsed > task.heartbeat_timeout_seconds) return { status: 'dead', text: '已超时' };
  if (elapsed > task.heartbeat_timeout_seconds * 0.5)
    return { status: 'warning', text: ` ${formatDuration(Math.floor(elapsed))}前` };
  return { status: 'ok', text: '正常' };
}

function TaskRow({
  task,
  onToggleExpand,
  isExpanded,
}: {
  task: Task;
  onToggleExpand: () => void;
  isExpanded: boolean;
}) {
  const deleteMutation = useDeleteTask();
  const triggerMutation = useTriggerTask();
  const cancelMutation = useCancelTask();
  const [isDeleting, setIsDeleting] = useState(false);

  const status = statusConfig[task.status] || statusConfig.pending;
  const StatusIcon = status.icon;
  const heartbeat = getHeartbeatStatus(task);

  const handleDelete = async () => {
    if (!confirm(`确定要删除任务「${task.name}」吗？`)) return;
    setIsDeleting(true);
    try {
      await deleteMutation.mutateAsync(task.id);
      toast.success('任务已删除');
    } catch (e) {
      toast.error(`删除失败: ${e}`);
    } finally {
      setIsDeleting(false);
    }
  };

  const handleTrigger = async () => {
    try {
      await triggerMutation.mutateAsync(task.id);
      toast.success('任务已触发');
    } catch (e) {
      toast.error(`触发失败: ${e}`);
    }
  };

  const handleCancel = async () => {
    try {
      await cancelMutation.mutateAsync(task.id);
      toast.success('任务已取消');
    } catch (e) {
      toast.error(`取消失败: ${e}`);
    }
  };

  const handleRetry = async () => {
    try {
      await triggerMutation.mutateAsync(task.id);
      toast.success('任务已重试');
    } catch (e) {
      toast.error(`重试失败: ${e}`);
    }
  };

  return (
    <div className="border-b border-cinema-800 last:border-b-0">
      <div
        className="flex items-center gap-3 px-4 py-3 hover:bg-cinema-800/30 transition-colors cursor-pointer"
        onClick={onToggleExpand}
      >
        <StatusIcon
          className={cn(
            'w-4 h-4 flex-shrink-0',
            status.color,
            task.status === 'running' && 'animate-spin'
          )}
        />

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium text-sm text-white truncate">{task.name}</span>
            <span
              className={cn(
                'text-xs px-1.5 py-0.5 rounded',
                status.color.replace('text-', 'bg-').replace('400', '500/20')
              )}
            >
              {status.label}
            </span>
          </div>
          <div className="flex items-center gap-3 mt-0.5 text-xs text-gray-500">
            <span>{scheduleTypeLabels[task.schedule_type] || task.schedule_type}</span>
            {task.cron_pattern && <span className="font-mono">{task.cron_pattern}</span>}
            {task.progress > 0 && task.status === 'running' && (
              <span className="text-blue-400">{task.progress}%</span>
            )}
            {task.retry_count > 0 && (
              <span className="text-orange-400">
                重试 {task.retry_count}/{task.max_retries}
              </span>
            )}
          </div>
        </div>

        {/* Heartbeat indicator */}
        {task.status === 'running' && (
          <div className="flex items-center gap-1 text-xs">
            <Heart
              className={cn(
                'w-3 h-3',
                heartbeat.status === 'ok' && 'text-green-400 fill-green-400',
                heartbeat.status === 'warning' && 'text-yellow-400',
                heartbeat.status === 'dead' && 'text-red-400'
              )}
            />
            <span
              className={cn(
                heartbeat.status === 'ok' && 'text-green-400',
                heartbeat.status === 'warning' && 'text-yellow-400',
                heartbeat.status === 'dead' && 'text-red-400'
              )}
            >
              {heartbeat.text}
            </span>
          </div>
        )}

        {/* Progress bar */}
        {task.status === 'running' && (
          <div className="w-24 h-1.5 bg-cinema-800 rounded-full overflow-hidden">
            <div
              className="h-full bg-blue-500 rounded-full transition-all duration-300"
              style={{ width: `${task.progress}%` }}
            />
          </div>
        )}

        {/* Actions */}
        <div className="flex items-center gap-1" onClick={e => e.stopPropagation()}>
          {task.status === 'running' ? (
            <button
              onClick={handleCancel}
              className="p-1.5 rounded hover:bg-red-500/20 text-gray-400 hover:text-red-400 transition-colors"
              title="取消"
            >
              <Square className="w-3.5 h-3.5" />
            </button>
          ) : task.status === 'failed' ? (
            <button
              onClick={handleRetry}
              className="p-1.5 rounded hover:bg-yellow-500/20 text-gray-400 hover:text-yellow-400 transition-colors"
              title="重试"
            >
              <Play className="w-3.5 h-3.5" />
            </button>
          ) : (
            <button
              onClick={handleTrigger}
              className="p-1.5 rounded hover:bg-green-500/20 text-gray-400 hover:text-green-400 transition-colors"
              title="执行"
            >
              <Play className="w-3.5 h-3.5" />
            </button>
          )}
          <button
            onClick={handleDelete}
            disabled={isDeleting}
            className="p-1.5 rounded hover:bg-red-500/20 text-gray-400 hover:text-red-400 transition-colors"
            title="删除"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-gray-500" />
          ) : (
            <ChevronDown className="w-4 h-4 text-gray-500" />
          )}
        </div>
      </div>

      {/* Expanded detail */}
      {isExpanded && <TaskDetail task={task} />}
    </div>
  );
}

function CascadeRewriteDetail({ task }: { task: Task }) {
  const { data: result, isLoading } = useCascadeRewriteResult(task.id);
  const applyMutation = useApplyCascadeRewrite();
  const rejectMutation = useRejectCascadeRewrite();

  if (isLoading) {
    return <div className="mt-3 text-xs text-gray-500">加载改写结果...</div>;
  }
  if (!result) {
    return <div className="mt-3 text-xs text-gray-500">暂无改写结果</div>;
  }

  const pendingIndices = result.segments
    .map((s, i) => ({ s, i }))
    .filter(({ s }) => s.user_decision === 'pending')
    .map(({ i }) => i);

  const handleAccept = (idx: number) => {
    applyMutation.mutate(
      { taskId: task.id, acceptedIndices: [idx] },
      {
        onSuccess: () => toast.success('已接受改写'),
        onError: e => toast.error(`接受失败: ${e}`),
      }
    );
  };

  const handleReject = (idx: number) => {
    rejectMutation.mutate(
      { taskId: task.id, rejectedIndices: [idx] },
      {
        onSuccess: () => toast.success('已拒绝改写'),
        onError: e => toast.error(`拒绝失败: ${e}`),
      }
    );
  };

  const handleAcceptAll = () => {
    if (pendingIndices.length === 0) return;
    applyMutation.mutate(
      { taskId: task.id, acceptedIndices: pendingIndices },
      {
        onSuccess: () => toast.success(`已接受 ${pendingIndices.length} 处改写`),
        onError: e => toast.error(`接受失败: ${e}`),
      }
    );
  };

  const handleRejectAll = () => {
    if (pendingIndices.length === 0) return;
    rejectMutation.mutate(
      { taskId: task.id, rejectedIndices: pendingIndices },
      {
        onSuccess: () => toast.success(`已拒绝 ${pendingIndices.length} 处改写`),
        onError: e => toast.error(`拒绝失败: ${e}`),
      }
    );
  };

  return (
    <div className="mt-3">
      <div className="flex items-center justify-between mb-2">
        <h4 className="text-xs font-medium text-gray-400">
          级联改写预览 ({result.segments.length} 处)
        </h4>
        {pendingIndices.length > 0 && (
          <div className="flex gap-2">
            <button
              onClick={handleAcceptAll}
              disabled={applyMutation.isPending}
              className="px-2 py-1 text-[10px] bg-green-500/20 text-green-400 rounded hover:bg-green-500/30 transition-colors disabled:opacity-50"
            >
              全部接受
            </button>
            <button
              onClick={handleRejectAll}
              disabled={rejectMutation.isPending}
              className="px-2 py-1 text-[10px] bg-gray-500/20 text-gray-400 rounded hover:bg-gray-500/30 transition-colors disabled:opacity-50"
            >
              全部拒绝
            </button>
          </div>
        )}
      </div>

      {result.warnings.length > 0 && (
        <div className="mb-2 p-2 bg-yellow-500/10 border border-yellow-500/20 rounded text-xs text-yellow-400">
          {result.warnings.map((w, i) => (
            <div key={i}>⚠ {w}</div>
          ))}
        </div>
      )}

      <div className="space-y-2 max-h-96 overflow-y-auto">
        {result.segments.map((segment, idx) => (
          <div key={idx} className="p-2 bg-cinema-900 rounded border border-cinema-700">
            <div className="flex items-center justify-between mb-1">
              <span className="text-[10px] text-gray-500">
                场景 {segment.scene_id.slice(0, 8)}... · 段落 {segment.paragraph_index + 1}
              </span>
              <span
                className={cn(
                  'text-[10px] px-1.5 py-0.5 rounded',
                  segment.user_decision === 'pending' && 'bg-yellow-500/20 text-yellow-400',
                  segment.user_decision === 'accepted' && 'bg-green-500/20 text-green-400',
                  segment.user_decision === 'rejected' && 'bg-gray-500/20 text-gray-400'
                )}
              >
                {segment.user_decision === 'pending' && '待确认'}
                {segment.user_decision === 'accepted' && '已接受'}
                {segment.user_decision === 'rejected' && '已拒绝'}
              </span>
            </div>
            <p className="text-[10px] text-cinema-gold mb-1">{segment.change_reason}</p>
            <div className="grid grid-cols-2 gap-2">
              <div className="p-1.5 bg-red-500/10 rounded">
                <p className="text-[10px] text-red-400 mb-0.5">原文</p>
                <p className="text-[11px] text-gray-400 line-through">{segment.original_text}</p>
              </div>
              <div className="p-1.5 bg-green-500/10 rounded">
                <p className="text-[10px] text-green-400 mb-0.5">改写</p>
                <p className="text-[11px] text-gray-300">{segment.rewritten_text}</p>
              </div>
            </div>
            {segment.user_decision === 'pending' && (
              <div className="flex gap-2 mt-1.5">
                <button
                  onClick={() => handleAccept(idx)}
                  disabled={applyMutation.isPending}
                  className="px-2 py-0.5 text-[10px] bg-green-500/20 text-green-400 rounded hover:bg-green-500/30 transition-colors disabled:opacity-50"
                >
                  接受
                </button>
                <button
                  onClick={() => handleReject(idx)}
                  disabled={rejectMutation.isPending}
                  className="px-2 py-0.5 text-[10px] bg-gray-500/20 text-gray-400 rounded hover:bg-gray-500/30 transition-colors disabled:opacity-50"
                >
                  拒绝
                </button>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function TaskDetail({ task }: { task: Task }) {
  const { data: logs } = useTaskLogs(task.id);

  return (
    <div className="px-4 pb-3 bg-cinema-900/50">
      {task.description && <p className="text-xs text-gray-400 mt-2">{task.description}</p>}

      <div className="grid grid-cols-2 gap-2 mt-2 text-xs text-gray-500">
        <div>类型: {task.task_type}</div>
        <div>调度: {scheduleTypeLabels[task.schedule_type] || task.schedule_type}</div>
        {task.last_run_at && <div>上次运行: {new Date(task.last_run_at).toLocaleString()}</div>}
        {task.next_run_at && <div>下次运行: {new Date(task.next_run_at).toLocaleString()}</div>}
        {task.last_heartbeat_at && (
          <div>最后心跳: {new Date(task.last_heartbeat_at).toLocaleString()}</div>
        )}
        <div>超时阈值: {formatDuration(task.heartbeat_timeout_seconds)}</div>
      </div>

      {task.error_message && (
        <div className="mt-2 p-2 bg-red-500/10 border border-red-500/20 rounded text-xs text-red-400">
          {task.error_message}
        </div>
      )}

      {/* Cascade Rewrite Diff Viewer */}
      {task.task_type === 'cascade_rewrite' && task.result && <CascadeRewriteDetail task={task} />}

      {/* Result Preview (non-cascade) */}
      {task.result && task.task_type !== 'cascade_rewrite' && (
        <div className="mt-3">
          <h4 className="text-xs font-medium text-gray-400 mb-1">执行结果</h4>
          {(() => {
            try {
              const parsed = JSON.parse(task.result);
              if (typeof parsed.overall_score === 'number') {
                const score = Math.round(parsed.overall_score * 100);
                return (
                  <div className="mb-2 flex items-center gap-2">
                    <span className="text-xs text-gray-500">审稿评分:</span>
                    <span
                      className={cn(
                        'text-sm font-bold',
                        score >= 80
                          ? 'text-green-400'
                          : score >= 60
                            ? 'text-yellow-400'
                            : 'text-red-400'
                      )}
                    >
                      {score}%
                    </span>
                  </div>
                );
              }
              return null;
            } catch {
              return null;
            }
          })()}
          <div className="max-h-40 overflow-y-auto p-2 bg-cinema-900 rounded border border-cinema-700">
            <pre className="text-[10px] text-gray-400 whitespace-pre-wrap break-all">
              {(() => {
                try {
                  const parsed = JSON.parse(task.result);
                  return JSON.stringify(parsed, null, 2);
                } catch {
                  return task.result;
                }
              })()}
            </pre>
          </div>
        </div>
      )}

      {/* Logs */}
      {logs && logs.length > 0 && (
        <div className="mt-3">
          <h4 className="text-xs font-medium text-gray-400 mb-1">执行日志</h4>
          <div className="max-h-32 overflow-y-auto space-y-1">
            {logs.map(log => (
              <div key={log.id} className="flex items-start gap-2 text-xs">
                <span
                  className={cn(
                    'px-1 rounded text-[10px] flex-shrink-0',
                    log.log_level === 'error' && 'bg-red-500/20 text-red-400',
                    log.log_level === 'warn' && 'bg-orange-500/20 text-orange-400',
                    log.log_level === 'info' && 'bg-blue-500/20 text-blue-400'
                  )}
                >
                  {log.log_level}
                </span>
                <span className="text-gray-400">{log.message}</span>
                <span className="text-gray-600 ml-auto flex-shrink-0">
                  {new Date(log.created_at).toLocaleTimeString()}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export function Tasks() {
  const [filter, setFilter] = useState<StatusFilter>('all');
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const { data: tasks, isLoading } = useTasks(filter === 'all' ? undefined : filter);
  const createMutation = useCreateTask();
  const [showCreate, setShowCreate] = useState(false);
  const [newTask, setNewTask] = useState({
    name: '',
    description: '',
    task_type: 'custom',
    schedule_type: 'once',
    cron_pattern: '',
  });

  // Listen for task events
  useEffect(() => {
    const unlisten = listen('task-status-changed', (event: any) => {
      const { task_id, status, message } = event.payload || {};
      if (status === 'completed') {
        toast.success(message || `任务 ${task_id} 已完成`);
      } else if (status === 'failed') {
        toast.error(message || `任务 ${task_id} 失败`);
      }
    });
    return () => {
      unlisten.then(f => f());
    };
  }, []);

  const handleCreate = async () => {
    if (!newTask.name.trim()) {
      toast.error('请输入任务名称');
      return;
    }
    try {
      await createMutation.mutateAsync({
        name: newTask.name,
        description: newTask.description || undefined,
        task_type: newTask.task_type,
        schedule_type: newTask.schedule_type,
        cron_pattern: newTask.cron_pattern || undefined,
      });
      toast.success('任务创建成功');
      setShowCreate(false);
      setNewTask({
        name: '',
        description: '',
        task_type: 'custom',
        schedule_type: 'once',
        cron_pattern: '',
      });
    } catch (e) {
      toast.error(`创建失败: ${e}`);
    }
  };

  const filteredTasks = tasks || [];

  const groupedTasks = {
    running: filteredTasks.filter(t => t.status === 'running'),
    pending: filteredTasks.filter(t => t.status === 'pending'),
    completed: filteredTasks.filter(t => t.status === 'completed'),
    failed: filteredTasks.filter(t => t.status === 'failed'),
    cancelled: filteredTasks.filter(t => t.status === 'cancelled'),
  };

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <ListChecks className="w-6 h-6 text-cinema-gold" />
          <h1 className="text-2xl font-display font-bold text-white">任务管理</h1>
          <span className="text-sm text-gray-500">({filteredTasks.length})</span>
        </div>
        <button
          onClick={() => setShowCreate(!showCreate)}
          className="flex items-center gap-2 px-4 py-2 bg-cinema-gold/20 text-cinema-gold rounded-lg hover:bg-cinema-gold/30 transition-colors"
        >
          <Plus className="w-4 h-4" />
          新建任务
        </button>
      </div>

      {/* Create form */}
      {showCreate && (
        <div className="mb-6 p-4 bg-cinema-800/50 rounded-lg border border-cinema-700">
          <h3 className="text-sm font-medium text-white mb-3">新建任务</h3>
          <div className="grid grid-cols-2 gap-3">
            <input
              type="text"
              placeholder="任务名称"
              value={newTask.name}
              onChange={e => setNewTask({ ...newTask, name: e.target.value })}
              className="px-3 py-2 bg-cinema-900 border border-cinema-700 rounded text-sm text-white placeholder-gray-500 focus:outline-none focus:border-cinema-gold"
            />
            <select
              value={newTask.schedule_type}
              onChange={e => setNewTask({ ...newTask, schedule_type: e.target.value })}
              className="px-3 py-2 bg-cinema-900 border border-cinema-700 rounded text-sm text-white focus:outline-none focus:border-cinema-gold"
            >
              <option value="once">一次性</option>
              <option value="daily">每天</option>
              <option value="weekly">每周</option>
              <option value="cron">定时 (cron)</option>
            </select>
            <select
              value={newTask.task_type}
              onChange={e => setNewTask({ ...newTask, task_type: e.target.value })}
              className="px-3 py-2 bg-cinema-900 border border-cinema-700 rounded text-sm text-white focus:outline-none focus:border-cinema-gold"
            >
              <option value="custom">自定义</option>
              <option value="cascade_rewrite">级联改写</option>
              <option value="book_deconstruction">拆书分析</option>
              <option value="ai_generation">AI 生成</option>
              <option value="pipeline_review">Pipeline 审校</option>
              <option value="ingest">知识图谱 Ingest</option>
            </select>
            <input
              type="text"
              placeholder="描述 (可选)"
              value={newTask.description}
              onChange={e => setNewTask({ ...newTask, description: e.target.value })}
              className="px-3 py-2 bg-cinema-900 border border-cinema-700 rounded text-sm text-white placeholder-gray-500 focus:outline-none focus:border-cinema-gold"
            />
            {newTask.schedule_type === 'cron' && (
              <input
                type="text"
                placeholder="Cron 表达式 (如: 0 3 * * *)"
                value={newTask.cron_pattern}
                onChange={e => setNewTask({ ...newTask, cron_pattern: e.target.value })}
                className="px-3 py-2 bg-cinema-900 border border-cinema-700 rounded text-sm text-white placeholder-gray-500 focus:outline-none focus:border-cinema-gold"
              />
            )}
          </div>
          <div className="flex gap-2 mt-3">
            <button
              onClick={handleCreate}
              disabled={createMutation.isPending}
              className="px-4 py-1.5 bg-cinema-gold/20 text-cinema-gold rounded text-sm hover:bg-cinema-gold/30 transition-colors disabled:opacity-50"
            >
              {createMutation.isPending ? '创建中...' : '创建'}
            </button>
            <button
              onClick={() => setShowCreate(false)}
              className="px-4 py-1.5 text-gray-400 rounded text-sm hover:text-white transition-colors"
            >
              取消
            </button>
          </div>
        </div>
      )}

      {/* Filter tabs */}
      <div className="flex gap-1 mb-4">
        {(['all', 'running', 'pending', 'completed', 'failed'] as StatusFilter[]).map(f => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={cn(
              'px-3 py-1.5 rounded-lg text-xs font-medium transition-colors',
              filter === f
                ? 'bg-cinema-gold/20 text-cinema-gold'
                : 'text-gray-500 hover:text-gray-300 hover:bg-cinema-800/50'
            )}
          >
            {f === 'all' && '全部'}
            {f === 'running' && '执行中'}
            {f === 'pending' && '等待中'}
            {f === 'completed' && '已完成'}
            {f === 'failed' && '失败'}
          </button>
        ))}
      </div>

      {/* Loading */}
      {isLoading && (
        <div className="flex items-center justify-center py-20">
          <Loader2 className="w-6 h-6 text-gray-500 animate-spin" />
          <span className="ml-2 text-sm text-gray-500">加载中...</span>
        </div>
      )}

      {/* Empty state */}
      {!isLoading && filteredTasks.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-gray-500">
          <ListChecks className="w-12 h-12 mb-3 opacity-30" />
          <p className="text-sm">暂无任务</p>
          <p className="text-xs mt-1 opacity-50">创建一次性或定时任务来自动化工作流</p>
        </div>
      )}

      {/* Task list */}
      {!isLoading && filteredTasks.length > 0 && (
        <div className="bg-cinema-900/50 rounded-lg border border-cinema-800 overflow-hidden">
          {filter === 'all' ? (
            // Grouped by status
            <>
              {groupedTasks.running.length > 0 && (
                <>
                  <div className="px-4 py-2 bg-cinema-800/30 text-xs font-medium text-blue-400">
                    执行中 ({groupedTasks.running.length})
                  </div>
                  {groupedTasks.running.map(task => (
                    <TaskRow
                      key={task.id}
                      task={task}
                      isExpanded={expandedId === task.id}
                      onToggleExpand={() => setExpandedId(expandedId === task.id ? null : task.id)}
                    />
                  ))}
                </>
              )}
              {groupedTasks.pending.length > 0 && (
                <>
                  <div className="px-4 py-2 bg-cinema-800/30 text-xs font-medium text-gray-400">
                    等待中 ({groupedTasks.pending.length})
                  </div>
                  {groupedTasks.pending.map(task => (
                    <TaskRow
                      key={task.id}
                      task={task}
                      isExpanded={expandedId === task.id}
                      onToggleExpand={() => setExpandedId(expandedId === task.id ? null : task.id)}
                    />
                  ))}
                </>
              )}
              {groupedTasks.completed.length > 0 && (
                <>
                  <div className="px-4 py-2 bg-cinema-800/30 text-xs font-medium text-green-400">
                    已完成 ({groupedTasks.completed.length})
                  </div>
                  {groupedTasks.completed.map(task => (
                    <TaskRow
                      key={task.id}
                      task={task}
                      isExpanded={expandedId === task.id}
                      onToggleExpand={() => setExpandedId(expandedId === task.id ? null : task.id)}
                    />
                  ))}
                </>
              )}
              {groupedTasks.failed.length > 0 && (
                <>
                  <div className="px-4 py-2 bg-cinema-800/30 text-xs font-medium text-red-400">
                    失败 ({groupedTasks.failed.length})
                  </div>
                  {groupedTasks.failed.map(task => (
                    <TaskRow
                      key={task.id}
                      task={task}
                      isExpanded={expandedId === task.id}
                      onToggleExpand={() => setExpandedId(expandedId === task.id ? null : task.id)}
                    />
                  ))}
                </>
              )}
            </>
          ) : (
            // Flat list for filtered view
            filteredTasks.map(task => (
              <TaskRow
                key={task.id}
                task={task}
                isExpanded={expandedId === task.id}
                onToggleExpand={() => setExpandedId(expandedId === task.id ? null : task.id)}
              />
            ))
          )}
        </div>
      )}
    </div>
  );
}
