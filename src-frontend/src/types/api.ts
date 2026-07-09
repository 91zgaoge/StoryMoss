// Auto-generated from services/tauri.ts
// These types were originally inline in tauri.ts and have been migrated here.

export interface CharacterQuickView {
  id: string;
  name: string;
  appearance_summary: string;
  status_tags: string[];
  last_seen_chapter: number;
}

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

export interface AutoCreateContractsResult {
  created_master_setting: boolean;
  created_chapter_contract: boolean;
  created_outline: boolean;
  message: string;
}

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

export interface SubscriptionStatus {
  user_id: string;
  tier: string;
  status: string;
  expires_at?: string;
}

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
  kg_entity_id?: string | null;
}

export interface UnifiedMemoryFact {
  id: string;
  story_id: string;
  record_kind: 'kg_entity' | 'memory_item' | string;
  category: string;
  subject: string | null;
  field: string | null;
  value: string;
  source_chapter: number | null;
  confidence: number;
  status: string;
  updated_at: string;
  kg_entity_id: string | null;
  memory_item_id: string | null;
}

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

export interface GenreProfile {
  id: number;
  genre_name: string;
  canonical_name: string;
  aliases: string[];
  core_tone: string;
  pacing_strategy: string;
  anti_patterns: string[];
  reference_tables: string[];
  typical_structure: { title: string; description: string }[];
  typical_structure_json?: string;
  is_builtin: boolean;
  created_at: string;
}

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
