/**
 * 续写训练场（v0.88.0）
 *
 * 定时闭环训练：配置原书来源（栗子读书 MCP / 本地 epub·txt）→ 定时任务
 * 自动取若干章灌入独立沙盒故事 → 按原文风格续写下一章 → 与真实后续章
 * 五维对比评审 → 改进建议自动注入下一轮。报告按书聚合，看总分趋势。
 */

import { useEffect, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Dumbbell, Play } from 'lucide-react';
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
import { useCreateTask, useTriggerTask, useUpdateTask, type Task } from '@/hooks/useTasks';
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

export default function TrainingLab() {
  const currentStory = useAppStore(s => s.currentStory);
  const createTask = useCreateTask();
  const updateTask = useUpdateTask();
  const triggerTask = useTriggerTask();

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

  // 现查任务列表找同书名的既有定时计划（v0.93.2 融合：计划管理统一在任务页，
  // 训练页不再常驻轮询任务列表）。只认 cron 计划——once 行无法改为定时语义。
  const findPlanByBook = async (title: string): Promise<Task | null> => {
    const all = await loggedInvoke<Task[]>('list_tasks', { status_filter: null });
    const hit = all.find(t => {
      if (t.task_type !== 'continuation_training' || t.schedule_type !== 'cron') return false;
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
    try {
      const payload = JSON.stringify(buildPayload());
      const cronPattern = cron.trim() || DEFAULT_CRON;
      const existing = await findPlanByBook(bookTitle.trim());
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
        toast.success('训练计划已创建，到点自动执行');
      }
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  // v0.93.2 融合：立即跑 = 同步当前配置到既有计划后触发；无计划则建一次性任务
  const handleRunNow = async () => {
    try {
      const payload = JSON.stringify(buildPayload());
      const cronPattern = cron.trim() || DEFAULT_CRON;
      const existing = await findPlanByBook(bookTitle.trim());
      if (existing) {
        await updateTask.mutateAsync({
          id: existing.id,
          input: { cron_pattern: cronPattern, payload, enabled: true },
        });
        await triggerTask.mutateAsync(existing.id);
        toast.success('已按当前配置触发训练（计划与运行历史见「任务」页）');
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
        toast.success('训练任务已排队（取章→灌沙盒→续写→对比，约需几分钟）');
      }
    } catch (e) {
      toast.error((e as Error).message);
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
                  isLoading={createTask.isPending}
                >
                  创建定时训练
                </Button>
                <Button
                  variant="secondary"
                  onClick={handleRunNow}
                  isLoading={triggerTask.isPending || updateTask.isPending}
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
                      <span className="text-[10px] text-ai-ink-3">{r.created_at}</span>
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
        {detailQuery.data ? (
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
