/**
 * 模型管理 Hook
 *
 * 用于管理当前模型、模型状态和模型切换
 */

import { useState, useEffect, useCallback } from 'react';
import { ModelConfig } from '@/config/models';
import { modelService, ChatMessage } from '@/services/modelService';
import { createLogger } from '@/utils/logger';
import { getConfig } from '@/services/tauri';

const modelLogger = createLogger('hooks:useModel');

export interface ModelState {
  currentModel: ModelConfig;
  status: 'connected' | 'disconnected' | 'connecting';
  availableModels: ModelConfig[];
}

const DEFAULT_MODEL: ModelConfig = {
  id: 'default',
  name: '默认模型',
  type: 'language',
  baseUrl: '',
  useApiKey: false,
  description: '等待后端配置...',
};

export function useModel() {
  const [state, setState] = useState<ModelState>({
    currentModel: DEFAULT_MODEL,
    status: 'connecting',
    availableModels: [],
  });

  // 检查模型状态
  const checkStatus = useCallback(async () => {
    setState(prev => ({ ...prev, status: 'connecting' }));
    const status = await modelService.checkModelStatus();
    setState(prev => ({ ...prev, status }));
    return status;
  }, []);

  // 切换模型（通过后端设置更新）
  const switchModel = useCallback(
    (modelId: string) => {
      setState(prev => ({
        ...prev,
        currentModel: { ...prev.currentModel, id: modelId, name: modelId },
        status: 'connecting',
      }));
      // 切换后检查新模型状态
      checkStatus();
    },
    [checkStatus]
  );

  // 发送聊天消息
  const chat = useCallback(
    async (
      messages: ChatMessage[],
      options?: {
        stream?: boolean;
        onStream?: (chunk: string) => void;
      }
    ) => {
      return modelService.chat(messages, options);
    },
    []
  );

  // 初始检查状态
  useEffect(() => {
    // 先获取后端配置
    getConfig()
      .then(config => {
        const adaptedModel: ModelConfig = {
          id: config.model || 'default',
          name: `${config.provider} - ${config.model}`,
          type: 'language',
          baseUrl: config.base_url || '',
          apiKey: config.api_key,
          useApiKey: !!config.api_key,
          description: '后端配置模型',
          maxTokens: config.max_tokens,
          temperature: config.temperature,
        };
        setState(prev => ({
          ...prev,
          currentModel: adaptedModel,
        }));
      })
      .catch(err => {
        modelLogger.warn('Failed to load backend model config', { error: err });
      })
      .finally(() => {
        checkStatus();
      });

    // 每30秒检查一次状态
    const interval = setInterval(checkStatus, 30000);
    return () => clearInterval(interval);
  }, [checkStatus]);

  return {
    ...state,
    checkStatus,
    switchModel,
    chat,
  };
}

export default useModel;
