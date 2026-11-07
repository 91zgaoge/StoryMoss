import { describe, it, expect } from 'vitest';
import { parseAgentMention, DEFAULT_CHAT_ROLE, MENTION_ROLES } from '../agentMention';

describe('parseAgentMention', () => {
  it('should parse leading mention with space', () => {
    const m = parseAgentMention('@导演 现在进度怎么样');
    expect(m).not.toBeNull();
    expect(m!.targetRole).toBe('director');
    expect(m!.cleanText).toBe('现在进度怎么样');
  });

  it('should parse mention without space (紧跟文字)', () => {
    const m = parseAgentMention('@导演进度如何');
    expect(m).not.toBeNull();
    expect(m!.targetRole).toBe('director');
    expect(m!.cleanText).toBe('进度如何');
  });

  it('should parse all four roles', () => {
    expect(parseAgentMention('@管理 补齐世界观')!.targetRole).toBe('producer');
    expect(parseAgentMention('@编审 复审最新一章')!.targetRole).toBe('editor_auditor');
    expect(parseAgentMention('@主创 续写下一拍')!.targetRole).toBe('lead_writer');
    expect(parseAgentMention('@导演 巡检')!.targetRole).toBe('director');
  });

  it('should handle fullwidth @', () => {
    const m = parseAgentMention('＠导演 你好');
    expect(m).not.toBeNull();
    expect(m!.targetRole).toBe('director');
    expect(m!.cleanText).toBe('你好');
  });

  it('should return cleanText empty when message is only the mention', () => {
    const m = parseAgentMention('  @导演  ');
    expect(m).not.toBeNull();
    expect(m!.cleanText).toBe('');
  });

  it('should return null for mid-sentence mention (叙述非指令)', () => {
    expect(parseAgentMention('他提到 @导演 的安排')).toBeNull();
    expect(parseAgentMention('看看@导演说了什么')).toBeNull();
  });

  it('should return null for plain text', () => {
    expect(parseAgentMention('继续写下一章')).toBeNull();
    expect(parseAgentMention('')).toBeNull();
  });

  it('should not treat unknown role token as mention', () => {
    expect(parseAgentMention('@路人甲 你好')).toBeNull();
  });
});

describe('DEFAULT_CHAT_ROLE', () => {
  it('should be director (幕前对话默认 @导演)', () => {
    expect(DEFAULT_CHAT_ROLE).toBe('director');
  });

  it('should map every mention token to a role key with a name entry', () => {
    // 四个角色 key 必须与 AgencyStudio ROLE_NAMES 对齐
    const known = new Set([
      'director',
      'producer',
      'editor_auditor',
      'lead_writer',
      'writer',
      'inspector',
      'outline_planner',
      'style_mimic',
    ]);
    for (const role of Object.values(MENTION_ROLES)) {
      expect(known.has(role)).toBe(true);
    }
  });
});
