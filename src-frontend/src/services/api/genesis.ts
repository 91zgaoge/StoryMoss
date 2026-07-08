import { loggedInvoke } from './core';
import type {
  StoryOutline,
  CharacterRelationship,
  SelectedStrategy,
  StrategySelectionRequest,
} from '@/types/index';

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
  typical_structure_json?: string;
}) => loggedInvoke<import('@/types/api').GenreProfile>('save_genre_profile', params);

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

export interface GenesisRun {
  id: string;
  story_id?: string;
  session_id: string;
  /** 关联的生成链路 trace_id（后端可选返回） */
  trace_id?: string;
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

export const selectCreationStrategy = (req: StrategySelectionRequest) =>
  loggedInvoke<SelectedStrategy>('select_creation_strategy', { ...req });
