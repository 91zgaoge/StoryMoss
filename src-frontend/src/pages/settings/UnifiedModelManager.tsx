import { useState, useEffect, useMemo } from 'react';
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
import { useSettingsContext } from '@/hooks/useSettingsContext';
import { useModelConnectionStore } from '@/stores/modelConnectionStore';
import type { ModelConfig, ModelType } from '@/types/llm';
import { cn } from '@/utils/cn';

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

  const { models, settings, isLoading, setActiveModel, deleteModel } = useSettingsContext();
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const activeModelIds = settings?.active_models ?? {};

  // 模型连接状态管理
  const { states, checkModels, checkModel } = useModelConnectionStore();

  // 自动检测所有模型连接
  useEffect(() => {
    if (!models.length) return;
    const modelIds = models.map(m => m.id);
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
    if (!confirm('确定要删除该模型配置吗？如果该模型正被 Agent 使用，对应映射会被自动清除。')) {
      return;
    }
    setDeletingId(modelId);
    try {
      await deleteModel(modelId);
    } finally {
      setDeletingId(null);
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
        />
      )}

      {/* 模型列表 - 按类型分组 */}
      {models.length === 0 ? (
        <Card>
          <CardContent className="p-12 text-center">
            <Database className="w-16 h-16 text-gray-600 mx-auto mb-4" />
            <h3 className="text-lg font-medium text-white mb-2">暂无模型配置</h3>
            <p className="text-gray-500 mb-4">点击下方按钮添加第一个模型配置</p>
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
    desc: '正文生成、Writer、改写 — 质量优先',
    icon: <Paintbrush className="w-4 h-4" />,
    color: 'text-amber-400',
  },
  tool: {
    label: '工具模型',
    desc: 'Call 1 路由、探测、JSON 提取 — 速度优先',
    icon: <Wrench className="w-4 h-4" />,
    color: 'text-blue-400',
  },
  background: {
    label: '后台任务模型',
    desc: 'BGP 审计/入库/洞察、Genesis 后台流水线',
    icon: <Settings className="w-4 h-4" />,
    color: 'text-purple-400',
  },
};

function ModelRoleCard({
  models,
  activeModels,
  onSetRole,
}: {
  models: ModelConfig[];
  activeModels: Record<string, string | undefined>;
  onSetRole: (modelId: string, role: ModelRoleKey) => void;
}) {
  // 只为聊天模型提供角色选择
  const chatModels = models.filter(m => m.type === 'chat');

  return (
    <Card>
      <CardContent className="p-4">
        <h3 className="text-base font-semibold text-white">模型角色分配</h3>
        <p className="text-xs text-gray-500 mt-1 mb-3">
          为不同任务类型指定默认模型。未设置时网关自动分配（快模型→工具，闲置→后台，创作回退当前模型）。
        </p>
        <div className="space-y-3">
          {(Object.entries(ROLE_CONFIG) as [ModelRoleKey, (typeof ROLE_CONFIG)['creative']][]).map(
            ([role, config]) => {
              const currentId = activeModels[role];
              const currentModel = chatModels.find(m => m.id === currentId);
              return (
                <div key={role} className="flex items-center gap-3 p-2 rounded-lg bg-cinema-900/50">
                  <span
                    className={cn(
                      'flex items-center gap-1.5 text-sm font-medium min-w-[120px]',
                      config.color
                    )}
                  >
                    {config.icon}
                    {config.label}
                  </span>
                  <span className="text-xs text-gray-600 hidden sm:block flex-1">
                    {config.desc}
                  </span>
                  <select
                    className="bg-cinema-800 border border-cinema-700 rounded-md text-sm text-white px-2 py-1.5 min-w-[160px] focus:border-cinema-gold focus:outline-none"
                    value={currentId || ''}
                    onChange={e => {
                      if (e.target.value) {
                        onSetRole(e.target.value, role);
                      }
                    }}
                  >
                    <option value="">自动分配（网关判断）</option>
                    {chatModels.map(m => (
                      <option key={m.id} value={m.id}>
                        {m.name}
                      </option>
                    ))}
                  </select>
                  {currentModel && (
                    <span className="text-xs text-cinema-gold bg-cinema-gold/10 px-2 py-0.5 rounded">
                      {currentModel.name.length > 15
                        ? currentModel.name.slice(0, 15) + '...'
                        : currentModel.name}
                    </span>
                  )}
                </div>
              );
            }
          )}
        </div>
      </CardContent>
    </Card>
  );
}
