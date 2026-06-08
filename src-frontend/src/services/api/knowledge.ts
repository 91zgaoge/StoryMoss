import { loggedInvoke } from './core';
import type { VectorSearchRequest, SimilarityResult } from '@/types/index';
import type {
  StoryGraph,
  Entity,
  Relation,
  RetentionReport,
  ArchiveResult,
  AgentResult,
  StorySummary,
  VectorSearchResult,
} from '@/types/v3';
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
