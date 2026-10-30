import { loggedInvoke } from './core';

export interface GateHistoryItem {
  key: string;
  outcome: string;
  weighted: number | null;
  code: number | null;
  rule: number | null;
  model: number | null;
  created_at: string;
}

export interface PurposeUsage {
  purpose: string;
  calls: number;
  total_tokens: number;
  total_duration_ms: number;
}

export interface AgencyCheckpoint {
  id: string;
  run_id: string;
  story_id: string;
  milestone: string;
  chapter_number: number | null;
  metrics_json: string;
  created_at: string;
}

export interface HumanSignal {
  scene_id: string;
  chapter_number: number;
  delivered_chars: number;
  current_chars: number;
  modification_ratio: number;
  evaluated_at: string;
}

export interface StoryTokens {
  total_tokens: number;
  run_count: number;
}

/** P1-4（v0.68.0）：每章判官一致率（以该章最新一条 gate 记录为准）。 */
export interface ChapterAgreement {
  chapter_number: number;
  /** 该章 blocking 问题总条数。 */
  findings: number;
  /** 可判定条数（引用 ≥6 字且有成稿可比）。 */
  judgeable: number;
  /** 被采纳条数（引用未在成稿中留存）。 */
  adopted: number;
  /** 采纳率 = adopted / judgeable。 */
  adoption_rate: number;
  /** 作者修改率（human_signals 口径；无配对为 null）。 */
  modification_ratio: number | null;
}

export interface EditorAgreementReport {
  chapters: ChapterAgreement[];
  total_findings: number;
  total_judgeable: number;
  total_adopted: number;
  adoption_rate: number;
  /** 命中密度 = total_findings / (成稿总字数 / 1000)。 */
  per_kilo_chars: number;
}

/** P1-5（v0.69.0）：单条手法的作者裁决统计（accept/reject 按时间窗归因）。 */
export interface TechniqueStat {
  technique_id: string;
  label: string;
  accepts: number;
  rejects: number;
}

export interface EvalOverview {
  gate_history: GateHistoryItem[];
  pass_rate: number;
  checkpoints: AgencyCheckpoint[];
  human_signals: HumanSignal[];
  editor_agreement: EditorAgreementReport;
  token_usage: PurposeUsage[];
  story_tokens: StoryTokens;
}

export interface CheckpointDiff {
  words_delta: number;
  chapters_delta: number;
  tokens_delta: number;
  gate_weighted_delta: number;
}

export function getEvalOverview(storyId: string) {
  return loggedInvoke<EvalOverview>('agency_eval_overview', { story_id: storyId });
}

export function listCheckpoints(storyId: string) {
  return loggedInvoke<AgencyCheckpoint[]>('agency_list_checkpoints', { story_id: storyId });
}

export function compareCheckpoints(checkpointA: string, checkpointB: string) {
  return loggedInvoke<CheckpointDiff>('agency_compare_checkpoints', {
    checkpoint_a: checkpointA,
    checkpoint_b: checkpointB,
  });
}

export function getHumanSignals(storyId: string) {
  return loggedInvoke<HumanSignal[]>('agency_human_signals', { story_id: storyId });
}

export interface Instinct {
  id: string;
  trigger: string;
  action: string;
  confidence: number;
  evidence_count: number;
  scope: string;
  status: string;
  created_at: string;
  updated_at: string;
  evolved_from: string[];
}

export interface Observation {
  ts: string;
  story_id: string;
  kind: string;
  actor: string;
  payload: Record<string, unknown>;
}

export interface LearningOverview {
  instincts: Instinct[];
  candidates: Instinct[];
  recent_observations: Observation[];
  unanalyzed_count: number;
  analyze_min_new: number;
  technique_stats: TechniqueStat[];
}

export interface AnalyzeOutcome {
  new_instincts: number;
  updated_instincts: number;
  analyzed: number;
}

export interface PromoteOutcome {
  instinct: Instinct;
  skill_id: string;
}

export function getLearningOverview(storyId: string) {
  return loggedInvoke<LearningOverview>('agency_learning_overview', { story_id: storyId });
}

/** P1-5：重置手法统计（清空归因基底，后续重新积累）。返回删除行数。 */
export function resetTechniqueStats(storyId: string) {
  return loggedInvoke<number>('agency_reset_technique_stats', { story_id: storyId });
}

export function analyzeLearning(storyId: string) {
  return loggedInvoke<AnalyzeOutcome>('agency_analyze_learning', { story_id: storyId });
}

export function confirmPromotion(storyId: string, instinctId: string) {
  return loggedInvoke<PromoteOutcome>('agency_confirm_promotion', {
    story_id: storyId,
    instinct_id: instinctId,
  });
}

export function rejectPromotion(storyId: string, instinctId: string) {
  return loggedInvoke<Instinct>('agency_reject_promotion', {
    story_id: storyId,
    instinct_id: instinctId,
  });
}

export function instinctFeedback(storyId: string, instinctId: string, accepted: boolean) {
  return loggedInvoke<Instinct>('agency_instinct_feedback', {
    story_id: storyId,
    instinct_id: instinctId,
    accepted,
  });
}

export interface BoardItem {
  id: string;
  run_id: string;
  story_id: string;
  zone: 'asset' | 'draft' | 'review' | 'schedule' | 'direction';
  item_type: string;
  key: string;
  content: string;
  summary: string;
  version: number;
  producer: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface AgencyRun {
  id: string;
  story_id: string | null;
  premise: string;
  status: string;
  phase: string;
  /** V149：顶层 run 的并发车道 write | review | inspect | chat */
  lane?: string;
  result_json: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export function getRun(runId: string) {
  return loggedInvoke<AgencyRun | null>('agency_get_run', { run_id: runId });
}

export function listRuns(storyId: string) {
  return loggedInvoke<AgencyRun[]>('agency_list_runs', { story_id: storyId });
}

export function listBoard(runId: string) {
  return loggedInvoke<BoardItem[]>('agency_list_board', { run_id: runId });
}

/** 代理活动日志条目（activity + progress 事件持久化到 DB） */
export interface AgencyActivity {
  id: number;
  run_id: string;
  event_type: 'activity' | 'progress';
  role: string | null;
  action: string | null;
  detail: string | null;
  phase: string | null;
  status: string | null;
  message: string | null;
  created_at: string;
}

export function listActivities(runId: string) {
  return loggedInvoke<AgencyActivity[]>('agency_list_activities', { run_id: runId });
}

// ---- v0.74.0 导演 / 剧组对话 / 任务进程 ----

export interface ChatSession {
  id: string;
  story_id: string;
  title: string;
  target_role: string;
  summary?: string | null;
  summary_upto_message_id?: string | null;
  archived: boolean;
  created_at: string;
  updated_at: string;
}

export interface ChatMessage {
  id: string;
  session_id: string;
  role: string;
  content: string;
  status: string;
  run_key?: string | null;
  task_id?: string | null;
  task_ids_json?: string | null;
  actions_json?: string | null;
  meta_json?: string | null;
  error?: string | null;
  created_at: string;
}

export interface ChatSendResult {
  session_id: string;
  message_id: string;
  turn_task_id: string;
}

export interface AgencyAgentTask {
  id: string;
  run_id: string;
  story_id: string;
  role: string;
  parent_task_id?: string | null;
  task: string;
  status: string;
  phase?: string | null;
  turn_count: number;
  tool_calls: number;
  last_tool?: string | null;
  heartbeat_at?: string | null;
  started_at?: string | null;
  finished_at?: string | null;
  error?: string | null;
  result_summary?: string | null;
  meta_json: string;
  created_at: string;
}

export interface ResourceLease {
  id: string;
  story_id: string;
  resource: string;
  owner_run_id?: string | null;
  owner_task_id?: string | null;
  acquired_at: string;
  heartbeat_at: string;
  expires_at: string;
}

export interface CrewAlert {
  severity: string;
  kind: string;
  message: string;
  role?: string | null;
  task_id?: string | null;
  run_id?: string | null;
}

export interface CrewSnapshot {
  story_id: string;
  story_title: string;
  runs: AgencyRun[];
  tasks: AgencyAgentTask[];
  leases: ResourceLease[];
  corpus_count: number;
  alerts: CrewAlert[];
}

export function listChatSessions(storyId: string) {
  return loggedInvoke<ChatSession[]>('agency_chat_sessions', { story_id: storyId });
}

export function listChatMessages(sessionId: string, limit = 200) {
  return loggedInvoke<ChatMessage[]>('agency_chat_messages', { session_id: sessionId, limit });
}

export function newChatSession(storyId: string, title?: string) {
  return loggedInvoke<ChatSession>('agency_chat_new_session', { story_id: storyId, title });
}

export function deleteChatSession(sessionId: string) {
  return loggedInvoke<void>('agency_chat_delete_session', { session_id: sessionId });
}

export function sendChat(params: {
  story_id: string;
  session_id?: string | null;
  message: string;
  target_role?: string | null;
  corpus_ids?: string[];
}) {
  return loggedInvoke<ChatSendResult>('agency_chat_send', {
    story_id: params.story_id,
    session_id: params.session_id ?? null,
    message: params.message,
    target_role: params.target_role ?? 'director',
    corpus_ids: params.corpus_ids ?? null,
  });
}

export function crewStatus(storyId: string) {
  return loggedInvoke<CrewSnapshot>('agency_crew_status', { story_id: storyId });
}

export function crewDispatch(params: {
  story_id: string;
  role: string;
  instruction: string;
  premise?: string;
  run_key?: string;
  session_id?: string;
  corpus_ids?: string[];
}) {
  return loggedInvoke<string>('agency_crew_dispatch', {
    story_id: params.story_id,
    role: params.role,
    instruction: params.instruction,
    premise: params.premise ?? null,
    run_key: params.run_key ?? null,
    session_id: params.session_id ?? null,
    corpus_ids: params.corpus_ids ?? null,
  });
}

export function crewCancelTask(taskId: string) {
  return loggedInvoke<void>('agency_crew_cancel_task', { task_id: taskId });
}

export function crewRetryTask(taskId: string) {
  return loggedInvoke<string>('agency_crew_retry_task', { task_id: taskId });
}

export function releaseLease(storyId: string, resource: string) {
  return loggedInvoke<void>('agency_crew_release_lease', { story_id: storyId, resource });
}

export function directorExecute(params: {
  story_id: string;
  session_id: string;
  message_id: string;
  premise?: string;
  action_ids: string[];
}) {
  return loggedInvoke<string[]>('agency_director_execute', {
    story_id: params.story_id,
    session_id: params.session_id,
    message_id: params.message_id,
    premise: params.premise ?? null,
    action_ids: params.action_ids,
  });
}

export function directorReject(actionIds: string[], messageId?: string) {
  return loggedInvoke<void>('agency_director_reject', {
    action_ids: actionIds,
    message_id: messageId ?? null,
  });
}

export function directorPatrol(storyId: string) {
  return loggedInvoke<CrewSnapshot>('agency_director_patrol', { story_id: storyId });
}
