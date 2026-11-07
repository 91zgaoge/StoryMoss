/**
 * 剧组对话面板 —— 幕后 AgencyStudio 与幕前对话浮层共享（v0.87.0 统一对话）。
 *
 * 纯展示组件：消息列表 / 角色切换 / 会话选择 / 审批卡 / 输入栏。
 * 数据获取（轮询）与发送留在调用方，两个入口共享同一套查询缓存
 * （['agency-chat', sessionId] 等）与刷新逻辑，会话在幕后/幕前之间天然同步。
 * 只引用 --ai-* 语义令牌（幕后 tokens.css / 幕前 frontstage.css 各自定义）。
 */

import { useEffect, useRef } from 'react';
import { MessageSquare } from 'lucide-react';
import { cn } from '@/utils/cn';
import { AiPromptBar } from '@/components/ui/ai/AiPromptBar';
import type { ChatMessage, ChatSession } from '@/services/api/agency';

export const AGENCY_ROLE_NAMES: Record<string, string> = {
  director: '导演',
  lead_writer: '主创',
  producer: '管理',
  editor_auditor: '编审',
  writer: '写手',
  inspector: '检查',
  outline_planner: '大纲',
  style_mimic: '风格',
};

export function agencyRoleName(key: string) {
  return AGENCY_ROLE_NAMES[key] ?? key;
}

export function parseApprovalActions(msg: ChatMessage): Array<Record<string, unknown>> {
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

export function approvalActionSummary(action: Record<string, unknown>) {
  let spec: Record<string, unknown> = {};
  try {
    spec = JSON.parse(String(action.params_json ?? '{}'));
  } catch {
    /* ignore */
  }
  const type = String(spec.type ?? action.action_type ?? '动作');
  const role = spec.role ? agencyRoleName(String(spec.role)) : '';
  // v0.82.3：导演动作目标可视化——场景序号优先于章号
  // v0.86.0：target_end_chapter 显示「续写至第N章」
  const target = spec.target_scene
    ? `场景${spec.target_scene}`
    : spec.target_chapter
      ? `第${spec.target_chapter}章`
      : spec.target_end_chapter
        ? `续写至第${spec.target_end_chapter}章`
        : '';
  const instruction = spec.instruction ? String(spec.instruction) : '';
  return `${type}${role ? ` · ${role}` : ''}${target ? ` · ${target}` : ''}${
    instruction ? `：${instruction}` : ''
  }`;
}

function hhmmss(at: number) {
  const d = new Date(at);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

function messageStatusColor(status: string) {
  if (status === 'failed' || status === 'stale') return 'text-ai-red';
  if (status === 'pending' || status === 'acting') return 'text-ai-orange';
  return 'text-ai-green';
}

export interface ChatPanelProps {
  messages: ChatMessage[];
  /** 角色胶囊（默认主创/管理/编审/导演）。 */
  roles: { key: string; name: string }[];
  targetRole: string;
  onTargetRoleChange: (role: string) => void;
  input: string;
  onInputChange: (value: string) => void;
  onSend: () => void;
  onConfirmActions: (msg: ChatMessage) => void;
  onRejectActions: (msg: ChatMessage) => void;
  /** 会话选择器（缺省隐藏——幕前浮层只有一个隐式会话列表时仍可传入）。 */
  sessions?: ChatSession[];
  sessionId?: string | null;
  onSessionChange?: (id: string) => void;
  /** 空态快捷指令。 */
  quickPrompts?: { label: string; text: string }[];
  /** 输入栏 slash 命令建议。 */
  quickCommands?: { key: string; name: string; desc: string }[];
  placeholder?: string;
  /** 空态引导文案（快捷指令上方）。 */
  emptyHint?: string;
  /** 输入栏下方的固定快捷指令（如 /巡检）。 */
  inlineCommands?: string[];
  onInlineCommand?: (command: string) => void;
  testId?: string;
}

export function ChatPanel({
  messages,
  roles,
  targetRole,
  onTargetRoleChange,
  input,
  onInputChange,
  onSend,
  onConfirmActions,
  onRejectActions,
  sessions,
  sessionId,
  onSessionChange,
  quickPrompts,
  quickCommands,
  placeholder,
  emptyHint,
  inlineCommands,
  onInlineCommand,
  testId,
}: ChatPanelProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null);

  // 新消息（含子 agent 回报）到底部跟随
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages.length, messages[messages.length - 1]?.status]);

  return (
    <div data-testid={testId} className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-2 border-b border-ai-line px-3 py-2">
        <div className="flex flex-wrap gap-1">
          {roles.map(r => (
            <button
              key={r.key}
              type="button"
              onClick={() => onTargetRoleChange(r.key)}
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
        {sessions && onSessionChange && (
          <select
            aria-label="会话选择"
            className="ml-auto rounded border border-ai-line bg-ai-field px-1.5 py-0.5 text-[10px] text-ai-ink-2"
            value={sessionId ?? ''}
            onChange={e => onSessionChange(e.target.value)}
          >
            <option value="">选择会话</option>
            {sessions.map(s => (
              <option key={s.id} value={s.id}>
                {s.title}
              </option>
            ))}
          </select>
        )}
      </div>

      <div ref={scrollRef} className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto p-3">
        {messages.length === 0 ? (
          <div className="flex flex-col gap-2 rounded border border-ai-line bg-ai-inset p-3">
            <div className="text-xs text-ai-ink-2">
              {emptyHint ?? '剧组已就位。我会先看现场，再决定派谁。你可以直接说要什么结果。'}
            </div>
            {quickPrompts && quickPrompts.length > 0 && (
              <div className="grid grid-cols-1 gap-1 sm:grid-cols-2">
                {quickPrompts.map(q => (
                  <button
                    key={q.label}
                    type="button"
                    onClick={() => {
                      onInputChange(q.text);
                      onSend();
                    }}
                    className="flex items-center gap-1 rounded border border-ai-line px-2 py-1 text-[10px] text-ai-ink-2 hover:bg-ai-hover"
                  >
                    <MessageSquare className="h-3 w-3" />
                    {q.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        ) : (
          messages.map(m => {
            const isUser = m.role === 'user';
            const approvals = parseApprovalActions(m);
            const name = isUser ? '你' : agencyRoleName(m.role);
            return (
              <div
                key={m.id}
                className={cn('flex flex-col gap-0.5', isUser ? 'items-end' : 'items-start')}
              >
                <div className="flex items-center gap-1.5 text-[10px] text-ai-ink-3">
                  <span
                    className={cn('font-semibold', isUser ? 'text-ai-accent-ink' : 'text-ai-ink')}
                  >
                    {name}
                  </span>
                  <span>{hhmmss(new Date(m.created_at).getTime())}</span>
                  <span className={messageStatusColor(m.status)}>{m.status}</span>
                </div>
                <div
                  className={cn(
                    'max-w-[92%] rounded border px-2.5 py-2 text-xs leading-5',
                    isUser ? 'border-ai-accent/40 bg-ai-accent-tint' : 'border-ai-line bg-ai-inset'
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
                          <li key={String(a.id)}>{approvalActionSummary(a)}</li>
                        ))}
                      </ul>
                      <div className="mt-2 flex gap-1.5">
                        <button
                          type="button"
                          onClick={() => onConfirmActions(m)}
                          className="rounded border border-ai-accent bg-ai-accent px-2 py-0.5 text-[10px] text-ai-on-accent"
                        >
                          确认执行
                        </button>
                        <button
                          type="button"
                          onClick={() => onRejectActions(m)}
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
        {inlineCommands && inlineCommands.length > 0 && (
          <div className="mb-1 flex flex-wrap gap-1">
            {inlineCommands.map(c => (
              <button
                key={c}
                type="button"
                onClick={() => (onInlineCommand ? onInlineCommand(c) : onInputChange(c))}
                className="rounded-full border border-ai-line px-1.5 py-0.5 text-[9px] text-ai-ink-3 hover:text-ai-accent-ink"
              >
                {c}
              </button>
            ))}
          </div>
        )}
        <AiPromptBar
          value={input}
          onChange={onInputChange}
          onSend={onSend}
          placeholder={
            placeholder ?? `对${agencyRoleName(targetRole)}说，或 @管理/@编审/@主创 直接下指令…`
          }
          sources={roles
            .filter(r => r.key !== 'director')
            .map(r => ({ key: r.key, name: r.name, desc: r.key }))}
          commands={quickCommands}
        />
      </div>
    </div>
  );
}
