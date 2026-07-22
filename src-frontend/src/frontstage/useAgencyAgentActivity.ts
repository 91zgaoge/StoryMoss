import { useEffect, useState } from 'react';

/**
 * v0.30.17: 监听 Agency 三 agent（主创/管理/编辑审计）的活动事件，
 * 为幕前顶部状态栏提供"主创正在写第一章 · 管理已完成深度资产 · 编辑审计正在质检"
 * 式的逐角色进度文案。
 *
 * 后端 `agency-agent-activity` 事件（coordinator.rs emit_activity）已存在，
 * 此前仅幕后 AgencyStudio 消费；本 hook 让幕前也订阅。
 */

const ROLES: { key: string; name: string }[] = [
  { key: 'lead_writer', name: '主创' },
  { key: 'producer', name: '管理' },
  { key: 'editor_auditor', name: '编辑审计' },
];

/** detail（动作对象）-> 动词短语，用于拼"正在X / 已完成X" */
const DETAIL_VERB: Record<string, string> = {
  概念: '构思概念',
  首章: '写第一章',
  深度资产: '生成深度资产',
  审查: '质检',
  装配: '装配最终稿',
};

export interface AgentActivityEvent {
  run_id: string;
  role: string;
  action: string; // "start" | "done"
  detail: string; // 概念/首章/深度资产/审查/装配
  at: number;
}

function friendlyText(role: string, action: string, detail: string): string {
  const name = ROLES.find(r => r.key === role)?.name ?? role;
  // 已完成：直接用阶段名作宾语（"已完成首章" / "已完成深度资产"），更自然；
  // 进行中：用动词短语（"正在写第一章" / "正在生成深度资产"）。
  if (action === 'done') return `${name}已完成${detail}`;
  const verb = DETAIL_VERB[detail] ?? detail;
  return `${name}正在${verb}`;
}

export function useAgencyAgentActivity() {
  // 各角色最新一条活动（role -> event）
  const [latest, setLatest] = useState<Record<string, AgentActivityEvent>>({});

  useEffect(() => {
    let unActivity: (() => void) | undefined;
    let unProgress: (() => void) | undefined;
    (async () => {
      const { listen } = await import('@tauri-apps/api/event');
      unActivity = await listen<Omit<AgentActivityEvent, 'at'>>('agency-agent-activity', e => {
        setLatest(prev => ({ ...prev, [e.payload.role]: { ...e.payload, at: Date.now() } }));
      });
      // run 结束（completed/failed/cancelled）时清空，避免创世结束后仍显示陈旧进度
      unProgress = await listen<{ run_id: string; status: string }>('agency-run-progress', e => {
        const s = (e.payload.status || '').toLowerCase();
        if (s === 'completed' || s === 'failed' || s === 'cancelled' || s === 'error') {
          setLatest({});
        }
      });
    })();
    return () => {
      unActivity?.();
      unProgress?.();
    };
  }, []);

  // 仅返回有事件的角色，按 主创/管理/编辑审计 顺序。
  // done=true 表示该角色当前阶段已结束（绿色 saved 态），false 表示进行中（琥珀 saving 态）。
  const lines: { text: string; done: boolean }[] = ROLES.filter(r => latest[r.key]).map(r => {
    const a = latest[r.key];
    return { text: friendlyText(a.role, a.action, a.detail), done: a.action === 'done' };
  });

  return { lines, hasActivity: lines.length > 0, latest };
}
