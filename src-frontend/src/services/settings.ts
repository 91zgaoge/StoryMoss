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

function isTauriApp(): boolean {
  return !!(window as unknown as { __TAURI__?: unknown }).__TAURI__;
}

// v0.23.14: 浏览器开发环境 fallback —— 空模型列表，不再注入硬编码 LAN IP 模型。
// 避免前端任何代码路径误用死模型 id。Tauri 生产环境走后端 IPC，不受影响。
const BROWSER_FALLBACK_MODELS: ModelConfig[] = [];

// 浏览器开发环境 fallback 设置
const BROWSER_FALLBACK_SETTINGS: AppSettings = {
  version: '0.1.0',
  updated_at: new Date().toISOString(),
  models: {
    chat: [],
    embedding: [],
    multimodal: [],
    image: [],
  },
  active_models: {
    chat: '',
    embedding: '',
    multimodal: '',
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
    if (!isTauriApp()) {
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
    if (!isTauriApp()) {
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

// 设置激活的模型（v0.23.66: 支持 role 参数指定创意/工具/后台角色）
export async function setActiveModel(
  type: ModelConfig['type'],
  modelId: string,
  role?: 'creative' | 'tool' | 'background'
): Promise<void> {
  const params: Record<string, string | null> = { modelType: type, modelId };
  if (role) {
    params.role = role;
  }
  await loggedInvoke<void>('set_active_model', params);
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
  } catch (e: unknown) {
    const latency = Math.round(performance.now() - start);
    const lastRunningIdx = steps.findIndex(s => s.status === 'running');
    if (lastRunningIdx >= 0) {
      const err = e instanceof Error ? e : new Error(String(e));
      const errMsg = err.name === 'AbortError' ? '连接超时' : err.message || '连接失败';
      steps[lastRunningIdx] = {
        ...steps[lastRunningIdx],
        status: 'failed',
        detail: errMsg,
      };
    }
    const err = e instanceof Error ? e : new Error(String(e));
    return {
      success: false,
      latency,
      error: err.name === 'AbortError' ? '连接超时' : err.message || '连接失败',
      steps,
    };
  }
}

// 测试模型连接
export async function testModelConnection(modelId: string): Promise<ConnectionTestResult> {
  try {
    return await loggedInvoke<ConnectionTestResult>('test_model_connection', { modelId });
  } catch (e) {
    if (!isTauriApp()) {
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
    if (!isTauriApp()) {
      try {
        const headers: Record<string, string> = {};
        if (apiKey) headers['Authorization'] = `Bearer ${apiKey}`;
        const resp = await fetch(`${baseUrl}/v1/models`, { headers });
        const data = await resp.json();
        return (data.data as Array<{ id: string }>)?.map(m => m.id) || [];
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
