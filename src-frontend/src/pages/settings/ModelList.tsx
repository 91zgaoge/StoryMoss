import { Plus, Database } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import type { ModelType, ModelConfig, ConnectionTestResult } from '@/types/llm';
import { ModelCard } from './ModelCard';

export function ModelList({
  type,
  models,
  activeModelId,
  connectionStatus,
  onAdd,
  onEdit,
  onSetActive,
  onRetry,
  onDelete,
  onToggleEnabled,
  togglingEnabledId,
  deletingId,
  groupTitle,
  showAddButton = true,
  showTypeHeader = true,
  /** v0.23.66: 模型角色分配信息（用于徽章显示） */
  activeModels,
}: {
  type: ModelType;
  models: ModelConfig[];
  activeModelId?: string;
  connectionStatus: Record<string, { result: ConnectionTestResult; isChecking: boolean }>;
  onAdd: () => void;
  onEdit: (model: ModelConfig) => void;
  onSetActive: (modelId: string) => void;
  onRetry?: (modelId: string) => void;
  onDelete?: (modelId: string) => void;
  onToggleEnabled?: (model: ModelConfig) => void;
  togglingEnabledId?: string | null;
  deletingId?: string | null;
  groupTitle?: string;
  showAddButton?: boolean;
  showTypeHeader?: boolean;
  activeModels?: Record<string, string | undefined>;
}) {
  const title =
    groupTitle ||
    (type === 'chat'
      ? '聊天模型配置'
      : type === 'embedding'
        ? '嵌入模型配置'
        : type === 'multimodal'
          ? '多模态模型配置'
          : type === 'image'
            ? '图像生成模型配置'
            : '模型配置');

  return (
    <div className="space-y-4">
      {showTypeHeader && (
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold text-white">{title}</h2>
          {showAddButton && (
            <Button variant="primary" onClick={onAdd}>
              <Plus className="w-4 h-4 mr-2" />
              添加模型
            </Button>
          )}
        </div>
      )}

      {models.length === 0 ? (
        <Card>
          <CardContent className="p-12 text-center">
            <Database className="w-16 h-16 text-gray-600 mx-auto mb-4" />
            <h3 className="text-lg font-medium text-white mb-2">暂无模型配置</h3>
            <p className="text-gray-500 mb-4">
              {showAddButton ? '点击上方按钮添加第一个模型配置' : '该类型暂无模型配置'}
            </p>
            {showAddButton && (
              <Button variant="primary" onClick={onAdd}>
                <Plus className="w-4 h-4 mr-2" />
                添加模型
              </Button>
            )}
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4">
          {models.map(model => {
            // v0.23.66: 计算模型角色徽章
            const roles: string[] = [];
            if (activeModels?.creative === model.id) roles.push('创作');
            if (activeModels?.tool === model.id) roles.push('工具');
            if (activeModels?.background === model.id) roles.push('后台');
            return (
              <ModelCard
                key={model.id}
                model={model}
                isActive={model.id === activeModelId}
                connectionStatus={connectionStatus[model.id]}
                isDeleting={deletingId === model.id}
                onEdit={() => onEdit(model)}
                onSetActive={() => onSetActive(model.id)}
                onRetry={onRetry ? () => onRetry(model.id) : undefined}
                onDelete={onDelete ? () => onDelete(model.id) : undefined}
                onToggleEnabled={onToggleEnabled ? () => onToggleEnabled(model) : undefined}
                togglingEnabled={togglingEnabledId === model.id}
                roleBadges={roles}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}
