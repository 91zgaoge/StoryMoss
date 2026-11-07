/**
 * 幕前剧组对话浮层（v0.87.0 统一幕后/幕前人机对话）。
 *
 * 由幕前底部输入栏智能路由打开：
 * - 消息以 @导演/@管理/@编审/@主创 开头 → 直接发送（autoSend）；
 * - classify_intent 判定 is_chat → 预填输入，用户确认后发送（双保险）。
 * 幕前默认 @导演（DEFAULT_CHAT_ROLE）；会话与幕后 AgencyStudio 共享
 * （同 story_id 的 agency_chat_sessions），幕后发起的对话在这里继续可见。
 */

import React, { useEffect, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { X } from 'lucide-react';
import { ChatPanel } from '@/components/agency/ChatPanel';
import { parseAgentMention, DEFAULT_CHAT_ROLE } from '@/utils/agentMention';
import {
  directorExecute,
  directorPatrol,
  directorReject,
  listChatMessages,
  listChatSessions,
  newChatSession,
  sendChat,
  type ChatMessage,
} from '@/services/api/agency';
import { AGENCY_ROLES } from '@/stores/agencyActivityStore';
import { createLogger } from '@/utils/logger';

const frontstageChatLogger = createLogger('ui:FrontstageChat');

export interface FrontstageChatOverlayProps {
  open: boolean;
  onClose: () => void;
  storyId: string | null;
  /** 路由带来的预填消息；消费后调用 onConsume 清空。 */
  initialMessage: string | null;
  /** true=预填消息直接发送（@提及快路）；false=仅预填待确认（意图分类路）。 */
  autoSend: boolean;
  onConsume: () => void;
}

export const FrontstageChatOverlay: React.FC<FrontstageChatOverlayProps> = ({
  open,
  onClose,
  storyId,
  initialMessage,
  autoSend,
  onConsume,
}) => {
  const [targetRole, setTargetRole] = useState<string>(DEFAULT_CHAT_ROLE);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);

  const sessionsQuery = useQuery({
    queryKey: ['agency-chat-sessions', storyId],
    queryFn: () => listChatSessions(storyId!),
    enabled: open && !!storyId,
    refetchInterval: 10_000,
  });
  const sessions = sessionsQuery.data ?? [];

  const messagesQuery = useQuery({
    queryKey: ['agency-chat', sessionId],
    queryFn: () => listChatMessages(sessionId!, 200),
    enabled: open && !!sessionId,
    refetchInterval: 2_000,
  });
  const messages: ChatMessage[] = messagesQuery.data ?? [];

  // 无会话时取最新会话（与幕后同一列表），发消息时后端也会自动建会话
  useEffect(() => {
    if (open && !sessionId && sessions.length > 0) setSessionId(sessions[0].id);
  }, [open, sessionId, sessions]);

  const sendRef = useRef<(text: string) => Promise<void>>(async () => {});

  const handleSend = async (text: string) => {
    const msg = text.trim();
    if (!msg || busy || !storyId) return;
    setBusy(true);
    try {
      const mention = parseAgentMention(msg);
      const res = await sendChat({
        story_id: storyId,
        session_id: sessionId,
        message: msg,
        target_role: mention ? mention.targetRole : targetRole,
      });
      if (!sessionId) setSessionId(res.session_id);
      setInput('');
    } catch (e) {
      frontstageChatLogger.warn('发送剧组消息失败', { error: String(e) });
      throw e instanceof Error ? e : new Error(String(e));
    } finally {
      setBusy(false);
    }
  };
  sendRef.current = handleSend;

  // 消费路由预填：autoSend（@提及快路）直接发送；否则仅预填等用户确认
  useEffect(() => {
    if (!open || initialMessage === null) return;
    const text = initialMessage;
    onConsume();
    if (!storyId) return;
    if (autoSend) {
      void sendRef.current(text).catch(() => {
        /* 发送失败：消息已退回输入框下方 toast 由调用方处理 */
        setInput(text);
      });
    } else {
      setInput(text);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, initialMessage, autoSend, storyId]);

  const handleConfirmActions = async (m: ChatMessage) => {
    if (!storyId || !sessionId) return;
    try {
      const actions = (m.actions_json ? JSON.parse(m.actions_json) : { actions: [] }) as {
        actions?: Array<{ id?: unknown }>;
      };
      const ids = (actions.actions ?? []).map(a => String(a.id ?? '')).filter(Boolean);
      if (!ids.length) return;
      await directorExecute({
        story_id: storyId,
        session_id: sessionId,
        message_id: m.id,
        action_ids: ids,
      });
    } catch (e) {
      frontstageChatLogger.warn('确认导演动作失败', { error: String(e) });
    }
  };

  const handleRejectActions = async (m: ChatMessage) => {
    try {
      const actions = (m.actions_json ? JSON.parse(m.actions_json) : { actions: [] }) as {
        actions?: Array<{ id?: unknown }>;
      };
      const ids = (actions.actions ?? []).map(a => String(a.id ?? '')).filter(Boolean);
      if (!ids.length) return;
      await directorReject(ids, m.id);
    } catch (e) {
      frontstageChatLogger.warn('暂缓导演动作失败', { error: String(e) });
    }
  };

  if (!open) return null;

  return (
    <div
      data-testid="frontstage-chat-overlay"
      className="fixed bottom-24 right-6 z-40 flex max-h-[70vh] w-[420px] max-w-[calc(100vw-3rem)] flex-col overflow-hidden rounded-xl border border-[var(--warm-sand)] bg-[var(--parchment-dark)] shadow-2xl"
    >
      <div className="flex items-center justify-between border-b border-[var(--warm-sand)] px-4 py-2.5">
        <span className="text-sm font-medium text-[var(--charcoal)]">剧组对话 · 默认 @导演</span>
        <div className="flex items-center gap-2">
          <button
            type="button"
            title="新建会话"
            className="text-[11px] text-[var(--stone-gray)] hover:text-[var(--charcoal)]"
            onClick={async () => {
              if (!storyId) return;
              try {
                const s = await newChatSession(storyId);
                setSessionId(s.id);
              } catch (e) {
                frontstageChatLogger.warn('新建会话失败', { error: String(e) });
              }
            }}
          >
            新会话
          </button>
          <button
            type="button"
            title="导演巡检"
            className="text-[11px] text-[var(--stone-gray)] hover:text-[var(--charcoal)]"
            onClick={async () => {
              if (!storyId) return;
              try {
                await directorPatrol(storyId);
              } catch (e) {
                frontstageChatLogger.warn('巡检失败', { error: String(e) });
              }
            }}
          >
            巡检
          </button>
          <button
            type="button"
            onClick={onClose}
            className="text-[var(--stone-gray)] transition-colors hover:text-[var(--charcoal)]"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </div>
      <div className="flex min-h-[380px] flex-1 flex-col">
        {!storyId ? (
          <p className="p-6 text-xs text-[var(--stone-gray)]">
            先创建或选择一个故事，剧组才会上场。
          </p>
        ) : (
          <ChatPanel
            testId="frontstage-chat-panel"
            messages={messages}
            roles={AGENCY_ROLES}
            targetRole={targetRole}
            onTargetRoleChange={setTargetRole}
            input={input}
            onInputChange={setInput}
            onSend={() => {
              void handleSend(input).catch(() => setInput(input));
            }}
            onConfirmActions={handleConfirmActions}
            onRejectActions={handleRejectActions}
            sessions={sessions}
            sessionId={sessionId}
            onSessionChange={setSessionId}
            quickPrompts={[
              { label: '巡检全书状态', text: '/巡检' },
              { label: '让主创续写下一拍', text: '让主创续写下一拍；先给方案，等我确认再动正文。' },
            ]}
            quickCommands={[
              { key: 'patrol', name: '/巡检', desc: '导演看现场' },
              { key: 'continue', name: '/续写', desc: '派主创续写（需确认）' },
            ]}
            placeholder="对导演说，或 @管理/@编审/@主创 直接下指令…"
          />
        )}
      </div>
    </div>
  );
};
