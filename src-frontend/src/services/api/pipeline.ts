import { loggedInvoke } from './core';
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
