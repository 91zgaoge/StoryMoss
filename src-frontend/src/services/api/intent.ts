import { loggedInvoke } from './core';
import type { Intent, IntentParseRequest, IntentExecutionResult } from '@/types/index';
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
  /** 最后发给模型的完整提示词，用于 RLHF / 诊断 */
  original_prompt?: string;
  /** AI 生成的原始文本 */
  generated_content?: string;
  /** 用户在 AI 生成基础上的后续编辑差异（modify 场景） */
  subsequent_edit_diff?: string;
}

export interface LearningPoint {
  category: string;
  observation: string;
  impact: string;
}

export const recordFeedback = (req: RecordFeedbackRequest) =>
  loggedInvoke<LearningPoint[]>('record_feedback', { request: req });
