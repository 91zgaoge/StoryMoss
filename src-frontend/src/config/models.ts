/**
 * 草苔项目模型配置
 *
 * 配置三个模型：
 * 1. 多模态模型 - Gemma-4-31B-it-Q6_K
 * 2. 语言模型 - Qwen3.5-27B-Uncensored-Q4_K_M
 * 3. Embedding嵌入模型 - bge-m3
 */

export interface ModelConfig {
  id: string;
  name: string;
  type: 'multimodal' | 'language' | 'embedding';
  baseUrl: string;
  apiKey?: string;
  useApiKey: boolean;
  description: string;
  maxTokens?: number;
  temperature?: number;
}

export const MODELS: Record<string, ModelConfig> = {
  // 多模态模型
  gemma4: {
    id: 'Gemma-4-31B-it-Q6_K',
    name: 'Gemma 4 多模态',
    type: 'multimodal',
    baseUrl: 'http://10.62.239.13:17099/v1',
    useApiKey: false,
    description: '多模态模型，支持图文理解',
    maxTokens: 8192,
    temperature: 0.7,
  },

  // 语言模型（默认）
  qwen35: {
    id: 'Qwen3.5-27B-Uncensored-Q4_K_M',
    name: 'Qwen 3.5 语言模型',
    type: 'language',
    baseUrl: 'http://10.62.239.13:17098/v1',
    useApiKey: false,
    description: '强大的语言模型，用于文本生成和对话',
    maxTokens: 8192,
    temperature: 0.8,
  },

  // Embedding嵌入模型
  bgeM3: {
    id: 'bge-m3',
    name: 'BGE-M3 Embedding',
    type: 'embedding',
    baseUrl: 'http://10.62.239.13:8089',
    apiKey: '76e0e2bc84c45374999a1d5e66962544c09cc00ae42ad25cd6a2a07a9d7fe330',
    useApiKey: true,
    description: '文本嵌入模型，用于语义搜索和向量化',
  },
};

// 默认使用的模型
export const DEFAULT_MODEL_ID = 'qwen35';

// 获取模型配置
export function getModelConfig(modelId: string): ModelConfig {
  const config = MODELS[modelId];
  if (!config) {
    throw new Error(`未找到模型配置: ${modelId}`);
  }
  return config;
}

// 获取所有可用模型列表
export function getAvailableModels(): ModelConfig[] {
  return Object.values(MODELS).filter(m => m.type !== 'embedding');
}

// 获取当前对话模型（语言或多模态）
export function getChatModel(modelId?: string): ModelConfig {
  if (modelId && MODELS[modelId] && MODELS[modelId].type !== 'embedding') {
    return MODELS[modelId];
  }
  return MODELS[DEFAULT_MODEL_ID];
}

// 获取Embedding模型
export function getEmbeddingModel(): ModelConfig {
  return MODELS.bgeM3;
}
