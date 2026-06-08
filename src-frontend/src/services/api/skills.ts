import { loggedInvoke } from './core';
import type { Skill, McpServer, McpTool } from '@/types/index';
import type { SceneAnnotation, TextAnnotation, ParagraphCommentary } from '@/types/v3';
// Skills
export const getSkills = () => loggedInvoke<Skill[]>('get_skills');

export const getSkill = (skillId: string) =>
  loggedInvoke<Skill>('get_skill', { skill_id: skillId });

export const importSkill = (path: string) => loggedInvoke<Skill>('import_skill', { path });

export const enableSkill = (skillId: string) =>
  loggedInvoke<void>('enable_skill', { skill_id: skillId });

export const disableSkill = (skillId: string) =>
  loggedInvoke<void>('disable_skill', { skill_id: skillId });

export const uninstallSkill = (skillId: string) =>
  loggedInvoke<void>('uninstall_skill', { skill_id: skillId });

export const updateSkill = (skillId: string, manifest: Partial<Skill>) =>
  loggedInvoke<void>('update_skill', { skill_id: skillId, manifest });

export const executeSkill = (skillId: string, params: Record<string, unknown>) =>
  loggedInvoke<unknown>('execute_skill', { skill_id: skillId, params });

export const formatText = (content: string) => loggedInvoke<string>('format_text', { content });
// MCP
/** @deprecated 暂时保留 — 待 MCP 外部服务器 UI 完成后启用 */
export const connectMcpServer = (config: McpServer) =>
  loggedInvoke<McpTool[]>('connect_mcp_server', { config });

export const callMcpTool = (serverId: string, toolName: string, args: unknown) =>
  loggedInvoke<unknown>('call_mcp_tool', {
    server_id: serverId,
    tool_name: toolName,
    arguments: args,
  });

export const disconnectMcpServer = (serverId: string) =>
  loggedInvoke<void>('disconnect_mcp_server', { server_id: serverId });

export const getMcpConnections = () =>
  loggedInvoke<Array<{ id: string; tools: number; resources: number }>>('get_mcp_connections');

export const listMcpTools = () => loggedInvoke<McpTool[]>('list_mcp_tools');

export const executeMcpTool = (toolName: string, args: unknown) =>
  loggedInvoke<unknown>('execute_mcp_tool', { tool_name: toolName, arguments: args });

export const registerMcpTool = (tool: McpTool) => loggedInvoke<void>('register_mcp_tool', { tool });

export const unregisterMcpTool = (toolName: string) =>
  loggedInvoke<void>('unregister_mcp_tool', { tool_name: toolName });
export const runCreationWorkflow = (storyId: string, mode: string, initialInput: string) =>
  loggedInvoke<{
    success: boolean;
    current_phase: string;
    completed_phases: string[];
    output_preview?: string;
    quality_report?: unknown;
    error?: string;
  }>('run_creation_workflow', { story_id: storyId, mode, initial_input: initialInput });

export const listStyleDnas = () =>
  loggedInvoke<
    Array<{
      id: string;
      name: string;
      author?: string;
      is_builtin: boolean;
      is_user_created: boolean;
    }>
  >('list_style_dnas');

export const setStoryStyleDna = (storyId: string, styleDnaId: string | null) =>
  loggedInvoke<void>('set_story_style_dna', { story_id: storyId, style_dna_id: styleDnaId });

export const analyzeStyleSample = (text: string, name?: string) =>
  loggedInvoke<{
    id: string;
    name: string;
    author?: string;
    is_builtin: boolean;
    is_user_created: boolean;
  }>('analyze_style_sample', { text, name });
// v4.4.0 - 风格混合命令
export const getStoryStyleBlend = (storyId: string) =>
  loggedInvoke<{
    id: string;
    story_id: string;
    name: string;
    blend: import('@/types/index').StyleBlendConfig;
    is_active: boolean;
  } | null>('get_story_style_blend', { story_id: storyId });

export const setStoryStyleBlend = (storyId: string, name: string, blendJson: string) =>
  loggedInvoke<{
    id: string;
    story_id: string;
    name: string;
    blend: import('@/types/index').StyleBlendConfig;
    is_active: boolean;
    updated?: boolean;
    created?: boolean;
  }>('set_story_style_blend', { story_id: storyId, name, blend_json: blendJson });

export const updateSceneStyleBlend = (sceneId: string, blendOverride?: string) =>
  loggedInvoke<void>('update_scene_style_blend', {
    scene_id: sceneId,
    blend_override: blendOverride,
  });

export const checkStyleDrift = (text: string, storyId: string, sceneNumber?: number) =>
  loggedInvoke<import('@/types/index').DriftCheckResult>('check_style_drift', {
    text,
    story_id: storyId,
    scene_number: sceneNumber,
  });
