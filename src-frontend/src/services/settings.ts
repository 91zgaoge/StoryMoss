/**
 * Settings Service
 *
 * 与后端通信管理应用设置
 */

import { loggedInvoke } from '@/services/tauri';
import { createLogger } from '@/utils/logger';
import type {
  AppSettings,
  ModelConfig,
  AgentModelMapping,
  SettingsExport,
  ConnectionTestResult,
  ConnectionTestStep,
} from '@/types/llm';

const settingsServiceLogger = createLogger('services:settings');

// 浏览器开发环境 fallback：三个真实本地模型
const BROWSER_FALLBACK_MODELS: ModelConfig[] = [
  {
    id: 'Qwen3.5-27B-Uncensored-Q4_K_M',
    name: 'Qwen 3.5 语言模型',
    description: '本地语言模型，用于文本生成和对话',
    provider: 'custom',
    model_source: 'local',
    model: 'Qwen3.5-27B-Uncensored-Q4_K_M',
    api_key: '',
    api_base: 'http://10.62.239.13:17098/v1',
    timeout_seconds: 120,
    is_default: true,
    enabled: true,
    type: 'chat',
    temperature: 0.8,
    max_tokens: 8192,
    capabilities: ['chat', 'completion', 'long_context'],
  },
  {
    id: 'Gemma-4-31B-it-Q6_K',
    name: 'Gemma 4 多模态',
    description: '本地多模态模型，支持图文理解',
    provider: 'custom',
    model_source: 'local',
    model: 'Gemma-4-31B-it-Q6_K',
    api_key: '',
    api_base: 'http://10.62.239.13:17099/v1',
    timeout_seconds: 120,
    is_default: false,
    enabled: true,
    type: 'multimodal',
    temperature: 0.7,
    max_tokens: 8192,
    supports_vision: true,
    supports_audio: false,
    capabilities: ['chat', 'vision', 'long_context'],
  },
  {
    id: 'bge-m3',
    name: 'BGE-M3 Embedding',
    description: '文本嵌入模型，用于语义搜索和向量化',
    provider: 'custom',
    model_source: 'local',
    model: 'bge-m3',
    api_key: '',
    api_base: 'http://10.62.239.13:8089',
    timeout_seconds: 120,
    is_default: true,
    enabled: true,
    type: 'embedding',
    dimensions: 1024,
    max_input_tokens: 8192,
  },
];

// 浏览器开发环境 fallback 设置
const BROWSER_FALLBACK_SETTINGS: AppSettings = {
  version: '0.1.0',
  updated_at: new Date().toISOString(),
  models: {
    chat: BROWSER_FALLBACK_MODELS.filter(m => m.type === 'chat') as any,
    embedding: BROWSER_FALLBACK_MODELS.filter(m => m.type === 'embedding') as any,
    multimodal: BROWSER_FALLBACK_MODELS.filter(m => m.type === 'multimodal') as any,
    image: [],
  },
  active_models: {
    chat: 'Qwen3.5-27B-Uncensored-Q4_K_M',
    embedding: 'bge-m3',
    multimodal: 'Gemma-4-31B-it-Q6_K',
  },
  agent_mappings: [],
  general: {
    theme: 'dark',
    language: 'zh-CN',
    auto_save: true,
    auto_save_interval: 30,
    font_size: 16,
    line_height: 1.6,
  },
  privacy: {
    share_usage_data: false,
    store_api_keys_securely: true,
  },
  book_deconstruction_concurrency: 3,
  rewrite_threshold: 0.75,
  max_feedback_loops: 2,
  writing_strategy: {
    run_mode: 'fast',
    conflict_level: 50,
    pace: 'balanced',
    ai_freedom: 'medium',
  },
};

export async function getSettings(): Promise<AppSettings> {
  try {
    return await loggedInvoke<AppSettings>('get_settings');
  } catch (e) {
    const isTauri = !!(window as any).__TAURI__;
    if (!isTauri) {
      return BROWSER_FALLBACK_SETTINGS;
    }
    throw e;
  }
}

// 保存设置
export async function saveSettings(settings: Partial<AppSettings>): Promise<void> {
  await loggedInvoke<void>('save_settings', { settings });
}

// 导出设置
export async function exportSettings(): Promise<SettingsExport> {
  return loggedInvoke<SettingsExport>('export_settings');
}

// 导入设置
export async function importSettings(data: SettingsExport): Promise<void> {
  await loggedInvoke<void>('import_settings', { data });
}

// 获取所有模型配置
export async function getModels(): Promise<ModelConfig[]> {
  try {
    return await loggedInvoke<ModelConfig[]>('get_models');
  } catch (e) {
    const isTauri = !!(window as any).__TAURI__;
    if (!isTauri) {
      settingsServiceLogger.debug('[Browser Fallback] Using local real models');
      return BROWSER_FALLBACK_MODELS;
    }
    throw e;
  }
}

// 创建模型配置
export async function createModel(config: Omit<ModelConfig, 'id'>): Promise<ModelConfig> {
  return loggedInvoke<ModelConfig>('create_model', { config });
}

// 更新模型配置
export async function updateModel(id: string, config: Partial<ModelConfig>): Promise<void> {
  await loggedInvoke<void>('update_model', { id, config });
}

// 删除模型配置
export async function deleteModel(id: string): Promise<void> {
  await loggedInvoke<void>('delete_model', { id });
}

// 设置激活的模型
export async function setActiveModel(type: ModelConfig['type'], modelId: string): Promise<void> {
  await loggedInvoke<void>('set_active_model', { modelType: type, modelId });
}

// 获取Agent模型映射
export async function getAgentMappings(): Promise<AgentModelMapping[]> {
  return loggedInvoke<AgentModelMapping[]>('get_agent_mappings');
}

// 更新Agent模型映射
export async function updateAgentMapping(mapping: AgentModelMapping): Promise<void> {
  await loggedInvoke<void>('update_agent_mapping', { mapping });
}

// 浏览器环境下简单的连接探测
async function browserTestModelConnection(modelId: string): Promise<ConnectionTestResult> {
  const steps: ConnectionTestStep[] = [];

  const model = BROWSER_FALLBACK_MODELS.find(m => m.id === modelId);
  if (!model) {
    return {
      success: false,
      latency: 0,
      error: '未知模型',
      steps: [{ name: '查找模型配置', status: 'failed', detail: '未知模型' }],
    };
  }
  if (!model.api_base) {
    return {
      success: false,
      latency: 0,
      error: '未配置 API Base',
      steps: [{ name: '检查 API 地址', status: 'failed', detail: '未配置 API Base' }],
    };
  }

  const start = performance.now();
  steps.push({ name: '探测模型端点', status: 'running' });

  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 5000);
    // 先尝试 GET /models
    const resp = await fetch(`${model.api_base}/models`, {
      method: 'GET',
      signal: controller.signal,
    });
    clearTimeout(timeout);
    if (resp.ok) {
      const latency = Math.round(performance.now() - start);
      steps[0] = { name: '探测模型端点', status: 'success' };
      return { success: true, latency, steps };
    }
    // /models 404 时尝试 POST /chat/completions 轻量探测
    steps[0] = { name: '探测模型列表', status: 'success' };
    steps.push({ name: '测试 API 响应', status: 'running' });

    const postStart = performance.now();
    const postController = new AbortController();
    const postTimeout = setTimeout(() => postController.abort(), 5000);
    await fetch(`${model.api_base}/chat/completions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        model: 'test',
        messages: [{ role: 'user', content: 'hi' }],
        max_tokens: 1,
      }),
      signal: postController.signal,
    });
    clearTimeout(postTimeout);

    const latency = Math.round(performance.now() - postStart);
    steps[1] = { name: '测试 API 响应', status: 'success' };
    return { success: true, latency, steps };
  } catch (e: any) {
    const latency = Math.round(performance.now() - start);
    const lastRunningIdx = steps.findIndex(s => s.status === 'running');
    if (lastRunningIdx >= 0) {
      const errMsg = e.name === 'AbortError' ? '连接超时' : e.message || '连接失败';
      steps[lastRunningIdx] = {
        ...steps[lastRunningIdx],
        status: 'failed',
        detail: errMsg,
      };
    }
    return {
      success: false,
      latency,
      error: e.name === 'AbortError' ? '连接超时' : e.message || '连接失败',
      steps,
    };
  }
}

// 测试模型连接
export async function testModelConnection(modelId: string): Promise<ConnectionTestResult> {
  try {
    return await loggedInvoke<ConnectionTestResult>('test_model_connection', { modelId });
  } catch (e) {
    const isTauri = !!(window as any).__TAURI__;
    if (!isTauri) {
      return browserTestModelConnection(modelId);
    }
    throw e;
  }
}

// 获取模型真实 API Key（编辑时明文显示用）
export async function getModelApiKey(modelId: string): Promise<string | null> {
  return loggedInvoke<string | null>('get_model_api_key', { modelId });
}

// 从 API 地址获取可用模型列表
export async function fetchModelsFromApi(baseUrl: string, apiKey?: string): Promise<string[]> {
  try {
    return await loggedInvoke<string[]>('fetch_models', { baseUrl, apiKey });
  } catch (e) {
    const isTauri = !!(window as any).__TAURI__;
    if (!isTauri) {
      try {
        const headers: Record<string, string> = {};
        if (apiKey) headers['Authorization'] = `Bearer ${apiKey}`;
        const resp = await fetch(`${baseUrl}/v1/models`, { headers });
        const data = await resp.json();
        return data.data?.map((m: any) => m.id) || [];
      } catch {
        return [];
      }
    }
    throw e;
  }
}

// 获取模型提供商列表
export function getModelProviders(): Array<{
  id: string;
  name: string;
  requiresApiKey: boolean;
  supports: ModelConfig['type'][];
}> {
  return [
    {
      id: 'openai',
      name: 'OpenAI',
      requiresApiKey: true,
      supports: ['chat', 'embedding', 'multimodal', 'image'],
    },
    { id: 'anthropic', name: 'Anthropic', requiresApiKey: true, supports: ['chat', 'multimodal'] },
    { id: 'azure', name: 'Azure OpenAI', requiresApiKey: true, supports: ['chat', 'embedding'] },
    {
      id: 'ollama',
      name: 'Ollama (Local)',
      requiresApiKey: false,
      supports: ['chat', 'embedding'],
    },
    { id: 'deepseek', name: 'DeepSeek', requiresApiKey: true, supports: ['chat'] },
    { id: 'qwen', name: '通义千问', requiresApiKey: true, supports: ['chat', 'multimodal'] },
    { id: 'moonshot', name: 'Moonshot', requiresApiKey: true, supports: ['chat'] },
    { id: 'zhipu', name: '智谱AI', requiresApiKey: true, supports: ['chat', 'multimodal'] },
    {
      id: 'custom',
      name: 'Custom',
      requiresApiKey: false,
      supports: ['chat', 'embedding', 'multimodal'],
    },
  ];
}

// 获取提供商默认模型
export function getProviderDefaultModels(provider: string): string[] {
  const defaults: Record<string, string[]> = {
    openai: ['gpt-4', 'gpt-4-turbo-preview', 'gpt-3.5-turbo', 'text-embedding-3-small', 'dall-e-3'],
    anthropic: ['claude-3-opus-20240229', 'claude-3-sonnet-20240229', 'claude-3-haiku-20240307'],
    azure: ['gpt-4', 'gpt-35-turbo', 'text-embedding-ada-002'],
    ollama: ['llama2', 'mistral', 'codellama', 'nomic-embed-text'],
    deepseek: ['deepseek-chat', 'deepseek-coder'],
    qwen: ['qwen-turbo', 'qwen-plus', 'qwen-max'],
    moonshot: ['moonshot-v1-8k', 'moonshot-v1-32k', 'moonshot-v1-128k'],
    zhipu: ['glm-4', 'glm-3-turbo'],
    custom: ['custom-model'],
  };
  return defaults[provider] || ['custom-model'];
}
