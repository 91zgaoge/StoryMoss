/**
 * LLM 相关类型定义
 *
 * 支持多种类型的模型配置：
 * - Chat/Completion: 文本生成模型
 * - Embedding: 向量嵌入模型
 * - Multimodal: 多模态模型（支持图文）
 * - Image: 图像生成模型
 */

// 模型能力
export type ModelCapability =
  | 'chat'
  | 'completion'
  | 'function_calling'
  | 'json_mode'
  | 'vision'
  | 'long_context'
  | 'embedding'
  | 'image_generation';

// 模型类型
export type ModelType = 'chat' | 'embedding' | 'multimodal' | 'image';

// 提供商类型
export type LlmProvider =
  | 'openai'
  | 'anthropic'
  | 'azure'
  | 'ollama'
  | 'deepseek'
  | 'qwen'
  | 'moonshot'
  | 'zhipu'
  | 'custom';

// 模型来源 — 与后端 ModelSource 对应
export type ModelSource = 'platform' | 'local' | 'user_owned';

// 基础模型配置（通用字段）
export interface BaseModelConfig {
  id: string;
  name: string;
  description?: string;
  provider: LlmProvider;
  model_source?: ModelSource;
  model: string;
  api_key?: string;
  api_base?: string;
  timeout_seconds?: number;
  is_default?: boolean;
  enabled: boolean;
}

// 文本生成模型配置
export interface ChatModelConfig extends BaseModelConfig {
  type: 'chat';
  temperature: number;
  max_tokens: number;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
  capabilities: ModelCapability[];
}

// 嵌入模型配置
export interface EmbeddingModelConfig extends BaseModelConfig {
  type: 'embedding';
  dimensions: number;
  max_input_tokens: number;
  batch_size?: number;
}

// 多模态模型配置
export interface MultimodalModelConfig extends BaseModelConfig {
  type: 'multimodal';
  temperature: number;
  max_tokens: number;
  supports_vision: boolean;
  supports_audio: boolean;
  max_image_size?: number;
  supported_formats?: string[];
  capabilities: ModelCapability[];
}

// 图像生成模型配置
export interface ImageModelConfig extends BaseModelConfig {
  type: 'image';
  image_size?: '256x256' | '512x512' | '1024x1024' | '1792x1024' | '1024x1792';
  quality?: 'standard' | 'hd';
  style?: 'vivid' | 'natural';
}

// 统一模型配置类型
export type ModelConfig =
  | ChatModelConfig
  | EmbeddingModelConfig
  | MultimodalModelConfig
  | ImageModelConfig;

// Agent 模型选择配置
export interface AgentModelMapping {
  agent_id: string;
  agent_name: string;
  chat_model_id?: string;
  embedding_model_id?: string;
  multimodal_model_id?: string;
  description?: string;
}

// 完整应用配置
export interface AppSettings {
  version: string;
  updated_at: string;

  // 模型配置
  models: {
    chat: ChatModelConfig[];
    embedding: EmbeddingModelConfig[];
    multimodal: MultimodalModelConfig[];
    image: ImageModelConfig[];
  };

  // 当前激活的配置ID
  active_models: {
    chat?: string;
    embedding?: string;
    multimodal?: string;
    image?: string;
  };

  // Agent模型映射
  agent_mappings: AgentModelMapping[];

  // 通用设置
  general: {
    theme: 'dark' | 'light' | 'system';
    language: string;
    auto_save: boolean;
    auto_save_interval: number;
    font_size: number;
    line_height: number;
  };

  // 隐私设置
  privacy: {
    share_usage_data: boolean;
    store_api_keys_securely: boolean;
  };

  // 拆书分析 LLM 并发数（默认 3，本地模型可调到 50）
  book_deconstruction_concurrency: number;

  // AgentOrchestrator 配置
  rewrite_threshold: number;
  max_feedback_loops: number;

  // 写作策略配置
  writing_strategy: WritingStrategy;
}

export interface WritingStrategy {
  run_mode: 'fast' | 'polish';
  conflict_level: number;
  pace: 'slow' | 'balanced' | 'fast';
  ai_freedom: 'low' | 'medium' | 'high';
}

// 设置导出/导入格式
export interface SettingsExport {
  version: string;
  exported_at: string;
  settings: Omit<AppSettings, 'version' | 'updated_at'>;
}

// API Key 安全存储（加密后）
export interface EncryptedApiKey {
  model_id: string;
  encrypted_key: string;
  iv: string;
}

// 连接测试步骤
export interface ConnectionTestStep {
  name: string;
  status: 'pending' | 'running' | 'success' | 'failed';
  detail?: string;
}

// 连接测试结果
export interface ConnectionTestResult {
  success: boolean;
  latency: number;
  error?: string;
  steps: ConnectionTestStep[];
}

// 预设模型配置
export const PRESET_MODELS: Partial<ModelConfig>[] = [
  // OpenAI Chat Models
  {
    id: 'preset-openai-gpt4',
    name: 'GPT-4',
    provider: 'openai',
    model: 'gpt-4',
    type: 'chat',
    temperature: 0.7,
    max_tokens: 4096,
    capabilities: ['chat', 'function_calling', 'json_mode', 'long_context'],
  },
  {
    id: 'preset-openai-gpt4-turbo',
    name: 'GPT-4 Turbo',
    provider: 'openai',
    model: 'gpt-4-turbo-preview',
    type: 'chat',
    temperature: 0.7,
    max_tokens: 4096,
    capabilities: ['chat', 'function_calling', 'json_mode', 'vision', 'long_context'],
  },
  {
    id: 'preset-openai-gpt35',
    name: 'GPT-3.5 Turbo',
    provider: 'openai',
    model: 'gpt-3.5-turbo',
    type: 'chat',
    temperature: 0.7,
    max_tokens: 4096,
    capabilities: ['chat', 'function_calling', 'json_mode'],
  },
  // OpenAI Embedding
  {
    id: 'preset-openai-embedding-3-small',
    name: 'OpenAI Embedding 3 Small',
    provider: 'openai',
    model: 'text-embedding-3-small',
    type: 'embedding',
    dimensions: 1536,
    max_input_tokens: 8192,
  },
  {
    id: 'preset-openai-embedding-3-large',
    name: 'OpenAI Embedding 3 Large',
    provider: 'openai',
    model: 'text-embedding-3-large',
    type: 'embedding',
    dimensions: 3072,
    max_input_tokens: 8192,
  },
  // Anthropic
  {
    id: 'preset-anthropic-claude-3-opus',
    name: 'Claude 3 Opus',
    provider: 'anthropic',
    model: 'claude-3-opus-20240229',
    type: 'chat',
    temperature: 0.7,
    max_tokens: 4096,
    capabilities: ['chat', 'vision', 'long_context'],
  },
  {
    id: 'preset-anthropic-claude-3-sonnet',
    name: 'Claude 3 Sonnet',
    provider: 'anthropic',
    model: 'claude-3-sonnet-20240229',
    type: 'chat',
    temperature: 0.7,
    max_tokens: 4096,
    capabilities: ['chat', 'vision'],
  },
  // Ollama (Local)
  {
    id: 'preset-ollama-llama2',
    name: 'Llama 2 (Local)',
    provider: 'ollama',
    model: 'llama2',
    type: 'chat',
    temperature: 0.7,
    max_tokens: 4096,
    capabilities: ['chat', 'completion'],
  },
  {
    id: 'preset-ollama-mistral',
    name: 'Mistral (Local)',
    provider: 'ollama',
    model: 'mistral',
    type: 'chat',
    temperature: 0.7,
    max_tokens: 4096,
    capabilities: ['chat'],
  },
  {
    id: 'preset-ollama-embeddings',
    name: 'Ollama Embeddings (Local)',
    provider: 'ollama',
    model: 'nomic-embed-text',
    type: 'embedding',
    dimensions: 768,
    max_input_tokens: 8192,
  },
  // 多模态
  {
    id: 'preset-openai-gpt4-vision',
    name: 'GPT-4 Vision',
    provider: 'openai',
    model: 'gpt-4-vision-preview',
    type: 'multimodal',
    temperature: 0.7,
    max_tokens: 4096,
    supports_vision: true,
    supports_audio: false,
    capabilities: ['chat', 'vision', 'long_context'],
  },
  // 图像生成
  {
    id: 'preset-openai-dall-e-3',
    name: 'DALL-E 3',
    provider: 'openai',
    model: 'dall-e-3',
    type: 'image',
    image_size: '1024x1024',
    quality: 'standard',
    style: 'vivid',
  },
];

// 默认Agent模型映射
export const DEFAULT_AGENT_MAPPINGS: AgentModelMapping[] = [
  {
    agent_id: 'writer',
    agent_name: '写作助手',
    description: '负责章节生成、改写',
  },
  {
    agent_id: 'inspector',
    agent_name: '质检员',
    description: '负责内容质量检查',
  },
  {
    agent_id: 'outline_planner',
    agent_name: '大纲规划师',
    description: '负责故事大纲设计',
  },
  {
    agent_id: 'style_mimic',
    agent_name: '风格模仿师',
    description: '负责文风分析与模仿',
  },
  {
    agent_id: 'plot_analyzer',
    agent_name: '情节分析师',
    description: '负责情节复杂度分析',
  },
];
