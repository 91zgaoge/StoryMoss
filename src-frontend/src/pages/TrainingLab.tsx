/**
 * 续写训练场（v0.88.0）
 *
 * 定时闭环训练：配置原书来源（栗子读书 MCP / 本地 epub·txt）→ 定时任务
 * 自动取若干章灌入独立沙盒故事 → 按原文风格续写下一章 → 与真实后续章
 * 五维对比评审 → 改进建议自动注入下一轮。报告按书聚合，看总分趋势。
 */

import { useEffect, useMemo, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { AlertTriangle, CheckCircle2, Dumbbell, Loader2, Play, RotateCcw } from 'lucide-react';
import toast from 'react-hot-toast';
import { PageHeader } from '@/components/ui/PageHeader';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input, Select, Textarea } from '@/components/ui/controls';
import { Field } from '@/components/ui/Field';
import { Modal } from '@/components/ui/Modal';
import { Panel } from '@/components/ui/Panel';
import { AiInsightCards } from '@/components/ui/ai/AiInsightCards';
import { loggedInvoke } from '@/services/tauri';
import { mcpListSavedServers, type SavedMcpServer } from '@/services/api/skills';
import { useMcpTools } from '@/hooks/useMcpTools';
import {
  useCreateTask,
  useTriggerTask,
  useUpdateTask,
  useTasks,
  useTaskLogs,
  type Task,
} from '@/hooks/useTasks';
import { focusTaskInAllTasks, showTaskToast } from '@/utils/taskToast';
import { formatLocalDateTime } from '@/utils/datetime';
import { useAppStore } from '@/stores/appStore';
import {
  bareMcpToolName,
  buildMcpArgsTemplate,
  formatArgsTemplate,
  pickChapterTool,
  serverIdFromMcpToolName,
} from '@/utils/mcp';
import { cn } from '@/utils/cn';

interface TrainingReportSummary {
  id: string;
  book_title: string;
  story_id: string;
  start_chapter: number;
  context_chapters: number;
  generated_chapter: number | null;
  overall: number | null;
  created_at: string;
}

interface TrainingReportDetail {
  id: string;
  book_title: string;
  report_md: string;
  improvement_md: string | null;
  truth_content: string | null;
  scores_json: string | null;
  created_at: string;
}

const DEFAULT_CRON = '0 9 * * *';

/** v0.93.12: 训练任务状态面板用——读取 payload 里的书名聚合键 */
function payloadBookTitle(task: Task): string | null {
  try {
    const parsed = JSON.parse(task.payload ?? '{}') as { book_title?: string };
    return parsed.book_title?.trim() || null;
  } catch {
    return null;
  }
}

const TRAINING_STATUS_LABEL: Record<string, string> = {
  running: '执行中',
  failed: '失败',
  completed: '已完成',
  pending: '等待执行',
  cancelled: '已取消',
};

/** 状态色令牌（tint 走 color-mix，避免 tailwind /N 在纯 var() 上失效） */
function trainingStatusTone(status: string): { color: string; tint: string } {
  if (status === 'failed') {
    return {
      color: 'text-ai-red',
      tint: 'color-mix(in srgb, var(--ai-red) 12%, transparent)',
    };
  }
  if (status === 'completed') {
    return {
      color: 'text-ai-green',
      tint: 'color-mix(in srgb, var(--ai-green) 12%, transparent)',
    };
  }
  if (status === 'running') {
    return { color: 'text-ai-accent-ink', tint: 'var(--ai-accent-tint)' };
  }
  return { color: 'text-ai-ink-2', tint: 'var(--ai-hover)' };
}

export default function TrainingLab() {
  const currentStory = useAppStore(s => s.currentStory);
  const createTask = useCreateTask();
  const updateTask = useUpdateTask();
  const triggerTask = useTriggerTask();
  // v0.94.x: find→create/update 的异步 upsert 互斥锁。仅靠 mutation.isPending
  // 挡不住「两次点击都先读到不存在再各自创建」——ref 同步置位，state 供 UI 禁用。
  const upsertBusyRef = useRef(false);
  const [upsertBusy, setUpsertBusy] = useState(false);
  const acquireUpsertLock = (): boolean => {
    if (upsertBusyRef.current) return false;
    upsertBusyRef.current = true;
    setUpsertBusy(true);
    return true;
  };
  const releaseUpsertLock = () => {
    upsertBusyRef.current = false;
    setUpsertBusy(false);
  };

  // ── 配置表单 ──────────────────────────────────────────────
  const [bookTitle, setBookTitle] = useState('');
  const [sourceKind, setSourceKind] = useState<'mcp' | 'file'>('mcp');
  const [serverId, setServerId] = useState('');
  const [chapterTool, setChapterTool] = useState('');
  const [argsTemplate, setArgsTemplate] = useState('');
  const [filePath, setFilePath] = useState('');
  const [chapterCount, setChapterCount] = useState(10);
  const [strategy, setStrategy] = useState('fixed');
  const [startChapter, setStartChapter] = useState(1);
  const [estTotal, setEstTotal] = useState(0);
  const [cron, setCron] = useState(DEFAULT_CRON);

  const { externalTools, connections, refreshConnections, isConnecting } = useMcpTools();
  const savedServersQuery = useQuery({
    queryKey: ['mcp-saved-servers'],
    queryFn: mcpListSavedServers,
  });
  const savedServers: SavedMcpServer[] = savedServersQuery.data ?? [];

  useEffect(() => {
    void refreshConnections();
  }, [refreshConnections]);

  const serverTools = useMemo(
    () =>
      serverId ? externalTools.filter(tool => serverIdFromMcpToolName(tool.name) === serverId) : [],
    [externalTools, serverId]
  );

  useEffect(() => {
    if (!serverId && savedServers.length > 0) {
      const preferred = savedServers.find(server => server.enabled);
      if (preferred) setServerId(preferred.id);
    }
  }, [savedServers, serverId]);

  useEffect(() => {
    if (!serverId || serverTools.length === 0) return;
    const available = serverTools.map(tool => bareMcpToolName(tool.name));
    const preferred =
      chapterTool && available.includes(chapterTool) ? chapterTool : pickChapterTool(serverTools);
    if (!preferred) return;
    if (preferred !== chapterTool) setChapterTool(preferred);
    if (!argsTemplate.trim()) {
      const tool = serverTools.find(item => bareMcpToolName(item.name) === preferred);
      setArgsTemplate(formatArgsTemplate(buildMcpArgsTemplate(tool)));
    }
  }, [serverId, serverTools, chapterTool, argsTemplate]);

  const reportsQuery = useQuery({
    queryKey: ['training-reports'],
    queryFn: () =>
      loggedInvoke<TrainingReportSummary[]>('training_list_reports', { book_title: null }),
    refetchInterval: 15_000,
  });
  const reports = reportsQuery.data ?? [];

  // v0.93.12: 训练任务状态面板（此前训练页只有报告，没有任务运行/失败状态；
  // 真机 2026-11-09 14:37 run 917dd138 失败后用户停留在训练页完全无感）。
  const tasksQuery = useTasks();
  const trainingTasks = useMemo(
    () => (tasksQuery.data ?? []).filter(t => t.task_type === 'continuation_training'),
    [tasksQuery.data]
  );
  const statusTask = useMemo(() => {
    const input = bookTitle.trim();
    const sorted = [...trainingTasks].sort((a, b) =>
      (b.updated_at ?? '').localeCompare(a.updated_at ?? '')
    );
    // v0.94.x: 填了书名时只显示该书的训练任务；找不到就是没有，
    // 不再回退到「最近一条训练任务」（那会把别的书的运行态显示成本书状态）。
    if (input) return sorted.find(t => payloadBookTitle(t) === input);
    return sorted[0];
  }, [trainingTasks, bookTitle]);
  const statusLogs = useTaskLogs(statusTask?.id);
  const latestStatusLogs = (statusLogs.data ?? []).slice(0, 2);

  // 现查任务列表找同书名的既有训练任务（v0.93.2 融合：计划管理统一在任务页，
  // 训练页不再常驻轮询任务列表）。v0.93.7：cron 计划与一次性任务都按书名
  // 复用——同书任务始终只有一行，历史日志聚合在一处。
  const findTrainingTaskByBook = async (
    title: string,
    scheduleType: 'cron' | 'once'
  ): Promise<Task | null> => {
    const all = await loggedInvoke<Task[]>('list_tasks', { status_filter: null });
    const hit = all.find(t => {
      if (t.task_type !== 'continuation_training' || t.schedule_type !== scheduleType) return false;
      try {
        return (JSON.parse(t.payload ?? '{}') as { book_title?: string }).book_title === title;
      } catch {
        return false;
      }
    });
    return hit ?? null;
  };

  const books = useMemo(() => [...new Set(reports.map(r => r.book_title))], [reports]);

  const trendByBook = useMemo(() => {
    // 每本书最近 10 轮 overall 趋势（旧→新）
    const byBook: Record<string, number[]> = {};
    for (const r of [...reports].reverse()) {
      if (r.overall == null) continue;
      (byBook[r.book_title] ??= []).push(r.overall);
    }
    return byBook;
  }, [reports]);

  const [detailId, setDetailId] = useState<string | null>(null);
  const detailQuery = useQuery({
    queryKey: ['training-report', detailId],
    queryFn: () =>
      loggedInvoke<TrainingReportDetail>('training_get_report', { report_id: detailId! }),
    enabled: !!detailId,
  });

  const buildPayload = () => {
    if (!bookTitle.trim()) throw new Error('请填写书名');
    if (sourceKind === 'mcp') {
      if (!serverId) throw new Error('请选择 MCP 服务器');
      let template: Record<string, unknown>;
      try {
        template = argsTemplate.trim() ? (JSON.parse(argsTemplate) as Record<string, unknown>) : {};
      } catch {
        throw new Error('参数模板不是合法 JSON');
      }
      return {
        book_title: bookTitle.trim(),
        chapter_count: chapterCount,
        est_total_chapters: estTotal,
        strategy,
        start_chapter: strategy === 'fixed' ? startChapter : undefined,
        source: {
          kind: 'mcp',
          server_id: serverId,
          chapter_tool: chapterTool.trim(),
          args_template: template,
        },
      };
    }
    if (!filePath.trim()) throw new Error('请填写本地书籍路径（epub/txt/pdf）');
    return {
      book_title: bookTitle.trim(),
      chapter_count: chapterCount,
      est_total_chapters: estTotal,
      strategy,
      start_chapter: strategy === 'fixed' ? startChapter : undefined,
      source: { kind: 'file', path: filePath.trim() },
    };
  };

  // v0.93.2 融合：创建/更新定时训练（同书名计划 upsert，任务表每书一行）
  const handleCreateSchedule = async () => {
    if (!acquireUpsertLock()) return;
    try {
      const payload = JSON.stringify(buildPayload());
      const cronPattern = cron.trim() || DEFAULT_CRON;
      const existing = await findTrainingTaskByBook(bookTitle.trim(), 'cron');
      if (existing) {
        await updateTask.mutateAsync({
          id: existing.id,
          input: { cron_pattern: cronPattern, payload, enabled: true },
        });
        toast.success('已更新该书名下的既有训练计划');
      } else {
        await createTask.mutateAsync({
          name: `续写训练 · ${bookTitle.trim()}`,
          description: `定时取《${bookTitle.trim()}》章节续写并与真实后续章对比`,
          task_type: 'continuation_training',
          schedule_type: 'cron',
          cron_pattern: cronPattern,
          payload,
          enabled: true,
          max_retries: 1,
          heartbeat_timeout_seconds: 3600,
        });
        // 创建成功的通知由 App 顶层 task-created 监听统一弹出（带「查看任务」）
      }
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      releaseUpsertLock();
    }
  };

  // v0.93.12: 训练任务失败时的就地重试（无需先去「全部任务」页）
  const handleRetryTask = async (task: Task) => {
    try {
      await triggerTask.mutateAsync(task.id);
      showTaskToast({
        taskId: task.id,
        title: '已重新触发训练任务',
        message: '进度与日志见「查看任务」',
        tone: 'info',
      });
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  // v0.93.2 融合：立即跑 = 同步当前配置到既有计划后触发；无计划则建一次性任务
  const handleRunNow = async () => {
    if (!acquireUpsertLock()) return;
    try {
      const payload = JSON.stringify(buildPayload());
      const cronPattern = cron.trim() || DEFAULT_CRON;
      const existing = await findTrainingTaskByBook(bookTitle.trim(), 'cron');
      if (existing) {
        await updateTask.mutateAsync({
          id: existing.id,
          input: { cron_pattern: cronPattern, payload, enabled: true },
        });
        await triggerTask.mutateAsync(existing.id);
        showTaskToast({
          taskId: existing.id,
          title: '训练已触发',
          message: '按当前配置执行一轮训练，进度与日志见「查看任务」',
          tone: 'info',
        });
      } else {
        // v0.93.7: 无 cron 计划时也复用同书一次性任务行（此前每次「立即跑」
        // 都新建一行堆积）；once 行无法改定时语义，只同步配置后触发
        const onceRow = await findTrainingTaskByBook(bookTitle.trim(), 'once');
        if (onceRow) {
          await updateTask.mutateAsync({
            id: onceRow.id,
            input: { payload, enabled: true },
          });
          await triggerTask.mutateAsync(onceRow.id);
          showTaskToast({
            taskId: onceRow.id,
            title: '训练已触发',
            message: '按当前配置执行一轮训练，进度与日志见「查看任务」',
            tone: 'info',
          });
        } else {
          await createTask.mutateAsync({
            name: `续写训练 · ${bookTitle.trim()}（手动）`,
            description: '手动触发一轮训练',
            task_type: 'continuation_training',
            schedule_type: 'once',
            payload,
            enabled: true,
            max_retries: 0,
            heartbeat_timeout_seconds: 3600,
          });
          // 创建成功的通知由 App 顶层 task-created 监听统一弹出（带「查看任务」）
        }
      }
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      releaseUpsertLock();
    }
  };

  return (
    <div
      className="flex h-full min-h-0 flex-col gap-4 overflow-y-auto p-4"
      data-testid="training-lab"
    >
      <PageHeader
        title="续写训练"
        meta="取原书章节 → 沙盒续写 → 与真实后续章五维对比 → 改进建议自动注入下一轮"
      />

      {/* v0.93.12: 常驻训练任务状态（完成/失败/执行中）——此前训练页只有报告，
          任务失败时用户停留在本页完全无感，也没有跳到具体任务的入口。 */}
      {statusTask &&
        (() => {
          const tone = trainingStatusTone(statusTask.status);
          const label = TRAINING_STATUS_LABEL[statusTask.status] ?? statusTask.status;
          const book = payloadBookTitle(statusTask);
          const running = statusTask.status === 'running';
          const failed = statusTask.status === 'failed';
          const completed = statusTask.status === 'completed';
          return (
            <Card data-testid="training-status-card">
              <CardContent className="flex flex-wrap items-start gap-3 p-4">
                <span
                  className="flex size-8 shrink-0 items-center justify-center rounded-full"
                  style={{ background: tone.tint }}
                  aria-hidden
                >
                  {failed ? (
                    <AlertTriangle className={cn('h-4 w-4', tone.color)} />
                  ) : running ? (
                    <Loader2 className={cn('h-4 w-4 animate-spin', tone.color)} />
                  ) : (
                    <CheckCircle2 className={cn('h-4 w-4', tone.color)} />
                  )}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <h3 className="text-sm font-semibold text-ai-ink">训练任务状态</h3>
                    <span
                      className="rounded-full px-2 py-0.5 text-[11px] font-medium"
                      style={{ background: tone.tint }}
                    >
                      <span className={tone.color}>{label}</span>
                    </span>
                    {running && statusTask.progress > 0 && (
                      <span className="text-[11px] text-ai-ink-3">进度 {statusTask.progress}%</span>
                    )}
                    <span className="text-[11px] text-ai-ink-3">
                      {book ? `《${book}》 · ` : ''}
                      {statusTask.name} · 更新 {formatLocalDateTime(statusTask.updated_at)}
                    </span>
                  </div>
                  {failed && statusTask.error_message && (
                    <div
                      className="mt-2 rounded-[8px] p-2 text-xs leading-5"
                      style={{
                        background: 'color-mix(in srgb, var(--ai-red) 10%, transparent)',
                      }}
                    >
                      <p className="text-ai-red">{statusTask.error_message}</p>
                      <p className="mt-1 text-ai-ink-3">
                        可在「全部任务」查看完整执行日志并重试；训练页下方报告只反映成功轮次。
                      </p>
                    </div>
                  )}
                  {running && latestStatusLogs.length > 0 && (
                    <p className="mt-1 truncate text-[11px] text-ai-ink-3">
                      最新日志 {formatLocalDateTime(latestStatusLogs[0].created_at)}：
                      {latestStatusLogs[0].message}
                    </p>
                  )}
                  {completed && (
                    <p className="mt-1 text-[11px] text-ai-ink-3">
                      训练完成；报告见下方列表，点击「查看任务」可看执行结果与日志。
                    </p>
                  )}
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  {failed && (
                    <Button
                      variant="secondary"
                      onClick={() => void handleRetryTask(statusTask)}
                      isLoading={triggerTask.isPending}
                    >
                      <RotateCcw className="mr-1 h-3.5 w-3.5" />
                      重试
                    </Button>
                  )}
                  <Button
                    variant="secondary"
                    onClick={() => focusTaskInAllTasks(statusTask.id)}
                    data-testid="training-status-open-task"
                  >
                    查看任务
                  </Button>
                </div>
              </CardContent>
            </Card>
          );
        })()}

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-[420px_minmax(0,1fr)]">
        {/* 配置列 */}
        <div className="flex flex-col gap-4">
          <Card>
            <CardContent className="flex flex-col gap-3 p-4">
              <Field label="书名（报告聚合键）">
                <Input
                  value={bookTitle}
                  onChange={e => setBookTitle(e.target.value)}
                  placeholder="例如：阴脉先生"
                />
              </Field>
              <Field label="章节来源">
                <Select
                  value={sourceKind}
                  onChange={e => setSourceKind(e.target.value as 'mcp' | 'file')}
                >
                  <option value="mcp">MCP 服务器（栗子读书等）</option>
                  <option value="file">本地文件（epub/txt/pdf）</option>
                </Select>
              </Field>
              {sourceKind === 'mcp' ? (
                <>
                  <Field label="MCP 服务器（自动连接）">
                    <Select
                      value={serverId}
                      onChange={e => {
                        setServerId(e.target.value);
                        setChapterTool('');
                        setArgsTemplate('');
                      }}
                    >
                      <option value="">
                        {savedServers.length ? '自动选择服务器' : '暂无已保存服务器'}
                      </option>
                      {savedServers.map(s => {
                        const status = connections.find(item => item.id === s.id);
                        return (
                          <option key={s.id} value={s.id}>
                            {s.name}（{s.config.url ? 'HTTP' : 'stdio'} ·{' '}
                            {status?.connected ? '已连接' : s.enabled ? '未连接' : '已停用'}）
                          </option>
                        );
                      })}
                    </Select>
                  </Field>
                  <Field
                    label={isConnecting ? '章节正文工具（自动加载中）' : '章节正文工具（自动选择）'}
                  >
                    <Select
                      value={chapterTool}
                      onChange={e => {
                        setChapterTool(e.target.value);
                        const tool = serverTools.find(
                          item => bareMcpToolName(item.name) === e.target.value
                        );
                        setArgsTemplate(formatArgsTemplate(buildMcpArgsTemplate(tool)));
                      }}
                    >
                      <option value="">
                        {serverTools.length ? '自动选择工具' : '连接服务器后自动选择'}
                      </option>
                      {serverTools.map(tool => {
                        const bare = bareMcpToolName(tool.name);
                        return (
                          <option key={tool.name} value={bare}>
                            {bare}
                          </option>
                        );
                      })}
                    </Select>
                  </Field>
                  <Panel title="高级参数" collapsible defaultOpen={false}>
                    <Field label="工具参数模板（自动生成，可覆盖）">
                      <Textarea
                        value={argsTemplate}
                        onChange={e => setArgsTemplate(e.target.value)}
                        rows={3}
                        className="font-mono text-xs"
                      />
                    </Field>
                  </Panel>
                </>
              ) : (
                <Field label="本地书籍路径">
                  <Input
                    value={filePath}
                    onChange={e => setFilePath(e.target.value)}
                    placeholder="/path/to/book.epub"
                  />
                </Field>
              )}
              <div className="grid grid-cols-2 gap-3">
                <Field label="上下文章数（2-20）">
                  <Input
                    type="number"
                    value={chapterCount}
                    min={2}
                    max={20}
                    onChange={e => setChapterCount(Number(e.target.value) || 10)}
                  />
                </Field>
                <Field label="取章策略">
                  <Select value={strategy} onChange={e => setStrategy(e.target.value)}>
                    <option value="fixed">从指定章连取</option>
                    <option value="random">随机起点（需预计总章数）</option>
                  </Select>
                </Field>
              </div>
              {strategy === 'fixed' ? (
                <Field label="起始章号">
                  <Input
                    type="number"
                    value={startChapter}
                    min={1}
                    onChange={e => setStartChapter(Number(e.target.value) || 1)}
                  />
                </Field>
              ) : (
                <Field label="预计总章数（随机上界）">
                  <Input
                    type="number"
                    value={estTotal}
                    min={0}
                    onChange={e => setEstTotal(Number(e.target.value) || 0)}
                  />
                </Field>
              )}
              <Field label="定时计划（cron，留空默认每天 09:00）">
                <Input
                  value={cron}
                  onChange={e => setCron(e.target.value)}
                  placeholder={DEFAULT_CRON}
                />
              </Field>
              <div className="flex gap-2">
                <Button
                  variant="primary"
                  onClick={handleCreateSchedule}
                  isLoading={createTask.isPending || upsertBusy}
                  disabled={upsertBusy}
                >
                  创建定时训练
                </Button>
                <Button
                  variant="secondary"
                  onClick={handleRunNow}
                  isLoading={triggerTask.isPending || updateTask.isPending || upsertBusy}
                  disabled={upsertBusy}
                >
                  <Play className="mr-1 h-3.5 w-3.5" />
                  立即跑一轮
                </Button>
              </div>
              <p className="text-[11px] text-ai-ink-3">
                训练计划与运行历史在「任务」页统一管理（同书名计划只保留一份）。
              </p>
              {sourceKind === 'mcp' && currentStory && (
                <p className="text-[11px] text-ai-ink-3">
                  训练使用独立沙盒故事，不影响当前故事《{currentStory.title}》。
                </p>
              )}
            </CardContent>
          </Card>
        </div>

        {/* 报告列 */}
        <div className="flex flex-col gap-4">
          {books.length > 0 && (
            <Card>
              <CardContent className="p-4">
                <h3 className="mb-2 text-sm font-semibold text-ai-ink">
                  总分趋势（每本书最近 10 轮，旧→新）
                </h3>
                <div className="flex flex-col gap-2">
                  {Object.entries(trendByBook).map(([book, scores]) => (
                    <div key={book} className="flex items-center gap-2 text-xs">
                      <span className="w-28 shrink-0 truncate text-ai-ink-2">{book}</span>
                      <div
                        className="flex min-w-0 flex-1 items-end gap-1"
                        data-testid={`training-trend-${book}`}
                      >
                        {scores.slice(-10).map((v, i) => (
                          <span
                            key={i}
                            title={v.toFixed(2)}
                            className={cn(
                              'inline-block w-3 rounded-sm',
                              v >= 0.75
                                ? 'bg-ai-green/70'
                                : v >= 0.5
                                  ? 'bg-ai-orange/70'
                                  : 'bg-ai-red/70'
                            )}
                            style={{ height: `${Math.max(4, Math.round(v * 32))}px` }}
                          />
                        ))}
                        <span className="ml-2 text-[10px] text-ai-ink-3">
                          最新 {(scores[scores.length - 1] ?? 0).toFixed(2)}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}

          <Card>
            <CardContent className="p-4">
              <h3 className="mb-2 text-sm font-semibold text-ai-ink">
                训练报告（{reports.length}）
              </h3>
              {reports.length === 0 ? (
                <p className="text-xs text-ai-ink-3">
                  还没有训练报告。填好上面的配置后「立即跑一轮」试试。
                </p>
              ) : (
                <div className="flex flex-col gap-1">
                  {reports.map(r => (
                    <button
                      key={r.id}
                      type="button"
                      onClick={() => setDetailId(r.id)}
                      className="flex items-center gap-2 rounded border border-ai-line px-2 py-1.5 text-left text-xs hover:bg-ai-hover"
                    >
                      <Dumbbell className="h-3.5 w-3.5 shrink-0 text-ai-accent-ink" />
                      <span className="w-28 shrink-0 truncate text-ai-ink">{r.book_title}</span>
                      <span className="text-[10px] text-ai-ink-3">
                        第 {r.start_chapter}–{r.start_chapter + r.context_chapters - 1} 章 → 续写第{' '}
                        {r.generated_chapter ?? '?'} 章
                      </span>
                      <span
                        className={cn(
                          'ml-auto rounded px-1.5 py-0.5 text-[10px]',
                          (r.overall ?? 0) >= 0.75
                            ? 'bg-ai-green/15 text-ai-green'
                            : (r.overall ?? 0) >= 0.5
                              ? 'bg-ai-orange/15 text-ai-orange'
                              : 'bg-ai-red/15 text-ai-red'
                        )}
                      >
                        {r.overall != null ? r.overall.toFixed(2) : '—'}
                      </span>
                      <span className="text-[10px] text-ai-ink-3">
                        {formatLocalDateTime(r.created_at)}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      </div>

      {/* 报告详情 */}
      <Modal
        open={!!detailId}
        onClose={() => setDetailId(null)}
        title={`训练报告 · ${detailQuery.data?.book_title ?? ''}`}
        size="xl"
        testId="training-report-modal"
      >
        {detailQuery.isError ? (
          <p className="p-4 text-xs text-ai-red">
            加载训练报告失败：{String(detailQuery.error ?? '未知错误')}
          </p>
        ) : detailQuery.data ? (
          <div
            className="flex flex-col gap-3 overflow-y-auto"
            style={{ maxHeight: 'min(560px, 60vh)' }}
          >
            <AiInsightCards
              columns={2}
              items={[
                {
                  key: 'improvement',
                  label: '下一轮改进（自动注入续写指令）',
                  value: detailQuery.data.improvement_md || '（本轮无改进建议）',
                  tone: 'accent',
                },
              ]}
            />
            <pre className="whitespace-pre-wrap rounded border border-ai-line bg-ai-inset p-3 font-mono text-xs leading-5 text-ai-ink">
              {detailQuery.data.report_md}
            </pre>
          </div>
        ) : (
          <p className="p-4 text-xs text-ai-ink-3">加载中…</p>
        )}
      </Modal>
    </div>
  );
}
