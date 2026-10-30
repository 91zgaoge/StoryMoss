import { useEffect, useMemo, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Activity,
  Bot,
  Clapperboard,
  Layers,
  ListChecks,
  MessageSquare,
  PenLine,
  Plus,
  RefreshCw,
  ShieldCheck,
  Square,
  Wifi,
} from 'lucide-react';
import toast from 'react-hot-toast';
import { useAppStore } from '@/stores/appStore';
import {
  AGENCY_ROLES,
  useAgencyActivityStore,
  type AgentActivityEvent,
  type AgentProgressEvent,
} from '@/stores/agencyActivityStore';
import { EmptyHint } from '@/components/ui/EmptyHint';
import { AiThinking } from '@/components/ui/ai/AiThinking';
import { AiContextCards } from '@/components/ui/ai/AiContextCards';
import { AiPromptBar } from '@/components/ui/ai/AiPromptBar';
import { loggedInvoke } from '@/services/tauri';
import {
  crewCancelTask,
  crewRetryTask,
  crewStatus,
  directorExecute,
  directorPatrol,
  directorReject,
  getRun,
  listActivities,
  listBoard,
  listChatMessages,
  listChatSessions,
  listRuns,
  newChatSession,
  sendChat,
  type AgencyRun,
  type ChatMessage,
  type ChatSession,
  type CrewAlert,
  type CrewSnapshot,
} from '@/services/api/agency';
import type { BoardItem } from '@/services/api/agency';
import type { ViewType } from '@/types';
import { cn } from '@/utils/cn';

const ROLES = AGENCY_ROLES;
const ROLE_NAMES: Record<string, string> = {
  director: '导演',
  lead_writer: '主创',
  producer: '管理',
  editor_auditor: '编审',
  writer: '写手',
  inspector: '检查',
  outline_planner: '大纲',
  style_mimic: '风格',
};
const ROLE_META: Record<string, { icon: typeof Bot; role: string; lane: string }> = {
  director: { icon: Clapperboard, role: '总控 · 统筹调度', lane: 'inspect' },
  producer: { icon: Layers, role: '资产 · 计划', lane: 'write' },
  editor_auditor: { icon: ShieldCheck, role: '审查 · 质量门', lane: 'review' },
  lead_writer: { icon: PenLine, role: '正文 · 续写', lane: 'write' },
};
const ZONES: { key: BoardItem['zone']; name: string }[] = [
  { key: 'asset', name: '资产' },
  { key: 'draft', name: '草稿' },
  { key: 'review', name: '审查' },
  { key: 'schedule', name: '计划' },
  { key: 'direction', name: '导演区' },
];
const ZONE_NAMES = Object.fromEntries(ZONES.map(z => [z.key, z.name]));
const QUICK = [
  { label: '巡检全书状态', text: '/巡检' },
  { label: '让管理补齐资产', text: '让管理补齐世界观和角色关系，只补空，不覆盖手写内容。' },
  { label: '让编审复审最新章', text: '让编审复审最新一章，把阻塞问题列出来。' },
  { label: '让主创续写下一拍', text: '让主创续写下一拍；先给方案，等我确认再动正文。' },
];

interface TimelineEntry {
  at: number;
  text: string;
  role?: string;
  action?: string;
  detail?: string;
  phase?: string;
  status?: string;
}

function timelineKey(t: TimelineEntry): string {
  return `${t.role ?? ''}|${t.action ?? ''}|${t.detail ?? ''}|${t.phase ?? ''}|${t.status ?? ''}`;
}

function hhmmss(at: number) {
  const d = new Date(at);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

function roleName(key: string) {
  return ROLE_NAMES[key] ?? key;
}

function isRunActive(status: string) {
  return status === 'pending' || status === 'running' || status === 'observing';
}

function runStatusLabel(status: string) {
  switch (status) {
    case 'completed':
      return '完成';
    case 'failed':
      return '失败';
    case 'cancelled':
      return '取消';
    case 'pending':
      return '等待';
    case 'running':
      return '运行中';
    case 'observing':
      return '观察中';
    case 'idle':
      return '待命';
    default:
      return status;
  }
}

function statusColor(status: string) {
  if (status === 'failed' || status === 'stale') return 'text-ai-red';
  if (
    status === 'running' ||
    status === 'queued' ||
    status === 'cancelling' ||
    status === 'waiting'
  )
    return 'text-ai-orange';
  return 'text-ai-green';
}

function boardItemView(itemType: string): ViewType | null {
  switch (itemType) {
    case 'character':
    case 'relationship':
      return 'characters';
    case 'world':
      return 'world_building';
    case 'outline':
    case 'scene_outline':
    case 'story_outline':
      return 'stories';
    default:
      return null;
  }
}

function parseApprovalActions(msg: ChatMessage): Array<Record<string, unknown>> {
  if (!msg.actions_json) return [];
  try {
    const parsed = JSON.parse(msg.actions_json);
    if (Array.isArray(parsed.actions)) return parsed.actions;
    if (Array.isArray(parsed)) return parsed;
  } catch {
    /* ignore malformed */
  }
  return [];
}

function actionSummary(action: Record<string, unknown>) {
  let spec: Record<string, unknown> = {};
  try {
    spec = JSON.parse(String(action.params_json ?? '{}'));
  } catch {
    /* ignore */
  }
  const type = String(spec.type ?? action.action_type ?? '动作');
  const role = spec.role ? roleName(String(spec.role)) : '';
  const instruction = spec.instruction ? String(spec.instruction) : '';
  return `${type}${role ? ` · ${role}` : ''}${instruction ? `：${instruction}` : ''}`;
}

export default function AgencyStudio() {
  const currentStory = useAppStore(s => s.currentStory);
  const setCurrentView = useAppStore(s => s.setCurrentView);
  const queryClient = useQueryClient();
  const activities = useAgencyActivityStore(s => s.activities);
  const progress = useAgencyActivityStore(s => s.progress);
  const activeRunId = useAgencyActivityStore(s => s.activeRunId);
  const hydrateFromRuns = useAgencyActivityStore(s => s.hydrateFromRuns);
  const setActiveRunId = useAgencyActivityStore(s => s.setActiveRunId);

  const [targetRole, setTargetRole] = useState('director');
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [input, setInput] = useState('');
  const [liveTab, setLiveTab] = useState<'now' | 'board' | 'timeline'>('now');
  const [focus, setFocus] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const storyId = currentStory?.id ?? null;
  const runsQuery = useQuery({
    queryKey: ['agency-runs', storyId],
    queryFn: () => listRuns(storyId!),
    enabled: !!storyId,
    refetchInterval: 10_000,
  });
  const runs = runsQuery.data ?? [];
  useEffect(() => {
    if (runsQuery.data && currentStory) hydrateFromRuns(runsQuery.data, currentStory.id);
  }, [runsQuery.data, currentStory, hydrateFromRuns]);

  const boardQuery = useQuery({
    queryKey: ['agency-board', activeRunId],
    queryFn: () => listBoard(activeRunId!),
    enabled: !!activeRunId,
    refetchInterval: 10_000,
  });
  const board = boardQuery.data;
  const runQuery = useQuery({
    queryKey: ['agency-run', activeRunId],
    queryFn: () => getRun(activeRunId!),
    enabled: !!activeRunId,
    refetchInterval: 10_000,
  });
  const run = runQuery.data;
  const activitiesQuery = useQuery({
    queryKey: ['agency-activities', activeRunId],
    queryFn: () => listActivities(activeRunId!),
    enabled: !!activeRunId,
    refetchInterval: 3_000,
  });
  const dbLogs = activitiesQuery.data ?? [];
  const sessionsQuery = useQuery({
    queryKey: ['agency-chat-sessions', storyId],
    queryFn: () => listChatSessions(storyId!),
    enabled: !!storyId,
    refetchInterval: 10_000,
  });
  const sessions = sessionsQuery.data ?? [];
  const crewQuery = useQuery({
    queryKey: ['agency-crew', storyId],
    queryFn: () => crewStatus(storyId!),
    enabled: !!storyId,
    refetchInterval: 3_000,
  });
  const crew: CrewSnapshot | undefined = crewQuery.data;
  const messagesQuery = useQuery({
    queryKey: ['agency-chat', sessionId],
    queryFn: () => listChatMessages(sessionId!, 200),
    enabled: !!sessionId,
    refetchInterval: 2_000,
  });
  const messages = messagesQuery.data ?? [];

  useEffect(() => {
    if (!sessionId && sessions.length > 0) setSessionId(sessions[0].id);
  }, [sessionId, sessions]);

  // 语料页「交给导演」通过 sessionStorage 传过来
  useEffect(() => {
    const raw = sessionStorage.getItem('corpus_handoff');
    if (!raw) return;
    try {
      const c = JSON.parse(raw) as { title: string; summary: string };
      setInput(`用这条语料《${c.title}》丰富世界观：${c.summary} 先给方案，不要覆盖已有人工设定。`);
      setTargetRole('director');
    } catch {
      /* ignore */
    }
    sessionStorage.removeItem('corpus_handoff');
  }, []);

  const queryError =
    runsQuery.error ?? boardQuery.error ?? runQuery.error ?? crewQuery.error ?? messagesQuery.error;

  if (!currentStory) return <p className="p-6 text-ai-ink-3">请先选择一个故事</p>;

  const dbActivities: AgentActivityEvent[] = dbLogs
    .filter(a => a.event_type === 'activity')
    .map(a => ({
      run_id: a.run_id,
      role: a.role ?? '',
      action: a.action ?? '',
      detail: a.detail ?? '',
      at: new Date(a.created_at).getTime(),
    }));
  const dbProgress: AgentProgressEvent[] = dbLogs
    .filter(a => a.event_type === 'progress')
    .map(a => ({
      run_id: a.run_id,
      phase: a.phase ?? '',
      status: a.status ?? '',
      message: a.message ?? '',
      at: new Date(a.created_at).getTime(),
    }));
  const seenActKeys = new Set(dbActivities.map(a => `${a.role}|${a.action}|${a.detail}`));
  const runActivities = [
    ...dbActivities,
    ...(activeRunId
      ? activities.filter(
          a => a.run_id === activeRunId && !seenActKeys.has(`${a.role}|${a.action}|${a.detail}`)
        )
      : []),
  ];
  const seenProgKeys = new Set(dbProgress.map(p => `${p.phase}|${p.status}|${p.message}`));
  const runProgress = [
    ...dbProgress,
    ...(activeRunId
      ? progress.filter(
          p => p.run_id === activeRunId && !seenProgKeys.has(`${p.phase}|${p.status}|${p.message}`)
        )
      : []),
  ];
  const runStatus = runProgress.length
    ? `${runProgress[runProgress.length - 1].phase} · ${runStatusLabel(
        runProgress[runProgress.length - 1].status
      )}`
    : run
      ? `${run.phase} · ${runStatusLabel(run.status)}`
      : queryError
        ? '状态获取失败'
        : '-';
  const lastAction = (role: string) => {
    const a = [...runActivities].reverse().find(x => x.role === role);
    if (a) return `${a.action} ${a.detail}`;
    const item = (board ?? [])
      .filter(i => i.producer === role)
      .sort((x, y) => new Date(y.created_at).getTime() - new Date(x.created_at).getTime())[0];
    if (item) return `创建 ${ZONE_NAMES[item.zone] ?? item.zone}：${item.key}`;
    return '-';
  };
  const byZone = (zone: BoardItem['zone']) => (board ?? []).filter(i => i.zone === zone);

  const liveTimeline: TimelineEntry[] = [
    ...runActivities.map(a => ({
      at: a.at,
      text: `${roleName(a.role)} ${a.action} ${a.detail}`,
      role: a.role,
      action: a.action,
      detail: a.detail,
    })),
    ...runProgress.map(p => ({
      at: p.at,
      text: `${p.phase} ${p.status} ${p.message}`,
      detail: p.message,
      phase: p.phase,
      status: p.status,
    })),
  ];
  const historicalTimeline: TimelineEntry[] = [];
  if (board) {
    for (const item of board) {
      const ts = new Date(item.created_at).getTime();
      if (!isNaN(ts)) {
        historicalTimeline.push({
          at: ts,
          text: `${roleName(item.producer)} 创建 ${ZONE_NAMES[item.zone] ?? item.zone}：${item.key}`,
          role: item.producer,
          action: '创建',
          detail: `${item.zone}:${item.key}`,
        });
      }
    }
  }
  if (run) {
    const startTs = new Date(run.created_at).getTime();
    if (!isNaN(startTs)) {
      historicalTimeline.push({ at: startTs, text: `运行启动 - ${run.premise.slice(0, 50)}` });
    }
    const endTs = new Date(run.updated_at).getTime();
    if (!isNaN(endTs) && !isRunActive(run.status) && run.status !== 'idle') {
      historicalTimeline.push({
        at: endTs,
        text: `运行${runStatusLabel(run.status)} - ${run.phase}`,
        phase: run.phase,
        status: run.status,
      });
    }
  }
  const seen = new Set<string>();
  const timeline = [...liveTimeline, ...historicalTimeline]
    .filter(t => {
      const key = timelineKey(t);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    })
    .sort((x, y) => y.at - x.at)
    .slice(0, 100);

  const refreshStudio = () => {
    queryClient.invalidateQueries({ queryKey: ['agency-crew', storyId] });
    queryClient.invalidateQueries({ queryKey: ['agency-chat', sessionId] });
    queryClient.invalidateQueries({ queryKey: ['agency-chat-sessions', storyId] });
    queryClient.invalidateQueries({ queryKey: ['agency-board', activeRunId] });
    queryClient.invalidateQueries({ queryKey: ['agency-activities', activeRunId] });
  };

  const handleSend = async (text = input) => {
    const msg = text.trim();
    if (!msg || busy) return;
    setBusy('send');
    try {
      const res = await sendChat({
        story_id: currentStory.id,
        session_id: sessionId,
        message: msg,
        target_role: targetRole,
      });
      if (!sessionId) setSessionId(res.session_id);
      setInput('');
      refreshStudio();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleNewSession = async () => {
    try {
      const s = await newChatSession(currentStory.id);
      setSessionId(s.id);
      queryClient.invalidateQueries({ queryKey: ['agency-chat-sessions', currentStory.id] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handlePatrol = async () => {
    try {
      await directorPatrol(currentStory.id);
      setLiveTab('now');
      refreshStudio();
      toast.success('巡检完成，已刷新现场');
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleCancelRun = async () => {
    if (!activeRunId) return;
    try {
      await loggedInvoke('agency_cancel_run', { run_id: activeRunId });
      toast.success('已请求取消当前运行');
      refreshStudio();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleRetry = async (taskId?: string | null) => {
    if (!taskId) return;
    try {
      await crewRetryTask(taskId);
      refreshStudio();
      toast.success('已重试任务');
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleCancelTask = async (taskId?: string | null) => {
    if (!taskId) return;
    try {
      await crewCancelTask(taskId);
      refreshStudio();
      toast.success('已请求停止任务');
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleConfirmActions = async (msg: ChatMessage) => {
    const actions = parseApprovalActions(msg);
    const ids = actions.map(a => String(a.id ?? '')).filter(Boolean);
    if (!ids.length || !sessionId) return;
    try {
      await directorExecute({
        story_id: currentStory.id,
        session_id: sessionId,
        message_id: msg.id,
        action_ids: ids,
      });
      refreshStudio();
      toast.success('已确认并异步派发');
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleRejectActions = async (msg: ChatMessage) => {
    const actions = parseApprovalActions(msg);
    const ids = actions.map(a => String(a.id ?? '')).filter(Boolean);
    if (!ids.length) return;
    try {
      await directorReject(ids, msg.id);
      refreshStudio();
      toast.success('已暂缓');
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleAlertDirector = (alert: CrewAlert) => {
    setTargetRole('director');
    setInput(`@导演 ${alert.kind}：${alert.message}。帮我判断重试、停止还是交给我处理。`);
  };

  const crewTasks = crew?.tasks ?? [];
  const crewAlerts = crew?.alerts ?? [];
  const leases = crew?.leases ?? [];
  const activeCount = crewTasks.filter(t => t.status === 'running').length;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <div className="flex flex-wrap items-end gap-3">
        <div>
          <h1 className="text-lg font-semibold">代理工作室 · {currentStory.title}</h1>
          <p className="text-[11px] text-ai-ink-3">
            导演统筹 · 管理补资产 · 编审看质量 · 主创写正文 · {activeCount} 个子任务在跑
          </p>
        </div>
        <div className="ml-auto flex flex-wrap items-center gap-2">
          {runs.length > 0 ? (
            <select
              aria-label="run 选择"
              className="rounded border border-ai-line bg-ai-field px-2 py-1 text-xs text-ai-ink"
              value={activeRunId ?? ''}
              onChange={e => setActiveRunId(e.target.value)}
            >
              {runs.map(r => (
                <option key={r.id} value={r.id}>
                  [{r.lane ?? 'write'}] {r.status} · {r.phase} - {r.premise.slice(0, 24)}
                </option>
              ))}
            </select>
          ) : (
            <span className="text-[11px] text-ai-ink-3">
              {runsQuery.isLoading ? '加载中…' : '暂无 run'}
            </span>
          )}
          <button
            type="button"
            onClick={handlePatrol}
            className="inline-flex items-center gap-1 rounded border border-ai-line px-2 py-1 text-xs text-ai-ink-2 hover:bg-ai-hover"
          >
            <Activity className="h-3.5 w-3.5" />
            巡检
          </button>
          <button
            type="button"
            onClick={() => setFocus(f => !f)}
            className="rounded border border-ai-line px-2 py-1 text-xs text-ai-ink-2 hover:bg-ai-hover"
          >
            {focus ? '展开现场' : '专注'}
          </button>
          <button
            type="button"
            onClick={handleCancelRun}
            className="inline-flex items-center gap-1 rounded border border-ai-line px-2 py-1 text-xs text-ai-red hover:bg-ai-hover"
          >
            <Square className="h-3.5 w-3.5" />
            取消运行
          </button>
        </div>
      </div>

      {queryError && (
        <p
          className="rounded border border-ai-line p-3 text-sm text-ai-red"
          style={{ background: 'color-mix(in srgb, var(--ai-red) 12%, transparent)' }}
        >
          代理状态获取失败：{queryError instanceof Error ? queryError.message : String(queryError)}
        </p>
      )}

      <div
        className={cn(
          'grid min-h-0 flex-1 gap-3',
          focus
            ? 'grid-cols-1 lg:grid-cols-[170px_minmax(0,1fr)]'
            : 'grid-cols-1 lg:grid-cols-[210px_minmax(0,1fr)_330px]'
        )}
      >
        <aside className="flex min-w-0 flex-col gap-2 rounded border border-ai-line bg-ai-surface p-2">
          <div className="flex items-center justify-between px-1 text-[11px] text-ai-ink-3">
            <span className="font-semibold tracking-wider text-ai-ink-2">剧组</span>
            <span>{ROLES.length} 位成员</span>
          </div>
          {ROLES.map(r => {
            const meta = ROLE_META[r.key];
            const tasks = crewTasks.filter(t => t.role === r.key);
            const running = tasks.find(t => t.status === 'running');
            const failed = tasks.find(t => t.status === 'failed' || t.status === 'stale');
            const status = failed ? '失败' : running ? '执行中' : '待命';
            const Icon = meta?.icon ?? Bot;
            const selected = targetRole === r.key;
            return (
              <button
                key={r.key}
                type="button"
                onClick={() => setTargetRole(r.key)}
                className={cn(
                  'flex flex-col gap-1 rounded border p-2 text-left',
                  selected
                    ? 'border-ai-accent bg-ai-accent-tint'
                    : 'border-ai-line bg-ai-inset hover:bg-ai-hover'
                )}
              >
                <div className="flex items-center gap-2">
                  <span className="flex h-7 w-7 items-center justify-center rounded border border-ai-line bg-ai-field">
                    <Icon className="h-3.5 w-3.5" />
                  </span>
                  <span className="min-w-0">
                    <span className="block text-xs font-semibold text-ai-ink">{r.name}</span>
                    <span className="block truncate text-[10px] text-ai-ink-3">{meta?.role}</span>
                  </span>
                  <span
                    className={cn(
                      'ml-auto text-[10px]',
                      status === '待命'
                        ? 'text-ai-ink-3'
                        : failed
                          ? 'text-ai-red'
                          : 'text-ai-orange'
                    )}
                  >
                    {status}
                  </span>
                </div>
                <div className="truncate text-[10px] text-ai-ink-2">
                  {running?.task || failed?.error || `最近动作：${lastAction(r.key)}`}
                </div>
                <div className="text-[9px] text-ai-ink-3">run 状态：{runStatus}</div>
              </button>
            );
          })}
          <button
            type="button"
            onClick={handleNewSession}
            className="mt-1 flex w-full items-center justify-center gap-1 rounded border border-dashed border-ai-line px-2 py-1 text-[10px] text-ai-ink-3 hover:border-ai-accent hover:text-ai-accent-ink"
          >
            <Plus className="h-3 w-3" />
            新会话
          </button>
        </aside>

        <section className="flex min-w-0 flex-col rounded border border-ai-line bg-ai-surface">
          <div className="flex flex-wrap items-center gap-2 border-b border-ai-line px-3 py-2">
            <div className="flex flex-wrap gap-1">
              {ROLES.map(r => (
                <button
                  key={r.key}
                  type="button"
                  onClick={() => setTargetRole(r.key)}
                  className={cn(
                    'rounded-full border px-2 py-0.5 text-[10px]',
                    targetRole === r.key
                      ? 'border-ai-accent bg-ai-accent-tint text-ai-accent-ink'
                      : 'border-ai-line text-ai-ink-2'
                  )}
                >
                  {r.name}
                </button>
              ))}
            </div>
            <select
              aria-label="会话选择"
              className="ml-auto rounded border border-ai-line bg-ai-field px-1.5 py-0.5 text-[10px] text-ai-ink-2"
              value={sessionId ?? ''}
              onChange={e => setSessionId(e.target.value)}
            >
              <option value="">选择会话</option>
              {sessions.map(s => (
                <option key={s.id} value={s.id}>
                  {s.title}
                </option>
              ))}
            </select>
          </div>

          <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto p-3">
            {messages.length === 0 ? (
              <div className="flex flex-col gap-2 rounded border border-ai-line bg-ai-inset p-3">
                <div className="text-xs text-ai-ink-2">
                  剧组已就位。我会先看现场，再决定派谁。你可以直接说要什么结果。
                </div>
                <div className="grid grid-cols-1 gap-1 sm:grid-cols-2">
                  {QUICK.map(q => (
                    <button
                      key={q.label}
                      type="button"
                      onClick={() => handleSend(q.text)}
                      className="flex items-center gap-1 rounded border border-ai-line px-2 py-1 text-[10px] text-ai-ink-2 hover:bg-ai-hover"
                    >
                      <MessageSquare className="h-3 w-3" />
                      {q.label}
                    </button>
                  ))}
                </div>
              </div>
            ) : (
              messages.map(m => {
                const isUser = m.role === 'user';
                const approvals = parseApprovalActions(m);
                const name = isUser ? '你' : roleName(m.role);
                return (
                  <div
                    key={m.id}
                    className={cn('flex flex-col gap-0.5', isUser ? 'items-end' : 'items-start')}
                  >
                    <div className="flex items-center gap-1.5 text-[10px] text-ai-ink-3">
                      <span
                        className={cn(
                          'font-semibold',
                          isUser ? 'text-ai-accent-ink' : 'text-ai-ink'
                        )}
                      >
                        {name}
                      </span>
                      <span>{hhmmss(new Date(m.created_at).getTime())}</span>
                      <span
                        className={cn(statusColor(m.status === 'done' ? 'completed' : m.status))}
                      >
                        {m.status}
                      </span>
                    </div>
                    <div
                      className={cn(
                        'max-w-[92%] rounded border px-2.5 py-2 text-xs leading-5',
                        isUser
                          ? 'border-ai-accent/40 bg-ai-accent-tint'
                          : 'border-ai-line bg-ai-inset'
                      )}
                    >
                      <div className="whitespace-pre-wrap">{m.content || '…'}</div>
                      {approvals.length > 0 && (
                        <div className="mt-2 rounded border border-ai-accent/40 bg-ai-surface p-2">
                          <div className="mb-1 text-[10px] font-semibold text-ai-accent-ink">
                            需要你确认
                          </div>
                          <ul className="list-disc space-y-1 pl-4 text-[10px] text-ai-ink-2">
                            {approvals.map(a => (
                              <li key={String(a.id)}>{actionSummary(a)}</li>
                            ))}
                          </ul>
                          <div className="mt-2 flex gap-1.5">
                            <button
                              type="button"
                              onClick={() => handleConfirmActions(m)}
                              className="rounded border border-ai-accent bg-ai-accent px-2 py-0.5 text-[10px] text-ai-on-accent"
                            >
                              确认执行
                            </button>
                            <button
                              type="button"
                              onClick={() => handleRejectActions(m)}
                              className="rounded border border-ai-line px-2 py-0.5 text-[10px] text-ai-ink-2"
                            >
                              先不动
                            </button>
                          </div>
                        </div>
                      )}
                      {m.error && <div className="mt-1 text-[10px] text-ai-red">{m.error}</div>}
                    </div>
                  </div>
                );
              })
            )}
          </div>

          <div className="border-t border-ai-line p-2">
            <div className="mb-1 flex flex-wrap gap-1">
              {['/巡检', '/修复', '/续写', '/语料'].map(c => (
                <button
                  key={c}
                  type="button"
                  onClick={() => setInput(c)}
                  className="rounded-full border border-ai-line px-1.5 py-0.5 text-[9px] text-ai-ink-3 hover:text-ai-accent-ink"
                >
                  {c}
                </button>
              ))}
            </div>
            <AiPromptBar
              value={input}
              onChange={setInput}
              onSend={() => handleSend()}
              placeholder={`对${roleName(targetRole)}说，或 @管理/@编审/@主创 直接下指令…`}
              sources={ROLES.filter(r => r.key !== 'director').map(r => ({
                key: r.key,
                name: r.name,
                desc: r.key,
              }))}
              commands={[
                { key: 'patrol', name: '/巡检', desc: '导演看现场' },
                { key: 'repair', name: '/修复', desc: '导演修故障' },
                { key: 'continue', name: '/续写', desc: '派主创续写（需确认）' },
                { key: 'corpus', name: '/语料', desc: '引用语料' },
              ]}
            />
          </div>
        </section>

        {!focus && (
          <aside className="flex min-w-0 flex-col rounded border border-ai-line bg-ai-surface">
            <div className="flex gap-1 border-b border-ai-line px-2 py-1.5">
              {(['now', 'board', 'timeline'] as const).map(tab => (
                <button
                  key={tab}
                  type="button"
                  onClick={() => setLiveTab(tab)}
                  className={cn(
                    'rounded px-2 py-0.5 text-[10px]',
                    liveTab === tab
                      ? 'border-b-2 border-ai-accent text-ai-accent-ink'
                      : 'text-ai-ink-3'
                  )}
                >
                  {tab === 'now' ? '现场' : tab === 'board' ? '黑板' : '时间线'}
                </button>
              ))}
            </div>
            <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto p-2">
              {liveTab === 'now' && (
                <>
                  <div className="rounded border border-ai-line p-2">
                    <div className="mb-1 flex items-center gap-1 text-[10px] font-semibold text-ai-ink-2">
                      <Activity className="h-3 w-3" />
                      当前运行
                    </div>
                    <div className="flex items-center gap-1 text-xs">
                      {run ? `${runStatusLabel(run.status)} · ${run.phase}` : '暂无 run'}
                    </div>
                    <div className="mt-1 text-[10px] text-ai-ink-3">{runStatus}</div>
                  </div>

                  <div className="rounded border border-ai-line p-2">
                    <div className="mb-1 flex items-center gap-1 text-[10px] font-semibold text-ai-ink-2">
                      <ListChecks className="h-3 w-3" />
                      任务进程
                    </div>
                    {crewTasks.length === 0 ? (
                      <div className="text-[10px] text-ai-ink-3">
                        暂无任务。发指令后这里会显示子 agent 进程。
                      </div>
                    ) : (
                      crewTasks.map(t => (
                        <div
                          key={t.id}
                          className="flex items-start gap-1.5 border-t border-ai-line py-1 first:border-0 first:pt-0"
                        >
                          <span
                            className={cn(
                              'mt-1 h-1.5 w-1.5 rounded-full bg-current',
                              statusColor(t.status)
                            )}
                          />
                          <div className="min-w-0 flex-1">
                            <div className="truncate text-[10px] text-ai-ink">
                              {roleName(t.role)} · {t.task}
                            </div>
                            <div className="text-[9px] text-ai-ink-3">
                              {t.status} ·{' '}
                              {t.heartbeat_at
                                ? `心跳 ${hhmmss(new Date(t.heartbeat_at).getTime())}`
                                : '无心跳'}{' '}
                              · {t.turn_count} 轮
                            </div>
                          </div>
                          <div className="flex gap-1">
                            {(t.status === 'failed' || t.status === 'stale') && (
                              <button
                                type="button"
                                onClick={() => handleRetry(t.id)}
                                className="rounded border border-ai-line px-1 py-0.5 text-[9px] text-ai-ink-2"
                              >
                                重试
                              </button>
                            )}
                            {t.status === 'running' && (
                              <button
                                type="button"
                                onClick={() => handleCancelTask(t.id)}
                                className="rounded border border-ai-line px-1 py-0.5 text-[9px] text-ai-red"
                              >
                                停止
                              </button>
                            )}
                          </div>
                        </div>
                      ))
                    )}
                  </div>

                  {crewAlerts.length > 0 && (
                    <div className="rounded border border-ai-red/30 bg-ai-red/10 p-2">
                      <div className="mb-1 flex items-center gap-1 text-[10px] font-semibold text-ai-red">
                        <RefreshCw className="h-3 w-3" />
                        告警 {crewAlerts.length}
                      </div>
                      {crewAlerts.map((a, i) => (
                        <div key={`${a.kind}-${i}`} className="text-[10px] text-ai-ink-2">
                          <div>{a.message}</div>
                          <div className="mt-1 flex gap-1">
                            {a.task_id && (
                              <button
                                type="button"
                                onClick={() => handleRetry(a.task_id)}
                                className="rounded border border-ai-line px-1 py-0.5 text-[9px] text-ai-ink-2"
                              >
                                重试
                              </button>
                            )}
                            <button
                              type="button"
                              onClick={() => handleAlertDirector(a)}
                              className="rounded border border-ai-line px-1 py-0.5 text-[9px] text-ai-ink-2"
                            >
                              交给导演
                            </button>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}

                  <div className="rounded border border-ai-line p-2">
                    <div className="mb-1 flex items-center gap-1 text-[10px] font-semibold text-ai-ink-2">
                      <Wifi className="h-3 w-3" />
                      资源租约
                    </div>
                    {leases.length === 0 ? (
                      <div className="text-[10px] text-ai-ink-3">（无租约）</div>
                    ) : (
                      leases.map(l => (
                        <div key={l.id} className="text-[10px] text-ai-ink-2">
                          {l.resource} · owner {l.owner_task_id?.slice(0, 8) ?? '?'} · 至{' '}
                          {l.expires_at}
                        </div>
                      ))
                    )}
                  </div>
                </>
              )}

              {liveTab === 'board' && (
                <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                  {ZONES.map(z => (
                    <div key={z.key} className="rounded border border-ai-line p-2">
                      <div className="mb-1 flex justify-between text-[10px] text-ai-ink-3">
                        <span>{z.name}</span>
                        <span>{byZone(z.key).length}</span>
                      </div>
                      {byZone(z.key).length === 0 ? (
                        <div className="text-[10px] text-ai-ink-3">（空）</div>
                      ) : (
                        <AiContextCards
                          title={z.name}
                          count={byZone(z.key).length}
                          onItemActivate={
                            z.key === 'asset'
                              ? item => {
                                  const bi = byZone('asset').find(i => i.id === item.key);
                                  const view = bi ? boardItemView(bi.item_type) : null;
                                  if (view) setCurrentView(view);
                                }
                              : undefined
                          }
                          items={byZone(z.key).map(item => ({
                            key: item.id,
                            title: item.key,
                            meta: `v${item.version} · ${item.status}`,
                            body: item.summary,
                          }))}
                        />
                      )}
                    </div>
                  ))}
                </div>
              )}

              {liveTab === 'timeline' && (
                <div className="flex flex-col gap-1 text-[10px]">
                  {runActivities.length > 0 && (
                    <AiThinking
                      title="当前执行轨迹"
                      doneTitle="执行轨迹（已结束）"
                      working={isRunActive(run?.status ?? '')}
                      rows={runActivities.slice(-12).map(a => ({
                        id: `${a.role}|${a.action}|${a.detail}|${a.at}`,
                        primary: `${roleName(a.role)} ${a.action}`,
                        secondary: a.detail || undefined,
                      }))}
                      defaultExpanded={isRunActive(run?.status ?? '')}
                    />
                  )}
                  {timeline.length === 0 ? (
                    <div className="text-[10px] text-ai-ink-3">暂无记录</div>
                  ) : (
                    timeline.map((t, i) => (
                      <div key={i} className="flex gap-1.5">
                        <span className="text-ai-ink-3">{hhmmss(t.at)}</span>
                        <span className="text-ai-ink-2">{t.text}</span>
                      </div>
                    ))
                  )}
                </div>
              )}
            </div>
          </aside>
        )}
      </div>

      {runsQuery.isError && <EmptyHint>代理状态加载失败；每 10 秒会自动重试。</EmptyHint>}
    </div>
  );
}
