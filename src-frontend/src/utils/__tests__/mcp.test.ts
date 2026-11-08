import { describe, expect, it } from 'vitest';
import type { McpTool } from '@/hooks/useMcpTools';
import {
  bareMcpToolName,
  buildMcpArgsTemplate,
  pickChapterTool,
  serverIdFromMcpToolName,
} from '../mcp';

const tool = (name: string, description = '', parameters?: Record<string, unknown>): McpTool => ({
  name,
  description,
  parameters,
});

describe('mcp utils', () => {
  it('parses server id from prefixed tool names', () => {
    expect(serverIdFromMcpToolName('mcp.legado.get_chapter_content')).toBe('legado');
    expect(serverIdFromMcpToolName('get_chapter_content')).toBeNull();
    expect(bareMcpToolName('mcp.legado.get_chapter_content')).toBe('get_chapter_content');
  });

  it('prefers get_chapter_content and falls back to chapter/content hints', () => {
    expect(
      pickChapterTool([tool('mcp.legado.list_books'), tool('mcp.legado.get_chapter_content')])
    ).toBe('get_chapter_content');
    expect(pickChapterTool([tool('mcp.other.read_chapter_text')])).toBe('read_chapter_text');
    expect(pickChapterTool([tool('mcp.other.list_books', '列出书架')])).toBeNull();
  });

  it('builds automatic args from tool schema', () => {
    const template = buildMcpArgsTemplate(
      tool('mcp.legado.get_chapter_content', '', {
        type: 'object',
        properties: {
          book_url: { type: 'string' },
          index: { type: 'integer' },
          fetch_online: { type: 'boolean' },
        },
      })
    );
    expect(template).toEqual({
      book_url: '$book',
      index: '$index',
      fetch_online: true,
    });
  });
});
