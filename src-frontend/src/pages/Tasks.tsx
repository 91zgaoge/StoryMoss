import { confirmDialog } from '@/components/ui/confirm';
import { useState, useEffect, type CSSProperties } from 'react';
import { listen } from '@tauri-apps/api/event';
import { ListChecks, Play, Square, Trash2, Loader2, Plus } from 'lucide-react';
import { cn } from '@/utils/cn';
import { Button } from '@/components/ui/Button';
import { PageHeader } from '@/components/ui/PageHeader';
import { EmptyState } from '@/components/ui/EmptyState';
import { AiCodeBlock } from '@/components/ui/ai/AiCodeBlock';
import { AiToolChips } from '@/components/ui/ai/AiToolChips';
import { AiRecommendationCard } from '@/components/ui/ai/AiRecommendationCard';
import { AiTaskRows, type AiTaskRowItem } from '@/components/ui/ai/AiTaskRows';
import { AgencyTaskList } from '@/components/AgencyTaskList';
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
} from '@/hooks/useTasks';
import toast from 'react-hot-toast';
import { useAppStore } from '@/stores/appStore';

type StatusFilter = 'all' | 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

const statusConfig: Record<string, { label: string }> = {
  pending: { label: '等待中' },
  running: { label: '执行中' },
  completed: { label: '已完成' },
  failed: { label: '失败' },
  cancelled: { label: '已取消' },
};

const scheduleTypeLabels: Record<string, string> = {
  once: '一次性',
  daily: '每天',
  weekly: '每周',
  cron: '定时',
};

// v0.93.2: task_system 类型中文标签（训练等由专属页面创建的任务在详情里可辨识；
// 泛用创建表单不提供 continuation_training——配置复杂，必须走续写训练页）
const TASK_TYPE_LABELS: Record<string, string> = {
  custom: '自定义',
  cascade_rewrite: '级联改写',
  book_deconstruction: '拆书分析',
  guidebook_distillation: '指导书提炼',
  ai_generation: 'AI 生成',
  pipeline_review: 'Pipeline 审校',
  ingest: '知识图谱采摘',
  continuation_training: '续写训练',
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
  const heartbeat = getHeartbeatStatus(task);

  const handleDelete = async () => {
    if (!(await confirmDialog({ title: `确定要删除任务「${task.name}」吗？` }))) return;
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

  const metaParts = [
    scheduleTypeLabels[task.schedule_type] || task.schedule_type,
    task.cron_pattern || null,
    task.status === 'running' && task.progress > 0 ? `${task.progress}%` : null,
    task.retry_count > 0 ? `重试 ${task.retry_count}/${task.max_retries}` : null,
    task.status === 'running' ? `心跳${heartbeat.text}` : null,
  ].filter(Boolean);

  // P4 Task4: 纯 var() 令牌不支持 tailwind /N 透明度修饰符（构建期失效），tint 底色改 color-mix 内联
  const statusPill: Record<string, { className: string; style?: CSSProperties }> = {
    running: { className: 'bg-ai-accent-tint text-ai-accent-ink' },
    completed: {
      className: 'text-ai-green',
      style: { background: 'color-mix(in srgb, var(--ai-green) 10%, transparent)' },
    },
    failed: {
      className: 'text-ai-red',
      style: { background: 'color-mix(in srgb, var(--ai-red) 10%, transparent)' },
    },
    cancelled: {
      className: 'text-ai-orange',
      style: { background: 'color-mix(in srgb, var(--ai-orange) 10%, transparent)' },
    },
    pending: { className: 'bg-ai-hover text-ai-ink-2' },
  };

  const item: AiTaskRowItem<Task> = {
    key: task.id,
    status: task.status in statusPill ? task.status : 'pending',
    progress: task.status === 'running' ? task.progress : undefined,
    label: task.name,
    meta: metaParts.join(' · '),
    pill: (() => {
      const pill = statusPill[task.status] || statusPill.pending;
      return (
        <span
          className={cn(
            'inline-flex h-[22px] items-center rounded-full px-2 text-[11.5px] font-medium',
            pill.className
          )}
          style={pill.style}
        >
          {status.label}
        </span>
      );
    })(),
    trailing: (
      <>
        {task.status === 'running' ? (
          <button
            onClick={handleCancel}
            className="p-1.5 rounded hover:bg-[color-mix(in_srgb,var(--ai-red)_10%,transparent)] text-ai-ink-3 hover:text-ai-red transition-colors"
            title="取消"
          >
            <Square className="w-3.5 h-3.5" />
          </button>
        ) : task.status === 'failed' ? (
          <button
            onClick={handleRetry}
            className="p-1.5 rounded hover:bg-[color-mix(in_srgb,var(--ai-orange)_10%,transparent)] text-ai-ink-3 hover:text-ai-orange transition-colors"
            title="重试"
          >
            <Play className="w-3.5 h-3.5" />
          </button>
        ) : (
          <button
            onClick={handleTrigger}
            className="p-1.5 rounded hover:bg-[color-mix(in_srgb,var(--ai-green)_10%,transparent)] text-ai-ink-3 hover:text-ai-green transition-colors"
            title="执行"
          >
            <Play className="w-3.5 h-3.5" />
          </button>
        )}
        <button
          onClick={handleDelete}
          disabled={isDeleting}
          className="p-1.5 rounded hover:bg-[color-mix(in_srgb,var(--ai-red)_10%,transparent)] text-ai-ink-3 hover:text-ai-red transition-colors"
          title="删除"
        >
          <Trash2 className="w-3.5 h-3.5" />
        </button>
      </>
    ),
    payload: task,
  };

  return (
    <AiTaskRows
      rows={[item]}
      expandedKey={isExpanded ? task.id : null}
      onToggle={() => onToggleExpand()}
      renderDetail={() => <TaskDetail task={task} />}
    />
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
          <AiRecommendationCard
            key={idx}
            title={`场景 ${segment.scene_id.slice(0, 8)}… · 段落 ${segment.paragraph_index + 1}：${segment.change_reason}`}
            status={segment.user_decision}
            options={[
              {
                key: String(idx),
                short: segment.change_reason,
                signal: 0,
                label: 'AI 改写建议',
                body: (
                  <div className="grid grid-cols-2 gap-2">
                    <div
                      className="rounded-[8px] p-1.5"
                      style={{ background: 'color-mix(in srgb, var(--ai-red) 10%, transparent)' }}
                    >
                      <p className="mb-0.5 text-[10px] text-ai-red">原文</p>
                      <p className="text-[11px] text-ai-ink-3 line-through">
                        {segment.original_text}
                      </p>
                    </div>
                    <div
                      className="rounded-[8px] p-1.5"
                      style={{ background: 'color-mix(in srgb, var(--ai-green) 10%, transparent)' }}
                    >
                      <p className="mb-0.5 text-[10px] text-ai-green">改写</p>
                      <p className="text-[11px] text-ai-ink">{segment.rewritten_text}</p>
                    </div>
                  </div>
                ),
              },
            ]}
            onAccept={() => handleAccept(idx)}
            onReject={() => handleReject(idx)}
          />
        ))}
      </div>
    </div>
  );
}

/// v0.93.2: 续写训练任务详情补充——展示训练配置摘要；跑完后 result 带
/// report_id，提供跳转续写训练页查看完整报告的入口
function TrainingTaskDetail({ task }: { task: Task }) {
  const setCurrentView = useAppStore(s => s.setCurrentView);
  let config: { book_title?: string; chapter_count?: number } = {};
  let reportId: string | null = null;
  try {
    config = JSON.parse(task.payload ?? '{}') as { book_title?: string; chapter_count?: number };
  } catch {
    // 旧任务 payload 缺失时保持空
  }
  try {
    if (task.result) {
      reportId = (JSON.parse(task.result) as { report_id?: string }).report_id ?? null;
    }
  } catch {
    // result 非 JSON
  }
  if (!config.book_title && !reportId) return null;

  return (
    <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-gray-500">
      {config.book_title && (
        <span>
          训练配置：《{config.book_title}》· 上下文 {config.chapter_count ?? 10} 章
        </span>
      )}
      {reportId && (
        <button
          type="button"
          onClick={() => setCurrentView('training-lab')}
          className="rounded border border-gray-700 px-2 py-0.5 text-[11px] text-gray-300 hover:bg-gray-800"
        >
          查看训练报告
        </button>
      )}
    </div>
  );
}

function TaskDetail({ task }: { task: Task }) {
  const { data: logs } = useTaskLogs(task.id);

  return (
    <div className="px-4 pb-3 bg-cinema-900/50">
      {task.description && <p className="text-xs text-gray-400 mt-2">{task.description}</p>}

      <div className="grid grid-cols-2 gap-2 mt-2 text-xs text-gray-500">
        <div>类型: {TASK_TYPE_LABELS[task.task_type] || task.task_type}</div>
        <div>调度: {scheduleTypeLabels[task.schedule_type] || task.schedule_type}</div>
        {task.last_run_at && <div>上次运行: {new Date(task.last_run_at).toLocaleString()}</div>}
        {task.next_run_at && <div>下次运行: {new Date(task.next_run_at).toLocaleString()}</div>}
        {task.last_heartbeat_at && (
          <div>最后心跳: {new Date(task.last_heartbeat_at).toLocaleString()}</div>
        )}
        <div>超时阈值: {formatDuration(task.heartbeat_timeout_seconds)}</div>
      </div>

      {/* v0.93.2: 续写训练任务——配置摘要 + 报告入口 */}
      {task.task_type === 'continuation_training' && <TrainingTaskDetail task={task} />}

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
          <AiCodeBlock
            code={(() => {
              try {
                const parsed = JSON.parse(task.result);
                return JSON.stringify(parsed, null, 2);
              } catch {
                return task.result;
              }
            })()}
            language="json"
            maxHeight={160}
          />
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
  const [source, setSource] = useState<'automation' | 'agency'>('automation');
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
    <div className="p-6 max-w-6xl mx-auto min-w-0">
      {/* Header */}
      <PageHeader
        title={
          <span className="flex items-center gap-3">
            <ListChecks className="w-6 h-6 text-cinema-gold" />
            任务
          </span>
        }
        count={filteredTasks.length}
        actions={
          source === 'agency' ? (
            <Button variant="secondary" onClick={() => setSource('automation')}>
              查看自动化任务
            </Button>
          ) : (
            <Button variant="secondary" onClick={() => setShowCreate(!showCreate)}>
              <Plus className="w-4 h-4" />
              新建任务
            </Button>
          )
        }
      />

      {/* v0.91.0 B3：任务来源切换（自动化 task_system / 代理四角色） */}
      <div className="mb-3">
        <AiToolChips
          ariaLabel="任务来源"
          activeKey={source}
          onSelect={key => setSource(key as 'automation' | 'agency')}
          items={[
            { key: 'automation', label: '自动化任务' },
            { key: 'agency', label: '代理任务' },
          ]}
        />
      </div>

      {source === 'agency' && <AgencyTaskList />}

      {/* Create form */}
      {source === 'automation' && showCreate && (
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
              <option value="ingest">知识图谱采摘</option>
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

      {source === 'automation' && (
        <>
          {/* Filter tabs */}
          <div className="mb-4">
            <AiToolChips
              ariaLabel="任务状态筛选"
              activeKey={filter}
              onSelect={key => setFilter(key as StatusFilter)}
              items={[
                { key: 'all', label: '全部' },
                { key: 'running', label: '执行中' },
                { key: 'pending', label: '等待中' },
                { key: 'completed', label: '已完成' },
                { key: 'failed', label: '失败' },
              ]}
            />
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
            <EmptyState
              icon={ListChecks}
              title="暂无任务"
              action={{ label: '新建任务', onClick: () => setShowCreate(true) }}
            />
          )}

          {/* Task list */}
          {!isLoading && filteredTasks.length > 0 && (
            <div>
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
                          onToggleExpand={() =>
                            setExpandedId(expandedId === task.id ? null : task.id)
                          }
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
                          onToggleExpand={() =>
                            setExpandedId(expandedId === task.id ? null : task.id)
                          }
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
                          onToggleExpand={() =>
                            setExpandedId(expandedId === task.id ? null : task.id)
                          }
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
                          onToggleExpand={() =>
                            setExpandedId(expandedId === task.id ? null : task.id)
                          }
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
        </>
      )}
    </div>
  );
}
