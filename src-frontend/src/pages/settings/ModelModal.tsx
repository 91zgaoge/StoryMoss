import { useState, useEffect } from 'react';
import { X, Eye, EyeOff, RefreshCw, MessageSquare, Database, Sparkles, Image } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useForm } from 'react-hook-form';
import { useSettingsContext } from '@/hooks/useSettingsContext';
import {
  getModelProviders,
  getProviderDefaultModels,
  fetchModelsFromApi,
  getModelApiKey,
  detectModelContext,
} from '@/services/settings';
import toast from 'react-hot-toast';
import type {
  ModelType,
  ModelConfig,
  LlmProvider,
  ChatModelConfig,
  MultimodalModelConfig,
  ModelCapability,
} from '@/types/llm';
import { cn } from '@/utils/cn';
import { normalizeFloat } from '@/utils/numberFormat';
import { extractMessage } from '@/utils/errorHandler';

interface ModelFormData {
  name: string;
  provider: LlmProvider;
  model: string;
  description?: string;
  api_key?: string;
  api_base?: string;
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
  dimensions?: number;
  max_context_length?: number;
  is_default?: boolean;
  enabled?: boolean;
  system_prompt_override?: string;
}

function isChatOrMultimodal(model: ModelConfig): model is ChatModelConfig | MultimodalModelConfig {
  return model.type === 'chat' || model.type === 'multimodal';
}

function isChatModel(model: ModelConfig): model is ChatModelConfig {
  return model.type === 'chat';
}

const typeLabels: Record<ModelType, string> = {
  chat: '聊天',
  embedding: '嵌入',
  multimodal: '多模态',
  image: '图像',
};

const typeIcons: Record<ModelType, React.ReactNode> = {
  chat: <MessageSquare className="w-5 h-5" />,
  embedding: <Database className="w-5 h-5" />,
  multimodal: <Sparkles className="w-5 h-5" />,
  image: <Image className="w-5 h-5" />,
};

export function ModelModal({
  type: initialType,
  model,
  onClose,
}: {
  type: ModelType;
  model: ModelConfig | null;
  onClose: () => void;
}) {
  // 编辑时固定为 model.type，新建时可选择类型
  const [selectedType, setSelectedType] = useState<ModelType>(model?.type || initialType);
  const effectiveType = model ? model.type : selectedType;

  const defaultValues: ModelFormData = {
    name: '',
    provider: 'openai' as LlmProvider,
    model: '',
    api_key: '',
    api_base: '',
    temperature: 0.7,
    max_tokens: 4096,
    top_p: undefined,
    frequency_penalty: undefined,
    presence_penalty: undefined,
    dimensions: 1536,
    is_default: false,
    enabled: true,
  };

  const { register, handleSubmit, watch, setValue, getValues } = useForm<ModelFormData>({
    defaultValues: model
      ? {
          ...defaultValues,
          ...model,
          api_key: model.api_key === '***' ? '' : model.api_key || '',
          temperature: isChatOrMultimodal(model)
            ? normalizeFloat(model.temperature ?? 0.7, 2)
            : 0.7,
          top_p: isChatModel(model) ? model.top_p : undefined,
          frequency_penalty: isChatModel(model) ? model.frequency_penalty : undefined,
          presence_penalty: isChatModel(model) ? model.presence_penalty : undefined,
        }
      : defaultValues,
  });

  // 类型切换时，若当前 provider 不被新类型支持则重置为第一个支持的 provider
  useEffect(() => {
    const currentProvider = getValues('provider');
    const supportedProviders = getModelProviders().filter(p => p.supports.includes(effectiveType));
    const isSupported = supportedProviders.some(p => p.id === currentProvider);
    if (!isSupported && supportedProviders.length > 0) {
      setValue('provider', supportedProviders[0].id as LlmProvider);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [effectiveType]);

  const provider = watch('provider');
  const providers = getModelProviders().filter(p => p.supports.includes(effectiveType));
  const defaultModels = getProviderDefaultModels(provider);
  const requiresApiKey = providers.find(p => p.id === provider)?.requiresApiKey ?? true;
  const showApiKeyField = requiresApiKey || provider === 'custom';

  const { createModel, updateModel } = useSettingsContext();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [fetchedModels, setFetchedModels] = useState<string[]>([]);
  const [isFetchingModels, setIsFetchingModels] = useState(false);
  const [fetchModelsError, setFetchModelsError] = useState<string | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);
  const [isRevealing, setIsRevealing] = useState(false);
  // v0.73.7: 上下文长度自动检测
  const [isDetectingContext, setIsDetectingContext] = useState(false);
  const [contextSource, setContextSource] = useState<string | null>(null);

  const formatContext = (n: number) => (n >= 1024 ? `${Math.round(n / 1024)}K` : `${n}`);

  const contextSourceLabel =
    contextSource === 'api'
      ? '已从服务端 /models 检测'
      : contextSource === 'ollama'
        ? '已从 Ollama /api/show 检测'
        : contextSource === 'catalog'
          ? '内置型号目录参考值（可手动覆盖）'
          : contextSource === 'unknown'
            ? '未检测到，请手动填写'
            : null;

  const detectContext = async (modelOverride?: string, silent = false) => {
    const provider = getValues('provider');
    const modelId = (modelOverride ?? getValues('model') ?? '').trim();
    if (!modelId) {
      if (!silent) toast.error('请先填写模型 ID');
      return;
    }
    setIsDetectingContext(true);
    try {
      const info = await detectModelContext({
        provider,
        model: modelId,
        apiBase: getValues('api_base') || undefined,
        apiKey: getValues('api_key') || undefined,
      });
      setContextSource(info.source);
      if (info.max_context_length) {
        setValue('max_context_length', info.max_context_length);
        if (!silent) {
          toast.success(`已检测：${formatContext(info.max_context_length)} tokens`);
        }
      } else if (!silent) {
        toast.error('未能自动检测，请手动填写上下文长度');
      }
    } catch (e) {
      if (!silent) toast.error(extractMessage(e));
    } finally {
      setIsDetectingContext(false);
    }
  };

  const onSubmit = async (data: ModelFormData) => {
    const payload: {
      name: string;
      provider: LlmProvider;
      model: string;
      description?: string;
      api_base?: string;
      model_type: ModelType;
      is_default: boolean;
      enabled: boolean;
      api_key?: string;
      temperature?: number;
      max_tokens?: number;
      top_p?: number;
      frequency_penalty?: number;
      presence_penalty?: number;
      capabilities?: ModelCapability[];
      dimensions?: number;
      max_context_length?: number;
    } = {
      name: data.name,
      provider: data.provider,
      model: data.model,
      description: data.description || undefined,
      api_base: data.api_base || undefined,
      model_type: effectiveType,
      is_default: !!data.is_default,
      enabled: data.enabled !== false,
    };

    // API Key 处理：
    // - 新建模型：始终传递用户输入的值（包括空字符串）
    // - 编辑模型：
    //   * 如果原始模型有密钥（后端返回 '***'），用户输入非空值 → 更新为新密钥
    //   * 如果原始模型有密钥，用户未输入（空） → 不传递 api_key，后端保留旧值
    //   * 如果原始模型无密钥，用户输入了值 → 添加密钥
    //   * 如果原始模型无密钥，用户未输入 → 不传递，保持无密钥
    if (!model) {
      // 新建：传递用户输入，即使为空字符串
      payload.api_key = data.api_key || '';
    } else if (data.api_key && data.api_key !== '') {
      // 编辑：用户输入了非空值，传递它（即使是 '***' 也传递，后端会保存）
      payload.api_key = data.api_key;
    }
    // 编辑且空字符串：不传递 api_key 字段，后端保留旧值

    if (effectiveType === 'chat' || effectiveType === 'multimodal') {
      payload.temperature = normalizeFloat(Number(data.temperature), 2);
      payload.max_tokens = Number(data.max_tokens);
      if (data.top_p !== undefined) {
        payload.top_p = normalizeFloat(Number(data.top_p), 2);
      }
      if (data.frequency_penalty !== undefined) {
        payload.frequency_penalty = normalizeFloat(Number(data.frequency_penalty), 2);
      }
      if (data.presence_penalty !== undefined) {
        payload.presence_penalty = normalizeFloat(Number(data.presence_penalty), 2);
      }
      // v0.23.61: 系统提示词覆盖
      if (data.system_prompt_override !== undefined) {
        (payload as any).system_prompt_override = data.system_prompt_override;
      }
      payload.capabilities =
        effectiveType === 'chat'
          ? ['chat', 'completion', 'long_context']
          : ['chat', 'vision', 'long_context'];
    }

    if (effectiveType === 'embedding') {
      payload.dimensions = Number(data.dimensions);
    }

    // v0.73.7: 未填写上下文长度时自动检测；失败不阻断保存，后端按型号
    // 目录兜底（catalog_context_length）。
    if (effectiveType === 'chat' || effectiveType === 'multimodal') {
      let contextLength = Number(data.max_context_length);
      if (!Number.isFinite(contextLength) || contextLength <= 0) {
        try {
          const info = await detectModelContext({
            provider: data.provider,
            model: data.model,
            apiBase: data.api_base || undefined,
            apiKey: data.api_key || undefined,
          });
          if (info.max_context_length) contextLength = info.max_context_length;
        } catch {
          // 探测失败：交给后端目录/默认值
        }
      }
      if (Number.isFinite(contextLength) && contextLength > 0) {
        payload.max_context_length = contextLength;
      }
    }

    setIsSubmitting(true);
    try {
      if (model) {
        await updateModel(model.id, payload);
      } else {
        await createModel({ ...payload, type: effectiveType } as Omit<ModelConfig, 'id'>);
      }
      onClose();
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <Card className="w-full max-w-2xl max-h-[90vh] overflow-auto">
        <form onSubmit={handleSubmit(onSubmit)}>
          <CardContent className="p-6 space-y-6">
            <div className="flex items-center justify-between">
              <h2 className="font-display text-xl font-bold text-white">
                {model ? '编辑模型' : '添加模型'}
              </h2>
              <button type="button" onClick={onClose} className="text-gray-400 hover:text-white">
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* 模型类型选择（仅新建时） */}
            {!model && (
              <div className="space-y-3">
                <label className="block text-sm text-gray-400">选择模型类型</label>
                <div className="grid grid-cols-4 gap-3">
                  {(['chat', 'embedding', 'multimodal', 'image'] as ModelType[]).map(t => (
                    <button
                      key={t}
                      type="button"
                      onClick={() => setSelectedType(t)}
                      className={cn(
                        'flex flex-col items-center gap-2 p-4 rounded-xl border-2 transition-all',
                        selectedType === t
                          ? 'border-cinema-gold bg-cinema-gold/10 text-cinema-gold'
                          : 'border-cinema-700 bg-cinema-800 text-gray-400 hover:border-cinema-600'
                      )}
                    >
                      {typeIcons[t]}
                      <span className="text-sm font-medium">{typeLabels[t]}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}

            {/* 基本配置 */}
            <div className="grid grid-cols-2 gap-4">
              <div className="col-span-2">
                <label className="block text-sm text-gray-400 mb-1">名称 *</label>
                <input
                  {...register('name', { required: true })}
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  placeholder="例如: GPT-4"
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">提供商 *</label>
                <select
                  {...register('provider', { required: true })}
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                >
                  {providers.map(p => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">模型 *</label>
                <input
                  {...register('model', { required: true })}
                  list="model-suggestions"
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  placeholder="例如: gpt-4"
                />
                <datalist id="model-suggestions">
                  {defaultModels.map(m => (
                    <option key={m} value={m} />
                  ))}
                </datalist>
                {fetchedModels.length > 0 && (
                  <div className="mt-2">
                    <label className="text-xs text-gray-500 mb-1 block">检测到以下可用模型：</label>
                    <div className="flex flex-wrap gap-1.5 max-h-28 overflow-y-auto">
                      {fetchedModels.map(m => (
                        <button
                          key={m}
                          type="button"
                          onClick={() => {
                            setValue('model', m, { shouldValidate: true });
                            void detectContext(m, true);
                          }}
                          className="px-2 py-1 text-xs bg-cinema-700 hover:bg-cinema-600 text-gray-300 rounded-md transition-colors"
                        >
                          {m}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
                {fetchModelsError && (
                  <p className="text-xs text-red-400 mt-1">{fetchModelsError}</p>
                )}
              </div>
            </div>

            {/* API配置 */}
            <div className="space-y-4">
              <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider">
                API配置
              </h3>

              {showApiKeyField && (
                <div>
                  <label className="block text-sm text-gray-400 mb-1">API Key</label>
                  <div className="flex gap-2">
                    <input
                      {...register('api_key')}
                      type={showApiKey ? 'text' : 'password'}
                      className="flex-1 px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                      placeholder={model?.api_key === '***' ? '****' : 'sk-...'}
                    />
                    {model?.api_key === '***' && (
                      <button
                        type="button"
                        onClick={async () => {
                          if (showApiKey) {
                            setShowApiKey(false);
                            setValue('api_key', '');
                          } else {
                            setIsRevealing(true);
                            try {
                              const key = await getModelApiKey(model.id);
                              setValue('api_key', key || '');
                              setShowApiKey(true);
                            } catch {
                              toast.error('获取密钥失败');
                            } finally {
                              setIsRevealing(false);
                            }
                          }
                        }}
                        disabled={isRevealing}
                        className="px-3 py-2 bg-cinema-700 hover:bg-cinema-600 disabled:opacity-50 text-gray-300 rounded-xl transition-colors flex items-center justify-center"
                        title={showApiKey ? '隐藏密钥' : '显示密钥'}
                      >
                        {isRevealing ? (
                          <span className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                        ) : showApiKey ? (
                          <EyeOff className="w-4 h-4" />
                        ) : (
                          <Eye className="w-4 h-4" />
                        )}
                      </button>
                    )}
                  </div>
                  <p className="text-xs mt-1">
                    {model?.api_key === '***' ? (
                      <span className="text-green-400">
                        ✓ API Key 已设置{showApiKey ? '（明文显示中）' : '，输入新值覆盖'}
                      </span>
                    ) : (
                      <span className="text-gray-500">API Key 将被安全存储</span>
                    )}
                  </p>
                </div>
              )}

              <div className="flex items-end gap-2">
                <div className="flex-1">
                  <label className="block text-sm text-gray-400 mb-1">API Base (可选)</label>
                  <input
                    {...register('api_base')}
                    className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                    placeholder="https://api.openai.com/v1"
                  />
                </div>
                <button
                  type="button"
                  onClick={async () => {
                    const baseUrl = getValues('api_base');
                    const apiKey = getValues('api_key');
                    if (!baseUrl) {
                      setFetchModelsError('请先填写 API Base 地址');
                      return;
                    }
                    setIsFetchingModels(true);
                    setFetchModelsError(null);
                    setFetchedModels([]);
                    try {
                      const models = await fetchModelsFromApi(baseUrl, apiKey);
                      setFetchedModels(models);
                      if (models.length === 0) {
                        setFetchModelsError('未找到可用模型');
                      }
                    } catch (err) {
                      // 后端 AppError 经 Tauri 序列化为 { code, message, severity } 对象
                      // （非 Error 实例），String(err) 会得到 "[object Object]"（issue #11）。
                      // 用统一 extractMessage 兼容对象/Error/字符串三种形态。
                      const msg = extractMessage(err);
                      setFetchModelsError(msg);
                    } finally {
                      setIsFetchingModels(false);
                    }
                  }}
                  disabled={isFetchingModels}
                  className="px-3 py-2 bg-cinema-700 hover:bg-cinema-600 disabled:opacity-50 text-gray-300 text-sm rounded-xl transition-colors flex items-center gap-1.5 whitespace-nowrap"
                >
                  <RefreshCw className={`w-3.5 h-3.5 ${isFetchingModels ? 'animate-spin' : ''}`} />
                  {isFetchingModels ? '获取中...' : '获取模型'}
                </button>
              </div>
            </div>

            {/* 模型参数 */}
            {(effectiveType === 'chat' || effectiveType === 'multimodal') && (
              <div className="space-y-4">
                <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider">
                  模型参数
                </h3>

                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Temperature</label>
                    <input
                      {...register('temperature', { valueAsNumber: true })}
                      type="number"
                      step="0.1"
                      min="0"
                      max="2"
                      className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Max Tokens</label>
                    <input
                      {...register('max_tokens', { valueAsNumber: true })}
                      type="number"
                      className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Top P（可选）</label>
                    <input
                      {...register('top_p', { valueAsNumber: true })}
                      type="number"
                      step="0.05"
                      min="0"
                      max="1"
                      placeholder="默认不设置"
                      className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">
                      Frequency Penalty（可选）
                    </label>
                    <input
                      {...register('frequency_penalty', { valueAsNumber: true })}
                      type="number"
                      step="0.1"
                      min="-2"
                      max="2"
                      placeholder="默认不设置"
                      className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">
                      Presence Penalty（可选）
                    </label>
                    <input
                      {...register('presence_penalty', { valueAsNumber: true })}
                      type="number"
                      step="0.1"
                      min="-2"
                      max="2"
                      placeholder="默认不设置"
                      className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                    />
                  </div>
                  {/* v0.73.7: 上下文长度自动检测 */}
                  <div className="col-span-2">
                    <label className="block text-sm text-gray-400 mb-1">
                      上下文长度（tokens）
                      <span className="text-xs text-gray-500 ml-2">
                        — 路由判断提示词是否装得下；留空保存时自动检测
                      </span>
                    </label>
                    <div className="flex gap-2">
                      <input
                        {...register('max_context_length', { valueAsNumber: true })}
                        type="number"
                        min={512}
                        step={1024}
                        placeholder="留空自动检测（默认 8192）"
                        className="flex-1 px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                      />
                      <button
                        type="button"
                        onClick={() => void detectContext()}
                        disabled={isDetectingContext}
                        className="px-3 py-2 bg-cinema-700 hover:bg-cinema-600 disabled:opacity-50 text-gray-300 text-sm rounded-xl transition-colors flex items-center gap-1.5 whitespace-nowrap"
                      >
                        {isDetectingContext ? (
                          <span className="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin" />
                        ) : (
                          <RefreshCw className="w-3.5 h-3.5" />
                        )}
                        自动检测
                      </button>
                    </div>
                    <p className="text-xs text-gray-500 mt-1">
                      {contextSourceLabel ??
                        '检测顺序：服务端 /models（vLLM·OpenRouter）→ Ollama /api/show → 内置型号目录'}
                    </p>
                  </div>
                </div>
              </div>
            )}

            {effectiveType === 'embedding' && (
              <div className="space-y-4">
                <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider">
                  嵌入参数
                </h3>

                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Dimensions</label>
                    <input
                      {...register('dimensions' as const, { valueAsNumber: true })}
                      type="number"
                      className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                    />
                  </div>
                </div>
              </div>
            )}

            {/* 选项 */}
            <div className="flex items-center gap-6">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  {...register('is_default')}
                  type="checkbox"
                  className="w-4 h-4 rounded border-cinema-700 bg-cinema-800 text-cinema-gold"
                />
                <span className="text-sm text-gray-300">设为默认</span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  {...register('enabled')}
                  type="checkbox"
                  className="w-4 h-4 rounded border-cinema-700 bg-cinema-800 text-cinema-gold"
                />
                <span className="text-sm text-gray-300">启用</span>
              </label>
            </div>

            {/* v0.23.61: 系统提示词覆盖 */}
            {(effectiveType === 'chat' || effectiveType === 'multimodal') && (
              <div>
                <label className="block text-sm text-gray-400 mb-1">
                  系统提示词覆盖（可选，替换默认英文 system prompt）
                </label>
                <textarea
                  {...register('system_prompt_override')}
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-y"
                  placeholder="留空则使用全局默认。例如：你是专业的中文小说创作助手，擅长…"
                  rows={3}
                />
              </div>
            )}

            {/* 按钮 */}
            <div className="flex justify-end gap-3 pt-4 border-t border-cinema-800">
              <Button type="button" variant="ghost" onClick={onClose}>
                取消
              </Button>
              <Button
                type="submit"
                variant="primary"
                isLoading={isSubmitting}
                disabled={isSubmitting}
              >
                {model ? '保存' : '创建'}
              </Button>
            </div>
          </CardContent>
        </form>
      </Card>
    </div>
  );
}
