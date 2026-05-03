/**
 * Settings Page - 工作室配置
 * 
 * 支持多类型LLM配置管理：
 * - Chat/Completion 模型（文本生成）
 * - Embedding 模型（向量嵌入）
 * - Multimodal 模型（多模态）
 * - Image 模型（图像生成）
 * 
 * 功能：
 * - 添加/编辑/删除模型配置
 * - 设置默认模型
 * - Agent模型映射
 * - 设置导出/导入
 */

import { useState, useEffect } from 'react';
import { 
  Settings2, Key, Globe, Database, 
  Plus, Trash2, Edit2, Download, Upload,
  Check, X, Bot, Sparkles, Image, MessageSquare,
  RefreshCw, Star, BookOpen, Zap, Compass, PenTool,
  User, Shield, Link2, Eye, EyeOff,
  GitBranch, GitCommit, ArrowRight, Loader2
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useSettings, useSaveSettings, useModels, useExportSettings, useImportSettings, useCreateModel, useUpdateModel, useSetActiveModel } from '@/hooks/useSettings';
import { useWorkflows, useReloadWorkflows } from '@/hooks/useWorkflows';
import { useUpdateStory } from '@/hooks/useStories';
import { useAppStore } from '@/stores/appStore';
import { useAuthStore } from '@/stores/useAuthStore';
import toast from 'react-hot-toast';
import { colorThemeList, applyColorTheme, loadColorTheme, type ColorThemeId } from '@/frontstage/config/colorThemes';
import { useUpdater } from '@/hooks/useUpdater';
import { EditorSettings } from '@/components/EditorSettings';
import { useForm } from 'react-hook-form';
import { cn } from '@/utils/cn';
import type { ModelConfig, ModelType, LlmProvider } from '@/types/llm';
import { getModelProviders, getProviderDefaultModels, testModelConnection, fetchModelsFromApi, getModelApiKey } from '@/services/settings';

type TabType = 'chat' | 'embedding' | 'multimodal' | 'image' | 'agents' | 'methodology' | 'workflows' | 'general' | 'account';

export function Settings() {
  const [activeTab, setActiveTab] = useState<TabType>('chat');
  const [showAddModal, setShowAddModal] = useState(false);
  const [editingModel, setEditingModel] = useState<ModelConfig | null>(null);
  
  const { data: settings, isLoading: settingsLoading } = useSettings();
  const { data: models = [], isLoading: modelsLoading } = useModels();
  const [activeModelIds, setActiveModelIds] = useState<Record<string, string>>({});
  
  // 同步活跃模型状态
  useEffect(() => {
    if (settings?.active_models) {
      setActiveModelIds(settings.active_models);
    }
  }, [settings]);
  const exportSettings = useExportSettings();
  const importSettings = useImportSettings();
  const setActiveModelMutation = useSetActiveModel();
  
  const isLoading = settingsLoading || modelsLoading;
  
  // 模型连接状态
  const [connectionStatus, setConnectionStatus] = useState<Record<string, { loading: boolean; success?: boolean; latency?: number; error?: string }>>({});

  // 对当前 tab 的模型进行连接测试
  useEffect(() => {
    if (!models.length) return;

    const currentTabModels = models.filter(m => m.type === activeTab);
    if (!currentTabModels.length) return;

    const testConnections = async () => {
      // 将当前 tab 的模型设为加载中
      setConnectionStatus(prev => {
        const next = { ...prev };
        currentTabModels.forEach(m => { next[m.id] = { loading: true }; });
        return next;
      });

      for (const model of currentTabModels) {
        try {
          const result = await testModelConnection(model.id);
          setConnectionStatus(prev => ({
            ...prev,
            [model.id]: { loading: false, success: result.success, latency: result.latency, error: result.error }
          }));
        } catch (e) {
          setConnectionStatus(prev => ({
            ...prev,
            [model.id]: { loading: false, success: false, error: '测试失败' }
          }));
        }
      }
    };

    testConnections();
  }, [models, activeTab]);
  
  // 按类型过滤模型
  const filteredModels = models.filter(m => m.type === activeTab);
  
  // 处理设置导入
  const handleImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      importSettings.mutate(file);
    }
  };
  
  return (
    <div className="p-8 space-y-6 animate-fade-in">
      {/* 头部 */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">工作室配置</h1>
          <p className="text-gray-400">配置LLM模型和全局设置</p>
        </div>
        <div className="flex items-center gap-3">
          <Button variant="ghost" onClick={() => exportSettings.mutate()} isLoading={exportSettings.isPending}>
            <Download className="w-4 h-4 mr-2" />
            导出设置
          </Button>
          <label className="cursor-pointer inline-flex items-center gap-2 px-4 py-2 text-gray-400 hover:text-white hover:bg-cinema-800/50 rounded-xl transition-all">
            <input type="file" accept=".json" className="hidden" onChange={handleImport} />
            {importSettings.isPending ? (
              <span className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
            ) : (
              <Upload className="w-4 h-4" />
            )}
            导入设置
          </label>
        </div>
      </div>
      
      {/* 标签页 */}
      <div className="flex items-center gap-2 border-b border-cinema-800 pb-4 overflow-x-auto">
        <TabButton 
          active={activeTab === 'chat'} 
          onClick={() => setActiveTab('chat')}
          icon={<MessageSquare className="w-4 h-4" />}
          label="聊天模型"
        />
        <TabButton 
          active={activeTab === 'embedding'} 
          onClick={() => setActiveTab('embedding')}
          icon={<Database className="w-4 h-4" />}
          label="嵌入模型"
        />
        <TabButton 
          active={activeTab === 'multimodal'} 
          onClick={() => setActiveTab('multimodal')}
          icon={<Sparkles className="w-4 h-4" />}
          label="多模态"
        />
        <TabButton 
          active={activeTab === 'image'} 
          onClick={() => setActiveTab('image')}
          icon={<Image className="w-4 h-4" />}
          label="图像生成"
        />
        <TabButton 
          active={activeTab === 'agents'} 
          onClick={() => setActiveTab('agents')}
          icon={<Bot className="w-4 h-4" />}
          label="Agent配置"
        />
        <TabButton 
          active={activeTab === 'methodology'} 
          onClick={() => setActiveTab('methodology')}
          icon={<Compass className="w-4 h-4" />}
          label="创作方法论"
        />
        <TabButton 
          active={activeTab === 'workflows'} 
          onClick={() => setActiveTab('workflows')}
          icon={<GitBranch className="w-4 h-4" />}
          label="工作流"
        />
        <TabButton 
          active={activeTab === 'general'} 
          onClick={() => setActiveTab('general')}
          icon={<Settings2 className="w-4 h-4" />}
          label="通用设置"
        />
        <TabButton 
          active={activeTab === 'account'} 
          onClick={() => setActiveTab('account')}
          icon={<User className="w-4 h-4" />}
          label="账号与登录"
        />
      </div>
      
      {/* 内容区域 */}
      {isLoading ? (
        <div className="text-center py-12 text-gray-500">加载中...</div>
      ) : (
        <>
          {activeTab === 'chat' && (
            <ModelList 
              type="chat" 
              models={filteredModels}
              activeModelId={activeModelIds.chat}
              connectionStatus={connectionStatus}
              onAdd={() => setShowAddModal(true)}
              onEdit={setEditingModel}
              onSetActive={(modelId) => {
                setActiveModelMutation.mutate({ type: 'chat', modelId });
              }}
            />
          )}
          {activeTab === 'embedding' && (
            <ModelList 
              type="embedding" 
              models={filteredModels}
              activeModelId={activeModelIds.embedding}
              connectionStatus={connectionStatus}
              onAdd={() => setShowAddModal(true)}
              onEdit={setEditingModel}
              onSetActive={(modelId) => {
                setActiveModelMutation.mutate({ type: 'embedding', modelId });
              }}
            />
          )}
          {activeTab === 'multimodal' && (
            <ModelList 
              type="multimodal" 
              models={filteredModels}
              activeModelId={activeModelIds.multimodal}
              connectionStatus={connectionStatus}
              onAdd={() => setShowAddModal(true)}
              onEdit={setEditingModel}
              onSetActive={(modelId) => {
                setActiveModelMutation.mutate({ type: 'multimodal', modelId });
              }}
            />
          )}
          {activeTab === 'image' && (
            <ModelList 
              type="image" 
              models={filteredModels}
              activeModelId={activeModelIds.image}
              connectionStatus={connectionStatus}
              onAdd={() => setShowAddModal(true)}
              onEdit={setEditingModel}
              onSetActive={(modelId) => {
                setActiveModelMutation.mutate({ type: 'image', modelId });
              }}
            />
          )}
          {activeTab === 'agents' && <AgentConfig />}
          {activeTab === 'methodology' && <MethodologySettings />}
          {activeTab === 'workflows' && <WorkflowSettings />}
          {activeTab === 'general' && <GeneralSettings />}
          {activeTab === 'account' && <AccountSettings />}
        </>
      )}
      
      {/* 添加/编辑模态框 */}
      {(showAddModal || editingModel) && (
        <ModelModal 
          type={activeTab as ModelType}
          model={editingModel}
          onClose={() => {
            setShowAddModal(false);
            setEditingModel(null);
          }}
        />
      )}
    </div>
  );
}

// 标签按钮组件
function TabButton({ active, onClick, icon, label }: { 
  active: boolean; 
  onClick: () => void; 
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors whitespace-nowrap',
        active 
          ? 'bg-cinema-gold text-black' 
          : 'text-gray-400 hover:text-white hover:bg-cinema-800'
      )}
    >
      {icon}
      {label}
    </button>
  );
}

// 模型列表组件
function ModelList({ 
  type, 
  models,
  activeModelId,
  connectionStatus,
  onAdd,
  onEdit,
  onSetActive,
}: { 
  type: ModelType;
  models: ModelConfig[];
  activeModelId?: string;
  connectionStatus: Record<string, { loading: boolean; success?: boolean; latency?: number; error?: string }>;
  onAdd: () => void;
  onEdit: (model: ModelConfig) => void;
  onSetActive: (modelId: string) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold text-white">
          {type === 'chat' && '聊天模型配置'}
          {type === 'embedding' && '嵌入模型配置'}
          {type === 'multimodal' && '多模态模型配置'}
          {type === 'image' && '图像生成模型配置'}
        </h2>
        <Button variant="primary" onClick={onAdd}>
          <Plus className="w-4 h-4 mr-2" />
          添加模型
        </Button>
      </div>
      
      {models.length === 0 ? (
        <Card>
          <CardContent className="p-12 text-center">
            <Database className="w-16 h-16 text-gray-600 mx-auto mb-4" />
            <h3 className="text-lg font-medium text-white mb-2">暂无模型配置</h3>
            <p className="text-gray-500 mb-4">点击上方按钮添加第一个模型配置</p>
            <Button variant="primary" onClick={onAdd}>
              <Plus className="w-4 h-4 mr-2" />
              添加模型
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4">
          {models.map(model => (
            <ModelCard 
              key={model.id} 
              model={model} 
              isActive={model.id === activeModelId}
              connectionStatus={connectionStatus[model.id]}
              onEdit={() => onEdit(model)}
              onSetActive={() => onSetActive(model.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// 模型卡片组件
function ModelCard({ model, isActive, connectionStatus, onEdit, onSetActive }: { 
  model: ModelConfig; 
  isActive?: boolean;
  connectionStatus?: { loading: boolean; success?: boolean; latency?: number; error?: string };
  onEdit: () => void;
  onSetActive: () => void;
}) {
  const isDefault = model.is_default;
  const providerMeta = getModelProviders().find(p => p.id === model.provider);
  const requiresApiKey = providerMeta?.requiresApiKey ?? true;
  // 后端 api_key 返回 '***' 表示有密钥，null/undefined/'' 表示无密钥
  const hasApiKey = model.api_key === '***' || (!!model.api_key && model.api_key !== '');
  
  return (
    <Card className={cn(isActive && 'border-cinema-gold ring-1 ring-cinema-gold/30')}> 
      <CardContent className="p-5">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-4">
            {/* 提供商图标 */}
            <div className={cn(
              'w-12 h-12 rounded-xl flex items-center justify-center',
              isActive ? 'bg-cinema-gold/20' : 'bg-cinema-800'
            )}>
              {model.provider === 'openai' && <span className={cn('font-bold text-lg', isActive ? 'text-cinema-gold' : 'text-green-400')}>O</span>}
              {model.provider === 'anthropic' && <span className={cn('font-bold text-lg', isActive ? 'text-cinema-gold' : 'text-orange-400')}>A</span>}
              {model.provider === 'ollama' && <span className={cn('font-bold text-lg', isActive ? 'text-cinema-gold' : 'text-blue-400')}>L</span>}
              {model.provider === 'azure' && <span className={cn('font-bold text-lg', isActive ? 'text-cinema-gold' : 'text-blue-500')}>Az</span>}
              {!['openai', 'anthropic', 'ollama', 'azure'].includes(model.provider) && (
                <Globe className={cn('w-6 h-6', isActive ? 'text-cinema-gold' : 'text-gray-400')} />
              )}
            </div>
            
            <div>
              <div className="flex items-center gap-2">
                <h3 className="font-semibold text-white text-lg">{model.name}</h3>
                {isActive && (
                  <span className="px-2 py-0.5 bg-cinema-gold text-black text-xs rounded-full font-medium flex items-center gap-1">
                    <Star className="w-3 h-3" />
                    当前使用
                  </span>
                )}
                {!isActive && isDefault && (
                  <span className="px-2 py-0.5 bg-cinema-gold/20 text-cinema-gold text-xs rounded-full">
                    默认
                  </span>
                )}
                {!model.enabled && (
                  <span className="px-2 py-0.5 bg-red-500/20 text-red-400 text-xs rounded-full">
                    禁用
                  </span>
                )}
              </div>
              <p className="text-sm text-gray-500">
                {model.provider} · {model.model}
              </p>
              {model.description && (
                <p className="text-sm text-gray-400 mt-1">{model.description}</p>
              )}
            </div>
          </div>
          
          <div className="flex items-center gap-3">
            {connectionStatus && connectionStatus.loading && (
              <span className="flex items-center gap-1.5 text-gray-400 text-sm">
                <span className="w-3.5 h-3.5 rounded-full border-2 border-current border-t-transparent animate-spin" />
                检测中
              </span>
            )}
            {connectionStatus && !connectionStatus.loading && connectionStatus.success && (
              <span className="flex items-center gap-1.5 text-green-400 text-sm" title={`延迟 ${connectionStatus.latency}ms`}>
                <span className="w-2.5 h-2.5 rounded-full bg-green-400" />
                已连接 ({connectionStatus.latency}ms)
              </span>
            )}
            {connectionStatus && !connectionStatus.loading && !connectionStatus.success && (
              <span className="flex items-center gap-1.5 text-red-400 text-sm" title={connectionStatus.error}>
                <span className="w-2.5 h-2.5 rounded-full bg-red-400" />
                连接失败
              </span>
            )}
            {requiresApiKey && !hasApiKey && (
              <span className="flex items-center gap-1 text-amber-400 text-sm">
                <Key className="w-4 h-4" />
                需配置API Key
              </span>
            )}
            {!isActive && (
              <button
                onClick={onSetActive}
                className="px-3 py-1.5 text-xs font-medium text-cinema-gold bg-cinema-gold/10 hover:bg-cinema-gold/20 rounded-lg transition-colors flex items-center gap-1"
                title="设为当前使用"
              >
                <Star className="w-3.5 h-3.5" />
                设为当前
              </button>
            )}
            <Button variant="ghost" size="sm" onClick={onEdit}>
              <Edit2 className="w-4 h-4" />
            </Button>
          </div>
        </div>
        
        {/* 能力标签 */}
        {'capabilities' in model && model.capabilities && (
          <div className="flex flex-wrap gap-2 mt-4">
            {model.capabilities.map(cap => (
              <span 
                key={cap}
                className="px-2 py-1 bg-cinema-800 text-gray-400 text-xs rounded-lg"
              >
                {cap}
              </span>
            ))}
          </div>
        )}
        
        {'dimensions' in model && (
          <div className="mt-4 text-sm text-gray-500">
            维度: {model.dimensions} · 最大输入: {model.max_input_tokens} tokens
          </div>
        )}
      </CardContent>
    </Card>
  );
}

// 模型添加/编辑模态框
function ModelModal({ 
  type, 
  model,
  onClose,
}: { 
  type: ModelType;
  model: ModelConfig | null;
  onClose: () => void;
}) {
  const defaultValues = {
    name: '',
    provider: 'openai' as LlmProvider,
    model: '',
    api_key: '',
    api_base: '',
    temperature: 0.7,
    max_tokens: 4096,
    dimensions: 1536,
    is_default: false,
    enabled: true,
  };
  
  const { register, handleSubmit, watch, setValue, getValues } = useForm({
    defaultValues: model 
      ? { ...defaultValues, ...model, api_key: model.api_key === '***' ? '' : (model.api_key || '') } 
      : defaultValues
  });
  
  const provider = watch('provider');
  const providers = getModelProviders().filter(p => p.supports.includes(type));
  const defaultModels = getProviderDefaultModels(provider);
  const requiresApiKey = providers.find(p => p.id === provider)?.requiresApiKey ?? true;
  const showApiKeyField = requiresApiKey || provider === 'custom';
  
  const createModelMutation = useCreateModel();
  const updateModelMutation = useUpdateModel();
  const [fetchedModels, setFetchedModels] = useState<string[]>([]);
  const [isFetchingModels, setIsFetchingModels] = useState(false);
  const [fetchModelsError, setFetchModelsError] = useState<string | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);
  const [isRevealing, setIsRevealing] = useState(false);
  
  const onSubmit = (data: any) => {
    const payload: any = {
      name: data.name,
      provider: data.provider,
      model: data.model,
      description: data.description || undefined,
      api_base: data.api_base || undefined,
      model_type: type,
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
    
    if (type === 'chat' || type === 'multimodal') {
      payload.temperature = Number(data.temperature);
      payload.max_tokens = Number(data.max_tokens);
      payload.capabilities = type === 'chat'
        ? ['chat', 'completion', 'long_context']
        : ['chat', 'vision', 'long_context'];
    }
    
    if (type === 'embedding') {
      payload.dimensions = Number(data.dimensions);
    }
    
    if (model) {
      updateModelMutation.mutate(
        { id: model.id, config: payload },
        { onSuccess: onClose }
      );
    } else {
      createModelMutation.mutate(payload as Omit<ModelConfig, 'id'>, { onSuccess: onClose });
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
                    <option key={p.id} value={p.id}>{p.name}</option>
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
                  {defaultModels.map(m => <option key={m} value={m} />)}
                </datalist>
                {fetchedModels.length > 0 && (
                  <div className="mt-2">
                    <label className="text-xs text-gray-500 mb-1 block">检测到以下可用模型：</label>
                    <div className="flex flex-wrap gap-1.5 max-h-28 overflow-y-auto">
                      {fetchedModels.map(m => (
                        <button
                          key={m}
                          type="button"
                          onClick={() => setValue('model', m, { shouldValidate: true })}
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
              <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider">API配置</h3>
              
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
                      <span className="text-green-400">✓ API Key 已设置{showApiKey ? '（明文显示中）' : '，输入新值覆盖'}</span>
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
                      const msg = err instanceof Error ? err.message : String(err);
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
            {(type === 'chat' || type === 'multimodal') && (
              <div className="space-y-4">
                <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider">模型参数</h3>
                
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
                </div>
              </div>
            )}
            
            {type === 'embedding' && (
              <div className="space-y-4">
                <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider">嵌入参数</h3>
                
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
            
            {/* 按钮 */}
            <div className="flex justify-end gap-3 pt-4 border-t border-cinema-800">
              <Button type="button" variant="ghost" onClick={onClose}>
                取消
              </Button>
              <Button type="submit" variant="primary">
                {model ? '保存' : '创建'}
              </Button>
            </div>
          </CardContent>
        </form>
      </Card>
    </div>
  );
}

// Agent配置组件
function AgentConfig() {
  return (
    <Card>
      <CardContent className="p-8 text-center">
        <Bot className="w-16 h-16 text-gray-600 mx-auto mb-4" />
        <h3 className="text-lg font-medium text-white mb-2">Agent模型映射</h3>
        <p className="text-gray-500">为不同的Agent配置专用的LLM模型</p>
        <p className="text-sm text-gray-600 mt-4">功能开发中...</p>
      </CardContent>
    </Card>
  );
}

// 创作方法论配置
function MethodologySettings() {
  const currentStory = useAppStore((s) => s.currentStory);
  const updateStoryMutation = useUpdateStory();
  
  const [methodologyId, setMethodologyId] = useState(currentStory?.methodology_id || '');
  const [methodologyStep, setMethodologyStep] = useState(currentStory?.methodology_step || 1);
  
  useEffect(() => {
    if (currentStory) {
      setMethodologyId(currentStory.methodology_id || '');
      setMethodologyStep(currentStory.methodology_step || 1);
    }
  }, [currentStory?.id]);
  
  const methodologies = [
    { id: '', name: '无（自由创作）', description: '不指定特定方法论，AI 自由发挥' },
    { id: 'snowflake', name: '雪花法', description: '从一句话概括逐步扩展为完整故事，适合 plotter 型作者' },
    { id: 'scene_beat', name: '场景节拍', description: '以场景为单位构建叙事节拍，适合重视节奏的作者' },
    { id: 'hero_journey', name: '英雄之旅', description: '经典三幕式英雄旅程结构，适合史诗/冒险类故事' },
    { id: 'character_depth', name: '人物深度', description: '以人物为核心驱动故事，适合重视角色塑造的作者' },
  ];
  
  const snowflakeSteps = [
    '1. 一句话概括',
    '2. 一段式概括',
    '3. 人物概述',
    '4. 一页纸大纲',
    '5. 人物详细背景',
    '6. 四页纸大纲',
    '7. 人物完整档案',
    '8. 场景清单',
    '9. 场景扩展',
    '10. 初稿写作',
  ];
  
  const handleSave = () => {
    if (!currentStory) return;
    updateStoryMutation.mutate({
      id: currentStory.id,
      updates: {
        methodology_id: methodologyId || undefined,
        methodology_step: methodologyId === 'snowflake' ? methodologyStep : undefined,
      },
    }, {
      onSuccess: () => {
        toast.success('创作方法论已保存');
      },
      onError: (err: any) => {
        toast.error(`保存失败: ${err?.message || String(err)}`);
      },
    });
  };
  
  if (!currentStory) {
    return (
      <Card>
        <CardContent className="p-8 text-center">
          <Compass className="w-16 h-16 text-gray-600 mx-auto mb-4" />
          <h3 className="text-lg font-medium text-white mb-2">创作方法论</h3>
          <p className="text-gray-500">请先选择一个故事，再配置创作方法论</p>
        </CardContent>
      </Card>
    );
  }
  
  return (
    <div className="space-y-6">
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
              <Compass className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h3 className="text-lg font-medium text-white">创作方法论</h3>
              <p className="text-sm text-gray-500">为「{currentStory.title}」选择创作方法论</p>
            </div>
          </div>
          
          <div className="space-y-4">
            <div>
              <label className="block text-sm text-gray-400 mb-2">选择方法论</label>
              <div className="space-y-2">
                {methodologies.map((m) => (
                  <button
                    key={m.id}
                    onClick={() => setMethodologyId(m.id)}
                    className={`w-full p-3 rounded-lg text-left transition-colors border ${
                      methodologyId === m.id
                        ? 'bg-cinema-gold/20 border-cinema-gold/50'
                        : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                    }`}
                  >
                    <div className="font-medium text-white">{m.name}</div>
                    <div className="text-xs text-gray-400 mt-0.5">{m.description}</div>
                  </button>
                ))}
              </div>
            </div>
            
            {methodologyId === 'snowflake' && (
              <div>
                <label className="block text-sm text-gray-400 mb-2">当前步骤（雪花法）</label>
                <div className="space-y-1.5">
                  {snowflakeSteps.map((step, idx) => (
                    <button
                      key={idx}
                      onClick={() => setMethodologyStep(idx + 1)}
                      className={`w-full p-2 rounded-lg text-left text-sm transition-colors ${
                        methodologyStep === idx + 1
                          ? 'bg-cinema-gold/20 text-cinema-gold'
                          : 'bg-cinema-800 text-gray-400 hover:bg-cinema-700'
                      }`}
                    >
                      {step}
                    </button>
                  ))}
                </div>
              </div>
            )}
            
            <div className="flex justify-end pt-4 border-t border-cinema-800">
              <Button 
                variant="primary" 
                onClick={handleSave}
                isLoading={updateStoryMutation.isPending}
              >
                <Check className="w-4 h-4 mr-2" />
                保存
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

// 颜色主题选择器组件
function ColorThemeSelector() {
  const [currentTheme, setCurrentTheme] = useState<ColorThemeId>(() => loadColorTheme());

  const handleSelect = (themeId: ColorThemeId) => {
    setCurrentTheme(themeId);
    applyColorTheme(themeId);
    localStorage.setItem('storyforge-color-theme', themeId);
  };

  return (
    <div className="space-y-3">
      <label className="block text-sm text-gray-400">颜色主题</label>
      <div className="flex flex-wrap gap-3">
        {colorThemeList.map((theme) => (
          <button
            key={theme.id}
            onClick={() => handleSelect(theme.id)}
            className={cn(
              'flex items-center gap-2 px-4 py-2.5 rounded-xl border-2 transition-all',
              currentTheme === theme.id
                ? 'border-cinema-gold bg-cinema-gold/10'
                : 'border-cinema-700 bg-cinema-800/50 hover:border-cinema-600'
            )}
            title={theme.description}
          >
            <div
              className="w-5 h-5 rounded-full border border-white/10"
              style={{ backgroundColor: theme.terracotta }}
            />
            <span className="text-sm text-white">{theme.name}</span>
          </button>
        ))}
      </div>
      <p className="text-xs text-gray-500">选择后即时生效，同步影响幕前写作界面</p>
    </div>
  );
}

// 通用设置组件
function GeneralSettings() {
  const { 
    currentVersion, 
    hasUpdate, 
    latestVersion, 
    isChecking, 
    isInstalling,
    checkUpdate, 
    installUpdate 
  } = useUpdater(false);
  
  const { data: settings } = useSettings();
  const saveSettingsMutation = useSaveSettings();
  const [concurrency, setConcurrency] = useState(settings?.book_deconstruction_concurrency ?? 3);
  const [rewriteThreshold, setRewriteThreshold] = useState(settings?.rewrite_threshold ?? 0.75);
  const [maxFeedbackLoops, setMaxFeedbackLoops] = useState(settings?.max_feedback_loops ?? 2);
  const [writingStrategy, setWritingStrategy] = useState(settings?.writing_strategy ?? { run_mode: 'fast' as const, conflict_level: 50, pace: 'balanced' as const, ai_freedom: 'medium' as const });
  
  // 同步设置值
  useEffect(() => {
    if (settings?.book_deconstruction_concurrency !== undefined) {
      setConcurrency(settings.book_deconstruction_concurrency);
    }
    if (settings?.rewrite_threshold !== undefined) {
      setRewriteThreshold(settings.rewrite_threshold);
    }
    if (settings?.max_feedback_loops !== undefined) {
      setMaxFeedbackLoops(settings.max_feedback_loops);
    }
    if (settings?.writing_strategy !== undefined) {
      setWritingStrategy(settings.writing_strategy);
    }
  }, [settings?.book_deconstruction_concurrency, settings?.rewrite_threshold, settings?.max_feedback_loops, settings?.writing_strategy]);
  
  const handleConcurrencyChange = (value: number) => {
    setConcurrency(value);
    // 防抖保存：300ms 后保存
    const timer = setTimeout(() => {
      if (settings) {
        saveSettingsMutation.mutate({
          ...settings,
          book_deconstruction_concurrency: value,
        });
      }
    }, 300);
    return () => clearTimeout(timer);
  };

  const handleRewriteThresholdChange = (value: number) => {
    setRewriteThreshold(value);
    const timer = setTimeout(() => {
      if (settings) {
        saveSettingsMutation.mutate({
          ...settings,
          rewrite_threshold: value,
        });
      }
    }, 300);
    return () => clearTimeout(timer);
  };

  const handleMaxFeedbackLoopsChange = (value: number) => {
    setMaxFeedbackLoops(value);
    const timer = setTimeout(() => {
      if (settings) {
        saveSettingsMutation.mutate({
          ...settings,
          max_feedback_loops: value,
        });
      }
    }, 300);
    return () => clearTimeout(timer);
  };

  const handleWritingStrategyChange = (partial: Partial<typeof writingStrategy>) => {
    const next = { ...writingStrategy, ...partial };
    setWritingStrategy(next);
    const timer = setTimeout(() => {
      if (settings) {
        saveSettingsMutation.mutate({
          ...settings,
          writing_strategy: next,
        });
      }
    }, 300);
    return () => clearTimeout(timer);
  };

  return (
    <div className="space-y-6">
      {/* 版本信息 */}
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <div className="w-16 h-16 rounded-xl bg-gradient-to-br from-terracotta to-terracotta/60 flex items-center justify-center">
                <span className="text-white font-serif text-2xl font-bold">草</span>
              </div>
              <div>
                <h3 className="text-lg font-medium text-white">StoryForge (草苔)</h3>
                <p className="text-gray-400">当前版本: v{currentVersion}</p>
                {hasUpdate && (
                  <p className="text-terracotta text-sm">
                    新版本可用: v{latestVersion}
                  </p>
                )}
              </div>
            </div>
            <div className="flex gap-2">
              {hasUpdate ? (
                <Button 
                  variant="primary" 
                  onClick={installUpdate}
                  disabled={isInstalling}
                >
                  {isInstalling ? (
                    <>
                      <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                      安装中...
                    </>
                  ) : (
                    <>
                      <Download className="w-4 h-4 mr-2" />
                      立即更新
                    </>
                  )}
                </Button>
              ) : (
                <Button 
                  variant="secondary" 
                  onClick={checkUpdate}
                  disabled={isChecking}
                >
                  {isChecking ? (
                    <>
                      <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                      检查中...
                    </>
                  ) : (
                    <>
                      <RefreshCw className="w-4 h-4 mr-2" />
                      检查更新
                    </>
                  )}
                </Button>
              )}
            </div>
          </div>
        </CardContent>
      </Card>

      {/* 拆书分析并发设置 */}
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
              <BookOpen className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h3 className="text-lg font-medium text-white">拆书分析设置</h3>
              <p className="text-sm text-gray-500">调整拆书时的 LLM 并发数，本地模型可调大以加速分析</p>
            </div>
          </div>
          
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Zap className="w-4 h-4 text-cinema-gold" />
                <span className="text-sm text-white">LLM 并发数</span>
              </div>
              <span className="text-lg font-bold text-cinema-gold font-mono">{concurrency}</span>
            </div>
            
            <div className="flex items-center gap-4">
              <span className="text-xs text-gray-500 w-8">1</span>
              <input
                type="range"
                min={1}
                max={50}
                value={concurrency}
                onChange={(e) => handleConcurrencyChange(Number(e.target.value))}
                className="flex-1 h-2 bg-cinema-800 rounded-lg appearance-none cursor-pointer accent-cinema-gold"
              />
              <span className="text-xs text-gray-500 w-8">50</span>
            </div>
            
            <div className="flex items-center justify-between text-xs text-gray-500">
              <span>保守（慢但稳）</span>
              <span>激进（快但占用资源）</span>
            </div>
            
            <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
              <p className="text-xs text-gray-400">
                <span className="text-cinema-gold font-medium">提示：</span>
                远程 API 建议 1~5，本地模型（Ollama/vLLM）建议 10~50。
                当前设置会在下次拆书时生效。
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Agent 配置 */}
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
              <Bot className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h3 className="text-lg font-medium text-white">Agent 质检配置</h3>
              <p className="text-sm text-gray-500">调整 Writer → Inspector 闭环优化的质检严格度</p>
            </div>
          </div>
          
          <div className="space-y-6">
            {/* 质检阈值 */}
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Sparkles className="w-4 h-4 text-cinema-gold" />
                  <span className="text-sm text-white">质检阈值</span>
                </div>
                <span className="text-lg font-bold text-cinema-gold font-mono">{rewriteThreshold.toFixed(2)}</span>
              </div>
              
              <div className="flex items-center gap-4">
                <span className="text-xs text-gray-500 w-8">0.6</span>
                <input
                  type="range"
                  min={0.6}
                  max={0.9}
                  step={0.05}
                  value={rewriteThreshold}
                  onChange={(e) => handleRewriteThresholdChange(Number(e.target.value))}
                  className="flex-1 h-2 bg-cinema-800 rounded-lg appearance-none cursor-pointer accent-cinema-gold"
                />
                <span className="text-xs text-gray-500 w-8">0.9</span>
              </div>
              
              <div className="flex items-center justify-between text-xs text-gray-500">
                <span>宽松（易通过，改写少）</span>
                <span>严格（难通过，改写多）</span>
              </div>
              
              <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
                <p className="text-xs text-gray-400">
                  <span className="text-cinema-gold font-medium">提示：</span>
                  低于此阈值的文本将触发 Writer 自动改写。默认 0.75 是平衡点。
                </p>
              </div>
            </div>

            {/* 最大循环次数 */}
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <RefreshCw className="w-4 h-4 text-cinema-gold" />
                  <span className="text-sm text-white">最大改写轮数</span>
                </div>
                <span className="text-lg font-bold text-cinema-gold font-mono">{maxFeedbackLoops}</span>
              </div>
              
              <div className="flex items-center gap-4">
                <span className="text-xs text-gray-500 w-8">1</span>
                <input
                  type="range"
                  min={1}
                  max={5}
                  step={1}
                  value={maxFeedbackLoops}
                  onChange={(e) => handleMaxFeedbackLoopsChange(Number(e.target.value))}
                  className="flex-1 h-2 bg-cinema-800 rounded-lg appearance-none cursor-pointer accent-cinema-gold"
                />
                <span className="text-xs text-gray-500 w-8">5</span>
              </div>
              
              <div className="flex items-center justify-between text-xs text-gray-500">
                <span>快速（1轮）</span>
                <span>深度（5轮）</span>
              </div>
              
              <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
                <p className="text-xs text-gray-400">
                  <span className="text-cinema-gold font-medium">提示：</span>
                  每轮 Inspector 质检不通过都会触发 Writer 改写。轮数越多质量越高但耗时越长。
                </p>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* 写作策略 */}
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
              <PenTool className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h3 className="text-lg font-medium text-white">写作策略</h3>
              <p className="text-sm text-gray-500">调整 AI 生成内容的行为倾向</p>
            </div>
          </div>
          
          <div className="space-y-6">
            {/* 运行模式 */}
            <div>
              <label className="block text-sm text-gray-400 mb-2">运行模式</label>
              <div className="grid grid-cols-2 gap-3">
                <button
                  onClick={() => handleWritingStrategyChange({ run_mode: 'fast' })}
                  className={`p-3 rounded-lg text-left transition-colors border ${
                    writingStrategy.run_mode === 'fast'
                      ? 'bg-cinema-gold/20 border-cinema-gold/50'
                      : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                  }`}
                >
                  <div className="font-medium text-white">快速</div>
                  <div className="text-xs text-gray-400 mt-0.5">高 temperature，注重效率</div>
                </button>
                <button
                  onClick={() => handleWritingStrategyChange({ run_mode: 'polish' })}
                  className={`p-3 rounded-lg text-left transition-colors border ${
                    writingStrategy.run_mode === 'polish'
                      ? 'bg-cinema-gold/20 border-cinema-gold/50'
                      : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                  }`}
                >
                  <div className="font-medium text-white">精修</div>
                  <div className="text-xs text-gray-400 mt-0.5">低 temperature，注重质量</div>
                </button>
              </div>
            </div>

            {/* 冲突强度 */}
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Zap className="w-4 h-4 text-cinema-gold" />
                  <span className="text-sm text-white">冲突强度</span>
                </div>
                <span className="text-lg font-bold text-cinema-gold font-mono">{writingStrategy.conflict_level}</span>
              </div>
              <div className="flex items-center gap-4">
                <span className="text-xs text-gray-500 w-8">0</span>
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={writingStrategy.conflict_level}
                  onChange={(e) => handleWritingStrategyChange({ conflict_level: Number(e.target.value) })}
                  className="flex-1 h-2 bg-cinema-800 rounded-lg appearance-none cursor-pointer accent-cinema-gold"
                />
                <span className="text-xs text-gray-500 w-8">100</span>
              </div>
              <div className="flex items-center justify-between text-xs text-gray-500">
                <span>平和抒情</span>
                <span>激烈冲突</span>
              </div>
              {writingStrategy.conflict_level >= 80 && (
                <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
                  <p className="text-xs text-gray-400">
                    <span className="text-cinema-gold font-medium">提示：</span>
                    冲突强度 ≥ 80 时，AI 会确保每 500 字至少安排一次冲突或张力。
                  </p>
                </div>
              )}
            </div>

            {/* 叙事节奏 */}
            <div>
              <label className="block text-sm text-gray-400 mb-2">叙事节奏</label>
              <div className="grid grid-cols-3 gap-3">
                {[
                  { id: 'slow', label: '慢', desc: '细腻描写' },
                  { id: 'balanced', label: '均衡', desc: '动作描写交替' },
                  { id: 'fast', label: '快', desc: '快速推进' },
                ].map((opt) => (
                  <button
                    key={opt.id}
                    onClick={() => handleWritingStrategyChange({ pace: opt.id as typeof writingStrategy.pace })}
                    className={`p-3 rounded-lg text-left transition-colors border ${
                      writingStrategy.pace === opt.id
                        ? 'bg-cinema-gold/20 border-cinema-gold/50'
                        : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                    }`}
                  >
                    <div className="font-medium text-white">{opt.label}</div>
                    <div className="text-xs text-gray-400 mt-0.5">{opt.desc}</div>
                  </button>
                ))}
              </div>
            </div>

            {/* AI 自由度 */}
            <div>
              <label className="block text-sm text-gray-400 mb-2">AI 自由度</label>
              <div className="grid grid-cols-3 gap-3">
                {[
                  { id: 'low', label: '低', desc: '严格遵循设定' },
                  { id: 'medium', label: '中', desc: '核心约束+发挥' },
                  { id: 'high', label: '高', desc: '允许创新转折' },
                ].map((opt) => (
                  <button
                    key={opt.id}
                    onClick={() => handleWritingStrategyChange({ ai_freedom: opt.id as typeof writingStrategy.ai_freedom })}
                    className={`p-3 rounded-lg text-left transition-colors border ${
                      writingStrategy.ai_freedom === opt.id
                        ? 'bg-cinema-gold/20 border-cinema-gold/50'
                        : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                    }`}
                  >
                    <div className="font-medium text-white">{opt.label}</div>
                    <div className="text-xs text-gray-400 mt-0.5">{opt.desc}</div>
                  </button>
                ))}
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* 编辑器设置 */}
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
              <Settings2 className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h3 className="text-lg font-medium text-white">编辑器设置</h3>
              <p className="text-sm text-gray-500">幕前写作界面的字体、风格等配置</p>
            </div>
          </div>
          <EditorSettings />
        </CardContent>
      </Card>

      {/* 颜色主题 */}
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
              <Settings2 className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h3 className="text-lg font-medium text-white">颜色主题</h3>
              <p className="text-sm text-gray-500">幕前写作界面的冷暖撞色色调</p>
            </div>
          </div>
          <ColorThemeSelector />
        </CardContent>
      </Card>
    </div>
  );
}


// ==================== AccountSettings (v4.5.0) ====================

function AccountSettings() {
  const { user, isLoggedIn, logout } = useAuthStore();
  const [authConfig, setAuthConfig] = useState<{ google_enabled: boolean; github_enabled: boolean; wechat_enabled: boolean; qq_enabled: boolean } | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    loadAuthConfig();
  }, []);

  const loadAuthConfig = async () => {
    try {
      const config = await import('@/services/auth').then(m => m.getAuthConfig());
      setAuthConfig(config);
    } catch (e) {
      console.error('Failed to load auth config:', e);
    }
  };

  const handleLogout = async () => {
    setIsLoading(true);
    try {
      await logout();
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* 登录状态卡片 */}
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-4">
            <div className="w-14 h-14 rounded-full bg-cinema-800 flex items-center justify-center">
              {user?.avatar_url ? (
                <img src={user.avatar_url} alt="" className="w-14 h-14 rounded-full object-cover" />
              ) : (
                <User className="w-7 h-7 text-gray-400" />
              )}
            </div>
            <div className="flex-1">
              {isLoggedIn && user ? (
                <>
                  <h3 className="text-lg font-medium text-white">
                    {user.display_name || '已登录用户'}
                  </h3>
                  <p className="text-sm text-gray-400">{user.email || ''}</p>
                  <p className="text-xs text-green-500 mt-1 flex items-center gap-1">
                    <Shield className="w-3 h-3" />
                    已登录
                  </p>
                </>
              ) : (
                <>
                  <h3 className="text-lg font-medium text-white">未登录</h3>
                  <p className="text-sm text-gray-400">登录后可使用云同步等跨设备功能</p>
                </>
              )}
            </div>
            {isLoggedIn ? (
              <button
                onClick={handleLogout}
                disabled={isLoading}
                className="px-4 py-2 bg-red-500/10 text-red-400 rounded-lg hover:bg-red-500/20 transition-colors text-sm disabled:opacity-50"
              >
                {isLoading ? '退出中...' : '退出登录'}
              </button>
            ) : (
              <button
                onClick={() => window.dispatchEvent(new CustomEvent('show-login-modal'))}
                className="px-4 py-2 bg-cinema-gold text-cinema-900 rounded-lg hover:bg-cinema-gold-light transition-colors text-sm font-medium"
              >
                登录
              </button>
            )}
          </div>
        </CardContent>
      </Card>

      {/* OAuth 配置状态 */}
      <Card>
        <CardContent className="p-6">
          <h3 className="text-lg font-medium text-white mb-4 flex items-center gap-2">
            <Link2 className="w-5 h-5 text-cinema-gold" />
            OAuth 登录选项
          </h3>
          <div className="space-y-3">
            <ProviderStatus
              name="Google"
              enabled={authConfig?.google_enabled || false}
              icon={<span className="text-blue-400 font-medium text-sm">G</span>}
            />
            <ProviderStatus
              name="GitHub"
              enabled={authConfig?.github_enabled || false}
              icon={<span className="text-white font-medium text-sm">H</span>}
            />
            <ProviderStatus
              name="微信"
              enabled={authConfig?.wechat_enabled || false}
              icon={<span className="text-green-400 font-medium text-sm">W</span>}
            />
            <ProviderStatus
              name="QQ"
              enabled={authConfig?.qq_enabled || false}
              icon={<span className="text-blue-300 font-medium text-sm">Q</span>}
            />
          </div>
          <p className="text-xs text-gray-500 mt-4">
            在配置文件中设置 OAuth 客户端 ID 后，对应登录选项将自动启用。
            微信/QQ 登录需要在中国内地开放平台注册应用。
          </p>
        </CardContent>
      </Card>
    </div>
  );
}

function WorkflowSettings() {
  const { data: workflows = [], isLoading } = useWorkflows();
  const reload = useReloadWorkflows();

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="font-display text-lg font-bold text-white">工作流模板</h3>
          <p className="text-sm text-gray-400">从文件系统自动加载的工作流定义（JSON/YAML）</p>
        </div>
        <Button
          variant="secondary"
          onClick={() => reload.mutate()}
          isLoading={reload.isPending}
          className="gap-2"
        >
          <RefreshCw className="w-4 h-4" />
          重新加载
        </Button>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="w-6 h-6 text-cinema-gold animate-spin" />
        </div>
      ) : workflows.length === 0 ? (
        <div className="text-center py-12 text-gray-500">
          <GitBranch className="w-8 h-8 mx-auto mb-3 opacity-50" />
          <p>暂无工作流模板</p>
          <p className="text-sm mt-1">在应用数据目录 workflows/ 文件夹中放入 .json 或 .yaml 文件即可自动加载</p>
        </div>
      ) : (
        <div className="space-y-3">
          {workflows.map((wf) => (
            <Card key={wf.id} className="bg-cinema-900/50 border-cinema-800">
              <CardContent className="p-4">
                <div className="flex items-start justify-between gap-4">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <h4 className="font-medium text-white">{wf.name}</h4>
                      {wf.is_builtin && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-cinema-700 text-gray-400">内置</span>
                      )}
                    </div>
                    <p className="text-sm text-gray-400">{wf.description}</p>
                    <div className="flex items-center gap-4 mt-2 text-xs text-gray-500">
                      <span className="flex items-center gap-1">
                        <GitCommit className="w-3 h-3" />
                        {wf.nodes.length} 个节点
                      </span>
                      <span className="flex items-center gap-1">
                        <ArrowRight className="w-3 h-3" />
                        {wf.edges.length} 条边
                      </span>
                      <span>ID: {wf.id}</span>
                    </div>
                  </div>
                </div>

                {/* Node list */}
                <div className="mt-3 pt-3 border-t border-cinema-800">
                  <div className="flex flex-wrap gap-2">
                    {wf.nodes.map((node) => (
                      <span
                        key={node.id}
                        className={`text-xs px-2 py-1 rounded border ${
                          node.node_type === 'Start' || node.node_type === 'End'
                            ? 'bg-cinema-800 border-cinema-700 text-gray-400'
                            : 'bg-cinema-gold/5 border-cinema-gold/20 text-cinema-gold'
                        }`}
                      >
                        {node.name}
                      </span>
                    ))}
                  </div>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

function ProviderStatus({ name, enabled, icon }: { name: string; enabled: boolean; icon: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between py-2 px-3 rounded-lg bg-cinema-800/30">
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 rounded-lg bg-cinema-800 flex items-center justify-center">
          {icon}
        </div>
        <span className="text-sm text-gray-300">{name}</span>
      </div>
      <span className={`text-xs px-2 py-0.5 rounded-full ${enabled ? 'bg-green-500/10 text-green-400' : 'bg-gray-700 text-gray-500'}`}>
        {enabled ? '已启用' : '未配置'}
      </span>
    </div>
  );
}
