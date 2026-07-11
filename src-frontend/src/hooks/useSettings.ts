/**
 * useSettings - 应用设置管理 Hook
 *
 * 提供与 TanStack Query 交互的底层 hooks。
 *
 * 注意：后台设置页面已统一迁移到 {@link SettingsContext}，以获得乐观更新、
 * 统一缓存失效和失败回滚。这些 hooks 仍保留供其他页面或遗留代码使用，
 * 但新增后台设置功能时建议优先使用 SettingsContext。
 */

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getSettings,
  saveSettings,
  exportSettings,
  importSettings,
  getModels,
  createModel,
  updateModel,
  deleteModel,
  setActiveModel,
  getAgentMappings,
  updateAgentMapping,
} from '@/services/settings';
import type { AppSettings, ModelConfig, AgentModelMapping, SettingsExport } from '@/types/llm';
import toast from 'react-hot-toast';

export const SETTINGS_KEY = 'settings';
export const MODELS_KEY = 'models';
export const AGENT_MAPPINGS_KEY = 'agent-mappings';
const HEALTH_KEY = 'model-health-reports';

function invalidateSettingsFamily(queryClient: ReturnType<typeof useQueryClient>) {
  queryClient.invalidateQueries({ queryKey: [SETTINGS_KEY] });
  queryClient.invalidateQueries({ queryKey: [MODELS_KEY] });
  queryClient.invalidateQueries({ queryKey: [AGENT_MAPPINGS_KEY] });
  queryClient.invalidateQueries({ queryKey: [HEALTH_KEY] });
}

// 获取完整设置
export function useSettings() {
  return useQuery<AppSettings>({
    queryKey: [SETTINGS_KEY],
    queryFn: getSettings,
  });
}

// 保存设置（带乐观更新）
export function useSaveSettings() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: saveSettings,
    onMutate: async (newSettings: Partial<AppSettings>) => {
      await queryClient.cancelQueries({ queryKey: [SETTINGS_KEY] });
      const previousSettings = queryClient.getQueryData<AppSettings>([SETTINGS_KEY]);
      queryClient.setQueryData<AppSettings>([SETTINGS_KEY], old => {
        if (!old) return old;
        return { ...old, ...newSettings, updated_at: new Date().toISOString() };
      });
      return { previousSettings };
    },
    onError: (error: Error, _vars, context) => {
      if (context?.previousSettings) {
        queryClient.setQueryData([SETTINGS_KEY], context.previousSettings);
      }
      toast.error('保存失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('设置已保存');
    },
    onSettled: () => {
      invalidateSettingsFamily(queryClient);
    },
  });
}

// 导出设置
export function useExportSettings() {
  return useMutation({
    mutationFn: exportSettings,
    onSuccess: data => {
      // 下载JSON文件
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `storymoss-settings-${new Date().toISOString().split('T')[0]}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      toast.success('设置已导出');
    },
    onError: (error: Error) => {
      toast.error('导出失败: ' + error.message);
    },
  });
}

// 导入设置
export function useImportSettings() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (file: File) => {
      const text = await file.text();
      const data = JSON.parse(text) as SettingsExport;
      return importSettings(data);
    },
    onSuccess: () => {
      invalidateSettingsFamily(queryClient);
      toast.success('设置已导入');
    },
    onError: (error: Error) => {
      toast.error('导入失败: ' + error.message);
    },
  });
}

// 获取所有模型配置
export function useModels() {
  return useQuery<ModelConfig[]>({
    queryKey: [MODELS_KEY],
    queryFn: getModels,
  });
}

// 按类型获取模型
export function useModelsByType(type: ModelConfig['type']) {
  const { data: models = [] } = useModels();
  return models.filter(m => m.type === type);
}

// 创建模型配置（带乐观更新）
export function useCreateModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: createModel,
    onMutate: async (config: Omit<ModelConfig, 'id'>) => {
      await queryClient.cancelQueries({ queryKey: [MODELS_KEY] });
      const previousModels = queryClient.getQueryData<ModelConfig[]>([MODELS_KEY]);
      const optimisticModel = { ...config, id: `optimistic-${Date.now()}` } as ModelConfig;
      queryClient.setQueryData<ModelConfig[]>([MODELS_KEY], old => {
        return old ? ([...old, optimisticModel] as ModelConfig[]) : [optimisticModel];
      });
      return { previousModels };
    },
    onError: (error: Error, _vars, context) => {
      if (context?.previousModels) {
        queryClient.setQueryData([MODELS_KEY], context.previousModels);
      }
      toast.error('创建失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('模型配置已创建');
    },
    onSettled: () => {
      invalidateSettingsFamily(queryClient);
    },
  });
}

// 更新模型配置（带乐观更新）
export function useUpdateModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, config }: { id: string; config: Partial<ModelConfig> }) =>
      updateModel(id, config),
    onMutate: async ({ id, config }) => {
      await queryClient.cancelQueries({ queryKey: [MODELS_KEY] });
      const previousModels = queryClient.getQueryData<ModelConfig[]>([MODELS_KEY]);
      queryClient.setQueryData<ModelConfig[]>([MODELS_KEY], old => {
        if (!old) return old;
        return old.map(m => (m.id === id ? ({ ...m, ...config } as ModelConfig) : m));
      });
      return { previousModels };
    },
    onError: (error: Error, _vars, context) => {
      if (context?.previousModels) {
        queryClient.setQueryData([MODELS_KEY], context.previousModels);
      }
      toast.error('更新失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('模型配置已更新');
    },
    onSettled: () => {
      invalidateSettingsFamily(queryClient);
    },
  });
}

// 删除模型配置（带乐观更新）
export function useDeleteModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: deleteModel,
    onMutate: async (id: string) => {
      await queryClient.cancelQueries({ queryKey: [MODELS_KEY] });
      await queryClient.cancelQueries({ queryKey: [SETTINGS_KEY] });
      const previousModels = queryClient.getQueryData<ModelConfig[]>([MODELS_KEY]);
      const previousSettings = queryClient.getQueryData<AppSettings>([SETTINGS_KEY]);
      queryClient.setQueryData<ModelConfig[]>([MODELS_KEY], old => {
        return old?.filter(m => m.id !== id) ?? [];
      });
      queryClient.setQueryData<AppSettings>([SETTINGS_KEY], old => {
        if (!old) return old;
        const nextActiveModels = { ...old.active_models };
        (Object.keys(nextActiveModels) as Array<keyof typeof nextActiveModels>).forEach(type => {
          if (nextActiveModels[type] === id) {
            delete nextActiveModels[type];
          }
        });
        return { ...old, active_models: nextActiveModels };
      });
      return { previousModels, previousSettings };
    },
    onError: (error: Error, _vars, context) => {
      if (context?.previousModels) {
        queryClient.setQueryData([MODELS_KEY], context.previousModels);
      }
      if (context?.previousSettings) {
        queryClient.setQueryData([SETTINGS_KEY], context.previousSettings);
      }
      toast.error('删除失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('模型配置已删除');
    },
    onSettled: () => {
      invalidateSettingsFamily(queryClient);
    },
  });
}

// 设置活跃模型（带乐观更新）
export function useSetActiveModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ type, modelId }: { type: ModelConfig['type']; modelId: string }) =>
      setActiveModel(type, modelId),
    onMutate: async ({ type, modelId }) => {
      await queryClient.cancelQueries({ queryKey: [SETTINGS_KEY] });
      const previousSettings = queryClient.getQueryData<AppSettings>([SETTINGS_KEY]);
      queryClient.setQueryData<AppSettings>([SETTINGS_KEY], old => {
        if (!old) return old;
        return { ...old, active_models: { ...old.active_models, [type]: modelId } };
      });
      return { previousSettings };
    },
    onError: (error: Error, _vars, context) => {
      if (context?.previousSettings) {
        queryClient.setQueryData([SETTINGS_KEY], context.previousSettings);
      }
      toast.error('设置失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('已设为当前模型');
    },
    onSettled: () => {
      invalidateSettingsFamily(queryClient);
    },
  });
}

// 获取Agent模型映射
export function useAgentMappings() {
  return useQuery<AgentModelMapping[]>({
    queryKey: [AGENT_MAPPINGS_KEY],
    queryFn: getAgentMappings,
  });
}

// 更新Agent模型映射（带乐观更新）
export function useUpdateAgentMapping() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: updateAgentMapping,
    onMutate: async (mapping: AgentModelMapping) => {
      await queryClient.cancelQueries({ queryKey: [AGENT_MAPPINGS_KEY] });
      const previousMappings = queryClient.getQueryData<AgentModelMapping[]>([AGENT_MAPPINGS_KEY]);
      queryClient.setQueryData<AgentModelMapping[]>([AGENT_MAPPINGS_KEY], old => {
        if (!old) return [mapping];
        const exists = old.some(m => m.agent_id === mapping.agent_id);
        if (exists) {
          return old.map(m => (m.agent_id === mapping.agent_id ? mapping : m));
        }
        return [...old, mapping];
      });
      return { previousMappings };
    },
    onError: (error: Error, _vars, context) => {
      if (context?.previousMappings) {
        queryClient.setQueryData([AGENT_MAPPINGS_KEY], context.previousMappings);
      }
      toast.error('更新失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('Agent配置已更新');
    },
    onSettled: () => {
      invalidateSettingsFamily(queryClient);
    },
  });
}
