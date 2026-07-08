// CINEMA-AI Frontend Types
export * from './llm';
export * from './v3';
export * from './pipeline';

export interface User {
  id: string;
  name: string;
  email?: string;
  avatar?: string;
}

export interface Story {
  id: string;
  title: string;
  description?: string;
  genre?: string;
  tone?: string;
  pacing?: string;
  style_dna_id?: string;
  genre_profile_id?: string;
  methodology_id?: string;
  methodology_step?: number;
  character_count?: number;
  chapter_count?: number;
  word_count?: number;
  created_at: string;
  updated_at: string;
}

export interface StyleDNA {
  id: string;
  name: string;
  author?: string;
  is_builtin: boolean;
  is_user_created: boolean;
}

// v4.4.0 - 3风格三角框架
export type BlendRole = 'dominant' | 'secondary' | 'tertiary';

export interface BlendComponent {
  dna_id: string;
  dna_name: string;
  weight: number;
  role: BlendRole;
}

export interface StyleBlendConfig {
  name: string;
  components: BlendComponent[];
  drift_check_enabled: boolean;
}

export interface DriftCheckItem {
  dimension: string;
  target_min: number;
  target_max: number;
  actual_value: number;
  score: number;
  passed: boolean;
  suggestion: string;
}

export interface DriftCheckResult {
  passed: boolean;
  overall_score: number;
  checks: DriftCheckItem[];
}

export interface Character {
  id: string;
  story_id: string;
  name: string;
  background?: string;
  personality?: string;
  goals?: string;
  appearance?: string;
  gender?: string;
  age?: number;
  // v7.0.0: 动态状态字段
  cs_location?: string;
  cs_power_level?: string;
  cs_physical_state?: string;
  cs_mental_state?: string;
  cs_key_items?: string;
  cs_recent_events?: string;
  cs_updated_at_chapter?: number;
  cs_json?: string;
  // v7.0.0: 溯源字段
  source?: string;
  is_auto_generated?: boolean;
  created_at: string;
  updated_at: string;
}

export interface Chapter {
  id: string;
  story_id: string;
  title: string;
  outline?: string;
  content?: string;
  chapter_number: number;
  status: 'draft' | 'outline' | 'completed';
  word_count?: number;
  scene_id?: string;
  created_at: string;
  updated_at: string;
}

export type SkillCategory =
  | 'writing'
  | 'analysis'
  | 'character'
  | 'world_building'
  | 'style'
  | 'plot'
  | 'export'
  | 'integration'
  | 'custom';

export interface SkillParameter {
  name: string;
  description: string;
  param_type: string;
  required: boolean;
  default?: unknown;
}

export interface HookDefinition {
  event: string;
  handler: string;
  priority: number;
}

export interface Skill {
  id: string;
  name: string;
  description: string;
  category: SkillCategory;
  version: string;
  author: string;
  entry_point: string;
  parameters: SkillParameter[];
  capabilities: string[];
  hooks: HookDefinition[];
  config: Record<string, unknown>;
  path: string;
  is_enabled: boolean;
  loaded_at: string;
  runtime_type: string;
}

export interface McpServer {
  id: string;
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  enabled: boolean;
  tools?: McpTool[];
}

export interface McpTool {
  name: string;
  description?: string;
  input_schema?: Record<string, unknown>;
}

export interface LlmConfig {
  provider: 'openai' | 'anthropic' | 'ollama';
  api_key?: string;
  model: string;
  temperature: number;
  max_tokens: number;
  base_url?: string;
}

export interface WizardCreationResult {
  story: Story;
  world_building: import('./v3').WorldBuilding;
  writing_style: import('./v3').WritingStyle;
  first_scene: import('./v3').Scene;
  characters: Character[];
  ingested_entities: number;
  ingested_relations: number;
}

export interface AppSettings {
  llm: LlmConfig;
  theme: 'dark' | 'light' | 'system';
  language: string;
  auto_save: boolean;
}

export interface DashboardState {
  current_story?: Story;
  stories_count: number;
  characters_count: number;
  chapters_count: number;
}

export interface CreateStoryRequest {
  title: string;
  description?: string;
  genre?: string;
  style_dna_id?: string;
  genre_profile_id?: string;
  methodology_id?: string;
}

export interface SelectedStrategy {
  rationale: string;
  genre_profile_id?: string;
  methodology_id?: string;
  style_dna_ids: string[];
  skill_ids: string[];
  workflow_id?: string;
  parameters: Record<string, unknown>;
}

export interface StrategySelectionRequest {
  user_input: string;
  genre_hint?: string;
  methodology_hint?: string;
  word_count_target?: number;
}

export interface CreateCharacterRequest {
  story_id: string;
  name: string;
  background?: string;
  personality?: string;
  goals?: string;
  appearance?: string;
  gender?: string;
  age?: number;
}

export interface UpdateChapterRequest {
  title?: string;
  outline?: string;
  content?: string;
}

export interface SimilarityResult {
  id: string;
  text: string;
  score: number;
  chapter_id: string;
  chapter_number: number;
}

export interface VectorSearchRequest {
  story_id: string;
  query: string;
  top_k?: number;
}

export interface StoryOutline {
  id: string;
  story_id: string;
  content: string;
  structure_json?: string;
  act_count: number;
  total_scenes_estimate?: number;
  created_at: string;
  updated_at: string;
}

export interface CharacterRelationship {
  id: string;
  story_id: string;
  source_character_id: string;
  target_character_id: string;
  target_character_name?: string;
  relationship_type: string;
  description?: string;
  dynamic?: string;
  created_at: string;
}

export type ViewType =
  | 'dashboard'
  | 'stories'
  | 'characters'
  | 'world_building'
  | 'scenes'
  | 'knowledge-graph'
  | 'skills'
  | 'mcp'
  | 'book-deconstruction'
  | 'tasks'
  | 'foreshadowing'
  | 'narrative-analysis'
  | 'creation-wizard'
  | 'story-system'
  | 'usage-stats'
  | 'writing-stats'
  | 'intention-graph'
  | 'logs'
  | 'tracing'
  | 'settings';

// ===== Intent Engine Types =====

export type IntentType =
  | 'text_generate'
  | 'text_rewrite'
  | 'plot_suggest'
  | 'character_check'
  | 'world_consistency'
  | 'style_shift'
  | 'memory_ingest'
  | 'visual_generate'
  | 'scene_reorder'
  | 'outline_expand'
  | 'unknown';

export type ExecutionMode = 'serial' | 'parallel';

export type FeedbackType =
  | 'direct_apply'
  | 'suggestion_card'
  | 'diff_preview'
  | 'system_notice'
  | 'visual_highlight';

export interface IntentTarget {
  target_type?: string | null;
  id?: string | null;
  name?: string | null;
}

export interface Intent {
  intent_type: IntentType;
  target: IntentTarget;
  constraints: string[];
  required_agents: string[];
  execution_mode: ExecutionMode;
  feedback_type: FeedbackType;
}

export interface IntentParseRequest {
  user_input: string;
}

export interface AgentStepResult {
  agent_name: string;
  success: boolean;
  result?: {
    content: string;
    score?: number;
    suggestions: string[];
  };
  error?: string;
}

export interface IntentExecutionResult {
  intent_type: IntentType;
  feedback_type: FeedbackType;
  execution_mode: ExecutionMode;
  steps: AgentStepResult[];
  summary: string;
}
