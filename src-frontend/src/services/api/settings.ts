import { loggedInvoke } from './core';
import type { LlmConfig, VectorSearchRequest, SimilarityResult } from '@/types/index';
import type { AppSettings } from '@/types/llm';
// Vector Search (NEW - LanceDB)
export const searchSimilar = (req: VectorSearchRequest) =>
  loggedInvoke<SimilarityResult[]>('search_similar', {
    story_id: req.story_id,
    query: req.query,
    top_k: req.top_k,
  });
// Settings (兼容旧接口，内部映射到 get_settings / save_settings)
export const getConfig = async () => {
  const settings = await loggedInvoke<AppSettings>('get_settings');
  const chatModel =
    settings.models.chat?.find((m: any) => m.id === settings.active_models.chat) ||
    settings.models.chat?.[0];
  if (!chatModel) {
    throw new Error('No chat model configured');
  }
  return {
    provider: chatModel.provider || 'custom',
    api_key: chatModel.api_key || '',
    model: chatModel.model || '',
    temperature: chatModel.temperature ?? 0.8,
    max_tokens: chatModel.max_tokens ?? 4096,
    base_url: chatModel.api_base || '',
  } as LlmConfig;
};

export const updateConfig = async (config: { llm: LlmConfig }) => {
  const settings = await loggedInvoke<AppSettings>('get_settings');
  const chatModel =
    settings.models.chat?.find((m: any) => m.id === settings.active_models.chat) ||
    settings.models.chat?.[0];
  if (chatModel) {
    chatModel.provider = config.llm.provider;
    chatModel.api_key = config.llm.api_key || '';
    chatModel.model = config.llm.model;
    chatModel.temperature = config.llm.temperature;
    chatModel.max_tokens = config.llm.max_tokens;
    chatModel.api_base = config.llm.base_url;
  }
  await loggedInvoke<void>('save_settings', { settings });
};
