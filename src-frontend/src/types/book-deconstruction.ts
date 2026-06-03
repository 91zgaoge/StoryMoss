// Book Deconstruction Types

export interface ReferenceBook {
  id: string;
  title: string;
  author?: string;
  genre?: string;
  word_count?: number;
  file_format?: string;
  analysis_status: 'pending' | 'extracting' | 'analyzing' | 'completed' | 'failed' | 'cancelled';
  analysis_progress: number;
  analysis_error?: string;
  world_setting?: string;
  plot_summary?: string;
  story_arc?: string;
  // LitSeg: 分析后的叙事结构（起承转合幕级划分）
  analyzed_structure_json?: string;
  created_at: string;
  updated_at: string;
}

export interface ReferenceBookSummary {
  id: string;
  title: string;
  author?: string;
  genre?: string;
  word_count?: number;
  file_format?: string;
  analysis_status: string;
  analysis_progress: number;
  created_at: string;
}

export interface ReferenceCharacter {
  id: string;
  book_id: string;
  name: string;
  role_type?: string;
  personality?: string;
  appearance?: string;
  relationships?: string;
  importance_score?: number;
}

export interface ReferenceScene {
  id: string;
  book_id: string;
  sequence_number: number;
  title?: string;
  summary?: string;
  characters_present?: string;
  key_events?: string;
  conflict_type?: string;
  emotional_tone?: string;
  // LitSeg: 叙事分析字段
  narrative_intensity?: number;
  narrative_sentiment?: number;
  narrative_event_types?: string;
  act_number?: number;
  position_in_act?: number;
}

export interface BookAnalysisResult {
  book: ReferenceBook;
  characters: ReferenceCharacter[];
  scenes: ReferenceScene[];
}

export interface AnalysisStatusResponse {
  book_id: string;
  status: string;
  progress: number;
  current_step?: string;
  error?: string;
  task_id?: string;
  active_threads?: number;
  max_threads?: number;
}

export interface BookAnalysisProgressEvent {
  book_id: string;
  status: string;
  progress: number;
  current_step: string;
  message?: string;
  active_threads?: number;
  total_chunks?: number;
  processed_chunks?: number;
}
