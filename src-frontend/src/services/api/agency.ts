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

export interface EvalOverview {
  gate_history: GateHistoryItem[];
  pass_rate: number;
  checkpoints: AgencyCheckpoint[];
  human_signals: HumanSignal[];
  token_usage: PurposeUsage[];
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
