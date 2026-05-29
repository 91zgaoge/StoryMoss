/**
 * 模型连接状态全局管理 Store
 *
 * 统一管理所有模型的连接状态：
 * - 自动轮询检测（每30秒）
 * - 手动触发重试
 * - 状态变更时自动 toast 提示
 * - 提供连接质量评级
 */

import { create } from 'zustand';
import { testModelConnection } from '@/services/settings';
import type { ConnectionTestResult } from '@/types/llm';
import toast from 'react-hot-toast';

export interface ModelConnectionState {
  modelId: string;
  result: ConnectionTestResult;
  lastCheckedAt: number;
  isChecking: boolean;
}

interface ModelConnectionStore {
  // 各模型连接状态映射
  states: Record<string, ModelConnectionState>;
  // 正在检测中的模型ID集合
  checkingIds: Set<string>;
  // 启动自动轮询（返回取消函数）
  startPolling: (modelIds: string[], intervalMs?: number) => () => void;
  // 手动检测单个模型
  checkModel: (modelId: string) => Promise<ConnectionTestResult>;
  // 批量检测模型
  checkModels: (modelIds: string[]) => Promise<void>;
  // 获取模型连接状态
  getState: (modelId: string) => ModelConnectionState | undefined;
  // 清除所有状态
  clearAll: () => void;
}

export const useModelConnectionStore = create<ModelConnectionStore>((set, get) => ({
  states: {},
  checkingIds: new Set(),

  startPolling: (modelIds: string[], intervalMs = 30000) => {
    // 立即检测一次
    get().checkModels(modelIds);

    // 定时轮询
    const interval = setInterval(() => {
      get().checkModels(modelIds);
    }, intervalMs);

    // 返回取消函数
    return () => clearInterval(interval);
  },

  checkModel: async (modelId: string) => {
    const prevState = get().states[modelId];
    const wasConnected = prevState?.result?.success ?? false;

    // 标记为检测中
    set(s => ({
      checkingIds: new Set(s.checkingIds).add(modelId),
      states: {
        ...s.states,
        [modelId]: {
          modelId,
          result: prevState?.result ?? {
            success: false,
            latency: 0,
            steps: [],
          },
          lastCheckedAt: prevState?.lastCheckedAt ?? 0,
          isChecking: true,
        },
      },
    }));

    try {
      const result = await testModelConnection(modelId);

      // 状态变更 toast
      if (!wasConnected && result.success) {
        toast.success(`模型已恢复连接（${result.latency}ms）`);
      } else if (wasConnected && !result.success) {
        toast.error(`模型连接断开：${result.error || '未知原因'}`);
      }

      const connectionResult: ConnectionTestResult = {
        success: result.success,
        latency: result.latency,
        error: result.error,
        steps: result.steps || [],
      };

      set(s => {
        const nextChecking = new Set(s.checkingIds);
        nextChecking.delete(modelId);
        return {
          checkingIds: nextChecking,
          states: {
            ...s.states,
            [modelId]: {
              modelId,
              result: connectionResult,
              lastCheckedAt: Date.now(),
              isChecking: false,
            },
          },
        };
      });

      return connectionResult;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '检测失败';

      if (wasConnected) {
        toast.error(`模型连接检测失败：${errorMsg}`);
      }

      set(s => {
        const nextChecking = new Set(s.checkingIds);
        nextChecking.delete(modelId);
        return {
          checkingIds: nextChecking,
          states: {
            ...s.states,
            [modelId]: {
              modelId,
              result: {
                success: false,
                latency: 0,
                error: errorMsg,
                steps: [],
              },
              lastCheckedAt: Date.now(),
              isChecking: false,
            },
          },
        };
      });

      return {
        success: false,
        latency: 0,
        error: errorMsg,
        steps: [],
      };
    }
  },

  checkModels: async (modelIds: string[]) => {
    // 串行检测，避免并发请求过多
    for (const modelId of modelIds) {
      await get().checkModel(modelId);
    }
  },

  getState: (modelId: string) => {
    return get().states[modelId];
  },

  clearAll: () => {
    set({ states: {}, checkingIds: new Set() });
  },
}));
