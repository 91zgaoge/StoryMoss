import { confirmDialog } from '@/components/ui/confirm';
import { useState, useEffect, useMemo, useCallback } from 'react';
import {
  Plus,
  Database,
  MessageSquare,
  Sparkles,
  Image,
  Filter,
  Cpu,
  Paintbrush,
  Wrench,
  Settings,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input, Select } from '@/components/ui/controls';
import { useSettingsContext } from '@/hooks/useSettingsContext';
import { useModelConnectionStore } from '@/stores/modelConnectionStore';
import { saveSettings } from '@/services/settings';
import type { ModelConfig, ModelType, AppSettings } from '@/types/llm';
import { cn } from '@/utils/cn';
import { useQueryClient } from '@tanstack/react-query';

import { ModelList } from './ModelList';
import { ModelModal } from './ModelModal';

type FilterType = 'all' | ModelType;

const TYPE_CONFIG: Record<ModelType, { label: string; icon: React.ReactNode; color: string }> = {
  chat: { label: '聊天模型', icon: <MessageSquare className="w-4 h-4" />, color: 'text-blue-400' },
  embedding: {
    label: '嵌入模型',
    icon: <Database className="w-4 h-4" />,
    color: 'text-purple-400',
  },
  multimodal: { label: '多模态', icon: <Sparkles className="w-4 h-4" />, color: 'text-amber-400' },
  image: { label: '图像生成', icon: <Image className="w-4 h-4" />, color: 'text-pink-400' },
};

export function UnifiedModelManager() {
  const [filter, setFilter] = useState<FilterType>('all');
  const [showAddModal, setShowAddModal] = useState(false);
  const [editingModel, setEditingModel] = useState<ModelConfig | null>(null);
  const [addModelType, setAddModelType] = useState<ModelType>('chat');

  const { models, settings, isLoading, setActiveModel, deleteModel, updateModel } =
    useSettingsContext();
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [togglingEnabledId, setTogglingEnabledId] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const activeModelIds = settings?.active_models ?? {};

  // v0.23.66: 温度覆盖保存
  const handleSetTemperature = useCallback(
    async (field: string, value: number | undefined) => {
      if (!settings) return;
      const updated: AppSettings = { ...settings, [field]: value };
      await saveSettings(updated);
      queryClient.invalidateQueries({ queryKey: ['settings'] });
    },
    [settings, queryClient]
  );

  // v0.31.0: 续写参数（计划模式 / 目标字数）保存 — 复用温度的 patch 保存路径
  const handleSaveContinuationSetting = useCallback(
    async (
      field: 'plan_mode' | 'continuation_target_words',
      value: string | number | undefined
    ) => {
      if (!settings) return;
      const updated: AppSettings = { ...settings, [field]: value };
      await saveSettings(updated);
      queryClient.invalidateQueries({ queryKey: ['settings'] });
    },
    [settings, queryClient]
  );

  // 模型连接状态管理
  const { states, checkModels, checkModel } = useModelConnectionStore();

  // 自动检测已启用模型连接（禁用模型不探测）
  useEffect(() => {
    if (!models.length) return;
    const modelIds = models.filter(m => m.enabled).map(m => m.id);
    if (modelIds.length === 0) return;
    checkModels(modelIds);
    // 每30秒轮询
    const interval = setInterval(() => {
      checkModels(modelIds);
    }, 30000);
    return () => clearInterval(interval);
  }, [models, checkModels]);

  // 按类型分组的模型
  const groupedModels = useMemo(() => {
    const groups: Record<ModelType, ModelConfig[]> = {
      chat: [],
      embedding: [],
      multimodal: [],
      image: [],
    };
    for (const model of models) {
      if (groups[model.type]) {
        groups[model.type].push(model);
      }
    }
    return groups;
  }, [models]);

  // 筛选后的类型列表
  const visibleTypes = useMemo(() => {
    if (filter !== 'all') return [filter];
    return (['chat', 'embedding', 'multimodal', 'image'] as ModelType[]).filter(
      type => groupedModels[type].length > 0
    );
  }, [filter, groupedModels]);

  // 处理添加模型
  const handleAdd = (type: ModelType) => {
    setAddModelType(type);
    setShowAddModal(true);
  };

  // 处理重试连接检测
  const handleRetry = (modelId: string) => {
    checkModel(modelId);
  };

  // 处理删除模型
  const handleDelete = async (modelId: string) => {
    if (
      !(await confirmDialog({
        title: '确定要删除该模型配置吗？如果该模型正被 Agent 使用，对应映射会被自动清除。',
      }))
    ) {
      return;
    }
    setDeletingId(modelId);
    try {
      await deleteModel(modelId);
    } finally {
      setDeletingId(null);
    }
  };

  // v0.26.55: 列表页开启/关闭 — 禁用后后端 fail-closed 回退活跃/角色并跳过探测
  const handleToggleEnabled = async (model: ModelConfig) => {
    const next = !model.enabled;
    setTogglingEnabledId(model.id);
    try {
      // 后端 ModelConfigInput 需要 model_type（非前端 type 字段）
      await updateModel(model.id, {
        name: model.name,
        provider: model.provider,
        model: model.model,
        model_type: model.type,
        enabled: next,
        is_default: !!model.is_default,
        description: model.description,
        api_base: model.api_base,
        ...('temperature' in model ? { temperature: model.temperature } : {}),
        ...('max_tokens' in model ? { max_tokens: model.max_tokens } : {}),
        ...('capabilities' in model ? { capabilities: model.capabilities } : {}),
        ...('dimensions' in model ? { dimensions: model.dimensions } : {}),
      } as Partial<ModelConfig> & { model_type: ModelType });
    } finally {
      setTogglingEnabledId(null);
    }
  };

  if (isLoading) {
    return <div className="text-center py-12 text-gray-500">加载模型配置...</div>;
  }

  return (
    <div className="space-y-6">
      {/* 头部：标题 + 筛选器 */}
      <div className="flex items-center justify-between flex-wrap gap-4">
        <div className="flex items-center gap-3">
          <Cpu className="w-6 h-6 text-cinema-gold" />
          <div>
            <h2 className="text-xl font-semibold text-white">模型管理</h2>
            <p className="text-sm text-gray-500">
              共 {models.length} 个模型配置
              {models.length > 0 && (
                <span className="ml-2">
                  ({groupedModels.chat.length} 聊天 / {groupedModels.embedding.length} 嵌入 /{' '}
                  {groupedModels.multimodal.length} 多模态 / {groupedModels.image.length} 图像)
                </span>
              )}
            </p>
          </div>
        </div>

        {/* 筛选器 */}
        <div className="flex items-center gap-2">
          <Filter className="w-4 h-4 text-gray-500" />
          <div className="flex items-center gap-1">
            <FilterButton active={filter === 'all'} onClick={() => setFilter('all')} label="全部" />
            {(['chat', 'embedding', 'multimodal', 'image'] as ModelType[]).map(type => (
              <FilterButton
                key={type}
                active={filter === type}
                onClick={() => setFilter(type)}
                label={TYPE_CONFIG[type].label}
                icon={TYPE_CONFIG[type].icon}
                count={groupedModels[type].length}
              />
            ))}
          </div>
        </div>
      </div>

      {/* v0.23.66: 模型角色分配卡片 */}
      {models.length > 0 && (
        <ModelRoleCard
          models={models}
          activeModels={activeModelIds}
          onSetRole={(modelId, role) => setActiveModel('chat', modelId, role)}
          creativeTemp={settings?.creative_temperature}
          continuationTemp={settings?.continuation_temperature}
          toolTemp={settings?.tool_temperature}
          onSetTemperature={handleSetTemperature}
          planMode={settings?.plan_mode}
          continuationTargetWords={settings?.continuation_target_words}
          onSaveContinuationSetting={handleSaveContinuationSetting}
        />
      )}

      {/* 模型列表 - 按类型分组 */}
      {models.length === 0 ? (
        <Card>
          <CardContent className="p-12 text-center">
            <Database className="w-16 h-16 text-gray-600 mx-auto mb-4" />
            <h3 className="text-lg font-medium text-white mb-2">暂无模型配置</h3>
            <Button variant="primary" onClick={() => handleAdd('chat')}>
              <Plus className="w-4 h-4 mr-2" />
              添加模型
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-8">
          {visibleTypes.map(type => (
            <div key={type} className="space-y-3">
              {/* 类型标题 */}
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      'flex items-center gap-1.5 text-sm font-medium',
                      TYPE_CONFIG[type].color
                    )}
                  >
                    {TYPE_CONFIG[type].icon}
                    {TYPE_CONFIG[type].label}
                  </span>
                  <span className="text-xs text-gray-600 bg-cinema-800 px-2 py-0.5 rounded-full">
                    {groupedModels[type].length}
                  </span>
                </div>
                <Button variant="ghost" size="sm" onClick={() => handleAdd(type)}>
                  <Plus className="w-3.5 h-3.5 mr-1" />
                  添加
                  {type === 'chat'
                    ? '聊天'
                    : type === 'embedding'
                      ? '嵌入'
                      : type === 'multimodal'
                        ? '多模态'
                        : '图像'}
                  模型
                </Button>
              </div>

              {/* 模型列表 */}
              <ModelList
                type={type}
                models={groupedModels[type]}
                activeModelId={activeModelIds[type]}
                connectionStatus={states}
                onAdd={() => handleAdd(type)}
                onEdit={setEditingModel}
                onSetActive={modelId => {
                  setActiveModel(type, modelId);
                }}
                onRetry={handleRetry}
                onDelete={handleDelete}
                onToggleEnabled={handleToggleEnabled}
                togglingEnabledId={togglingEnabledId}
                deletingId={deletingId}
                showTypeHeader={false}
                activeModels={activeModelIds}
              />
            </div>
          ))}
        </div>
      )}

      {/* 添加/编辑模态框 */}
      {(showAddModal || editingModel) && (
        <ModelModal
          type={editingModel ? editingModel.type : addModelType}
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

function FilterButton({
  active,
  onClick,
  label,
  icon,
  count,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  icon?: React.ReactNode;
  count?: number;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors',
        active ? 'bg-cinema-gold text-black' : 'text-gray-400 hover:text-white hover:bg-cinema-800'
      )}
    >
      {icon}
      {label}
      {count !== undefined && (
        <span className={cn('ml-0.5', active ? 'text-black/60' : 'text-gray-600')}>{count}</span>
      )}
    </button>
  );
}

// ============================================================================
// v0.23.66: 模型角色分配卡片
// ============================================================================

type ModelRoleKey = 'creative' | 'tool' | 'background';

const ROLE_CONFIG: Record<
  ModelRoleKey,
  { label: string; desc: string; icon: React.ReactNode; color: string }
> = {
  creative: {
    label: '创作模型',
    desc: '主创续写与正文 — 质量优先。不要给管理和回流用。',
    icon: <Paintbrush className="w-4 h-4" />,
    color: 'text-amber-400',
  },
  tool: {
    label: '工具模型',
    desc: '管理 Agent、意图分类、JSON — 速度优先，须与创作模型分开。',
    icon: <Wrench className="w-4 h-4" />,
    color: 'text-blue-400',
  },
  background: {
    label: '后台任务模型',
    desc: '编辑审计、资产回流、后台入库 — 不得占用创作模型。',
    icon: <Settings className="w-4 h-4" />,
    color: 'text-purple-400',
  },
};

function ModelRoleCard({
  models,
  activeModels,
  onSetRole,
  creativeTemp,
  continuationTemp,
  toolTemp,
  onSetTemperature,
  planMode: _planMode,
  continuationTargetWords,
  onSaveContinuationSetting,
}: {
  models: ModelConfig[];
  activeModels: Record<string, string | undefined>;
  onSetRole: (modelId: string, role: ModelRoleKey) => void;
  creativeTemp?: number;
  continuationTemp?: number;
  toolTemp?: number;
  onSetTemperature: (field: string, value: number | undefined) => void;
  planMode?: 'beat' | 'single_writer';
  continuationTargetWords?: number;
  onSaveContinuationSetting: (
    field: 'plan_mode' | 'continuation_target_words',
    value: string | number | undefined
  ) => void;
}) {
  // 创作/工具/后台角色：chat 和 multimodal 模型都可以用于文本生成
  const eligibleModels = models.filter(m => m.type === 'chat' || m.type === 'multimodal');

  // 角色→温度字段映射
  const tempMap: Record<string, { key: string; value?: number; label: string }> = {
    creative: { key: 'creative_temperature', value: creativeTemp, label: '创世温度' },
    tool: { key: 'tool_temperature', value: toolTemp, label: '工具温度' },
    background: { key: 'continuation_temperature', value: continuationTemp, label: '续写温度' },
  };

  return (
    <Card className="@container" data-testid="model-role-card">
      <CardContent className="p-4 sm:p-5">
        <h3 className="font-body text-base font-semibold text-ai-ink">模型角色分配</h3>
        <p className="mb-4 mt-1 text-xs leading-5 text-ai-ink-3">
          三角色对应这三档：主创→创作，管理→工具，编辑审计与资产回流→后台。未设置时网关自动分配。
        </p>
        <div className="space-y-3">
          {(Object.entries(ROLE_CONFIG) as [ModelRoleKey, (typeof ROLE_CONFIG)['creative']][]).map(
            ([role, config]) => {
              const currentId = activeModels[role];
              const currentModel = eligibleModels.find(m => m.id === currentId);
              return (
                <div
                  key={role}
                  data-testid={`model-role-${role}`}
                  className="grid grid-cols-1 gap-4 rounded-xl border border-borderSubtle bg-cinema-900/45 p-4 @5xl:grid-cols-[minmax(180px,0.75fr)_minmax(0,1.25fr)] @5xl:items-center"
                >
                  <div className="min-w-0">
                    <span
                      className={cn(
                        'flex items-center gap-2 text-sm font-medium whitespace-nowrap',
                        config.color
                      )}
                    >
                      {config.icon}
                      {config.label}
                    </span>
                    <p className="mt-1.5 text-xs leading-5 text-ai-ink-3">{config.desc}</p>
                  </div>
                  <div className="grid min-w-0 grid-cols-1 items-center gap-3 @md:grid-cols-[minmax(0,1fr)_auto]">
                    <Select
                      aria-label={`${config.label}模型`}
                      className="w-full min-w-0"
                      value={currentId || ''}
                      onChange={e => {
                        if (e.target.value) {
                          onSetRole(e.target.value, role);
                        }
                      }}
                    >
                      <option value="">自动分配（网关判断）</option>
                      {eligibleModels.map(m => (
                        <option key={m.id} value={m.id}>
                          {m.name}
                        </option>
                      ))}
                    </Select>
                    <div className="flex min-w-0 flex-wrap items-center gap-2">
                      {tempMap[role] && (
                        <>
                          <span className="whitespace-nowrap text-xs text-ai-ink-3">
                            {tempMap[role].label}
                          </span>
                          <Input
                            aria-label={tempMap[role].label}
                            type="number"
                            className="w-20 shrink-0 text-center"
                            min={0.1}
                            max={2.0}
                            step={0.1}
                            placeholder="默认"
                            value={tempMap[role].value ?? ''}
                            onChange={e => {
                              const v = e.target.value;
                              onSetTemperature(
                                tempMap[role].key,
                                v === '' ? undefined : parseFloat(v)
                              );
                            }}
                          />
                        </>
                      )}
                      {currentModel && (
                        <span className="max-w-40 truncate whitespace-nowrap rounded border border-cinema-gold/20 bg-cinema-gold/10 px-2 py-1 text-xs text-cinema-gold">
                          {currentModel.name.length > 15
                            ? currentModel.name.slice(0, 15) + '...'
                            : currentModel.name}
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              );
            }
          )}
        </div>

        {/* v0.31.0: 续写参数（计划模式 / 目标字数） */}
        <div className="mt-3 pt-3 border-t border-cinema-800 space-y-3">
          <div className="grid grid-cols-1 gap-2 rounded-xl border border-borderSubtle bg-cinema-900/45 p-4">
            <span className="text-sm font-medium text-ai-ink-2">续写计划模式</span>
            <span className="text-xs leading-5 text-ai-ink-3">
              已废弃：续写统一走三角色主创，不再使用节拍规划双步链。
            </span>
          </div>
          <div className="grid grid-cols-1 items-center gap-3 rounded-xl border border-borderSubtle bg-cinema-900/45 p-4 @md:grid-cols-[minmax(0,1fr)_auto]">
            <div className="min-w-0">
              <span className="text-sm font-medium text-ai-ink-2">续写目标字数</span>
              <p className="mt-1 text-xs leading-5 text-ai-ink-3">
                建议范围 500-5000，默认 2000；实际目标区间 = 字数 × 0.7 ~ × 1.3
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Input
                aria-label="续写目标字数"
                type="number"
                className="w-24 text-center"
                min={500}
                max={5000}
                step={100}
                placeholder="2000"
                value={continuationTargetWords ?? ''}
                onChange={e => {
                  const v = e.target.value;
                  onSaveContinuationSetting(
                    'continuation_target_words',
                    v === '' ? undefined : parseInt(v, 10)
                  );
                }}
              />
              <span className="text-xs text-ai-ink-3">字</span>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
