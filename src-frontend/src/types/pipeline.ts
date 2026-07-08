// Pipeline 管线体系类型定义 (v7.0.0)

// ==================== Blueprint 蓝图 ====================

export interface Blueprint {
  id: string;
  story_id: string;
  chapter_number: number;
  title?: string;
  role?: string;
  purpose?: string;
  key_events?: string; // JSON string of string[]
  characters?: string; // JSON string of string[]
  suspense_hook?: string;
  user_guidance?: string;
  notes?: string;
  notes_updated_at?: string;
  knowledge_query_hint?: string;
  created_at: string;
  updated_at: string;
}

export interface CreateBlueprintRequest {
  story_id: string;
  chapter_number: number;
  title?: string;
  role?: string;
  purpose?: string;
  key_events?: string[];
  characters?: string[];
  suspense_hook?: string;
  user_guidance?: string;
  knowledge_query_hint?: string;
}

export interface UpdateBlueprintRequest {
  title?: string;
  role?: string;
  purpose?: string;
  key_events?: string[];
  characters?: string[];
  suspense_hook?: string;
  user_guidance?: string;
  notes?: string;
  knowledge_query_hint?: string;
}

// ==================== Draft 草稿 ====================

export type DraftStatus = 'draft' | 'refined' | 'reviewed' | 'finalized' | 'archived';
export type DraftSource = 'write' | 'rewrite' | 'refine' | 'review_fix';

export interface Draft {
  id: string;
  story_id: string;
  chapter_number: number;
  version: number;
  status: DraftStatus;
  source: DraftSource;
  content: string;
  word_count: number;
  model_used?: string;
  cost?: number;
  metadata?: string;
  created_at: string;
  updated_at: string;
}

// ==================== Revision 修稿 ====================

export type RevisionType = 'refine' | 'review_fix' | 'user_edit';
export type RevisionStatus = 'pending' | 'merged' | 'discarded' | 'superseded';

export interface Revision {
  id: string;
  story_id: string;
  draft_id: string;
  revision_index: number;
  revision_type: RevisionType;
  status: RevisionStatus;
  user_prompt?: string;
  original_content: string;
  revised_content: string;
  word_count: number;
  change_summary?: string;
  model_used?: string;
  cost?: number;
  metadata?: string;
  created_at: string;
  updated_at: string;
}

// ==================== Pipeline Review 审稿报告 ====================

export interface ReviewDimension {
  name: string;
  score: number;
  comment: string;
}

export interface ReviewIssueItem {
  severity: string;
  dimension: string;
  description: string;
  suggestion: string;
}

export interface PipelineReview {
  id: string;
  story_id: string;
  draft_id: string;
  review_index: number;
  content: string;
  dimensions?: string; // JSON string of ReviewDimension[]
  issues?: string; // JSON string of ReviewIssueItem[]
  overall_score?: number;
  review_focus?: string;
  model_used?: string;
  cost?: number;
  metadata?: string;
  created_at: string;
}

// ==================== Post Process 后处理 ====================

export type PostProcessStatus = 'running' | 'completed' | 'failed' | 'partial';
export type StepStatus = 'pending' | 'running' | 'success' | 'failed' | 'skipped';

export interface PostProcessRun {
  id: string;
  story_id: string;
  chapter_number: number;
  source_label: string;
  scope?: string;
  status: PostProcessStatus;
  started_at: string;
  completed_at?: string;
  error_message?: string;
}

export interface PostProcessStep {
  id: string;
  run_id: string;
  step_key: string;
  step_label: string;
  status: StepStatus;
  critical: boolean;
  log_output?: string;
  error_message?: string;
  started_at?: string;
  completed_at?: string;
}

// ==================== LLM Call 用量统计 ====================

export interface LlmCall {
  id: string;
  story_id?: string;
  draft_id?: string;
  revision_id?: string;
  model_id: string;
  model_name?: string;
  purpose: string;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  duration_ms: number;
  success: boolean;
  error_message?: string;
  prompt_preview?: string;
  metadata?: string;
  task_type?: string;
  created_at: string;
}

export interface RecordLlmCallRequest {
  story_id?: string;
  draft_id?: string;
  model_id: string;
  model_name?: string;
  purpose: string;
  prompt_tokens: number;
  completion_tokens: number;
  duration_ms: number;
  success: boolean;
  error_message?: string;
}

// ==================== Pipeline High-Level Results 管线高层结果 ====================

export interface RefineResult {
  revision_id: string;
  original_content: string;
  refined_content: string;
  word_count: number;
  change_summary?: string;
}

export interface ReviewDimensionResult {
  name: string;
  score: number;
  comment: string;
}

export interface ReviewIssueResult {
  severity: string;
  dimension: string;
  description: string;
  suggestion: string;
}

export interface ReviewResult {
  review_id: string;
  overall_score: number;
  dimensions: ReviewDimensionResult[];
  issues: ReviewIssueResult[];
  summary: string;
}

export interface PipelineResult {
  draft_id: string;
  chapter_number: number;
  refined_draft_id?: string;
  review_id?: string;
  finalized_draft_id?: string;
  post_process_run_id?: string;
  success: boolean;
  message: string;
}

// ==================== Character State 角色动态状态 ====================

export interface CharacterState {
  location?: string;
  power_level?: string;
  physical_state?: string;
  mental_state?: string;
  key_items?: string;
  recent_events?: string;
  updated_at_chapter?: number;
}
