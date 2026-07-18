import { useEffect, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import { getRun, listBoard } from '@/services/api/agency';
import type { BoardItem } from '@/services/api/agency';

/** 角色事件流中的 role 值（AgentRole::as_str）→ 显示名 */
const ROLES: { key: string; name: string }[] = [
  { key: 'lead_writer', name: '主创' },
  { key: 'producer', name: '管理' },
  { key: 'editor_auditor', name: '编辑审计' },
];

const ZONES: { key: BoardItem['zone']; name: string }[] = [
  { key: 'asset', name: '资产' },
  { key: 'draft', name: '草稿' },
  { key: 'review', name: '审查' },
  { key: 'schedule', name: '计划' },
];

interface ActivityEvent {
  run_id: string;
  role: string;
  action: string;
  detail: string;
  at: number;
}

interface ProgressEvent {
  run_id: string;
  phase: string;
  status: string;
  message: string;
  at: number;
}

function hhmmss(at: number) {
  const d = new Date(at);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

export default function AgencyStudio() {
  const currentStory = useAppStore(s => s.currentStory);
  const qc = useQueryClient();
  const [activities, setActivities] = useState<ActivityEvent[]>([]);
  const [progress, setProgress] = useState<ProgressEvent[]>([]);
  const [activeRunId, setActiveRunId] = useState<string | null>(null);

  // 事件接入：activity/progress 驱动时间线与角色卡；board-changed 失效黑板查询。
  // 活跃 run_id 全部从事件捕获（agency 无 list_runs）。
  useEffect(() => {
    let un1: (() => void) | undefined, un2: (() => void) | undefined, un3: (() => void) | undefined;
    (async () => {
      const { listen } = await import('@tauri-apps/api/event');
      un1 = await listen<Omit<ActivityEvent, 'at'>>('agency-agent-activity', e => {
        setActivities(prev => [...prev.slice(-99), { ...e.payload, at: Date.now() }]);
        setActiveRunId(e.payload.run_id);
      });
      un2 = await listen<Omit<ProgressEvent, 'at'>>('agency-run-progress', e => {
        setProgress(prev => [...prev.slice(-99), { ...e.payload, at: Date.now() }]);
        setActiveRunId(e.payload.run_id);
      });
      un3 = await listen<BoardItem>('agency-board-changed', e => {
        setActiveRunId(e.payload.run_id);
        qc.invalidateQueries({ queryKey: ['agency-board', e.payload.run_id] });
      });
    })();
    return () => {
      un1?.();
      un2?.();
      un3?.();
    };
  }, [qc]);

  const { data: board } = useQuery({
    queryKey: ['agency-board', activeRunId],
    queryFn: () => listBoard(activeRunId!),
    enabled: !!activeRunId,
    refetchInterval: 10_000,
  });
  const { data: run } = useQuery({
    queryKey: ['agency-run', activeRunId],
    queryFn: () => getRun(activeRunId!),
    enabled: !!activeRunId,
    refetchInterval: 10_000,
  });

  if (!currentStory) return <p className="p-6 text-gray-500">请先选择一个故事</p>;

  const latestProgress = progress.length > 0 ? progress[progress.length - 1] : null;
  const runStatus = latestProgress
    ? `${latestProgress.phase} · ${latestProgress.status}`
    : run
      ? `${run.phase} · ${run.status}`
      : '—';
  const lastAction = (role: string) => {
    const a = [...activities].reverse().find(x => x.role === role);
    return a ? `${a.action} ${a.detail}` : '—';
  };
  const byZone = (zone: BoardItem['zone']) => (board ?? []).filter(i => i.zone === zone);
  const timeline = [
    ...activities.map(a => ({ at: a.at, text: `${a.role} ${a.action} ${a.detail}` })),
    ...progress.map(p => ({ at: p.at, text: `${p.phase} ${p.status} ${p.message}` })),
  ]
    .sort((x, y) => y.at - x.at)
    .slice(0, 100);

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">代理工作室 · {currentStory.title}</h1>
        <span className="text-xs text-gray-400">
          {activeRunId ? `run ${activeRunId.slice(0, 8)}` : '等待事件'}
        </span>
      </div>

      <section className="grid grid-cols-3 gap-4">
        {ROLES.map(r => (
          <div key={r.key} className="rounded border p-4">
            <div className="font-medium">{r.name}</div>
            <div className="mt-2 text-sm text-gray-600">最近动作：{lastAction(r.key)}</div>
            <div className="mt-1 text-sm text-gray-600">run 状态：{runStatus}</div>
          </div>
        ))}
      </section>

      {!activeRunId && (
        <p className="rounded border border-dashed p-4 text-sm text-gray-500">
          暂无活动——启动创世或续写后，这里会实时显示代理动态。
        </p>
      )}

      {activeRunId && (
        <section>
          <h2 className="mb-2 font-medium">黑板</h2>
          <div className="grid grid-cols-4 gap-3">
            {ZONES.map(z => (
              <div key={z.key} className="rounded border p-3">
                <div className="mb-2 text-sm font-medium text-gray-500">{z.name}</div>
                {byZone(z.key).length === 0 && <p className="text-xs text-gray-400">（空）</p>}
                <div className="space-y-2">
                  {byZone(z.key).map(item => (
                    <div key={item.id} className="rounded bg-gray-50 p-2 text-sm">
                      <div className="flex items-center justify-between gap-2">
                        <span className="font-medium">{item.key}</span>
                        <span className="text-xs text-gray-400">
                          v{item.version} · {item.status}
                        </span>
                      </div>
                      <div className="truncate text-xs text-gray-500">{item.summary}</div>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      <section>
        <h2 className="mb-2 font-medium">时间线</h2>
        {timeline.length === 0 ? (
          activeRunId && <p className="text-sm text-gray-400">等待新事件…</p>
        ) : (
          <div className="space-y-1 text-sm">
            {timeline.map((t, idx) => (
              <div key={idx} className="flex gap-2">
                <span className="text-gray-400">[{hhmmss(t.at)}]</span>
                <span>{t.text}</span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
