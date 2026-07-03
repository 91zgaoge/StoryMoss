import { loggedInvoke } from './core';

export interface TraceStep {
  name: string;
  phase: string;
  start_ms: number;
  end_ms?: number;
  duration_ms?: number;
  model_id?: string;
  provider?: string;
  input_tokens?: number;
  output_tokens?: number;
  status?: string;
  error?: string;
  details?: unknown;
}

export interface GenerationTrace {
  trace_id: string;
  request_id?: string;
  story_id?: string;
  user_input?: string;
  created_at: string;
  finished_at?: string;
  steps: TraceStep[];
  status: string;
  error_message?: string;
}

export async function getGenerationTrace(trace_id: string): Promise<GenerationTrace> {
  return loggedInvoke('get_generation_trace', { trace_id });
}

export async function listRecentGenerationTraces(limit?: number): Promise<GenerationTrace[]> {
  return loggedInvoke('list_recent_generation_traces', { limit });
}
