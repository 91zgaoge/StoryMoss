/**
 * 生成状态 Store
 *
 * 将 FrontstageApp 中的高频生成状态（isGenerating / generationStatus /
 * orchestratorStatus）抽离到独立 Zustand store，避免单点状态变化触发
 * 整个应用重渲染。
 */

import { create } from 'zustand';

export interface OrchestratorStatus {
  stepType: string;
  loopIdx?: number;
  score?: number;
  message: string;
  detail?: string;
}

interface GenerationState {
  isGenerating: boolean;
  generationStatus: string;
  orchestratorStatus: OrchestratorStatus | null;

  setIsGenerating: (generating: boolean | ((prev: boolean) => boolean)) => void;
  setGenerationStatus: (status: string | ((prev: string) => string)) => void;
  setOrchestratorStatus: (
    status:
      | OrchestratorStatus
      | null
      | ((prev: OrchestratorStatus | null) => OrchestratorStatus | null)
  ) => void;
  resetGeneration: () => void;
}

export const useGenerationStore = create<GenerationState>(set => ({
  isGenerating: false,
  generationStatus: '',
  orchestratorStatus: null,

  setIsGenerating: generating =>
    set(state => ({
      isGenerating:
        typeof generating === 'function'
          ? (generating as (prev: boolean) => boolean)(state.isGenerating)
          : generating,
    })),

  setGenerationStatus: status =>
    set(state => ({
      generationStatus:
        typeof status === 'function'
          ? (status as (prev: string) => string)(state.generationStatus)
          : status,
    })),

  setOrchestratorStatus: status =>
    set(state => ({
      orchestratorStatus:
        typeof status === 'function'
          ? (status as (prev: OrchestratorStatus | null) => OrchestratorStatus | null)(
              state.orchestratorStatus
            )
          : status,
    })),

  resetGeneration: () =>
    set({
      isGenerating: false,
      generationStatus: '',
      orchestratorStatus: null,
    }),
}));
