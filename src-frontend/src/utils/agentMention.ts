/**
 * @角色 提及解析（v0.87.0 统一幕后/幕前人机对话）。
 *
 * 消息以 @导演/@管理/@编审/@主创 开头时，路由到对应 agent 的会话；
 * 提及覆盖当前选中的 target_role。仅识别消息起始处的提及，避免把
 * 「他提到 @导演 的安排」这类叙述误判为对 agent 下指令——非 @ 开头的
 * 对话意图由后端 classify_intent 的 is_chat 分类兜住。
 */

/** 四个剧组角色（与 agency/roles.rs 的 AgentRole key 一致）。 */
export const MENTION_ROLES: Record<string, string> = {
  '@导演': 'director',
  '@管理': 'producer',
  '@编审': 'editor_auditor',
  '@主创': 'lead_writer',
};

export interface AgentMention {
  /** 命中的角色 key（director/producer/editor_auditor/lead_writer）。 */
  targetRole: string;
  /** 去掉首个 @token 后的正文。 */
  cleanText: string;
}

/** 幕前默认对话对象：@导演（用户级需求：幕前的对话默认 @导演 agent）。 */
export const DEFAULT_CHAT_ROLE = 'director';

/**
 * 解析消息起始处的 @角色 提及。无命中返回 null。
 * 依次匹配最长 token（数组序即优先级），@ 后允许紧跟文字（如 "@导演进度如何"）。
 * 兼容中文输入法的全角 ＠。
 */
export function parseAgentMention(text: string): AgentMention | null {
  const trimmed = text.trim().replace(/^＠/, '@');
  for (const [token, role] of Object.entries(MENTION_ROLES)) {
    if (trimmed.startsWith(token)) {
      return { targetRole: role, cleanText: trimmed.slice(token.length).trim() };
    }
  }
  return null;
}
