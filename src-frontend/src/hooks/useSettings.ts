/**
 * useSettings - 应用设置管理 Hook
 *
 * 管理LLM配置、Agent模型映射、通用设置
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

const SETTINGS_KEY = 'settings';
const MODELS_KEY = 'models';
const AGENT_MAPPINGS_KEY = 'agent-mappings';

// 获取完整设置
export function useSettings() {
  return useQuery<AppSettings>({
    queryKey: [SETTINGS_KEY],
    queryFn: getSettings,
  });
}

// 保存设置
export function useSaveSettings() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: saveSettings,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [SETTINGS_KEY] });
      toast.success('设置已保存');
    },
    onError: (error: Error) => {
      toast.error('保存失败: ' + error.message);
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
      a.download = `storyforge-settings-${new Date().toISOString().split('T')[0]}.json`;
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
      queryClient.invalidateQueries({ queryKey: [SETTINGS_KEY] });
      queryClient.invalidateQueries({ queryKey: [MODELS_KEY] });
      queryClient.invalidateQueries({ queryKey: [AGENT_MAPPINGS_KEY] });
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

// 创建模型配置
export function useCreateModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: createModel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [MODELS_KEY] });
      toast.success('模型配置已创建');
    },
    onError: (error: Error) => {
      toast.error('创建失败: ' + error.message);
    },
  });
}

// 更新模型配置
export function useUpdateModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, config }: { id: string; config: Partial<ModelConfig> }) =>
      updateModel(id, config),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [MODELS_KEY] });
      toast.success('模型配置已更新');
    },
    onError: (error: Error) => {
      toast.error('更新失败: ' + error.message);
    },
  });
}

// 删除模型配置
export function useDeleteModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: deleteModel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [MODELS_KEY] });
      toast.success('模型配置已删除');
    },
    onError: (error: Error) => {
      toast.error('删除失败: ' + error.message);
    },
  });
}

// 设置活跃模型
export function useSetActiveModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ type, modelId }: { type: ModelConfig['type']; modelId: string }) =>
      setActiveModel(type, modelId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [SETTINGS_KEY] });
      toast.success('已设为当前模型');
    },
    onError: (error: Error) => {
      toast.error('设置失败: ' + error.message);
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

// 更新Agent模型映射
export function useUpdateAgentMapping() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: updateAgentMapping,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [AGENT_MAPPINGS_KEY] });
      toast.success('Agent配置已更新');
    },
    onError: (error: Error) => {
      toast.error('更新失败: ' + error.message);
    },
  });
}
