import { loggedInvoke } from './core';
// ==================== LLM Stream ====================

export const llmGenerateStream = (params: {
  request_id: string;
  prompt: string;
  context?: string;
  max_tokens?: number;
  temperature?: number;
}) => loggedInvoke<void>('llm_generate_stream', { request: params });

export const llmCancelGeneration = (requestId: string) =>
  loggedInvoke<void>('llm_cancel_generation', { request_id: requestId });
// Input hint — LLM智能输入建议
export const getInputHint = (currentContent?: string) =>
  loggedInvoke<string>('get_input_hint', { current_content: currentContent });

// v0.30.24: Logline 幽灵提示--用户输入简单创世指令时生成增强版 logline
export const generateLoglineHint = (userInput: string) =>
  loggedInvoke<string | null>('generate_logline_hint', { user_input: userInput });
