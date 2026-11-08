import type { McpTool } from '@/hooks/useMcpTools';

/** `mcp.{server_id}.{tool}` → server_id（不匹配返回 null）。 */
export function serverIdFromMcpToolName(name: string): string | null {
  if (!name.startsWith('mcp.')) return null;
  const rest = name.slice(4);
  const dot = rest.indexOf('.');
  if (dot <= 0 || dot === rest.length - 1) return null;
  return rest.slice(0, dot);
}

export function bareMcpToolName(name: string): string {
  const serverId = serverIdFromMcpToolName(name);
  return serverId ? name.slice(`mcp.${serverId}.`.length) : name;
}

export function toolsForServer(tools: McpTool[], serverId: string): McpTool[] {
  return tools.filter(tool => serverIdFromMcpToolName(tool.name) === serverId);
}

const CHAPTER_HINT = /chapter|content|正文|章节/i;

/** 自动选择章节正文工具：优先 get_chapter_content，其次名称/描述命中。 */
export function pickChapterTool(tools: McpTool[]): string | null {
  const candidates = tools.map(tool => ({
    name: bareMcpToolName(tool.name),
    description: tool.description ?? '',
  }));
  if (candidates.some(tool => tool.name === 'get_chapter_content')) {
    return 'get_chapter_content';
  }
  const hinted = candidates.find(
    tool => CHAPTER_HINT.test(tool.name) || CHAPTER_HINT.test(tool.description)
  );
  return hinted?.name ?? null;
}

/**
 * 从工具 JSON Schema 生成自动参数模板：
 * 书名/URL 参数 → $book，章节序号 → $index（后端会转数值），
 * fetch_online/include_content 默认 true。
 */
export function buildMcpArgsTemplate(tool?: McpTool | null): Record<string, unknown> {
  const parameters = (tool?.parameters ?? {}) as {
    properties?: Record<string, { type?: string; default?: unknown }>;
    required?: string[];
  };
  const properties = parameters.properties ?? {};
  const required = new Set(parameters.required ?? []);
  const result: Record<string, unknown> = {};

  for (const [name, schema] of Object.entries(properties)) {
    const lower = name.toLowerCase();
    const type = schema.type;
    if (lower.includes('book') && (type === 'string' || type === undefined)) {
      result[name] = '$book';
      continue;
    }
    if (
      (lower.includes('index') || lower.includes('chapter')) &&
      (type === 'integer' || type === 'number')
    ) {
      result[name] = '$index';
      continue;
    }
    if (lower === 'fetch_online' || lower === 'include_content') {
      result[name] = true;
      continue;
    }
    if (required.has(name) && schema.default !== undefined) {
      result[name] = schema.default;
    }
  }
  return result;
}

export function formatArgsTemplate(template: Record<string, unknown>): string {
  return JSON.stringify(template, null, 2);
}
