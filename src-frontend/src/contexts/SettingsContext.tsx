/**
 * SettingsContext
 *
 * 统一后台设置状态层。
 *
 * - 以 TanStack Query 为底层缓存。
 * - 所有写操作内置乐观更新与失败回滚。
 * - 统一缓存失效范围，确保跨标签页/跨组件即时同步。
 */

import { useCallback, useMemo, type ReactNode } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getSettings,
  saveSettings,
  getModels,
  createModel as createModelService,
  updateModel as updateModelService,
  deleteModel as deleteModelService,
  setActiveModel as setActiveModelService,
  getAgentMappings,
  updateAgentMapping as updateAgentMappingService,
} from '@/services/settings';
import { SettingsContext, type SettingsContextValue } from './settingsContextBase';
import type { AppSettings, ModelConfig, AgentModelMapping } from '@/types/llm';
import toast from 'react-hot-toast';

const SETTINGS_KEY = 'settings';
const MODELS_KEY = 'models';
const AGENT_MAPPINGS_KEY = 'agent-mappings';
const HEALTH_KEY = 'model-health-reports';

interface OptimisticContext<T> {
  previousData: T | undefined;
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();

  const { data: settings, isLoading: settingsLoading } = useQuery<AppSettings>({
    queryKey: [SETTINGS_KEY],
    queryFn: getSettings,
  });

  const { data: models = [], isLoading: modelsLoading } = useQuery<ModelConfig[]>({
    queryKey: [MODELS_KEY],
    queryFn: getModels,
  });

  const { data: agentMappings = [], isLoading: mappingsLoading } = useQuery<AgentModelMapping[]>({
    queryKey: [AGENT_MAPPINGS_KEY],
    queryFn: getAgentMappings,
  });

  const invalidateSettingsFamily = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: [SETTINGS_KEY] });
    queryClient.invalidateQueries({ queryKey: [MODELS_KEY] });
    queryClient.invalidateQueries({ queryKey: [AGENT_MAPPINGS_KEY] });
    queryClient.invalidateQueries({ queryKey: [HEALTH_KEY] });
  }, [queryClient]);

  const updateSettingsMutation = useMutation<
    unknown,
    Error,
    Partial<AppSettings>,
    OptimisticContext<AppSettings>
  >({
    // v0.17.1: 后端 save_settings 期望完整 AppSettingsData。
    // 之前直接发送 patch 会让 models / active_models / agent_mappings / general / privacy
    // 字段缺失，导致 IPC 反序列化失败（前端表现为「保存设置失败: undefined」），
    // 同时让 frontend_timeout_secs 等数字框无法保存。
    // 修复：读取 query 缓存中的完整 settings，合并 patch 后再下发。
    mutationFn: async patch => {
      const current = queryClient.getQueryData<AppSettings>([SETTINGS_KEY]);
      const merged: Partial<AppSettings> = current ? { ...current, ...patch } : patch;
      return saveSettings(merged);
    },
    onMutate: async patch => {
      await queryClient.cancelQueries({ queryKey: [SETTINGS_KEY] });
      const previousSettings = queryClient.getQueryData<AppSettings>([SETTINGS_KEY]);
      queryClient.setQueryData<AppSettings>([SETTINGS_KEY], old => {
        if (!old) return old;
        return { ...old, ...patch, updated_at: new Date().toISOString() };
      });
      return { previousData: previousSettings };
    },
    onError: (error, _patch, context) => {
      if (context?.previousData) {
        queryClient.setQueryData([SETTINGS_KEY], context.previousData);
      }
      toast.error('保存设置失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('设置已保存');
    },
    onSettled: () => {
      invalidateSettingsFamily();
    },
  });

  const createModelMutation = useMutation<
    ModelConfig,
    Error,
    Omit<ModelConfig, 'id'>,
    OptimisticContext<ModelConfig[]>
  >({
    mutationFn: createModelService,
    onMutate: async config => {
      await queryClient.cancelQueries({ queryKey: [MODELS_KEY] });
      const previousModels = queryClient.getQueryData<ModelConfig[]>([MODELS_KEY]);
      const optimisticModel = { ...config, id: `optimistic-${Date.now()}` } as ModelConfig;
      queryClient.setQueryData<ModelConfig[]>([MODELS_KEY], old => {
        return old ? ([...old, optimisticModel] as ModelConfig[]) : [optimisticModel];
      });
      return { previousData: previousModels };
    },
    onError: (error, _config, context) => {
      if (context?.previousData) {
        queryClient.setQueryData([MODELS_KEY], context.previousData);
      }
      toast.error('创建模型失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('模型配置已创建');
    },
    onSettled: () => {
      invalidateSettingsFamily();
    },
  });

  const updateModelMutation = useMutation<
    void,
    Error,
    { id: string; config: Partial<ModelConfig> },
    OptimisticContext<ModelConfig[]>
  >({
    mutationFn: ({ id, config }) => updateModelService(id, config),
    onMutate: async ({ id, config }) => {
      await queryClient.cancelQueries({ queryKey: [MODELS_KEY] });
      const previousModels = queryClient.getQueryData<ModelConfig[]>([MODELS_KEY]);
      queryClient.setQueryData<ModelConfig[]>([MODELS_KEY], old => {
        if (!old) return old;
        return old.map(m => (m.id === id ? ({ ...m, ...config } as ModelConfig) : m));
      });
      return { previousData: previousModels };
    },
    onError: (error, _vars, context) => {
      if (context?.previousData) {
        queryClient.setQueryData([MODELS_KEY], context.previousData);
      }
      toast.error('更新模型失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('模型配置已更新');
    },
    onSettled: () => {
      invalidateSettingsFamily();
    },
  });

  const deleteModelMutation = useMutation<
    void,
    Error,
    string,
    OptimisticContext<ModelConfig[]> & { previousSettings: AppSettings | undefined }
  >({
    mutationFn: deleteModelService,
    onMutate: async id => {
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
      return {
        previousData: previousModels,
        previousSettings,
      };
    },
    onError: (error, _id, context) => {
      if (context?.previousData) {
        queryClient.setQueryData([MODELS_KEY], context.previousData);
      }
      if (context?.previousSettings) {
        queryClient.setQueryData([SETTINGS_KEY], context.previousSettings);
      }
      toast.error('删除模型失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('模型配置已删除');
    },
    onSettled: () => {
      invalidateSettingsFamily();
    },
  });

  const setActiveModelMutation = useMutation<
    void,
    Error,
    { type: ModelConfig['type']; modelId: string; role?: 'creative' | 'tool' | 'background' },
    OptimisticContext<AppSettings>
  >({
    mutationFn: ({ type, modelId, role }) => setActiveModelService(type, modelId, role),
    onMutate: async ({ type, modelId, role }) => {
      await queryClient.cancelQueries({ queryKey: [SETTINGS_KEY] });
      const previousSettings = queryClient.getQueryData<AppSettings>([SETTINGS_KEY]);
      queryClient.setQueryData<AppSettings>([SETTINGS_KEY], old => {
        if (!old) return old;
        const key = role ? role : type;
        return { ...old, active_models: { ...old.active_models, [key]: modelId } };
      });
      return { previousData: previousSettings };
    },
    onError: (error, _vars, context) => {
      if (context?.previousData) {
        queryClient.setQueryData([SETTINGS_KEY], context.previousData);
      }
      toast.error('设置活跃模型失败: ' + error.message);
    },
    onSuccess: (_data, vars) => {
      toast.success(
        vars.role
          ? `已设为${vars.role === 'creative' ? '创作' : vars.role === 'tool' ? '工具' : '后台'}模型`
          : '已设为当前模型'
      );
    },
    onSettled: () => {
      invalidateSettingsFamily();
    },
  });

  const updateAgentMappingMutation = useMutation<
    void,
    Error,
    AgentModelMapping,
    OptimisticContext<AgentModelMapping[]>
  >({
    mutationFn: updateAgentMappingService,
    onMutate: async mapping => {
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
      return { previousData: previousMappings };
    },
    onError: (error, _mapping, context) => {
      if (context?.previousData) {
        queryClient.setQueryData([AGENT_MAPPINGS_KEY], context.previousData);
      }
      toast.error('Agent 配置更新失败: ' + error.message);
    },
    onSuccess: () => {
      toast.success('Agent配置已更新');
    },
    onSettled: () => {
      invalidateSettingsFamily();
    },
  });

  const isPending =
    updateSettingsMutation.isPending ||
    createModelMutation.isPending ||
    updateModelMutation.isPending ||
    deleteModelMutation.isPending ||
    setActiveModelMutation.isPending ||
    updateAgentMappingMutation.isPending;

  const updateModel = useCallback(
    (id: string, config: Partial<ModelConfig>) => updateModelMutation.mutateAsync({ id, config }),
    [updateModelMutation]
  );

  const setActiveModel = useCallback(
    (type: ModelConfig['type'], modelId: string, role?: 'creative' | 'tool' | 'background') =>
      setActiveModelMutation.mutateAsync({ type, modelId, role }),
    [setActiveModelMutation]
  );

  const value = useMemo<SettingsContextValue>(
    () => ({
      settings,
      models,
      agentMappings,
      isLoading: settingsLoading || modelsLoading || mappingsLoading,
      isPending,
      updateSettings: updateSettingsMutation.mutate,
      createModel: createModelMutation.mutateAsync,
      updateModel,
      deleteModel: deleteModelMutation.mutateAsync,
      setActiveModel,
      updateAgentMapping: updateAgentMappingMutation.mutate,
    }),
    [
      settings,
      models,
      agentMappings,
      settingsLoading,
      modelsLoading,
      mappingsLoading,
      isPending,
      updateSettingsMutation,
      createModelMutation,
      updateModel,
      deleteModelMutation,
      setActiveModel,
      updateAgentMappingMutation,
    ]
  );

  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}
