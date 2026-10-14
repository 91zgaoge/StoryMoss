import { loggedInvoke } from './core';
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

/** MASTER_SETTING 合同解析后的领域结构（后端 get_runtime_contract 返回的是解析结果，非 DB 行） */
export interface MasterSettingContract {
  schema_version: string;
  contract_type: string;
  generator_version: string;
  genre: string;
  core_tone: string;
  pacing_strategy: string;
  anti_patterns: string[];
  world_rules: string[];
}

export interface ChapterDirective {
  goal: string;
  must_cover_nodes: string[];
  forbidden_zones: string[];
  time_anchor?: string | null;
  chapter_span?: string | null;
}

/** CHAPTER 合同解析后的领域结构 */
export interface ChapterContract {
  schema_version: string;
  contract_type: string;
  generator_version: string;
  chapter_number: number;
  chapter_directive: ChapterDirective;
}

export interface RuntimeContract {
  master_setting: MasterSettingContract;
  chapter_contract: ChapterContract | null;
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

/** P1-3（v0.70.0）：卷纲（VOLUME 合同的结构化形态）。 */
export interface VolumePlan {
  schema_version: string;
  contract_type: string;
  generator_version: string;
  volume_number: number;
  start_chapter: number;
  end_chapter: number;
  goal: string;
  climax: string;
  foreshadowing_due: string[];
  new_thread_budget: number;
  scale_estimate: string;
}

/** 卷纲总览：状态 no_plan / in_volume / past_end + 下一个建议卷。 */
export interface VolumeOverview {
  plans: VolumePlan[];
  latest_chapter: number;
  state: string;
  current_volume_number: number | null;
  current_volume_remaining: number | null;
  next_volume_number: number | null;
  next_start_chapter: number | null;
}

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
