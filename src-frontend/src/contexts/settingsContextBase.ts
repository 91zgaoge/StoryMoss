import { createContext } from 'react';
import type { AppSettings, ModelConfig, AgentModelMapping } from '@/types/llm';

export interface SettingsContextValue {
  /** 完整应用设置 */
  settings: AppSettings | undefined;
  /** 所有模型配置 */
  models: ModelConfig[];
  /** Agent 模型映射 */
  agentMappings: AgentModelMapping[];
  /** 初始加载中 */
  isLoading: boolean;
  /** 任意设置写操作进行中 */
  isPending: boolean;

  /** 局部更新应用设置（乐观更新） */
  updateSettings: (patch: Partial<AppSettings>) => void;

  /** 创建模型并返回新模型 */
  createModel: (config: Omit<ModelConfig, 'id'>) => Promise<ModelConfig>;
  /** 更新模型 */
  updateModel: (id: string, config: Partial<ModelConfig>) => Promise<void>;
  /** 删除模型 */
  deleteModel: (id: string) => Promise<void>;
  /** 设置活跃模型（v0.23.66: 可选 role="creative"|"tool"|"background" 指定角色） */
  setActiveModel: (
    type: ModelConfig['type'],
    modelId: string,
    role?: 'creative' | 'tool' | 'background'
  ) => Promise<void>;

  /** 更新 Agent 模型映射（乐观更新） */
  updateAgentMapping: (mapping: AgentModelMapping) => void;
}

export const SettingsContext = createContext<SettingsContextValue | null>(null);
