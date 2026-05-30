import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/utils/logger';

const apiLogger = createLogger('api:tauri');

/** 参数脱敏：移除敏感字段并截断长内容 */
function sanitizeArgs(
  args: Record<string, unknown> | undefined
): Record<string, unknown> | undefined {
  if (!args) return undefined;
  const sanitized: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    if (
      key.includes('api_key') ||
      key.includes('token') ||
      key.includes('password') ||
      key.includes('secret')
    ) {
      sanitized[key] = '***';
    } else if (typeof value === 'string' && value.length > 500) {
      sanitized[key] = value.slice(0, 500) + '...';
    } else {
      sanitized[key] = value;
    }
  }
  return sanitized;
}

/** 带日志追踪的 invoke 包装 */
/** 带日志追踪的 invoke 包装 — 统一导出供全局使用 */
export async function loggedInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const start = performance.now();
  const safeArgs = sanitizeArgs(args);
  apiLogger.debug(`→ ${cmd}`, safeArgs);
  try {
    const result = await invoke<T>(cmd, args);
    const duration = Math.round(performance.now() - start);
    apiLogger.debug(`← ${cmd} ok (${duration}ms)`);
    return result;
  } catch (error) {
    const duration = Math.round(performance.now() - start);
    apiLogger.error(`✗ ${cmd} failed (${duration}ms)`, { error, args: safeArgs });
    throw error;
  }
}
import type {
  Story,
  Character,
  Chapter,
  Skill,
  McpServer,
  McpTool,
  DashboardState,
  CreateStoryRequest,
  CreateCharacterRequest,
  UpdateChapterRequest,
  LlmConfig,
  SimilarityResult,
  VectorSearchRequest,
  Intent,
  IntentParseRequest,
  IntentExecutionResult,
  StoryOutline,
  CharacterRelationship,
} from '@/types/index';
import type {
  StoryGraph,
  Entity,
  Relation,
  RetentionReport,
  ArchiveResult,
  WorldBuildingOption,
  CharacterProfileOption,
  WritingStyleOption,
  SceneProposal,
  SceneAnnotation,
  TextAnnotation,
  ParagraphCommentary,
  AgentResult,
  VectorSearchResult,
  StorySummary,
} from '@/types/v3';
import type { WizardCreationResult } from '@/types/index';
import type { AppSettings } from '@/types/llm';
import type {
  Draft,
  Revision,
  PipelineReview,
  PostProcessRun,
  PostProcessStep,
  LlmCall,
  CharacterState,
  RefineResult,
  ReviewResult,
  PipelineResult,
} from '@/types/pipeline';

// Health Check
export const healthCheck = () =>
  loggedInvoke<{ status: string; timestamp: string; version: string }>('health_check');

// Stories
export const listStories = () => loggedInvoke<Story[]>('list_stories');

export const createStory = (req: CreateStoryRequest) =>
  loggedInvoke<Story>('create_story', { ...req });

export const updateStory = (id: string, updates: Partial<Story>) =>
  loggedInvoke<void>('update_story', { id, ...updates });

export const deleteStory = (id: string) => loggedInvoke<void>('delete_story', { id });

// Characters
export const getStoryCharacters = (storyId: string) =>
  loggedInvoke<Character[]>('get_story_characters', { story_id: storyId });

export const createCharacter = (req: CreateCharacterRequest) =>
  loggedInvoke<Character>('create_character', { ...req });

export const updateCharacter = (id: string, updates: Partial<Character>) =>
  loggedInvoke<void>('update_character', { id, ...updates });

export const deleteCharacter = (id: string) => loggedInvoke<void>('delete_character', { id });

export interface CharacterQuickView {
  id: string;
  name: string;
  appearance_summary: string;
  status_tags: string[];
  last_seen_chapter: number;
}

export const getCharacterByName = (storyId: string, name: string) =>
  loggedInvoke<CharacterQuickView | null>('get_character_by_name', { story_id: storyId, name });

// Chapters
export const getStoryChapters = (storyId: string) =>
  loggedInvoke<Chapter[]>('get_story_chapters', { story_id: storyId });

export const getChapter = (id: string) => loggedInvoke<Chapter | null>('get_chapter', { id });

export const updateChapter = (id: string, updates: UpdateChapterRequest) =>
  loggedInvoke<void>('update_chapter', { id, ...updates });

export const deleteChapter = (id: string) => loggedInvoke<void>('delete_chapter', { id });

export const createChapter = (req: {
  story_id: string;
  chapter_number: number;
  title?: string;
  outline?: string;
  content?: string;
}) => loggedInvoke<Chapter>('create_chapter', { ...req });

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

// Vector Search (NEW - LanceDB)
export const searchSimilar = (req: VectorSearchRequest) =>
  loggedInvoke<SimilarityResult[]>('search_similar', {
    story_id: req.story_id,
    query: req.query,
    top_k: req.top_k,
  });

// Settings (兼容旧接口，内部映射到 get_settings / save_settings)
export const getConfig = async () => {
  const settings = await loggedInvoke<AppSettings>('get_settings');
  const chatModel =
    settings.models.chat?.find((m: any) => m.id === settings.active_models.chat) ||
    settings.models.chat?.[0];
  if (!chatModel) {
    throw new Error('No chat model configured');
  }
  return {
    provider: chatModel.provider || 'custom',
    api_key: chatModel.api_key || '',
    model: chatModel.model || '',
    temperature: chatModel.temperature ?? 0.8,
    max_tokens: chatModel.max_tokens ?? 4096,
    base_url: chatModel.api_base || '',
  } as LlmConfig;
};

export const updateConfig = async (config: { llm: LlmConfig }) => {
  const settings = await loggedInvoke<AppSettings>('get_settings');
  const chatModel =
    settings.models.chat?.find((m: any) => m.id === settings.active_models.chat) ||
    settings.models.chat?.[0];
  if (chatModel) {
    chatModel.provider = config.llm.provider;
    chatModel.api_key = config.llm.api_key || '';
    chatModel.model = config.llm.model;
    chatModel.temperature = config.llm.temperature;
    chatModel.max_tokens = config.llm.max_tokens;
    chatModel.api_base = config.llm.base_url;
  }
  await loggedInvoke<void>('save_settings', { settings });
};

// Intent Engine
export const parseIntent = (req: IntentParseRequest) =>
  loggedInvoke<Intent>('parse_intent', { user_input: req.user_input });

export const executeIntent = (intent: Intent, storyId: string) =>
  loggedInvoke<IntentExecutionResult>('execute_intent', { intent, story_id: storyId });

// Smart Execute - Model-driven orchestration
export interface SmartExecuteRequest {
  user_input: string;
  current_content?: string;
  selected_text?: string;
  style_weight?: number;
}

export interface SmartExecuteResult {
  success: boolean;
  steps_completed: number;
  final_content?: string;
  messages: string[];
}

export interface PreflightResult {
  ready: boolean;
  missing_contracts: string[];
  warnings: string[];
  blocking_issues: string[];
}

export const checkPreflight = (storyId: string, chapterNumber: number) =>
  loggedInvoke<PreflightResult>('check_preflight', {
    story_id: storyId,
    chapter_number: chapterNumber,
  });

export interface AutoCreateContractsResult {
  created_master_setting: boolean;
  created_chapter_contract: boolean;
  created_outline: boolean;
  message: string;
}

export const autoCreateMissingContracts = (
  storyId: string,
  chapterNumber: number,
  sceneId?: string
) =>
  loggedInvoke<AutoCreateContractsResult>('auto_create_missing_contracts', {
    story_id: storyId,
    chapter_number: chapterNumber,
    scene_id: sceneId,
  });

export const smartExecute = (req: SmartExecuteRequest) =>
  loggedInvoke<SmartExecuteResult>('smart_execute', {
    user_input: req.user_input,
    current_content: req.current_content,
    selected_text: req.selected_text,
  });

// Feedback Recording
export interface RecordFeedbackRequest {
  story_id: string;
  scene_id?: string;
  chapter_id?: string;
  feedback_type: 'accept' | 'reject' | 'modify';
  agent_type?: string;
  original_ai_text: string;
  final_text?: string;
}

export interface LearningPoint {
  category: string;
  observation: string;
  impact: string;
}

export const recordFeedback = (req: RecordFeedbackRequest) =>
  loggedInvoke<LearningPoint[]>('record_feedback', { request: req });

// Knowledge Graph
export const getStoryGraph = (storyId: string) =>
  loggedInvoke<StoryGraph>('get_story_graph', { story_id: storyId });

export const getRetentionReport = (storyId: string) =>
  loggedInvoke<RetentionReport>('get_retention_report', { story_id: storyId });

export const archiveForgottenEntities = (storyId: string) =>
  loggedInvoke<ArchiveResult>('archive_forgotten_entities', { story_id: storyId });

export const restoreArchivedEntity = (entityId: string) =>
  loggedInvoke<Entity>('restore_archived_entity', { entity_id: entityId });

export const getArchivedEntities = (storyId: string) =>
  loggedInvoke<Entity[]>('get_archived_entities', { story_id: storyId });

export const updateEntity = (
  entityId: string,
  updates: { name?: string; attributes?: Record<string, unknown> }
) =>
  loggedInvoke<Entity>('update_entity', {
    entity_id: entityId,
    name: updates.name,
    attributes: updates.attributes,
  });

// Novel Creation Wizard
export const generateWorldBuildingOptions = (userInput: string) =>
  loggedInvoke<WorldBuildingOption[]>('generate_world_building_options', { user_input: userInput });

export const generateCharacterProfiles = (worldBuilding: WorldBuildingOption) =>
  loggedInvoke<CharacterProfileOption[][]>('generate_character_profiles', {
    world_building: worldBuilding,
  });

export const generateWritingStyles = (genre: string, worldBuilding: WorldBuildingOption) =>
  loggedInvoke<WritingStyleOption[]>('generate_writing_styles', {
    genre,
    world_building: worldBuilding,
  });

export const generateFirstScene = (
  worldBuilding: WorldBuildingOption,
  characters: CharacterProfileOption[],
  writingStyle: WritingStyleOption
) =>
  loggedInvoke<SceneProposal>('generate_first_scene', {
    world_building: worldBuilding,
    characters,
    writing_style: writingStyle,
  });

export const createStoryWithWizard = (params: {
  title: string;
  description?: string;
  genre?: string;
  world_building: WorldBuildingOption;
  characters: CharacterProfileOption[];
  writing_style: WritingStyleOption;
  first_scene: SceneProposal;
}) =>
  loggedInvoke<import('@/types/index').WizardCreationResult>('create_story_with_wizard', params);

// Scene Annotations
export const createSceneAnnotation = (params: {
  scene_id: string;
  story_id: string;
  content: string;
  annotation_type: string;
}) => loggedInvoke<SceneAnnotation>('create_scene_annotation', params);

export const getSceneAnnotations = (sceneId: string) =>
  loggedInvoke<SceneAnnotation[]>('get_scene_annotations', { scene_id: sceneId });

export const getStoryUnresolvedAnnotations = (storyId: string) =>
  loggedInvoke<SceneAnnotation[]>('get_story_unresolved_annotations', { story_id: storyId });

export const updateSceneAnnotation = (annotationId: string, content: string) =>
  loggedInvoke<number>('update_scene_annotation', { annotation_id: annotationId, content });

export const resolveSceneAnnotation = (annotationId: string) =>
  loggedInvoke<number>('resolve_scene_annotation', { annotation_id: annotationId });

export const unresolveSceneAnnotation = (annotationId: string) =>
  loggedInvoke<number>('unresolve_scene_annotation', { annotation_id: annotationId });

export const deleteSceneAnnotation = (annotationId: string) =>
  loggedInvoke<number>('delete_scene_annotation', { annotation_id: annotationId });

// Text Inline Annotations
export const createTextAnnotation = (params: {
  story_id: string;
  scene_id?: string;
  chapter_id?: string;
  content: string;
  annotation_type: string;
  from_pos: number;
  to_pos: number;
}) => loggedInvoke<TextAnnotation>('create_text_annotation', params);

export const getTextAnnotationsByChapter = (chapterId: string) =>
  loggedInvoke<TextAnnotation[]>('get_text_annotations_by_chapter', { chapter_id: chapterId });

export const getTextAnnotationsByScene = (sceneId: string) =>
  loggedInvoke<TextAnnotation[]>('get_text_annotations_by_scene', { scene_id: sceneId });

export const updateTextAnnotation = (annotationId: string, content: string) =>
  loggedInvoke<number>('update_text_annotation', { annotation_id: annotationId, content });

export const resolveTextAnnotation = (annotationId: string) =>
  loggedInvoke<number>('resolve_text_annotation', { annotation_id: annotationId });

export const unresolveTextAnnotation = (annotationId: string) =>
  loggedInvoke<number>('unresolve_text_annotation', { annotation_id: annotationId });

export const deleteTextAnnotation = (annotationId: string) =>
  loggedInvoke<number>('delete_text_annotation', { annotation_id: annotationId });

// Commentator Agent
export const generateParagraphCommentaries = (params: {
  story_id: string;
  story_title: string;
  genre: string;
  text: string;
}) => loggedInvoke<string>('generate_paragraph_commentaries', params);

// Vector Search
export const textSearchVectors = (storyId: string, query: string, top_k?: number) =>
  loggedInvoke<VectorSearchResult[]>('text_search_vectors', { story_id: storyId, query, top_k });

export const hybridSearchVectors = (storyId: string, query: string, top_k?: number) =>
  loggedInvoke<VectorSearchResult[]>('hybrid_search_vectors', { story_id: storyId, query, top_k });

// Memory Compressor
export const compressContent = (params: {
  story_id: string;
  content: string;
  target_ratio?: number;
}) => loggedInvoke<AgentResult>('compress_content', params);

export const compressScene = (params: { scene_id: string; target_ratio?: number }) =>
  loggedInvoke<AgentResult>('compress_scene', params);

// Knowledge Distillation
export const distillStoryKnowledge = (storyId: string) =>
  loggedInvoke<StorySummary>('distill_story_knowledge', { story_id: storyId });

export const getStorySummaries = (storyId: string) =>
  loggedInvoke<StorySummary[]>('get_story_summaries', { story_id: storyId });

export const updateStorySummary = (summaryId: string, content: string) =>
  loggedInvoke<number>('update_story_summary', { summary_id: summaryId, content });

export const deleteStorySummary = (summaryId: string) =>
  loggedInvoke<number>('delete_story_summary', { summary_id: summaryId });

// ==================== LLM Stream ====================

export const llmGenerateStream = (params: {
  request_id: string;
  prompt: string;
  context?: string;
  max_tokens?: number;
  temperature?: number;
}) => loggedInvoke<void>('llm_generate_stream', { request: params });

export const llmCancelGeneration = (requestId: string) =>
  loggedInvoke<void>('llm_cancel_generation', { request_id: requestId });

// ==================== Subscription (Freemium) ====================

export interface SubscriptionStatus {
  user_id: string;
  tier: string;
  status: string;
  expires_at?: string;
}

export const getSubscriptionStatus = () =>
  loggedInvoke<SubscriptionStatus>('get_subscription_status');

export const devUpgradeSubscription = (tier: string) =>
  loggedInvoke<SubscriptionStatus>('dev_upgrade_subscription', { tier });

export const devDowngradeSubscription = () =>
  loggedInvoke<SubscriptionStatus>('dev_downgrade_subscription');

// ==================== 文思泉涌 ====================

export const autoWrite = (params: {
  story_id: string;
  chapter_id: string;
  target_chars: number;
  chars_per_loop: number;
  reference_text?: string;
  style_weight?: number;
}) =>
  loggedInvoke<{ task_id: string; actual_chars: number; loops: number; status: string }>(
    'auto_write',
    { request: params }
  );

export const autoWriteCancel = (taskId: string) =>
  loggedInvoke<void>('auto_write_cancel', { task_id: taskId });

export const autoRevise = (params: {
  story_id: string;
  chapter_id?: string;
  scope: string;
  selected_text?: string;
  revision_type: string;
}) =>
  loggedInvoke<{ task_id: string; revised_text: string; status: string }>('auto_revise', {
    request: params,
  });

export const autoReviseCancel = (taskId: string) =>
  loggedInvoke<void>('auto_revise_cancel', { task_id: taskId });

// Window communication
export const notifyFrontstageDataRefresh = (entity: string) =>
  loggedInvoke<void>('notify_frontstage_data_refresh', { entity });

// Input hint — LLM智能输入建议
export const getInputHint = (currentContent?: string) =>
  loggedInvoke<string>('get_input_hint', { current_content: currentContent });

// ==================== Genesis Engine (v5.0.0) ====================

export const getStoryOutline = (storyId: string) =>
  loggedInvoke<StoryOutline | null>('get_story_outline', { story_id: storyId });

export const updateStoryOutline = (storyId: string, content: string, structureJson?: string) =>
  loggedInvoke<void>('update_story_outline', {
    story_id: storyId,
    content,
    structure_json: structureJson,
  });

export const getCharacterRelationships = (storyId: string) =>
  loggedInvoke<CharacterRelationship[]>('get_character_relationships', { story_id: storyId });

export const createCharacterRelationship = (params: {
  story_id: string;
  character_a_id: string;
  character_b_id: string;
  relationship_type: string;
  description?: string;
}) => loggedInvoke<CharacterRelationship>('create_character_relationship', params);

export const updateCharacterRelationship = (
  relationshipId: string,
  updates: {
    relationship_type?: string;
    description?: string;
  }
) =>
  loggedInvoke<void>('update_character_relationship', {
    relationship_id: relationshipId,
    ...updates,
  });

export const deleteCharacterRelationship = (relationshipId: string) =>
  loggedInvoke<void>('delete_character_relationship', { relationship_id: relationshipId });

// Scenes
export const createScene = (params: {
  story_id: string;
  chapter_id?: string;
  scene_number: number;
  title?: string;
  content?: string;
  outline?: string;
}) => loggedInvoke<import('@/types/v3').Scene>('create_scene', params);

export const getStoryScenes = (storyId: string) =>
  loggedInvoke<import('@/types/v3').Scene[]>('get_story_scenes', { story_id: storyId });

export const getScene = (sceneId: string) =>
  loggedInvoke<import('@/types/v3').Scene | null>('get_scene', { scene_id: sceneId });

export const updateScene = (
  sceneId: string,
  updates: {
    title?: string;
    content?: string;
    outline?: string;
    scene_number?: number;
  }
) => loggedInvoke<void>('update_scene', { scene_id: sceneId, ...updates });

export const deleteScene = (sceneId: string) =>
  loggedInvoke<void>('delete_scene', { scene_id: sceneId });

export const reorderScenes = (
  storyId: string,
  sceneOrders: Array<{ scene_id: string; new_number: number }>
) => loggedInvoke<void>('reorder_scenes', { story_id: storyId, scene_orders: sceneOrders });

// World Building
export const createWorldBuilding = (params: {
  story_id: string;
  category: string;
  title: string;
  content: string;
}) => loggedInvoke<import('@/types/v3').WorldBuilding>('create_world_building', params);

export const getWorldBuilding = (storyId: string) =>
  loggedInvoke<import('@/types/v3').WorldBuilding[]>('get_world_building', { story_id: storyId });

export const updateWorldBuilding = (
  worldBuildingId: string,
  updates: {
    category?: string;
    title?: string;
    content?: string;
  }
) =>
  loggedInvoke<void>('update_world_building', { world_building_id: worldBuildingId, ...updates });

export const deleteWorldBuilding = (worldBuildingId: string) =>
  loggedInvoke<void>('delete_world_building', { world_building_id: worldBuildingId });

// Writing Style
export const createWritingStyle = (params: {
  story_id: string;
  name: string;
  description?: string;
  style_rules?: string;
}) => loggedInvoke<import('@/types/v3').WritingStyle>('create_writing_style', params);

export const getWritingStyle = (storyId: string) =>
  loggedInvoke<import('@/types/v3').WritingStyle | null>('get_writing_style', {
    story_id: storyId,
  });

export const updateWritingStyle = (
  styleId: string,
  updates: {
    name?: string;
    description?: string;
    style_rules?: string;
  }
) => loggedInvoke<void>('update_writing_style', { style_id: styleId, ...updates });

// Entities and Relations
export const createEntity = (params: {
  story_id: string;
  name: string;
  entity_type: string;
  attributes?: Record<string, any>;
}) => loggedInvoke<import('@/types/v3').Entity>('create_entity', params);

export const getStoryEntities = (storyId: string) =>
  loggedInvoke<import('@/types/v3').Entity[]>('get_story_entities', { story_id: storyId });

export const createRelation = (params: {
  story_id: string;
  from_entity_id: string;
  to_entity_id: string;
  relation_type: string;
  description?: string;
}) => loggedInvoke<import('@/types/v3').Relation>('create_relation', params);

export const getEntityRelations = (entityId: string) =>
  loggedInvoke<import('@/types/v3').Relation[]>('get_entity_relations', { entity_id: entityId });

export const getIngestJobs = (storyId: string, limit: number = 10) =>
  loggedInvoke<import('@/types/v3').IngestJob[]>('get_ingest_jobs', { story_id: storyId, limit });

export const checkProjectionHealth = (storyId: string, chapterNumber: number) =>
  loggedInvoke<import('@/types/v3').ProjectionHealthReport>('check_projection_health', {
    story_id: storyId,
    chapter_number: chapterNumber,
  });

export const saveGenreProfile = (params: {
  id?: string;
  genre_name: string;
  canonical_name: string;
  aliases_json?: string;
  core_tone?: string;
  pacing_strategy?: string;
  anti_patterns_json?: string;
  reference_tables_json?: string;
}) => loggedInvoke<GenreProfile>('save_genre_profile', params);

export const deleteGenreProfile = (id: string) =>
  loggedInvoke<number>('delete_genre_profile', { id });

export const getFeatureUsageStats = (days: number = 30) =>
  loggedInvoke<Array<{ feature_id: string; action: string; count: number }>>(
    'get_feature_usage_stats',
    { days }
  );

export const logFeatureUsage = (featureId: string, action: string, storyId?: string) =>
  loggedInvoke<void>('log_frontend_feature_usage', {
    feature_id: featureId,
    action,
    story_id: storyId,
  });

// ==================== v6.0.0: Story System ====================

export interface StoryContract {
  id: string;
  story_id: string;
  contract_type: string;
  contract_json: string;
  version: number;
  created_at: string;
  updated_at: string;
}

export interface ChapterCommit {
  id: string;
  story_id: string;
  scene_id: string | null;
  chapter_id: string | null;
  chapter_number: number;
  status: string;
  outline_snapshot_json: string | null;
  review_result_json: string | null;
  fulfillment_result_json: string | null;
  accepted_events_json: string | null;
  state_deltas_json: string | null;
  entity_deltas_json: string | null;
  summary_text: string | null;
  dominant_strand: string | null;
  projection_status_json: string | null;
  created_at: string;
}

export interface ContractTree {
  master_setting: StoryContract | null;
  volumes: Record<string, StoryContract>;
  chapters: Record<string, StoryContract>;
  reviews: Record<string, StoryContract>;
}

export interface RuntimeContract {
  master_setting: StoryContract;
  chapter_contract: StoryContract | null;
}

export const createMasterSetting = (params: {
  story_id: string;
  genre: string;
  core_tone: string;
  pacing_strategy: string;
  anti_patterns: string[];
  world_rules: string[];
}) => loggedInvoke<StoryContract>('create_master_setting', params);

export const createChapterContract = (params: {
  story_id: string;
  chapter_number: number;
  goal: string;
  must_cover_nodes: string[];
  forbidden_zones: string[];
  time_anchor?: string;
  chapter_span?: string;
}) => loggedInvoke<StoryContract>('create_chapter_contract', params);

export const getContractTree = (storyId: string) =>
  loggedInvoke<ContractTree>('get_contract_tree', { story_id: storyId });

export const getRuntimeContract = (storyId: string, chapterNumber: number) =>
  loggedInvoke<RuntimeContract>('get_runtime_contract', {
    story_id: storyId,
    chapter_number: chapterNumber,
  });

export const initChapterCommit = (
  storyId: string,
  chapterNumber: number,
  sceneId?: string,
  chapterId?: string
) =>
  loggedInvoke<ChapterCommit>('init_chapter_commit', {
    story_id: storyId,
    chapter_number: chapterNumber,
    scene_id: sceneId,
    chapter_id: chapterId,
  });

// W2-B5: apply_chapter_commit 已改为 update_chapter 成功后自动触发（30s debounce）
// 前端不再显式调用，保留 get_chapter_commits 用于展示 commit 历史
export const getChapterCommits = (storyId: string) =>
  loggedInvoke<ChapterCommit[]>('get_chapter_commits', { story_id: storyId });

// ==================== v6.0.0: Memory System ====================

export interface MemoryPack {
  working_memory: MemoryEntry[];
  episodic_memory: MemoryEntry[];
  semantic_memory: MemoryItemDto[];
  long_term_facts: MemoryItemDto[];
  active_constraints: string[];
  recent_changes: string[];
  warnings: MemoryWarning[];
  stats: MemoryStats;
}

export interface MemoryEntry {
  subject: string;
  field: string;
  value: string;
  source_chapter: number;
}

export interface MemoryItemDto {
  id: string;
  category: string;
  subject: string | null;
  field: string | null;
  value: string | null;
  source_chapter: number | null;
  confidence: number;
}

export interface MemoryWarning {
  category: string;
  subject: string;
  count: number;
}

export interface MemoryStats {
  total: number;
  working_total: number;
  episodic_total: number;
  semantic_total: number;
  injected: number;
  layered_total_injected: number;
  filtered: number;
  conflicts: number;
}

export interface MemoryItem {
  id: string;
  story_id: string;
  category: string;
  subject: string | null;
  field: string | null;
  value: string | null;
  source_chapter: number | null;
  confidence: number;
  status: string;
  updated_at: string;
}

export const buildMemoryPack = (
  storyId: string,
  chapterNumber: number,
  taskType: string,
  outline?: string
) =>
  loggedInvoke<MemoryPack>('build_memory_pack', {
    story_id: storyId,
    chapter_number: chapterNumber,
    task_type: taskType,
    outline,
  });

export const getMemoryItems = (storyId: string) =>
  loggedInvoke<MemoryItem[]>('get_memory_items', { story_id: storyId });

export const createMemoryItem = (params: {
  story_id: string;
  category: string;
  subject?: string;
  field?: string;
  value?: string;
  source_chapter?: number;
  confidence: number;
}) => loggedInvoke<MemoryItem>('create_memory_item', params);

// ==================== v6.0.0: Reading Power ====================

export interface ReadingPowerEvaluation {
  chapter_number: number;
  hook_type: string | null;
  hook_strength: string;
  coolpoint_patterns: string[];
  micropayoffs: string[];
  hard_violations: string[];
  soft_suggestions: string[];
  is_transition: boolean;
  override_count: number;
  debt_balance: number;
  score: number;
}

export interface ChaseDebt {
  id: number;
  story_id: string;
  debt_type: string;
  original_amount: number;
  current_amount: number;
  interest_rate: number;
  source_chapter: number;
  due_chapter: number;
  override_contract_id: number | null;
  status: string;
  created_at: string;
}

// ==================== v6.0.0: Story Audit ====================

export interface StoryAnalysisReport {
  story_id: string;
  overall_score: number;
  dimensions: AuditDimension[];
  findings: AuditFinding[];
  recommendations: string[];
}

export interface AuditDimension {
  name: string;
  score: number;
  weight: number;
  description: string;
  details: string[];
}

export interface AuditFinding {
  severity: 'Critical' | 'Warning' | 'Info';
  category: string;
  message: string;
  suggestion: string;
}

export const auditStory = (storyId: string) =>
  loggedInvoke<StoryAnalysisReport>('audit_story', { story_id: storyId });

export interface OverrideContract {
  id: number;
  story_id: string;
  chapter_number: number;
  constraint_type: string;
  constraint_id: string;
  rationale_type: string;
  rationale_text: string;
  payback_plan: string;
  due_chapter: number;
  status: string;
  fulfilled_at: string | null;
  created_at: string;
}

export const evaluateReadingPower = (storyId: string, chapterNumber: number) =>
  loggedInvoke<ReadingPowerEvaluation>('evaluate_reading_power', {
    story_id: storyId,
    chapter_number: chapterNumber,
  });

export const getReadingPowerTrend = (storyId: string, lastN: number) =>
  loggedInvoke<ReadingPowerEvaluation[]>('get_reading_power_trend', {
    story_id: storyId,
    last_n: lastN,
  });

export const getChaseDebts = (storyId: string) =>
  loggedInvoke<ChaseDebt[]>('get_chase_debts', { story_id: storyId });

export const createOverrideContract = (params: {
  story_id: string;
  chapter_number: number;
  constraint_type: string;
  constraint_id: string;
  rationale_type: string;
  rationale_text: string;
  payback_plan: string;
  due_chapter: number;
}) => loggedInvoke<OverrideContract>('create_override_contract', params);

// ==================== v6.0.0: Genre Profiles ====================

export interface GenreProfile {
  id: number;
  genre_name: string;
  canonical_name: string;
  aliases: string[];
  core_tone: string;
  pacing_strategy: string;
  anti_patterns: string[];
  reference_tables: string[];
  is_builtin: boolean;
  created_at: string;
}

export const getGenreProfiles = () => loggedInvoke<GenreProfile[]>('get_genre_profiles');

export const getGenreProfile = (genreName: string) =>
  loggedInvoke<GenreProfile | null>('get_genre_profile', { genre_name: genreName });

// ==================== v6.0.0: Anti-AI Review ====================

export interface AntiAiReview {
  overall_score: number;
  dimensions: DimensionScore[];
  issues: ReviewIssue[];
  suggestions: string[];
  flagged_passages: FlaggedPassage[];
}

export interface DimensionScore {
  name: string;
  score: number;
  weight: number;
  description: string;
}

export interface ReviewIssue {
  dimension: string;
  severity: string;
  description: string;
  example: string;
  suggestion: string;
}

export interface FlaggedPassage {
  text: string;
  dimension: string;
  reason: string;
  position: number;
}

export const antiAiReview = (text: string, genre?: string) =>
  loggedInvoke<AntiAiReview>('anti_ai_review', { text, genre });

export interface StyleDnaDelta {
  sentence_length_delta: number;
  dialogue_ratio_delta: number;
  metaphor_density_delta: number;
  interior_monologue_delta: number;
  emotion_density_delta: number;
  rhythm_score_delta: number;
  vocabulary_density_shift: string | null;
  expressiveness_shift: string | null;
  avoided_patterns_add: string[];
  reasons: string[];
}

export const evolveStyleFromAntiAiReview = (storyId: string, review: AntiAiReview) =>
  loggedInvoke<StyleDnaDelta>('evolve_style_from_anti_ai_review', { story_id: storyId, review });

// ==================== v7.0.0: Pipeline 管线体系 ====================

// --- Draft (保留前端查询) ---

export const getStoryChapterDrafts = (storyId: string, chapterNumber: number) =>
  loggedInvoke<Draft[]>('get_story_chapter_drafts', {
    story_id: storyId,
    chapter_number: chapterNumber,
  });

export const getLatestPipelineReview = (draftId: string) =>
  loggedInvoke<PipelineReview | null>('get_latest_pipeline_review', { draft_id: draftId });

// --- LLM Call (保留前端查询) ---

export const getStoryLlmCalls = (storyId: string, limit: number) =>
  loggedInvoke<LlmCall[]>('get_story_llm_calls', { story_id: storyId, limit });

export const getRecentLlmCalls = (limit: number) =>
  loggedInvoke<LlmCall[]>('get_recent_llm_calls', { limit });

export const getLlmCallStats = (storyId: string) =>
  loggedInvoke<{ count: number; total_tokens: number; total_cost: number }>('get_llm_call_stats', {
    story_id: storyId,
  });

// --- Writing Analytics ---

export const getWritingAnalytics = (storyId: string) =>
  loggedInvoke<import('@/types/v3').WritingAnalytics>('get_writing_analytics', {
    story_id: storyId,
  });

// --- Pipeline High-Level Commands ---

export const runRefine = (storyId: string, draftId: string, userPrompt?: string) =>
  loggedInvoke<RefineResult>('run_refine', {
    story_id: storyId,
    draft_id: draftId,
    user_prompt: userPrompt,
  });

export const runReview = (storyId: string, draftId: string, reviewFocus?: string) =>
  loggedInvoke<ReviewResult>('run_review', {
    story_id: storyId,
    draft_id: draftId,
    review_focus: reviewFocus,
  });

export const runFinalize = (
  storyId: string,
  draftId: string,
  chapterNumber: number,
  chapterTitle?: string
) =>
  loggedInvoke<PipelineResult>('run_finalize', {
    story_id: storyId,
    draft_id: draftId,
    chapter_number: chapterNumber,
    chapter_title: chapterTitle,
  });

export const repairFinalize = (storyId: string, chapterNumber: number) =>
  loggedInvoke<PipelineResult>('repair_finalize', {
    story_id: storyId,
    chapter_number: chapterNumber,
  });

export const getPipelineActiveDraft = (storyId: string, chapterNumber: number) =>
  loggedInvoke<Draft | null>('get_pipeline_active_draft', {
    story_id: storyId,
    chapter_number: chapterNumber,
  });

export const mergeRevision = (revisionId: string) =>
  loggedInvoke<number>('merge_revision', { revision_id: revisionId });

export const getDraftRevisionHistory = (draftId: string) =>
  loggedInvoke<Revision[]>('get_draft_revision_history', { draft_id: draftId });

export const getDraftReviewHistory = (draftId: string) =>
  loggedInvoke<PipelineReview[]>('get_draft_review_history', { draft_id: draftId });

export const getPostProcessStatus = (runId: string) =>
  loggedInvoke<{ run: PostProcessRun; steps: PostProcessStep[] } | null>(
    'get_post_process_status',
    { run_id: runId }
  );

// --- Character State ---

export const updateCharacterState = (characterId: string, state: CharacterState) =>
  loggedInvoke<number>('update_character_state', { character_id: characterId, state });

// --- Genesis Pipeline (W3-F1) ---

export interface GenesisRun {
  id: string;
  story_id?: string;
  session_id: string;
  premise: string;
  status: string;
  current_step?: string;
  current_step_number: number;
  total_steps: number;
  steps_json: string;
  error_message?: string;
  created_at: string;
  updated_at: string;
}

export const listGenesisRuns = (limit?: number) =>
  loggedInvoke<GenesisRun[]>('list_genesis_runs', { limit });

export const getGenesisRun = (id: string) =>
  loggedInvoke<GenesisRun | null>('get_genesis_run', { id });

export const cancelGenesisPipeline = (sessionId: string) =>
  loggedInvoke<boolean>('cancel_genesis_pipeline', { session_id: sessionId });

// --- StyleDNA (W3-F2) ---

export interface StyleSnapshot {
  id: string;
  story_id: string;
  chapter_number?: number;
  scene_number?: number;
  sentence_length: number;
  dialogue_ratio: number;
  metaphor_density: number;
  inner_monologue_ratio: number;
  emotion_density: number;
  rhythm_score: number;
  computed_at: string;
}

export const getLatestStyleSnapshot = (storyId: string) =>
  loggedInvoke<StyleSnapshot | null>('get_latest_style_snapshot', { story_id: storyId });

// --- LitSeg 叙事分析 ---

export interface NarrativeStructureAct {
  act_number: number;
  act_type: string;
  start_chapter: number;
  end_chapter: number;
  summary?: string;
}

export interface NarrativeEvent {
  scene_id: string;
  scene_number: number;
  title?: string;
  intensity?: number;
  sentiment?: number;
  event_types?: string;
  act_number?: number;
  position_in_act?: number;
}

export interface NarrativeThread {
  type: string;
  content: string;
  status: string;
  risk_score?: number;
}

export interface NarrativeChunk {
  id: string;
  story_id: string;
  chapter_range_start: number;
  chapter_range_end: number;
  text: string;
  chunk_type: string;
}

export const analyzeNarrativeStructure = (storyId: string) =>
  loggedInvoke<{ structure: NarrativeStructureAct[] }>('analyze_narrative_structure', {
    story_id: storyId,
  });

export const getNarrativeEvents = (storyId: string) =>
  loggedInvoke<{ count: number; events: NarrativeEvent[] }>('get_narrative_events', {
    story_id: storyId,
  });

export const getNarrativeThreads = (storyId: string) =>
  loggedInvoke<{ count: number; threads: NarrativeThread[] }>('get_narrative_threads', {
    story_id: storyId,
  });

export const getNarrativeChunks = (storyId: string) =>
  loggedInvoke<{ count: number; chunks: NarrativeChunk[] }>('get_narrative_chunks', {
    story_id: storyId,
  });
