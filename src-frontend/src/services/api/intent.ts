import { loggedInvoke } from './core';
import type { Intent, IntentParseRequest, IntentExecutionResult } from '@/types/index';
// Intent Engine
export const parseIntent = (req: IntentParseRequest) =>
  loggedInvoke<Intent>('parse_intent', { user_input: req.user_input });

// v0.30.11: LLM 写作意图分类（一次调用产出全部路由决策）。
// 替代前端 isNovelCreationIntent/isContinuationIntent 朴素子串匹配。
// 后端 8s 超时 + 保守兜底（is_new_novel=false）；前端会话缓存避免重复调用。
export interface WritingIntentClassification {
  is_new_novel: boolean;
  is_continuation: boolean;
  task_type: string;
  is_prose_request: boolean;
  input_clarity: string;
  detected_genre?: string;
  confidence: number;
  /** v0.87.0: 对话/调度意图——幕前路由到剧组对话（@导演），不进生成管线。 */
  is_chat?: boolean;
}

export const classifyIntent = (
  userInput: string,
  hasExistingStory: boolean,
  hasCurrentContent: boolean
) =>
  loggedInvoke<WritingIntentClassification>('classify_intent', {
    user_input: userInput,
    has_existing_story: hasExistingStory,
    has_current_content: hasCurrentContent,
  });

export const executeIntent = (intent: Intent, storyId: string) =>
  loggedInvoke<IntentExecutionResult>('execute_intent', { intent, story_id: storyId });
// Smart Execute - Model-driven orchestration
export interface SmartExecuteRequest {
  user_input: string;
  current_content?: string;
  selected_text?: string;
  scene_id?: string;
  style_weight?: number;
  /** v0.30.11: 前端 LLM 分类结果，传后端避免重复调用 */
  intent_classification?: WritingIntentClassification;
}

export interface SmartExecuteResult {
  success: boolean;
  steps_completed: number;
  final_content?: string;
  messages: string[];
  /** v0.31.x: 'audit_report' 时 final_content 为审计报告，渲染为报告消息而非追加手稿。
   *  v0.53.0: 'asset_refresh' 时不追加手稿。
   *  v0.53.5: 有 asset_refresh_draft 时弹确认框，确认前不写库。 */
  result_kind?: string;
  asset_refresh_draft?: AssetRefreshDraft;
}

export interface AssetRefreshDraft {
  story_id: string;
  scene_id?: string;
  overwrite_manual?: boolean;
  instruction: string;
  story_outline?: string;
  scene_outline?: string;
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
    scene_id: req.scene_id,
    intent_classification: req.intent_classification,
  });

export const confirmAssetRefresh = (req: {
  storyId: string;
  sceneId?: string;
  storyOutline?: string;
  sceneOutline?: string;
  overwriteManual?: boolean;
}) =>
  loggedInvoke<string>('confirm_asset_refresh', {
    story_id: req.storyId,
    scene_id: req.sceneId,
    story_outline: req.storyOutline,
    scene_outline: req.sceneOutline,
    overwrite_manual: req.overwriteManual,
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
